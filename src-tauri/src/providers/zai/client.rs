use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{BackendError, Result};
use crate::providers::common::format_http_error;

const DEFAULT_BASE_URL: &str = "https://api.z.ai";
const CN_BASE_URL: &str = "https://open.bigmodel.cn";
const QUOTA_PATH: &str = "api/monitor/usage/quota/limit";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZaiCredentials {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(
        rename = "apiKey",
        alias = "api_key",
        alias = "token",
        alias = "access_token",
        alias = "authToken"
    )]
    pub api_key: String,
    #[serde(rename = "apiHost", alias = "api_host", default)]
    pub api_host: Option<String>,
    #[serde(rename = "quotaUrl", alias = "quota_url", default)]
    pub quota_url: Option<String>,
    #[serde(rename = "apiRegion", alias = "api_region", default)]
    pub api_region: Option<String>,
}

impl ZaiCredentials {
    pub fn with_kind(mut self) -> Self {
        self.kind = Some("apiKey".to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZaiQuotaLimitResponse {
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub data: Option<ZaiQuotaLimitData>,
    #[serde(default)]
    pub success: bool,
}

impl ZaiQuotaLimitResponse {
    pub fn is_success(&self) -> bool {
        self.success && self.code == 200
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZaiQuotaLimitData {
    #[serde(default)]
    pub limits: Vec<ZaiLimitRaw>,
    #[serde(rename = "planName", default)]
    pub plan_name: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(rename = "plan_type", default)]
    pub plan_type: Option<String>,
    #[serde(rename = "packageName", default)]
    pub package_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZaiLimitRaw {
    #[serde(rename = "type", default)]
    pub limit_type: String,
    #[serde(default)]
    pub unit: i64,
    #[serde(default)]
    pub number: i64,
    #[serde(default)]
    pub usage: i64,
    #[serde(rename = "currentValue", default)]
    pub current_value: i64,
    #[serde(default)]
    pub remaining: i64,
    #[serde(default)]
    pub percentage: f64,
    #[serde(rename = "nextResetTime", default)]
    pub next_reset_time: Option<i64>,
}

pub async fn fetch_usage(credentials: &ZaiCredentials) -> Result<ZaiQuotaLimitResponse> {
    let api_key = credentials.api_key.trim();
    if api_key.is_empty() {
        return Err(BackendError::Provider("Missing Z.ai API key".to_string()));
    }

    let quota_url = resolve_quota_url(credentials)?;
    let client = Client::new();
    let mut request = client
        .get(quota_url)
        .header("accept", "application/json")
        .header("user-agent", "openburn");
    if api_key.to_ascii_lowercase().starts_with("bearer ") {
        request = request.header("authorization", api_key);
    } else {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("Z.ai usage request failed: {err}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| "".to_string());

    if !status.is_success() {
        let message = format_http_error("Z.ai usage request failed", status, &body);
        return Err(BackendError::Provider(message));
    }

    let payload = serde_json::from_str::<ZaiQuotaLimitResponse>(&body)
        .map_err(|err| BackendError::Provider(format!("Z.ai usage decode failed: {err}")))?;
    if payload.is_success() {
        return Ok(payload);
    }

    let detail = payload.msg.trim();
    let message = if detail.is_empty() {
        "Z.ai API error".to_string()
    } else {
        format!("Z.ai API error: {detail}")
    };
    Err(BackendError::Provider(message))
}

fn resolve_quota_url(credentials: &ZaiCredentials) -> Result<Url> {
    if let Some(quota_url) = cleaned(credentials.quota_url.as_deref()) {
        return build_quota_url(&quota_url);
    }
    if let Some(api_host) = cleaned(credentials.api_host.as_deref()) {
        return build_quota_url(&api_host);
    }
    let region = cleaned(credentials.api_region.as_deref());
    let base = base_url_for_region(region.as_deref());
    build_quota_url(base)
}

fn base_url_for_region(region: Option<&str>) -> &'static str {
    match region.map(|value| value.to_ascii_lowercase()) {
        Some(value)
            if value == "bigmodel-cn"
                || value == "bigmodelcn"
                || value == "cn"
                || value == "zhipu" =>
        {
            CN_BASE_URL
        }
        _ => DEFAULT_BASE_URL,
    }
}

fn build_quota_url(raw: &str) -> Result<Url> {
    let url =
        parse_url(raw).ok_or_else(|| BackendError::Provider(format!("Z.ai URL invalid: {raw}")))?;
    let mut url = url;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path(QUOTA_PATH);
    }
    Ok(url)
}

fn parse_url(raw: &str) -> Option<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(url) = Url::parse(trimmed) {
        return Some(url);
    }
    let with_scheme = format!("https://{trimmed}");
    Url::parse(&with_scheme).ok()
}

fn cleaned(raw: Option<&str>) -> Option<String> {
    let mut value = raw?.trim();
    if value.is_empty() {
        return None;
    }
    let has_wrapped_quotes = (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''));
    if has_wrapped_quotes && value.len() >= 2 {
        value = &value[1..value.len() - 1];
    }
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
