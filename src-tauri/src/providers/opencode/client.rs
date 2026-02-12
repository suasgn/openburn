use std::sync::OnceLock;

use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::error::{BackendError, Result};
use crate::providers::common::{format_http_error, shorten_body};

const BASE_URL: &str = "https://opencode.ai";
const SERVER_URL: &str = "https://opencode.ai/_server";
const USAGE_SERVER_ID: &str = "bbb1284bc5442ffc92d7d2ef43d0bae818b6a859d848d631e9fa8d26cf77b56c";
const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeCredentials {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(rename = "cookieHeader", alias = "cookie_header", alias = "cookie")]
    pub cookie_header: String,
}

impl OpenCodeCredentials {
    pub fn with_kind(mut self) -> Self {
        self.kind = Some("cookie".to_string());
        self
    }
}

pub fn cookie_header_from_pairs<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    let mut collected = Vec::new();
    let mut has_auth = false;

    for (name, value) in pairs {
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() || value.is_empty() {
            continue;
        }

        if name == "auth" || name == "__Host-auth" {
            has_auth = true;
        }

        collected.push(format!("{name}={value}"));
    }

    if !has_auth || collected.is_empty() {
        return None;
    }

    Some(collected.join("; "))
}

#[derive(Debug, Clone)]
pub struct OpenCodeUsageSnapshot {
    pub rolling_usage_percent: Option<f64>,
    pub weekly_usage_percent: Option<f64>,
    pub rolling_reset_in_sec: Option<i64>,
    pub weekly_reset_in_sec: Option<i64>,
    pub plan: Option<String>,
    pub monthly_total_cost_usd: Option<f64>,
    pub usage_rows: Option<usize>,
    pub api_keys: Option<usize>,
    pub models: Option<usize>,
    pub subscription_rows: Option<usize>,
}

#[derive(Debug, Clone)]
struct ServerRequest {
    server_id: &'static str,
    args: serde_json::Value,
    referer: String,
    server_instance: Option<String>,
}

pub async fn fetch_usage(
    cookie_header: &str,
    workspace_id: Option<&str>,
) -> Result<OpenCodeUsageSnapshot> {
    let cookie_header = cookie_header.trim();
    if cookie_header.is_empty() {
        return Err(BackendError::Provider(
            "OpenCode session cookie is invalid or expired.".to_string(),
        ));
    }

    let workspace_id = normalize_workspace_id(workspace_id).ok_or_else(|| {
        BackendError::Provider(
            "OpenCode workspaceId is missing. Reconnect OpenCode to capture workspace redirect."
                .to_string(),
        )
    })?;

    let has_auth_cookie = cookie_header.contains("auth=") || cookie_header.contains("__Host-auth=");
    log::info!(
        "[opencode] fetch_usage start workspace_id={} cookie_len={} has_auth_cookie={}",
        mask_workspace_id(&workspace_id),
        cookie_header.len(),
        has_auth_cookie
    );

    let client = Client::new();
    let payload = fetch_usage_text(&client, &workspace_id, cookie_header).await?;
    parse_usage_text(&payload, &workspace_id)
}

pub fn normalize_workspace_id(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }

    if raw.starts_with("wrk_") && raw.len() > 4 {
        return Some(raw.to_string());
    }

    if let Ok(url) = Url::parse(raw) {
        let mut parts = url.path_segments()?.collect::<Vec<_>>();
        for idx in 0..parts.len() {
            if parts[idx] != "workspace" {
                continue;
            }

            let candidate = parts.get_mut(idx + 1).map(|value| value.trim())?;
            if candidate.starts_with("wrk_") && candidate.len() > 4 {
                return Some(candidate.to_string());
            }
        }
    }

    workspace_id_regex()
        .find(raw)
        .map(|value| value.as_str().to_string())
}

