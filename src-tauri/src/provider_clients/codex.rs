use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{BackendError, Result};
use crate::provider_clients::shorten_body;
use crate::utils::now_unix_ms;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCredentials {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(rename = "access_token", alias = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refresh_token", alias = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expires_at", alias = "expiresAt", default)]
    pub expires_at: i64,
    #[serde(rename = "account_id", alias = "accountId", default)]
    pub account_id: Option<String>,
}

impl CodexCredentials {
    pub fn is_expired(&self) -> bool {
        now_unix_ms().saturating_add(60_000) >= self.expires_at
    }

    pub fn with_kind(mut self) -> Self {
        self.kind = Some("oauth".to_string());
        self
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexUsageResponse {
    #[serde(default)]
    pub plan_type: Option<String>,
    #[serde(default)]
    pub rate_limit: Option<CodexRateLimitStatus>,
    #[serde(rename = "code_review_rate_limit", default)]
    pub code_review_rate_limit: Option<CodexRateLimitStatus>,
    #[serde(default)]
    pub credits: Option<CodexCreditsStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRateLimitStatus {
    #[serde(default)]
    pub primary_window: Option<CodexRateLimitWindow>,
    #[serde(default)]
    pub secondary_window: Option<CodexRateLimitWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRateLimitWindow {
    #[serde(default)]
    pub used_percent: Option<f64>,
    #[serde(rename = "limit_window_seconds", default)]
    pub limit_window_seconds: Option<i64>,
    #[serde(rename = "reset_at", default)]
    pub reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCreditsStatus {
    #[serde(default)]
    pub has_credits: Option<bool>,
    #[serde(default)]
    pub unlimited: Option<bool>,
    #[serde(default)]
    pub balance: Option<String>,
}

pub async fn refresh_credentials(
    refresh_token: &str,
    account_id: Option<&str>,
) -> Result<CodexCredentials> {
    let client = Client::new();
    let response = client
        .post(TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Codex OAuth refresh failed: {err}")))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        let body = shorten_body(&body);
        let message = if body.is_empty() {
            format!("Codex OAuth refresh failed: HTTP {status}")
        } else {
            format!("Codex OAuth refresh failed: HTTP {status} - {body}")
        };
        return Err(BackendError::Provider(message));
    }

    let token = response
        .json::<TokenResponse>()
        .await
        .map_err(|err| BackendError::Provider(format!("Codex OAuth decode failed: {err}")))?;
    let expires_in = token.expires_in.unwrap_or(3600).max(1);
    let expires_at = now_unix_ms().saturating_add(expires_in.saturating_mul(1000));

    Ok(CodexCredentials {
        kind: Some("oauth".to_string()),
        access_token: token.access_token,
        refresh_token: token
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        expires_at,
        account_id: account_id.map(|value| value.to_string()),
    })
}

pub async fn fetch_usage(
    access_token: &str,
    account_id: Option<&str>,
) -> Result<CodexUsageResponse> {
    let client = Client::new();
    let mut request = client
        .get(USAGE_URL)
        .bearer_auth(access_token)
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .header("user-agent", "openburn");

    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Codex usage request failed: {err}")))?;
    let status = response.status();
    if status.is_success() {
        return response
            .json::<CodexUsageResponse>()
            .await
            .map_err(|err| BackendError::Provider(format!("Codex usage decode failed: {err}")));
    }

    let body = response.text().await.unwrap_or_else(|_| "".to_string());
    let body = shorten_body(&body);
    let message = if body.is_empty() {
        format!("Codex usage request failed: HTTP {status}")
    } else {
        format!("Codex usage request failed: HTTP {status} - {body}")
    };
    Err(BackendError::Provider(message))
}
