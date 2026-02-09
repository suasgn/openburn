use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::error::{BackendError, Result};
use crate::provider_clients::shorten_body;
use crate::utils::now_unix_ms;

const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USAGE_URL: &str = "https://api.github.com/copilot_internal/user";
const SCOPE: &str = "read:user";
const USER_AGENT: &str = "GitHubCopilotChat/0.26.7";
const EDITOR_VERSION: &str = "vscode/1.96.2";
const EDITOR_PLUGIN_VERSION: &str = "copilot-chat/0.26.7";
const API_VERSION: &str = "2025-04-01";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotCredentials {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(rename = "access_token", alias = "accessToken")]
    pub access_token: String,
    #[serde(rename = "token_type", alias = "tokenType", default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(rename = "expires_at", alias = "expiresAt", default)]
    pub expires_at: Option<i64>,
}

impl CopilotCredentials {
    pub fn with_kind(mut self) -> Self {
        self.kind = Some("oauth".to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotDeviceCodeResponse {
    #[serde(rename = "device_code")]
    pub device_code: String,
    #[serde(rename = "user_code")]
    pub user_code: String,
    #[serde(rename = "verification_uri")]
    pub verification_uri: String,
    #[serde(rename = "verification_uri_complete", default)]
    pub verification_uri_complete: Option<String>,
    #[serde(rename = "expires_in")]
    pub expires_in: i64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

pub async fn request_device_code() -> Result<CopilotDeviceCodeResponse> {
    let client = Client::new();
    let response = client
        .post(DEVICE_CODE_URL)
        .header("accept", "application/json")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", USER_AGENT)
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Copilot OAuth device request failed: {err}")))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        let body = shorten_body(&body);
        let message = if body.is_empty() {
            format!("Copilot OAuth device request failed: HTTP {status}")
        } else {
            format!("Copilot OAuth device request failed: HTTP {status} - {body}")
        };
        return Err(BackendError::Provider(message));
    }

    response
        .json::<CopilotDeviceCodeResponse>()
        .await
        .map_err(|err| BackendError::Provider(format!("Copilot OAuth device decode failed: {err}")))
}

pub async fn poll_for_token(
    device_code: &str,
    interval_seconds: u64,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<CopilotCredentials> {
    let client = Client::new();
    let mut interval_seconds = interval_seconds.max(1);

    loop {
        if is_cancelled(cancel_flag) {
            return Err(BackendError::Provider("OAuth cancelled".to_string()));
        }

        sleep(Duration::from_secs(interval_seconds)).await;
        if is_cancelled(cancel_flag) {
            return Err(BackendError::Provider("OAuth cancelled".to_string()));
        }

        let response = client
            .post(ACCESS_TOKEN_URL)
            .header("accept", "application/json")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("user-agent", USER_AGENT)
            .form(&[
                ("client_id", CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|err| BackendError::Provider(format!("Copilot OAuth token request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        if !status.is_success() {
            let body = shorten_body(&body);
            let message = if body.is_empty() {
                format!("Copilot OAuth token request failed: HTTP {status}")
            } else {
                format!("Copilot OAuth token request failed: HTTP {status} - {body}")
            };
            return Err(BackendError::Provider(message));
        }

        let token = serde_json::from_str::<DeviceTokenResponse>(&body)
            .map_err(|err| BackendError::Provider(format!("Copilot OAuth token decode failed: {err}")))?;

        if let Some(access_token) = token.access_token {
            let expires_at = token
                .expires_in
                .map(|expires_in| now_unix_ms().saturating_add(expires_in.saturating_mul(1000)));

            return Ok(CopilotCredentials {
                kind: Some("oauth".to_string()),
                access_token,
                token_type: token.token_type,
                scope: token.scope,
                expires_at,
            });
        }

        let error = token.error.unwrap_or_else(|| "unknown_error".to_string());
        match error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval_seconds = interval_seconds.saturating_add(5);
                continue;
            }
            "expired_token" => {
                return Err(BackendError::Provider(
                    "Copilot OAuth device code expired".to_string(),
                ))
            }
            _ => {
                let detail = token.error_description.unwrap_or_default();
                let detail = detail.trim();
                let message = if detail.is_empty() {
                    format!("Copilot OAuth token request failed: {error}")
                } else {
                    format!("Copilot OAuth token request failed: {error} - {detail}")
                };
                return Err(BackendError::Provider(message));
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotUsageResponse {
    #[serde(rename = "copilotPlan", alias = "copilot_plan", default)]
    pub copilot_plan: Option<String>,
    #[serde(rename = "quotaSnapshots", alias = "quota_snapshots", default)]
    pub quota_snapshots: Option<CopilotQuotaSnapshots>,
    #[serde(rename = "quotaResetDate", alias = "quota_reset_date", default)]
    pub quota_reset_date: Option<String>,
    #[serde(rename = "limitedUserQuotas", alias = "limited_user_quotas", default)]
    pub limited_user_quotas: Option<CopilotLimitedQuotas>,
    #[serde(rename = "monthlyQuotas", alias = "monthly_quotas", default)]
    pub monthly_quotas: Option<CopilotLimitedQuotas>,
    #[serde(
        rename = "limitedUserResetDate",
        alias = "limited_user_reset_date",
        default
    )]
    pub limited_user_reset_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotQuotaSnapshots {
    #[serde(
        rename = "premiumInteractions",
        alias = "premium_interactions",
        default
    )]
    pub premium_interactions: Option<CopilotQuotaSnapshot>,
    #[serde(default)]
    pub chat: Option<CopilotQuotaSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotQuotaSnapshot {
    #[serde(rename = "percentRemaining", alias = "percent_remaining", default)]
    pub percent_remaining: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotLimitedQuotas {
    #[serde(default)]
    pub chat: Option<f64>,
    #[serde(default)]
    pub completions: Option<f64>,
}

pub async fn fetch_usage(access_token: &str) -> Result<CopilotUsageResponse> {
    let client = Client::new();
    let response = client
        .get(USAGE_URL)
        .header("authorization", format!("token {access_token}"))
        .header("accept", "application/json")
        .header("editor-version", EDITOR_VERSION)
        .header("editor-plugin-version", EDITOR_PLUGIN_VERSION)
        .header("user-agent", USER_AGENT)
        .header("x-github-api-version", API_VERSION)
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Copilot usage request failed: {err}")))?;

    let status = response.status();
    if status.is_success() {
        return response
            .json::<CopilotUsageResponse>()
            .await
            .map_err(|err| BackendError::Provider(format!("Copilot usage decode failed: {err}")));
    }

    let body = response.text().await.unwrap_or_else(|_| "".to_string());
    let body = shorten_body(&body);
    let message = if body.is_empty() {
        format!("Copilot usage request failed: HTTP {status}")
    } else {
        format!("Copilot usage request failed: HTTP {status} - {body}")
    };
    Err(BackendError::Provider(message))
}

fn is_cancelled(cancel_flag: Option<&Arc<AtomicBool>>) -> bool {
    cancel_flag
        .map(|flag| flag.load(Ordering::SeqCst))
        .unwrap_or(false)
}