async fn fetch_usage_text(
    client: &Client,
    workspace_id: &str,
    cookie_header: &str,
) -> Result<String> {
    let now = OffsetDateTime::now_utc();
    let year = now.year();
    // OpenCode expects month as zero-based index (Jan=0, Feb=1, ...).
    let month = i64::from(u8::from(now.month()).saturating_sub(1));

    let payload = serde_json::json!({
        "t": {
            "t": 9,
            "i": 0,
            "l": 3,
            "a": [
                { "t": 1, "s": workspace_id },
                { "t": 0, "s": year },
                { "t": 0, "s": month }
            ],
            "o": 0
        },
        "f": 31,
        "m": []
    });

    let referer = format!("{BASE_URL}/workspace/{workspace_id}");
    fetch_server_text(
        client,
        ServerRequest {
            server_id: USAGE_SERVER_ID,
            args: payload,
            referer,
            server_instance: Some("server-fn:0".to_string()),
        },
        cookie_header,
    )
    .await
}

fn parse_usage_text(text: &str, workspace_id: &str) -> Result<OpenCodeUsageSnapshot> {
    if let Some(message) = extract_server_fn_error_message(text) {
        return Err(BackendError::Provider(format!(
            "OpenCode API error: {message}"
        )));
    }

    if is_server_fn_null_payload(text) {
        return Err(BackendError::Provider(format!(
            "OpenCode usage payload is empty for workspace {}",
            mask_workspace_id(workspace_id)
        )));
    }

    let rolling_usage_percent = extract_f64(text, rolling_usage_percent_regex());
    let rolling_reset_in_sec = extract_i64(text, rolling_reset_in_sec_regex());
    let weekly_usage_percent = extract_f64(text, weekly_usage_percent_regex());
    let weekly_reset_in_sec = extract_i64(text, weekly_reset_in_sec_regex());

    let plan = plan_regex()
        .captures(text)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_string())
        .filter(|value| !value.is_empty());

    let has_usage_array = usage_array_regex().is_match(text);
    let costs = extract_f64_values(text, total_cost_regex());
    let usage_rows = usage_entry_regex().find_iter(text).count();
    let total_cost = if costs.is_empty() {
        if has_usage_array {
            Some(0.0)
        } else {
            None
        }
    } else {
        Some(costs.iter().copied().sum::<f64>())
    };

    let key_names = extract_unique_strings(text, key_display_name_regex());
    let key_ids = extract_unique_strings(text, key_id_regex());
    let api_keys = if !key_names.is_empty() {
        key_names.len()
    } else {
        key_ids.len()
    };

    let models = extract_unique_strings(text, model_regex()).len();
    let subscription_rows = subscription_true_regex().find_iter(text).count();

    let has_usage =
        rolling_usage_percent.is_some() || weekly_usage_percent.is_some() || has_usage_array;
    if !has_usage {
        log_parse_summary(text);
        return Err(BackendError::Provider(
            "OpenCode parse error: Missing usage fields in _server payload.".to_string(),
        ));
    }

    Ok(OpenCodeUsageSnapshot {
        rolling_usage_percent,
        weekly_usage_percent,
        rolling_reset_in_sec,
        weekly_reset_in_sec,
        plan,
        monthly_total_cost_usd: total_cost,
        usage_rows: Some(usage_rows),
        api_keys: Some(api_keys),
        models: Some(models),
        subscription_rows: Some(subscription_rows),
    })
}

