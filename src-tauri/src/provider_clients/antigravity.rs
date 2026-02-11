use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use url::Url;

use crate::error::{BackendError, Result};
use crate::provider_clients::shorten_body;
use crate::utils::now_unix_ms;

const CLIENT_ID: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "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";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

const ENDPOINT_DAILY: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com";
const ENDPOINT_AUTOPUSH: &str = "https://autopush-cloudcode-pa.sandbox.googleapis.com";
const ENDPOINT_PROD: &str = "https://cloudcode-pa.googleapis.com";

const FETCH_ENDPOINTS: [&str; 3] = [ENDPOINT_DAILY, ENDPOINT_AUTOPUSH, ENDPOINT_PROD];
const LOAD_ENDPOINTS: [&str; 3] = [ENDPOINT_PROD, ENDPOINT_DAILY, ENDPOINT_AUTOPUSH];

pub const DEFAULT_PROJECT_ID: &str = "rising-fact-p41fc";

const SCOPES: [&str; 5] = [
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];

const USER_AGENT: &str = "antigravity/1.12.4 windows/amd64";
const LOAD_USER_AGENT: &str = "google-api-nodejs-client/9.15.1";
const API_CLIENT: &str = "google-cloud-sdk vscode_cloudshelleditor/0.1";
const CLIENT_METADATA: &str =
    "{\"ideType\":\"IDE_UNSPECIFIED\",\"platform\":\"PLATFORM_UNSPECIFIED\",\"pluginType\":\"GEMINI\"}";
const ONBOARD_ATTEMPTS: usize = 5;
const ONBOARD_DELAY_MS: u64 = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityCredentials {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(rename = "access_token", alias = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refresh_token", alias = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expires_at", alias = "expiresAt", default)]
    pub expires_at: i64,
    #[serde(rename = "project_id", alias = "projectId", default)]
    pub project_id: Option<String>,
    #[serde(rename = "managed_project_id", alias = "managedProjectId", default)]
    pub managed_project_id: Option<String>,
}

impl AntigravityCredentials {
    pub fn is_expired(&self) -> bool {
        now_unix_ms().saturating_add(60_000) >= self.expires_at
    }

    pub fn with_kind(mut self) -> Self {
        self.kind = Some("oauth".to_string());
        self
    }
}

