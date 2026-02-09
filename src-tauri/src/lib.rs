mod account_store;
mod auth;
#[cfg(target_os = "macos")]
mod app_nap;
mod error;
mod models;
mod panel;
mod probe;
mod provider_clients;
mod providers;
mod oauth;
mod secrets;
mod tray;
mod utils;
#[cfg(target_os = "macos")]
mod webkit_config;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;

use account_store::AccountStore;
use auth::{AuthState, PendingOAuth};
use models::{AccountRecord, CreateAccountInput, UpdateAccountInput};
use providers::{
    find_provider_contract, validate_auth_strategy_for_provider, ProviderDescriptor,
};
use probe::{ProbeBatchCompleteEvent, ProbeBatchStarted, ProbeResultEvent, ProviderMeta};
use tauri::{Emitter, Manager, State};
use tauri_plugin_log::{Target, TargetKind};
use utils::now_unix_ms;
use uuid::Uuid;

const DEFAULT_OAUTH_TIMEOUT_MS: u64 = 180_000;

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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthStartResponse {
    request_id: String,
    url: String,
    redirect_uri: String,
    user_code: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthResult {
    account_id: String,
    expires_at: i64,
}

fn normalized_callback_path(callback_path: &str) -> String {
    if callback_path.starts_with('/') {
        callback_path.to_string()
    } else {
        format!("/{callback_path}")
    }
}

fn ensure_oauth_account(
    store: &AccountStore,
    account_id: &str,
    expected_provider_id: &str,
    provider_label: &str,
) -> Result<AccountRecord, String> {
    let account = store
        .get_account(account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "Account not found".to_string())?;

    if account.provider_id != expected_provider_id {
        return Err(format!(
            "{provider_label} OAuth requires a {expected_provider_id} account"
        ));
    }

    let provider = find_provider_contract(expected_provider_id)
        .ok_or_else(|| format!("provider '{}' is not registered", expected_provider_id))?;

    let effective_strategy = account
        .auth_strategy_id
        .as_deref()
        .unwrap_or(provider.default_auth_strategy_id);

    validate_auth_strategy_for_provider(provider, Some(effective_strategy)).map_err(|err| err.to_string())?;

    if effective_strategy != "oauth" {
        return Err(format!(
            "{provider_label} OAuth requires authStrategyId 'oauth'"
        ));
    }

    Ok(account)
}

fn start_pkce_oauth_flow<F>(
    auth_state: &AuthState,
    account_id: String,
    callback_path: &str,
    callback_port: Option<u16>,
    build_url: F,
) -> Result<OAuthStartResponse, String>
where
    F: FnOnce(&str, &str, &str) -> Result<String, String>,
{
    let pkce = oauth::generate_pkce();
    let state = Uuid::new_v4().to_string();
    let (port, receiver, cancel_flag) = auth::start_local_callback_listener_with_options(
        state.clone(),
        callback_path,
        callback_port,
    )
    .map_err(|err| err.to_string())?;

    let callback_path = normalized_callback_path(callback_path);
    let redirect_uri = format!("http://127.0.0.1:{port}{callback_path}");
    let url = build_url(&redirect_uri, &pkce.challenge, &state)?;
    let request_id = Uuid::new_v4().to_string();

    let pending = PendingOAuth::new(
        account_id,
        pkce.verifier,
        redirect_uri.clone(),
        cancel_flag,
        receiver,
    );
    auth_state.insert(request_id.clone(), pending);

    Ok(OAuthStartResponse {
        request_id,
        url,
        redirect_uri,
        user_code: None,
    })
}

#[tauri::command]
fn start_codex_oauth(
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    account_id: String,
) -> Result<OAuthStartResponse, String> {
    let _account = ensure_oauth_account(store.inner(), &account_id, "codex", "Codex")?;
    start_pkce_oauth_flow(
        auth_state.inner(),
        account_id,
        "/auth/callback",
        Some(1455),
        |redirect_uri, challenge, state| {
            provider_clients::codex::build_authorize_url(redirect_uri, challenge, state)
                .map_err(|err| err.to_string())
        },
    )
}

#[tauri::command]
async fn finish_codex_oauth(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    request_id: String,
    timeout_ms: Option<u64>,
) -> Result<OAuthResult, String> {
    let pending = auth_state
        .get(&request_id)
        .ok_or_else(|| "OAuth flow not found".to_string())?;
    let receiver = pending
        .take_receiver()
        .ok_or_else(|| "OAuth flow is already waiting for completion".to_string())?;
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_OAUTH_TIMEOUT_MS).max(1);

    let callback = match tokio::time::timeout(Duration::from_millis(timeout_ms), receiver).await {
        Ok(result) => match result {
            Ok(callback) => callback.map_err(|err| err.to_string())?,
            Err(_) => {
                auth_state.remove(&request_id);
                return Err("OAuth callback channel closed".to_string());
            }
        },
        Err(_) => {
            pending.cancel_flag.store(true, Ordering::SeqCst);
            auth_state.remove(&request_id);
            return Err("OAuth callback timed out".to_string());
        }
    };

    let credentials = match provider_clients::codex::exchange_code(
        &callback.code,
        &pending.verifier,
        &pending.redirect_uri,
    )
    .await
    {
        Ok(credentials) => credentials,
        Err(err) => {
            auth_state.remove(&request_id);
            return Err(err.to_string());
        }
    };

    let credentials_value = serde_json::to_value(credentials.clone().with_kind())
        .map_err(|err| err.to_string())?;
    if let Err(err) =
        secrets::set_account_credentials(&app, store.inner(), &pending.account_id, &credentials_value)
    {
        auth_state.remove(&request_id);
        return Err(err.to_string());
    }

    auth_state.remove(&request_id);
    Ok(OAuthResult {
        account_id: pending.account_id.clone(),
        expires_at: credentials.expires_at,
    })
}

