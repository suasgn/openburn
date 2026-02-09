mod account_store;
#[cfg(target_os = "macos")]
mod app_nap;
mod error;
mod models;
mod panel;
mod probe;
mod provider_clients;
mod providers;
mod secrets;
mod tray;
mod utils;
#[cfg(target_os = "macos")]
mod webkit_config;

use std::collections::HashSet;

use account_store::AccountStore;
use models::{AccountRecord, CreateAccountInput, UpdateAccountInput};
use probe::{ProbeBatchCompleteEvent, ProbeBatchStarted, ProbeResultEvent, ProviderMeta};
use providers::ProviderDescriptor;
use tauri::{Emitter, Manager, State};
use tauri_plugin_log::{Target, TargetKind};
use uuid::Uuid;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn init_panel(app_handle: tauri::AppHandle) -> Result<(), String> {
    panel::init(&app_handle).map_err(|err| err.to_string())
}

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = app_handle.get_webview_panel("main") {
        panel.hide();
    }
}

#[tauri::command]
fn list_providers_meta() -> Vec<ProviderMeta> {
    probe::all_provider_meta()
}

#[tauri::command(rename_all = "camelCase")]
async fn start_provider_probe_batch(
    app_handle: tauri::AppHandle,
    store: State<'_, AccountStore>,
    batch_id: Option<String>,
    provider_ids: Option<Vec<String>>,
) -> Result<ProbeBatchStarted, String> {
    let batch_id = batch_id
        .and_then(|id| {
            let trimmed = id.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let known_ids = probe::all_provider_ids();
    let known_set: HashSet<String> = known_ids.iter().cloned().collect();

    let selected_ids = if let Some(requested) = provider_ids {
        let mut seen = HashSet::new();
        requested
            .into_iter()
            .map(|id| id.trim().to_ascii_lowercase())
            .filter(|id| !id.is_empty() && known_set.contains(id) && seen.insert(id.clone()))
            .collect::<Vec<_>>()
    } else {
        known_ids.clone()
    };

    if selected_ids.is_empty() {
        let _ = app_handle.emit(
            "probe:batch-complete",
            ProbeBatchCompleteEvent {
                batch_id: batch_id.clone(),
            },
        );
        return Ok(ProbeBatchStarted {
            batch_id,
            provider_ids: selected_ids,
        });
    }

    for provider_id in &selected_ids {
        let output = match probe::probe_provider(&app_handle, store.inner(), provider_id).await {
            Ok(output) => output,
            Err(err) => probe::build_error_output(provider_id, err.to_string()),
        };

        app_handle
            .emit(
                "probe:result",
                ProbeResultEvent {
                    batch_id: batch_id.clone(),
                    output,
                },
            )
            .map_err(|err| err.to_string())?;
    }

    app_handle
        .emit(
            "probe:batch-complete",
            ProbeBatchCompleteEvent {
                batch_id: batch_id.clone(),
            },
        )
        .map_err(|err| err.to_string())?;

    Ok(ProbeBatchStarted {
        batch_id,
        provider_ids: selected_ids,
    })
}

#[tauri::command]
fn list_providers() -> Vec<ProviderDescriptor> {
    providers::all_provider_descriptors()
}

#[tauri::command]
fn list_accounts(store: State<'_, AccountStore>) -> Result<Vec<AccountRecord>, String> {
    store.list_accounts().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_account(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<Option<AccountRecord>, String> {
    store
        .get_account(&account_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn create_account(
    store: State<'_, AccountStore>,
    input: CreateAccountInput,
) -> Result<AccountRecord, String> {
    store.create_account(input).map_err(|err| err.to_string())
}

#[tauri::command]
fn update_account(
    store: State<'_, AccountStore>,
    account_id: String,
    input: UpdateAccountInput,
) -> Result<AccountRecord, String> {
    store
        .update_account(&account_id, input)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn delete_account(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<Option<AccountRecord>, String> {
    store
        .delete_account(&account_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_account_credentials(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    account_id: String,
    credentials: serde_json::Value,
) -> Result<(), String> {
    secrets::set_account_credentials(&app, store.inner(), &account_id, &credentials)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn has_account_credentials(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<bool, String> {
    secrets::has_account_credentials(store.inner(), &account_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_account_credentials(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<(), String> {
    secrets::clear_account_credentials(store.inner(), &account_id).map_err(|err| err.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let has_updater_config = matches!(
        context.config().plugins.0.get("updater"),
        Some(value) if value.is_object()
    );

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_nspanel::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .max_file_size(10_000_000)
                .level(log::LevelFilter::Info)
                .level_for("hyper", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                app_nap::disable_app_nap();
                webkit_config::disable_webview_suspension(app.handle());
            }

            let store = AccountStore::load(app.handle())
                .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;
            app.manage(store);

            tray::create(app.handle())?;

            Ok(())
        });

    if has_updater_config {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            greet,
            init_panel,
            hide_panel,
            list_providers_meta,
            start_provider_probe_batch,
            list_providers,
            list_accounts,
            get_account,
            create_account,
            update_account,
            delete_account,
            set_account_credentials,
            has_account_credentials,
            clear_account_credentials
        ])
        .run(context)
        .expect("error while running tauri application");
}