#[derive(Debug)]
pub struct RefreshTokenParts {
    pub refresh_token: String,
    pub project_id: Option<String>,
    pub managed_project_id: Option<String>,
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
pub struct AntigravityUsageResponse {
    pub load: AntigravityLoadResponse,
    #[serde(default)]
    pub models: HashMap<String, AntigravityModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityLoadResponse {
    #[serde(rename = "cloudaicompanionProject", default)]
    pub cloudaicompanion_project: Option<serde_json::Value>,
    #[serde(rename = "planInfo", default)]
    pub plan_info: Option<AntigravityPlanInfo>,
    #[serde(rename = "availablePromptCredits", default)]
    pub available_prompt_credits: Option<f64>,
    #[serde(rename = "paidTier", default)]
    pub paid_tier: Option<AntigravityTier>,
    #[serde(rename = "currentTier", default)]
    pub current_tier: Option<AntigravityTier>,
    #[serde(rename = "allowedTiers", default)]
    pub allowed_tiers: Option<Vec<AntigravityTier>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityPlanInfo {
    #[serde(rename = "monthlyPromptCredits", default)]
    pub monthly_prompt_credits: Option<f64>,
    #[serde(rename = "planType", default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityTier {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "isDefault", default)]
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AntigravityOnboardResponse {
    #[serde(default)]
    pub done: Option<bool>,
    #[serde(default)]
    pub response: Option<AntigravityOnboardPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AntigravityOnboardPayload {
    #[serde(rename = "cloudaicompanionProject", default)]
    pub cloudaicompanion_project: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityModelInfo {
    #[serde(rename = "quotaInfo", default)]
    pub quota_info: Option<AntigravityQuotaInfo>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(rename = "isInternal", default)]
    pub is_internal: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityQuotaInfo {
    #[serde(rename = "remainingFraction", default)]
    pub remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime", default)]
    pub reset_time: Option<serde_json::Value>,
    #[serde(rename = "isExhausted", default)]
    pub is_exhausted: Option<bool>,
}

pub fn parse_refresh_token(raw: &str) -> RefreshTokenParts {
    let mut parts = raw.split('|');
    let refresh_token = parts.next().unwrap_or("").to_string();
    let project_id = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string());
    let managed_project_id = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string());
    RefreshTokenParts {
        refresh_token,
        project_id,
        managed_project_id,
    }
}

pub fn build_authorize_url(redirect_uri: &str, challenge: &str, state: &str) -> Result<String> {
    let mut url = Url::parse(AUTH_URL)
        .map_err(|err| BackendError::Provider(format!("OAuth URL invalid: {err}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &SCOPES.join(" "))
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    Ok(url.to_string())
}

pub async fn exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<AntigravityCredentials> {
    let client = Client::new();
    let response = client
        .post(TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|err| {
            BackendError::Provider(format!("Antigravity OAuth token request failed: {err}"))
        })?;

    let token = handle_token_response(response).await?;
    let refresh_token = token
        .refresh_token
        .ok_or_else(|| BackendError::Provider("Missing refresh token in response".to_string()))?;
    let expires_at = expires_at_from(token.expires_in);
    let project_id = fetch_project_id(&token.access_token).await;

    Ok(AntigravityCredentials {
        kind: Some("oauth".to_string()),
        access_token: token.access_token,
        refresh_token,
        expires_at,
        project_id,
        managed_project_id: None,
    })
}

pub async fn refresh_credentials(
    refresh_token: &str,
    project_id: Option<&str>,
    managed_project_id: Option<&str>,
) -> Result<AntigravityCredentials> {
    let client = Client::new();
    let response = client
        .post(TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|err| {
            BackendError::Provider(format!("Antigravity OAuth refresh failed: {err}"))
        })?;

    let token = handle_token_response(response).await?;
    let expires_at = expires_at_from(token.expires_in);
    let refresh_token = token
        .refresh_token
        .unwrap_or_else(|| refresh_token.to_string());

    Ok(AntigravityCredentials {
        kind: Some("oauth".to_string()),
        access_token: token.access_token,
        refresh_token,
        expires_at,
        project_id: project_id.map(|value| value.to_string()),
        managed_project_id: managed_project_id.map(|value| value.to_string()),
    })
}

pub async fn fetch_usage(
    access_token: &str,
    fallback_project_id: &str,
) -> Result<AntigravityUsageResponse> {
    let mut load = load_code_assist(access_token).await?;
    let mut project_id = extract_load_project_id(&load).filter(|value| !value.trim().is_empty());

    if project_id.is_none() {
        let tier_from_load = load
            .paid_tier
            .as_ref()
            .and_then(|tier| tier.id.as_deref())
            .or_else(|| {
                load.current_tier
                    .as_ref()
                    .and_then(|tier| tier.id.as_deref())
            });
        if let Some(tier_id) = pick_onboard_tier(load.allowed_tiers.as_deref(), tier_from_load) {
            if let Some(onboarded_project_id) = try_onboard_user(access_token, &tier_id).await {
                load.cloudaicompanion_project =
                    Some(serde_json::Value::String(onboarded_project_id.clone()));
                project_id = Some(onboarded_project_id);
            }
        }
    }

    let project_id = project_id.unwrap_or_else(|| fallback_project_id.to_string());
    let models = match fetch_available_models(access_token, &project_id).await {
        Ok(models) => models,
        Err(_) => HashMap::new(),
    };

    Ok(AntigravityUsageResponse { load, models })
}

pub async fn fetch_project_id(access_token: &str) -> Option<String> {
    let load = load_code_assist(access_token).await.ok()?;
    extract_load_project_id(&load)
}

async fn load_code_assist(access_token: &str) -> Result<AntigravityLoadResponse> {
    let client = Client::new();
    let request_body = serde_json::json!({ "metadata": metadata_payload() });
    let mut errors = Vec::new();
    let endpoints = load_endpoints();

    for endpoint in endpoints {
        let url = format!("{endpoint}/v1internal:loadCodeAssist");
        let response = client
            .post(&url)
            .bearer_auth(access_token)
            .header("content-type", "application/json")
            .header("user-agent", LOAD_USER_AGENT)
            .header("x-goog-api-client", API_CLIENT)
            .header("client-metadata", CLIENT_METADATA)
            .json(&request_body)
            .send()
            .await;

        let response = match response {
            Ok(response) => response,
            Err(err) => {
                errors.push(format!("{endpoint} request error: {err}"));
                continue;
            }
        };

        let status = response.status();
        if status.is_success() {
            return response
                .json::<AntigravityLoadResponse>()
                .await
                .map_err(|err| {
                    BackendError::Provider(format!(
                        "Antigravity loadCodeAssist decode failed: {err}"
                    ))
                });
        }

        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        let body = shorten_body(&body);
        let message = if body.is_empty() {
            format!("HTTP {status}")
        } else {
            format!("HTTP {status} - {body}")
        };
        errors.push(format!("{endpoint} {message}"));
    }

    let detail = if errors.is_empty() {
        "Antigravity loadCodeAssist failed".to_string()
    } else {
        format!("Antigravity loadCodeAssist failed: {}", errors.join("; "))
    };
    Err(BackendError::Provider(detail))
}

async fn fetch_available_models(
    access_token: &str,
    project_id: &str,
) -> Result<HashMap<String, AntigravityModelInfo>> {
    let client = Client::new();
    let request_body = serde_json::json!({ "project": project_id });
    let mut errors = Vec::new();

    for endpoint in FETCH_ENDPOINTS {
        let url = format!("{endpoint}/v1internal:fetchAvailableModels");
        let response = client
            .post(&url)
            .bearer_auth(access_token)
            .header("content-type", "application/json")
            .header("user-agent", USER_AGENT)
            .header("x-goog-api-client", API_CLIENT)
            .header("client-metadata", CLIENT_METADATA)
            .json(&request_body)
            .send()
            .await;

        let response = match response {
            Ok(response) => response,
            Err(err) => {
                errors.push(format!("{endpoint} request error: {err}"));
                continue;
            }
        };

        let status = response.status();
        if status.is_success() {
            let payload = response.json::<serde_json::Value>().await.map_err(|err| {
                BackendError::Provider(format!("Antigravity usage decode failed: {err}"))
            })?;
            let models = payload
                .get("models")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            let models = serde_json::from_value::<HashMap<String, AntigravityModelInfo>>(models)
                .map_err(|err| {
                    BackendError::Provider(format!("Antigravity model decode failed: {err}"))
                })?;
            return Ok(models);
        }

        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        let body = shorten_body(&body);
        let message = if body.is_empty() {
            format!("HTTP {status}")
        } else {
            format!("HTTP {status} - {body}")
        };
        errors.push(format!("{endpoint} {message}"));
    }

    let detail = if errors.is_empty() {
        "Antigravity usage request failed".to_string()
    } else {
        format!("Antigravity usage request failed: {}", errors.join("; "))
    };
    Err(BackendError::Provider(detail))
}

async fn try_onboard_user(access_token: &str, tier_id: &str) -> Option<String> {
    let client = Client::new();
    let request_body = serde_json::json!({
        "tierId": tier_id,
        "metadata": metadata_payload(),
    });

    for endpoint in FETCH_ENDPOINTS {
        let url = format!("{endpoint}/v1internal:onboardUser");
        for attempt in 0..ONBOARD_ATTEMPTS {
            let response = client
                .post(&url)
                .bearer_auth(access_token)
                .header("content-type", "application/json")
                .header("user-agent", USER_AGENT)
                .header("x-goog-api-client", API_CLIENT)
                .header("client-metadata", CLIENT_METADATA)
                .json(&request_body)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(_) => {
                    if attempt + 1 < ONBOARD_ATTEMPTS {
                        sleep(Duration::from_millis(ONBOARD_DELAY_MS)).await;
                    }
                    continue;
                }
            };

            if matches!(response.status().as_u16(), 401 | 403) {
                return None;
            }

            if response.status().is_success() {
                let payload = response.json::<AntigravityOnboardResponse>().await.ok();
                if let Some(payload) = payload {
                    if payload.done.unwrap_or(false) {
                        if let Some(project) = payload
                            .response
                            .and_then(|response| response.cloudaicompanion_project)
                            .as_ref()
                            .and_then(extract_project_id)
                        {
                            return Some(project);
                        }
                        return None;
                    }
                }
            }

            if attempt + 1 < ONBOARD_ATTEMPTS {
                sleep(Duration::from_millis(ONBOARD_DELAY_MS)).await;
            }
        }
    }

    None
}

fn pick_onboard_tier(
    allowed_tiers: Option<&[AntigravityTier]>,
    tier_from_load: Option<&str>,
) -> Option<String> {
    let tiers = allowed_tiers.unwrap_or(&[]);
    if tiers.is_empty() {
        return tier_from_load.map(|value| value.to_string());
    }
    if let Some(default_tier) = tiers.iter().find(|tier| tier.is_default.unwrap_or(false)) {
        if let Some(id) = default_tier.id.as_deref() {
            if !id.trim().is_empty() {
                return Some(id.to_string());
            }
        }
    }
    if let Some(first_tier) = tiers.iter().find(|tier| {
        tier.id
            .as_deref()
            .map(|id| !id.trim().is_empty())
            .unwrap_or(false)
    }) {
        return first_tier.id.as_ref().map(|id| id.to_string());
    }
    Some("LEGACY".to_string())
}

pub fn extract_load_project_id(payload: &AntigravityLoadResponse) -> Option<String> {
    payload
        .cloudaicompanion_project
        .as_ref()
        .and_then(extract_project_id)
}

fn metadata_payload() -> serde_json::Value {
    serde_json::json!({
        "ideType": "IDE_UNSPECIFIED",
        "platform": "PLATFORM_UNSPECIFIED",
        "pluginType": "GEMINI",
    })
}

fn load_endpoints() -> Vec<&'static str> {
    let mut endpoints = Vec::new();
    for endpoint in LOAD_ENDPOINTS.iter().chain(FETCH_ENDPOINTS.iter()) {
        if !endpoints.contains(endpoint) {
            endpoints.push(*endpoint);
        }
    }
    endpoints
}

fn extract_project_id(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(project_id) if !project_id.trim().is_empty() => {
            Some(project_id.to_string())
        }
        serde_json::Value::Object(project) => project
            .get("id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        _ => None,
    }
}

fn expires_at_from(expires_in: Option<i64>) -> i64 {
    let expires_in = expires_in.unwrap_or(3600).max(1);
    now_unix_ms().saturating_add(expires_in.saturating_mul(1000))
}

async fn handle_token_response(response: reqwest::Response) -> Result<TokenResponse> {
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

    response
        .json::<TokenResponse>()
        .await
        .map_err(|err| BackendError::Provider(format!("OAuth token decode failed: {err}")))
}