#[tauri::command]
fn cancel_codex_oauth(auth_state: State<'_, AuthState>, request_id: String) -> bool {
    auth_state.cancel(&request_id)
}

#[tauri::command]
fn start_claude_oauth(
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    account_id: String,
) -> Result<OAuthStartResponse, String> {
    let _account = ensure_oauth_account(store.inner(), &account_id, "claude", "Claude")?;
    start_pkce_oauth_flow(
        auth_state.inner(),
        account_id,
        "/callback",
        None,
        |redirect_uri, challenge, state| {
            provider_clients::claude::build_authorize_url(redirect_uri, challenge, state)
                .map_err(|err| err.to_string())
        },
    )
}

#[tauri::command]
async fn finish_claude_oauth(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    request_id: String,
    timeout_ms: Option<u64>,
) -> Result<OAuthResult, String> {
    let pending = auth_state
        .get(&request_id)
        .ok_or_else(|| "OAuth flow not found".to_string())?;
    let receiver = pending
        .take_receiver()
        .ok_or_else(|| "OAuth flow is already waiting for completion".to_string())?;
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_OAUTH_TIMEOUT_MS).max(1);

    let callback = match tokio::time::timeout(Duration::from_millis(timeout_ms), receiver).await {
        Ok(result) => match result {
            Ok(callback) => callback.map_err(|err| err.to_string())?,
            Err(_) => {
                auth_state.remove(&request_id);
                return Err("OAuth callback channel closed".to_string());
            }
        },
        Err(_) => {
            pending.cancel_flag.store(true, Ordering::SeqCst);
            auth_state.remove(&request_id);
            return Err("OAuth callback timed out".to_string());
        }
    };

    let credentials = match provider_clients::claude::exchange_code(
        &callback.code,
        &callback.state,
        &pending.verifier,
        &pending.redirect_uri,
    )
    .await
    {
        Ok(credentials) => credentials,
        Err(err) => {
            auth_state.remove(&request_id);
            return Err(err.to_string());
        }
    };

    let credentials_value = serde_json::to_value(credentials.clone().with_kind())
        .map_err(|err| err.to_string())?;
    if let Err(err) =
        secrets::set_account_credentials(&app, store.inner(), &pending.account_id, &credentials_value)
    {
        auth_state.remove(&request_id);
        return Err(err.to_string());
    }

    auth_state.remove(&request_id);
    Ok(OAuthResult {
        account_id: pending.account_id.clone(),
        expires_at: credentials.expires_at,
    })
}