async fn fetch_server_text(
    client: &Client,
    request: ServerRequest,
    cookie_header: &str,
) -> Result<String> {
    log::info!(
        "[opencode] _server request id={} method=POST referer={} instance={}",
        request.server_id,
        request.referer,
        request.server_instance.as_deref().unwrap_or("auto")
    );

    let server_instance = request
        .server_instance
        .unwrap_or_else(|| format!("server-fn:{}", Uuid::new_v4()));

    let response = client
        .post(SERVER_URL)
        .header("Cookie", cookie_header)
        .header("X-Server-Id", request.server_id)
        .header("X-Server-Instance", server_instance)
        .header("User-Agent", USER_AGENT)
        .header("Origin", BASE_URL)
        .header("Referer", request.referer)
        .header(
            "Accept",
            "text/javascript, application/json;q=0.9, */*;q=0.8",
        )
        .header("Content-Type", "application/json")
        .json(&request.args)
        .send()
        .await
        .map_err(|err| BackendError::Provider(format!("OpenCode network error: {err}")))?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let body = response.text().await.unwrap_or_else(|_| "".to_string());

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        log::warn!(
            "[opencode] _server unauthorized id={} status={} content_type={} body_hint={}",
            request.server_id,
            status,
            content_type,
            body_hint(&body)
        );
        return Err(BackendError::Provider(
            "OpenCode session cookie is invalid or expired.".to_string(),
        ));
    }

    if !status.is_success() {
        log::error!(
            "[opencode] _server request failed id={} status={} content_type={} body_len={} body_preview={}",
            request.server_id,
            status,
            content_type,
            body.len(),
            shorten_body(&body)
        );

        if looks_signed_out(&body) {
            return Err(BackendError::Provider(
                "OpenCode session cookie is invalid or expired.".to_string(),
            ));
        }

        if let Some(message) = extract_server_error_message(&body) {
            return Err(BackendError::Provider(format!(
                "OpenCode API error: HTTP {status} - {message}"
            )));
        }

        return Err(BackendError::Provider(format_http_error(
            "OpenCode API error",
            status,
            &body,
        )));
    }

    log::info!(
        "[opencode] _server success id={} status={} content_type={} body_len={} body_hint={}",
        request.server_id,
        status,
        content_type,
        body.len(),
        body_hint(&body)
    );

    if looks_signed_out(&body) {
        return Err(BackendError::Provider(
            "OpenCode session cookie is invalid or expired.".to_string(),
        ));
    }

    Ok(body)
}

fn workspace_id_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"wrk_[A-Za-z0-9]+$").expect("workspace regex should compile"))
}

fn rolling_usage_percent_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"rollingUsage[^}]*?usagePercent"?\s*[:=]\s*([0-9]+(?:\.[0-9]+)?)"#)
            .expect("rolling usage regex should compile")
    })
}

fn rolling_reset_in_sec_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"rollingUsage[^}]*?resetInSec"?\s*[:=]\s*([0-9]+)"#)
            .expect("rolling reset regex should compile")
    })
}

fn weekly_usage_percent_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"weeklyUsage[^}]*?usagePercent"?\s*[:=]\s*([0-9]+(?:\.[0-9]+)?)"#)
            .expect("weekly usage regex should compile")
    })
}

fn weekly_reset_in_sec_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"weeklyUsage[^}]*?resetInSec"?\s*[:=]\s*([0-9]+)"#)
            .expect("weekly reset regex should compile")
    })
}

fn plan_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?:planType|subscriptionType|planName|plan_type|plan_name)"?\s*[:=]\s*["']([^"']+)["']"#,
        )
        .expect("plan regex should compile")
    })
}

fn total_cost_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"totalCost\s*:\s*(-?[0-9]+(?:\.[0-9]+)?)"#)
            .expect("total cost regex should compile")
    })
}

fn key_id_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"id\s*:\s*\"(key_[A-Za-z0-9]+)\""#).expect("key id regex should compile")
    })
}

fn key_display_name_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"displayName\s*:\s*\"([^\"]+)\""#)
            .expect("key display name regex should compile")
    })
}

fn model_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"model\s*:\s*\"([^\"]+)\""#).expect("model regex should compile")
    })
}

fn subscription_true_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"subscription\s*:\s*(?:!0|true)"#).expect("subscription regex should compile")
    })
}

fn usage_array_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?:usage\s*:\s*\$R\[\d+\]\s*=\s*\[|\"usage\"\s*:\s*\[)"#)
            .expect("usage array regex should compile")
    })
}

fn usage_entry_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?:\{\s*date\s*:|\"date\"\s*:)"#).expect("usage entry regex should compile")
    })
}

fn server_fn_null_payload_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"\]=\[\],\s*null\)"#).expect("server-fn null payload regex should compile")
    })
}

fn server_fn_error_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"new Error\("((?:\\.|[^"])*)"\)"#)
            .expect("server-fn error regex should compile")
    })
}

