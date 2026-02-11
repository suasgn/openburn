use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{BackendError, Result};
use crate::provider_clients::shorten_body;
use crate::utils::now_unix_ms;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const SCOPE: &str = "openid profile email offline_access";
const ORIGINATOR: &str = "codex_cli_rs";

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
    #[serde(default)]
    id_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenClaims {
    #[serde(default)]
    pub chatgpt_account_id: Option<String>,
    #[serde(default)]
    pub organizations: Option<Vec<OpenAiOrganization>>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    pub openai_auth: Option<OpenAiAuthClaims>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiAuthClaims {
    #[serde(default)]
    pub chatgpt_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiOrganization {
    pub id: String,
}

pub fn build_authorize_url(redirect_uri: &str, challenge: &str, state: &str) -> Result<String> {
    let mut url = Url::parse(AUTH_URL)
        .map_err(|err| BackendError::Provider(format!("OAuth URL invalid: {err}")))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("state", state)
        .append_pair("originator", ORIGINATOR);
    Ok(url.to_string())
}

pub async fn exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<CodexCredentials> {
    let client = Client::new();
    let response = client
        .post(TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("OAuth token request failed: {err}")))?;

    handle_token_response(response, None).await
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

    handle_token_response(response, account_id)
        .await
        .map(|credentials| {
            if credentials.refresh_token.trim().is_empty() {
                CodexCredentials {
                    refresh_token: refresh_token.to_string(),
                    ..credentials
                }
            } else {
                credentials
            }
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

pub fn parse_jwt_claims(token: &str) -> Option<IdTokenClaims> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _signature = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice::<IdTokenClaims>(&decoded).ok()
}

pub fn extract_account_id_from_claims(claims: &IdTokenClaims) -> Option<String> {
    if let Some(account_id) = claims.chatgpt_account_id.as_ref() {
        return Some(account_id.to_string());
    }
    if let Some(openai_auth) = claims.openai_auth.as_ref() {
        if let Some(account_id) = openai_auth.chatgpt_account_id.as_ref() {
            return Some(account_id.to_string());
        }
    }
    if let Some(organizations) = claims.organizations.as_ref() {
        if let Some(first) = organizations.first() {
            return Some(first.id.to_string());
        }
    }
    None
}

fn extract_account_id(tokens: &TokenResponse) -> Option<String> {
    if let Some(id_token) = tokens.id_token.as_ref().map(String::as_str) {
        if !id_token.is_empty() {
            if let Some(claims) = parse_jwt_claims(id_token) {
                if let Some(account_id) = extract_account_id_from_claims(&claims) {
                    return Some(account_id);
                }
            }
        }
    }
    let access_token = tokens.access_token.as_str();
    if !access_token.is_empty() {
        if let Some(claims) = parse_jwt_claims(access_token) {
            return extract_account_id_from_claims(&claims);
        }
    }
    None
}

async fn handle_token_response(
    response: reqwest::Response,
    fallback_account_id: Option<&str>,
) -> Result<CodexCredentials> {
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
    let expires_in = token.expires_in.unwrap_or(3600).max(1);
    let expires_at = now_unix_ms().saturating_add(expires_in.saturating_mul(1000));
    let account_id =
        extract_account_id(&token).or_else(|| fallback_account_id.map(|value| value.to_string()));

    Ok(CodexCredentials {
        kind: Some("oauth".to_string()),
        access_token: token.access_token,
        refresh_token: token.refresh_token.unwrap_or_default(),
        expires_at,
        account_id,
    })
}
