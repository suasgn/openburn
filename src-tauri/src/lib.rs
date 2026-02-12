mod account_store;
#[cfg(target_os = "macos")]
mod app_nap;
mod auth;
mod error;
mod models;
mod oauth;
mod panel;
mod probe;
mod providers;
mod secrets;
mod tray;
mod utils;
#[cfg(target_os = "macos")]
mod webkit_config;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use account_store::AccountStore;
use auth::{AuthState, PendingOAuth};
use futures::future::join_all;
use models::{AccountRecord, CreateAccountInput, UpdateAccountInput};
use probe::{ProbeBatchCompleteEvent, ProbeBatchStarted, ProbeResultEvent, ProviderMeta};
use providers::{
    clients, find_provider_contract, validate_auth_strategy_for_provider, ProviderDescriptor,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_log::{Target, TargetKind};
use utils::now_unix_ms;
use uuid::Uuid;

const DEFAULT_OAUTH_TIMEOUT_MS: u64 = 180_000;
const OPENCODE_LOGIN_URL: &str = "https://opencode.ai/auth";
const OPENCODE_COOKIE_POLL_INTERVAL_MS: u64 = 400;
const OPENCODE_COOKIE_URLS: [&str; 3] = [
    "https://opencode.ai/_server",
    "https://opencode.ai/workspace/",
    "https://opencode.ai/auth",
];

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

    let outputs = join_all(selected_ids.iter().map(|provider_id| async {
        match probe::probe_provider(&app_handle, store.inner(), provider_id).await {
            Ok(output) => output,
            Err(err) => probe::build_error_output(provider_id, err.to_string()),
        }
    }))
    .await;

    for output in outputs {
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

fn ensure_provider_account_with_auth_strategy(
    store: &AccountStore,
    account_id: &str,
    expected_provider_id: &str,
    provider_label: &str,
    required_auth_strategy_id: &str,
    required_auth_label: &str,
) -> Result<AccountRecord, String> {
    let account = store
        .get_account(account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "Account not found".to_string())?;

    if account.provider_id != expected_provider_id {
        return Err(format!(
            "{provider_label} {required_auth_label} requires a {expected_provider_id} account"
        ));
    }

    let provider = find_provider_contract(expected_provider_id)
        .ok_or_else(|| format!("provider '{}' is not registered", expected_provider_id))?;

    let effective_strategy = account
        .auth_strategy_id
        .as_deref()
        .unwrap_or(provider.default_auth_strategy_id);

    validate_auth_strategy_for_provider(provider, Some(effective_strategy))
        .map_err(|err| err.to_string())?;

    if effective_strategy != required_auth_strategy_id {
        return Err(format!(
            "{provider_label} {required_auth_label} requires authStrategyId '{}'",
            required_auth_strategy_id
        ));
    }

    Ok(account)
}

fn ensure_oauth_account(
    store: &AccountStore,
    account_id: &str,
    expected_provider_id: &str,
    provider_label: &str,
) -> Result<AccountRecord, String> {
    ensure_provider_account_with_auth_strategy(
        store,
        account_id,
        expected_provider_id,
        provider_label,
        "oauth",
        "OAuth",
    )
}

fn opencode_auth_window_label(request_id: &str) -> String {
    format!("opencode-auth-{request_id}")
}

fn sanitize_url_for_log(url: &url::Url) -> String {
    let mut url = url.clone();
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

fn close_webview_window_if_exists(app: &tauri::AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    }
}

fn opencode_cookie_header_from_window(
    window: &tauri::WebviewWindow,
) -> Result<Option<String>, String> {
    let mut total_cookies_seen = 0usize;

    for raw_url in OPENCODE_COOKIE_URLS {
        let url = url::Url::parse(raw_url)
            .map_err(|err| format!("OpenCode cookie URL is invalid: {err}"))?;
        let cookies = window
            .cookies_for_url(url)
            .map_err(|err| format!("Failed to read OpenCode cookies: {err}"))?;
        let mut source_pairs: Vec<(String, String)> = Vec::new();
        let mut seen_names = HashSet::new();

        for cookie in cookies {
            let name = cookie.name().trim();
            let value = cookie.value().trim();
            if name.is_empty() || value.is_empty() {
                continue;
            }

            total_cookies_seen = total_cookies_seen.saturating_add(1);

            if !seen_names.insert(name.to_string()) {
                continue;
            }

            source_pairs.push((name.to_string(), value.to_string()));
        }

        let mut cookie_names = source_pairs
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        cookie_names.sort();

        let header = clients::opencode::cookie_header_from_pairs(
            source_pairs
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        );

        let has_auth_cookie = source_pairs
            .iter()
            .any(|(name, _)| name == "auth" || name == "__Host-auth");
        log::info!(
            "[opencode-auth] cookies source_url={} unique_names={} has_auth_cookie={} header_ready={} cookie_names={}",
            raw_url,
            source_pairs.len(),
            has_auth_cookie,
            header.is_some(),
            cookie_names.join(",")
        );

        if header.is_some() {
            log::info!(
                "[opencode-auth] selected cookie source source_url={} total_seen_so_far={}",
                raw_url,
                total_cookies_seen
            );
            return Ok(header);
        }
    }

    log::info!(
        "[opencode-auth] no usable auth cookie found total_seen={}",
        total_cookies_seen
    );
    Ok(None)
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
    let redirect_uri = format!("http://localhost:{port}{callback_path}");
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

async fn wait_for_pkce_callback(
    auth_state: &AuthState,
    request_id: &str,
    timeout_ms: Option<u64>,
) -> Result<(Arc<PendingOAuth>, auth::OAuthCallback), String> {
    let pending = auth_state
        .get(request_id)
        .ok_or_else(|| "OAuth flow not found".to_string())?;
    let receiver = pending
        .take_receiver()
        .ok_or_else(|| "OAuth flow is already waiting for completion".to_string())?;
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_OAUTH_TIMEOUT_MS).max(1);

    let callback = match tokio::time::timeout(Duration::from_millis(timeout_ms), receiver).await {
        Ok(result) => match result {
            Ok(callback) => callback.map_err(|err| err.to_string())?,
            Err(_) => {
                auth_state.remove(request_id);
                return Err("OAuth callback channel closed".to_string());
            }
        },
        Err(_) => {
            pending.cancel_flag.store(true, Ordering::SeqCst);
            auth_state.remove(request_id);
            return Err("OAuth callback timed out".to_string());
        }
    };

    Ok((pending, callback))
}

fn persist_oauth_credentials(
    app: &tauri::AppHandle,
    store: &AccountStore,
    auth_state: &AuthState,
    request_id: &str,
    account_id: &str,
    credentials: &serde_json::Value,
) -> Result<(), String> {
    if let Err(err) = secrets::set_account_credentials(app, store, account_id, credentials) {
        auth_state.remove(request_id);
        return Err(err.to_string());
    }

    auth_state.remove(request_id);
    Ok(())
}

fn persist_opencode_workspace_setting(
    store: &AccountStore,
    account_id: &str,
    workspace_id: &str,
) -> Result<(), String> {
    let workspace_id = workspace_id.trim();
    if workspace_id.is_empty() {
        return Ok(());
    }

    let account = store
        .get_account(account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "Account not found".to_string())?;

    if account.provider_id != "opencode" {
        return Ok(());
    }

    let mut settings = account
        .settings
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);

    let unchanged = settings
        .get("workspaceId")
        .and_then(|value| value.as_str())
        .map(|value| value == workspace_id)
        .unwrap_or(false);
    if unchanged {
        return Ok(());
    }

    settings.insert(
        "workspaceId".to_string(),
        serde_json::Value::String(workspace_id.to_string()),
    );

    store
        .update_account(
            account_id,
            UpdateAccountInput {
                auth_strategy_id: None,
                label: None,
                settings: Some(serde_json::Value::Object(settings)),
                clear_last_error: false,
            },
        )
        .map_err(|err| err.to_string())?;

    Ok(())
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
            clients::codex::build_authorize_url(redirect_uri, challenge, state)
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
    let (pending, callback) =
        wait_for_pkce_callback(auth_state.inner(), &request_id, timeout_ms).await?;

    let credentials = match clients::codex::exchange_code(
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

    let credentials_value =
        serde_json::to_value(credentials.clone().with_kind()).map_err(|err| err.to_string())?;
    persist_oauth_credentials(
        &app,
        store.inner(),
        auth_state.inner(),
        &request_id,
        &pending.account_id,
        &credentials_value,
    )?;

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
fn start_antigravity_oauth(
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    account_id: String,
) -> Result<OAuthStartResponse, String> {
    let _account = ensure_oauth_account(store.inner(), &account_id, "antigravity", "Antigravity")?;
    start_pkce_oauth_flow(
        auth_state.inner(),
        account_id,
        "/auth/callback",
        None,
        |redirect_uri, challenge, state| {
            clients::antigravity::build_authorize_url(redirect_uri, challenge, state)
                .map_err(|err| err.to_string())
        },
    )
}

#[tauri::command]
async fn finish_antigravity_oauth(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    request_id: String,
    timeout_ms: Option<u64>,
) -> Result<OAuthResult, String> {
    let (pending, callback) =
        wait_for_pkce_callback(auth_state.inner(), &request_id, timeout_ms).await?;

    let credentials = match clients::antigravity::exchange_code(
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

    let credentials_value =
        serde_json::to_value(credentials.clone().with_kind()).map_err(|err| err.to_string())?;
    persist_oauth_credentials(
        &app,
        store.inner(),
        auth_state.inner(),
        &request_id,
        &pending.account_id,
        &credentials_value,
    )?;

    Ok(OAuthResult {
        account_id: pending.account_id.clone(),
        expires_at: credentials.expires_at,
    })
}

#[tauri::command]
fn cancel_antigravity_oauth(auth_state: State<'_, AuthState>, request_id: String) -> bool {
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
            clients::claude::build_authorize_url(redirect_uri, challenge, state)
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
    let (pending, callback) =
        wait_for_pkce_callback(auth_state.inner(), &request_id, timeout_ms).await?;

    let credentials = match clients::claude::exchange_code(
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

    let credentials_value =
        serde_json::to_value(credentials.clone().with_kind()).map_err(|err| err.to_string())?;
    persist_oauth_credentials(
        &app,
        store.inner(),
        auth_state.inner(),
        &request_id,
        &pending.account_id,
        &credentials_value,
    )?;

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

    let device_response = clients::copilot::request_device_code()
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

    let poll_future =
        clients::copilot::poll_for_token(&device_code, interval, Some(&pending.cancel_flag));

    let credentials =
        match tokio::time::timeout(Duration::from_millis(timeout_ms), poll_future).await {
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

    let credentials_value =
        serde_json::to_value(credentials.clone().with_kind()).map_err(|err| err.to_string())?;
    persist_oauth_credentials(
        &app,
        store.inner(),
        auth_state.inner(),
        &request_id,
        &pending.account_id,
        &credentials_value,
    )?;

    Ok(OAuthResult {
        account_id: pending.account_id.clone(),
        expires_at: credentials.expires_at.unwrap_or(0),
    })
}

#[tauri::command]
fn cancel_copilot_oauth(auth_state: State<'_, AuthState>, request_id: String) -> bool {
    auth_state.cancel(&request_id)
}

#[tauri::command]
async fn start_opencode_oauth(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    account_id: String,
) -> Result<OAuthStartResponse, String> {
    let _account = ensure_provider_account_with_auth_strategy(
        store.inner(),
        &account_id,
        "opencode",
        "OpenCode",
        "cookie",
        "Cookie login",
    )?;

    let request_id = Uuid::new_v4().to_string();
    let window_label = opencode_auth_window_label(&request_id);
    close_webview_window_if_exists(&app, &window_label);

    let login_url = url::Url::parse(OPENCODE_LOGIN_URL)
        .map_err(|err| format!("OpenCode login URL is invalid: {err}"))?;

    tauri::WebviewWindowBuilder::new(
        &app,
        &window_label,
        tauri::WebviewUrl::External(login_url.clone()),
    )
    .title("OpenCode Login")
    .inner_size(1120.0, 760.0)
    .resizable(true)
    .incognito(true)
    .build()
    .map_err(|err| format!("Failed to open OpenCode login window: {err}"))?;

    log::info!(
        "[opencode-auth] login window opened label={} url={} account_id={}",
        window_label,
        OPENCODE_LOGIN_URL,
        account_id
    );

    let expires_at = now_unix_ms().saturating_add(DEFAULT_OAUTH_TIMEOUT_MS as i64);
    let pending = PendingOAuth::new_device_flow(account_id, window_label, 1, expires_at);
    auth_state.insert(request_id.clone(), pending);

    Ok(OAuthStartResponse {
        request_id,
        url: OPENCODE_LOGIN_URL.to_string(),
        redirect_uri: OPENCODE_LOGIN_URL.to_string(),
        user_code: None,
    })
}

#[tauri::command]
async fn finish_opencode_oauth(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    auth_state: State<'_, AuthState>,
    request_id: String,
    timeout_ms: Option<u64>,
) -> Result<OAuthResult, String> {
    let pending = auth_state
        .get(&request_id)
        .ok_or_else(|| "OAuth flow not found".to_string())?;

    let window_label = pending
        .device_code
        .clone()
        .ok_or_else(|| "OAuth flow not found".to_string())?;

    let mut timeout_ms = timeout_ms.unwrap_or(DEFAULT_OAUTH_TIMEOUT_MS).max(1);
    if let Some(expires_at) = pending.device_expires_at {
        let remaining = expires_at.saturating_sub(now_unix_ms());
        if remaining <= 0 {
            auth_state.remove(&request_id);
            close_webview_window_if_exists(&app, &window_label);
            log::warn!(
                "[opencode-auth] login flow timed out request_id={}",
                request_id
            );
            return Err("OAuth callback timed out".to_string());
        }
        timeout_ms = timeout_ms.min(remaining as u64);
    }

    log::info!(
        "[opencode-auth] waiting for session capture request_id={} timeout_ms={}",
        request_id,
        timeout_ms
    );

    let started_at = std::time::Instant::now();
    let mut last_url_seen: Option<String> = None;
    let mut captured_workspace_id: Option<String> = None;
    let mut logged_cookie_without_workspace = false;
    let mut logged_workspace_without_cookie = false;

    loop {
        if pending.cancel_flag.load(Ordering::SeqCst) {
            auth_state.remove(&request_id);
            close_webview_window_if_exists(&app, &window_label);
            log::warn!(
                "[opencode-auth] login flow cancelled request_id={}",
                request_id
            );
            return Err("OAuth cancelled".to_string());
        }

        if started_at.elapsed() >= Duration::from_millis(timeout_ms) {
            pending.cancel_flag.store(true, Ordering::SeqCst);
            auth_state.remove(&request_id);
            close_webview_window_if_exists(&app, &window_label);
            log::warn!(
                "[opencode-auth] login flow timed out request_id={}",
                request_id
            );
            return Err("OAuth callback timed out".to_string());
        }

        let Some(window) = app.get_webview_window(&window_label) else {
            auth_state.remove(&request_id);
            log::warn!(
                "[opencode-auth] login window closed before capture request_id={}",
                request_id
            );
            return Err("OpenCode login window closed before session was captured".to_string());
        };

        if let Ok(url) = window.url() {
            let sanitized = sanitize_url_for_log(&url);
            if last_url_seen.as_deref() != Some(sanitized.as_str()) {
                log::info!("[opencode-auth] navigation {}", sanitized);
                last_url_seen = Some(sanitized);
            }

            let workspace_id_from_url =
                clients::opencode::normalize_workspace_id(Some(url.as_str()));
            if let Some(workspace_id_from_url) = workspace_id_from_url {
                if captured_workspace_id.as_deref() != Some(workspace_id_from_url.as_str()) {
                    log::info!(
                        "[opencode-auth] captured workspace id from redirect workspace_id={}",
                        workspace_id_from_url
                    );
                }
                captured_workspace_id = Some(workspace_id_from_url);
            }
        }

        let workspace_id_for_credentials = captured_workspace_id.clone();

        let cookie_header = opencode_cookie_header_from_window(&window)?;
        if cookie_header.is_some()
            && workspace_id_for_credentials.is_none()
            && !logged_cookie_without_workspace
        {
            log::info!("[opencode-auth] auth cookie detected, waiting for workspace redirect");
            logged_cookie_without_workspace = true;
        }

        if workspace_id_for_credentials.is_some()
            && cookie_header.is_none()
            && !logged_workspace_without_cookie
        {
            log::info!("[opencode-auth] workspace URL detected, waiting for auth cookie");
            logged_workspace_without_cookie = true;
        }

        if let (Some(cookie_header), Some(workspace_id)) =
            (cookie_header, workspace_id_for_credentials)
        {
            let workspace_id_for_log = workspace_id.clone();
            let credentials = clients::opencode::OpenCodeCredentials {
                kind: Some("cookie".to_string()),
                cookie_header,
            };
            let credentials_value =
                serde_json::to_value(credentials.with_kind()).map_err(|err| err.to_string())?;

            persist_oauth_credentials(
                &app,
                store.inner(),
                auth_state.inner(),
                &request_id,
                &pending.account_id,
                &credentials_value,
            )?;

            persist_opencode_workspace_setting(
                store.inner(),
                &pending.account_id,
                &workspace_id_for_log,
            )?;

            close_webview_window_if_exists(&app, &window_label);
            log::info!(
                "[opencode-auth] session captured request_id={} account_id={} workspace_id={}",
                request_id,
                pending.account_id,
                workspace_id_for_log
            );
            return Ok(OAuthResult {
                account_id: pending.account_id.clone(),
                expires_at: 0,
            });
        }

        tokio::time::sleep(Duration::from_millis(OPENCODE_COOKIE_POLL_INTERVAL_MS)).await;
    }
}

#[tauri::command]
fn cancel_opencode_oauth(
    app: tauri::AppHandle,
    auth_state: State<'_, AuthState>,
    request_id: String,
) -> bool {
    let window_label = auth_state
        .get(&request_id)
        .and_then(|pending| pending.device_code.clone());

    let cancelled = auth_state.cancel(&request_id);
    if let Some(label) = window_label {
        close_webview_window_if_exists(&app, &label);
    }

    log::info!(
        "[opencode-auth] cancel requested request_id={} cancelled={}",
        request_id,
        cancelled
    );

    cancelled
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
            start_antigravity_oauth,
            finish_antigravity_oauth,
            cancel_antigravity_oauth,
            start_claude_oauth,
            finish_claude_oauth,
            cancel_claude_oauth,
            start_copilot_oauth,
            finish_copilot_oauth,
            cancel_copilot_oauth,
            start_opencode_oauth,
            finish_opencode_oauth,
            cancel_opencode_oauth
        ])
        .run(context)
        .expect("error while running tauri application");
}
