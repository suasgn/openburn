// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod account_store;
mod error;
mod models;
mod providers;
mod secrets;
mod utils;

use account_store::AccountStore;
use models::{AccountRecord, CreateAccountInput, UpdateAccountInput};
use providers::ProviderDescriptor;
use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn init_panel() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn hide_panel() -> Result<(), String> {
    Ok(())
}

fn provider_brand(id: &str) -> (&'static str, &'static str, &'static str) {
    match id {
        "codex" => ("/vite.svg", "#0EA5E9", "Pro"),
        "copilot" => ("/tauri.svg", "#8B5CF6", "Business"),
        "claude" => ("/tauri.svg", "#D97706", "Max"),
        "zai" => ("/vite.svg", "#14B8A6", "Standard"),
        _ => ("/vite.svg", "#6B7280", "Standard"),
    }
}

fn build_mock_provider_output(provider: &ProviderDescriptor) -> Value {
    let (icon_url, brand_color, plan) = provider_brand(provider.id);

    let seed = provider
        .id
        .bytes()
        .fold(0u32, |acc, byte| acc.wrapping_add(byte as u32));
    let limit = 100.0f64;
    let used = 20.0f64 + (seed % 70) as f64;
    let request_count = 300 + ((seed * 13) % 1400);
    let status_text = if used < 60.0 {
        "Healthy"
    } else if used < 85.0 {
        "Watch"
    } else {
        "High"
    };

    let resets_at = (OffsetDateTime::now_utc() + Duration::days(7))
        .format(&Rfc3339)
        .unwrap_or_else(|_| "2099-01-01T00:00:00Z".to_string());

    json!({
        "providerId": provider.id,
        "displayName": provider.name,
        "plan": plan,
        "iconUrl": icon_url,
        "lines": [
            {
                "type": "progress",
                "label": "Weekly usage",
                "used": used,
                "limit": limit,
                "format": { "kind": "percent" },
                "resetsAt": resets_at,
                "periodDurationMs": 604_800_000,
                "color": brand_color
            },
            {
                "type": "badge",
                "label": "Status",
                "text": status_text,
                "color": brand_color,
                "subtitle": "Mock provider data"
            },
            {
                "type": "text",
                "label": "Requests",
                "value": request_count.to_string(),
                "subtitle": "Current period"
            }
        ]
    })
}

#[tauri::command]
fn list_providers_meta() -> Vec<Value> {
    providers::all_provider_descriptors()
        .into_iter()
        .map(|provider| {
            let (icon_url, brand_color, _) = provider_brand(provider.id);
            json!({
                "id": provider.id,
                "name": provider.name,
                "iconUrl": icon_url,
                "brandColor": brand_color,
                "lines": [
                    { "type": "progress", "label": "Weekly usage", "scope": "overview" },
                    { "type": "badge", "label": "Status", "scope": "overview" },
                    { "type": "text", "label": "Requests", "scope": "detail" }
                ],
                "primaryCandidates": ["Weekly usage"]
            })
        })
        .collect()
}

#[tauri::command(rename_all = "camelCase")]
fn start_provider_probe_batch(
    app: tauri::AppHandle,
    batch_id: String,
    provider_ids: Option<Vec<String>>,
) -> Result<Value, String> {
    let providers = providers::all_provider_descriptors();
    let all_ids: Vec<String> = providers
        .iter()
        .map(|provider| provider.id.to_string())
        .collect();

    let selected_ids = if let Some(requested) = provider_ids {
        requested
            .into_iter()
            .filter(|id| all_ids.iter().any(|known| known == id))
            .collect::<Vec<_>>()
    } else {
        all_ids.clone()
    };

    for provider in providers
        .iter()
        .filter(|provider| selected_ids.iter().any(|id| id == provider.id))
    {
        let output = build_mock_provider_output(provider);
        app.emit(
            "probe:result",
            json!({
                "batchId": batch_id,
                "output": output,
            }),
        )
        .map_err(|err| err.to_string())?;
    }

    app.emit("probe:batch-complete", json!({ "batchId": batch_id }))
        .map_err(|err| err.to_string())?;

    Ok(json!({
        "batchId": batch_id,
        "providerIds": selected_ids,
    }))
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
        .setup(|app| {
            let store = AccountStore::load(app.handle())
                .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;
            app.manage(store);
            Ok(())
        })
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    if has_updater_config {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_opener::init())
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