fn html_title_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?is)<title>([^<]+)</title>").expect("html title regex should compile")
    })
}

fn extract_f64(text: &str, regex: &Regex) -> Option<f64> {
    regex
        .captures(text)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<f64>().ok())
}

fn extract_i64(text: &str, regex: &Regex) -> Option<i64> {
    regex
        .captures(text)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}

fn extract_f64_values(text: &str, regex: &Regex) -> Vec<f64> {
    regex
        .captures_iter(text)
        .filter_map(|captures| captures.get(1))
        .filter_map(|value| value.as_str().parse::<f64>().ok())
        .collect()
}

fn extract_unique_strings(text: &str, regex: &Regex) -> Vec<String> {
    let mut out = Vec::new();
    for capture in regex.captures_iter(text) {
        let Some(value) = capture.get(1) else {
            continue;
        };
        let value = value.as_str().trim();
        if value.is_empty() {
            continue;
        }
        push_unique_string(&mut out, value.to_string());
    }
    out
}

fn push_unique_string(target: &mut Vec<String>, value: String) {
    if target.iter().any(|existing| existing == &value) {
        return;
    }
    target.push(value);
}

fn is_server_fn_null_payload(text: &str) -> bool {
    server_fn_null_payload_regex().is_match(text)
}

fn extract_server_fn_error_message(text: &str) -> Option<String> {
    let captures = server_fn_error_regex().captures(text)?;
    let raw = captures.get(1)?.as_str();
    let decoded = decode_js_string(raw);
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn decode_js_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }

    out
}

fn extract_server_error_message(text: &str) -> Option<String> {
    let value = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => value,
        Err(_) => {
            return extract_html_title(text);
        }
    };

    let object = value.as_object()?;
    object
        .get("message")
        .and_then(|value| value.as_str())
        .or_else(|| object.get("error").and_then(|value| value.as_str()))
        .or_else(|| object.get("detail").and_then(|value| value.as_str()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_html_title(text: &str) -> Option<String> {
    let captures = html_title_regex().captures(text)?;
    captures
        .get(1)
        .map(|value| value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn looks_signed_out(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("login") || lower.contains("sign in") || lower.contains("auth/authorize")
}

fn body_hint(text: &str) -> &'static str {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "empty"
    } else if trimmed.starts_with('<') {
        "html"
    } else if trimmed.starts_with('{') || trimmed.starts_with('[') {
        "json"
    } else {
        "text"
    }
}

fn mask_workspace_id(workspace_id: &str) -> String {
    let visible_tail_len = 6;
    if workspace_id.len() <= visible_tail_len {
        return workspace_id.to_string();
    }

    let tail = &workspace_id[workspace_id.len() - visible_tail_len..];
    format!("***{tail}")
}

fn log_parse_summary(text: &str) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        let summary = summarize_json(&value, 0);
        if !summary.is_empty() {
            log::error!("[opencode] parse summary: {summary}");
        }
        return;
    }

    log::error!(
        "[opencode] parse summary non-json hint={} body_len={} body_preview={}",
        body_hint(text),
        text.len(),
        shorten_body(text)
    );
}

fn summarize_json(value: &serde_json::Value, depth: usize) -> String {
    if depth > 3 {
        return String::new();
    }

    match value {
        serde_json::Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let mut parts = Vec::new();

            for key in keys {
                let Some(inner) = map.get(key) else {
                    continue;
                };
                parts.push(format!(
                    "{key}:{}",
                    value_type_description(inner, depth + 1)
                ));
            }

            format!("{{{}}}", parts.join(", "))
        }
        serde_json::Value::Array(list) => {
            if let Some(first) = list.first() {
                format!("[{}]", value_type_description(first, depth + 1))
            } else {
                "[]".to_string()
            }
        }
        _ => scalar_type_description(value).to_string(),
    }
}

fn value_type_description(value: &serde_json::Value, depth: usize) -> String {
    match value {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => summarize_json(value, depth),
        _ => scalar_type_description(value).to_string(),
    }
}

fn scalar_type_description(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Null => "null",
        _ => "value",
    }
}
