use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{BackendError, Result};
use crate::provider_clients::shorten_body;
use crate::utils::now_unix_ms;

const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTH_URL: &str = "https://claude.ai/oauth/authorize";
const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA_HEADER: &str = "oauth-2025-04-20";
const SCOPE: &str =
    "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCredentials {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(rename = "access_token", alias = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refresh_token", alias = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expires_at", alias = "expiresAt", default)]
    pub expires_at: i64,
    #[serde(rename = "subscriptionType", alias = "subscription_type", default)]
    pub subscription_type: Option<String>,
}

impl ClaudeCredentials {
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
    refresh_token: String,
    expires_in: i64,
}

pub fn build_authorize_url(
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> Result<String> {
    let mut url = Url::parse(AUTH_URL)
        .map_err(|err| BackendError::Provider(format!("OAuth URL invalid: {err}")))?;
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    Ok(url.to_string())
}

pub async fn exchange_code(
    code: &str,
    state: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<ClaudeCredentials> {
    let client = Client::new();
    let response = client
        .post(TOKEN_URL)
        .json(&serde_json::json!({
            "code": code,
            "state": state,
            "grant_type": "authorization_code",
            "client_id": CLIENT_ID,
            "redirect_uri": redirect_uri,
            "code_verifier": verifier,
        }))
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("OAuth token request failed: {err}")))?;

    handle_token_response(response).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsageResponse {
    #[serde(rename = "five_hour")]
    pub five_hour: Option<ClaudeUsageWindow>,
    #[serde(rename = "seven_day")]
    pub seven_day: Option<ClaudeUsageWindow>,
    #[serde(rename = "seven_day_oauth_apps")]
    pub seven_day_oauth_apps: Option<ClaudeUsageWindow>,
    #[serde(rename = "seven_day_opus")]
    pub seven_day_opus: Option<ClaudeUsageWindow>,
    #[serde(rename = "seven_day_sonnet")]
    pub seven_day_sonnet: Option<ClaudeUsageWindow>,
    #[serde(rename = "iguana_necktie")]
    pub iguana_necktie: Option<ClaudeUsageWindow>,
    #[serde(rename = "extra_usage")]
    pub extra_usage: Option<ClaudeExtraUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsageWindow {
    pub utilization: Option<f64>,
    #[serde(rename = "resets_at")]
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeExtraUsage {
    #[serde(rename = "is_enabled")]
    pub is_enabled: Option<bool>,
    #[serde(rename = "monthly_limit")]
    pub monthly_limit: Option<f64>,
    #[serde(rename = "used_credits")]
    pub used_credits: Option<f64>,
    pub utilization: Option<f64>,
    pub currency: Option<String>,
}

pub async fn refresh_credentials(refresh_token: &str) -> Result<ClaudeCredentials> {
    let client = Client::new();
    let response = client
        .post(TOKEN_URL)
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLIENT_ID,
        }))
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Claude OAuth refresh failed: {err}")))?;

    handle_token_response(response).await
}

pub async fn fetch_usage(access_token: &str) -> Result<ClaudeUsageResponse> {
    let client = Client::new();
    let response = client
        .get(USAGE_URL)
        .bearer_auth(access_token)
        .header("anthropic-beta", BETA_HEADER)
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .header("user-agent", "openburn")
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Claude usage request failed: {err}")))?;

    let status = response.status();
    if status.is_success() {
        return response
            .json::<ClaudeUsageResponse>()
            .await
            .map_err(|err| BackendError::Provider(format!("Claude usage decode failed: {err}")));
    }

    let body = response.text().await.unwrap_or_else(|_| "".to_string());
    let body = shorten_body(&body);
    let message = if body.is_empty() {
        format!("Claude usage request failed: HTTP {status}")
    } else {
        format!("Claude usage request failed: HTTP {status} - {body}")
    };
    Err(BackendError::Provider(message))
}

async fn handle_token_response(response: reqwest::Response) -> Result<ClaudeCredentials> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        let body = shorten_body(&body);
        let message = if body.is_empty() {
            format!("OAuth token request failed: HTTP {status}")
        } else {
            format!("OAuth token request failed: HTTP {status} - {body}")
        };
        return Err(BackendError::Provider(message));
    }

    let token = response
        .json::<TokenResponse>()
        .await
        .map_err(|err| BackendError::Provider(format!("OAuth token decode failed: {err}")))?;
    let expires_at = now_unix_ms().saturating_add(token.expires_in.saturating_mul(1000));

    Ok(ClaudeCredentials {
        kind: Some("oauth".to_string()),
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at,
        subscription_type: None,
    })
}