#[tauri::command]
fn cancel_claude_oauth(auth_state: State<'_, AuthState>, request_id: String) -> bool {
    auth_state.cancel(&request_id)
}

#[tauri::command]
async fn start_copilot_oauth(
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    account_id: String,
) -> Result<OAuthStartResponse, String> {
    let _account = ensure_oauth_account(store.inner(), &account_id, "copilot", "Copilot")?;

    let device_response = provider_clients::copilot::request_device_code()
        .await
        .map_err(|err| err.to_string())?;
    let request_id = Uuid::new_v4().to_string();
    let expires_at = now_unix_ms().saturating_add(device_response.expires_in.saturating_mul(1000));

    let pending = PendingOAuth::new_device_flow(
        account_id,
        device_response.device_code.clone(),
        device_response.interval,
        expires_at,
    );
    auth_state.insert(request_id.clone(), pending);

    let redirect_uri = device_response.verification_uri.clone();
    let url = device_response
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| redirect_uri.clone());

    Ok(OAuthStartResponse {
        request_id,
        url,
        redirect_uri,
        user_code: Some(device_response.user_code),
    })
}

#[tauri::command]
async fn finish_copilot_oauth(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    request_id: String,
    timeout_ms: Option<u64>,
) -> Result<OAuthResult, String> {
    let pending = auth_state
        .get(&request_id)
        .ok_or_else(|| "OAuth flow not found".to_string())?;

    let device_code = pending
        .device_code
        .clone()
        .ok_or_else(|| "OAuth flow not found".to_string())?;
    let interval = pending.device_interval.unwrap_or(5).max(1);
    let mut timeout_ms = timeout_ms.unwrap_or(DEFAULT_OAUTH_TIMEOUT_MS).max(1);

    if let Some(expires_at) = pending.device_expires_at {
        let remaining = expires_at.saturating_sub(now_unix_ms());
        if remaining <= 0 {
            auth_state.remove(&request_id);
            return Err("OAuth device code expired".to_string());
        }
        timeout_ms = timeout_ms.min(remaining as u64);
    }

    let poll_future = provider_clients::copilot::poll_for_token(
        &device_code,
        interval,
        Some(&pending.cancel_flag),
    );

    let credentials = match tokio::time::timeout(Duration::from_millis(timeout_ms), poll_future).await
    {
        Ok(result) => match result {
            Ok(credentials) => credentials,
            Err(err) => {
                auth_state.remove(&request_id);
                return Err(err.to_string());
            }
        },
        Err(_) => {
            pending.cancel_flag.store(true, Ordering::SeqCst);
            auth_state.remove(&request_id);
            return Err("OAuth callback timed out".to_string());
        }
    };

    let credentials_value = serde_json::to_value(credentials.clone().with_kind())
        .map_err(|err| err.to_string())?;
    if let Err(err) =
        secrets::set_account_credentials(&app, store.inner(), &pending.account_id, &credentials_value)
    {
        auth_state.remove(&request_id);
        return Err(err.to_string());
    }

    auth_state.remove(&request_id);
    Ok(OAuthResult {
        account_id: pending.account_id.clone(),
        expires_at: credentials.expires_at.unwrap_or(0),
    })
}

#[tauri::command]
fn cancel_copilot_oauth(auth_state: State<'_, AuthState>, request_id: String) -> bool {
    auth_state.cancel(&request_id)
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
            app.manage(AuthState::new());

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
            clear_account_credentials,
            start_codex_oauth,
            finish_codex_oauth,
            cancel_codex_oauth,
            start_claude_oauth,
            finish_claude_oauth,
            cancel_claude_oauth,
            start_copilot_oauth,
            finish_copilot_oauth,
            cancel_copilot_oauth
        ])
        .run(context)
        .expect("error while running tauri application");
}
