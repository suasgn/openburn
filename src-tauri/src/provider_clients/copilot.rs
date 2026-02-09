use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{BackendError, Result};
use crate::provider_clients::shorten_body;

const USAGE_URL: &str = "https://api.github.com/copilot_internal/user";
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
