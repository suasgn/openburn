use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::providers::usage::{
    plan_label, read_json_string, status_line, unix_to_rfc3339, MetricLine, ProbeSuccess,
    ProgressFormat,
};

use super::client as zai;

pub async fn probe(
    account: &AccountRecord,
    credentials: serde_json::Value,
) -> Result<ProbeSuccess> {
    let mut credentials = serde_json::from_value::<zai::ZaiCredentials>(credentials)
        .map_err(|err| BackendError::Provider(format!("Invalid Z.ai credentials: {err}")))?;

    let mut updated = false;
    if credentials.kind.as_deref() != Some("apiKey") {
        credentials.kind = Some("apiKey".to_string());
        updated = true;
    }

    if credentials.api_key.trim().is_empty() {
        if let Some(value) = read_json_string(
            &account.settings,
            &["apiKey", "api_key", "token", "access_token", "authToken"],
        ) {
            credentials.api_key = value;
            updated = true;
        }
    }

    if credentials
        .api_host
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        if let Some(value) = read_json_string(&account.settings, &["apiHost", "api_host"]) {
            credentials.api_host = Some(value);
            updated = true;
        }
    }

    if credentials
        .quota_url
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        if let Some(value) = read_json_string(&account.settings, &["quotaUrl", "quota_url"]) {
            credentials.quota_url = Some(value);
            updated = true;
        }
    }

    if credentials
        .api_region
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        if let Some(value) = read_json_string(&account.settings, &["apiRegion", "api_region"]) {
            credentials.api_region = Some(value);
            updated = true;
        }
    }

    let usage = zai::fetch_usage(&credentials).await?;
    let mut lines = Vec::new();

    if let Some(data) = usage.data.as_ref() {
        let mut token_line = None;
        let mut utility_line = None;

        for limit in &data.limits {
            match limit.limit_type.as_str() {
                "TOKENS_LIMIT" => {
                    token_line = Some(MetricLine::Progress {
                        label: "Token Usage".to_string(),
                        used: zai_limit_used_percent(limit).clamp(0.0, 100.0),
                        limit: 100.0,
                        format: ProgressFormat::Percent,
                        resets_at: limit.next_reset_time.and_then(unix_to_rfc3339),
                        period_duration_ms: zai_limit_period_ms(limit),
                        color: None,
                    })
                }
                "TIME_LIMIT" => {
                    utility_line = Some(MetricLine::Progress {
                        label: "Utility Usage".to_string(),
                        used: zai_limit_used_percent(limit).clamp(0.0, 100.0),
                        limit: 100.0,
                        format: ProgressFormat::Percent,
                        resets_at: limit.next_reset_time.and_then(unix_to_rfc3339),
                        period_duration_ms: zai_limit_period_ms(limit),
                        color: None,
                    })
                }
                _ => {}
            }
        }

        if let Some(line) = token_line {
            lines.push(line);
        }
        if let Some(line) = utility_line {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    let plan = usage
        .data
        .as_ref()
        .and_then(|data| {
            data.plan_name
                .as_deref()
                .or(data.plan.as_deref())
                .or(data.plan_type.as_deref())
                .or(data.package_name.as_deref())
        })
        .map(plan_label)
        .filter(|value| !value.is_empty());

    let updated_credentials = if updated {
        Some(
            serde_json::to_value(credentials.with_kind()).map_err(|err| {
                BackendError::Provider(format!("Invalid Z.ai credentials: {err}"))
            })?,
        )
    } else {
        None
    };

    Ok(ProbeSuccess {
        plan,
        lines,
        updated_credentials,
    })
}

fn zai_limit_used_percent(limit: &zai::ZaiLimitRaw) -> f64 {
    let total = limit.usage.max(0);
    if total > 0 {
        let remaining = limit.remaining.max(0);
        let current = limit.current_value.max(0);
        let used_from_remaining = total.saturating_sub(remaining);
        let used = used_from_remaining.max(current).min(total);
        return used as f64 / total as f64 * 100.0;
    }
    limit.percentage
}

fn zai_limit_period_ms(limit: &zai::ZaiLimitRaw) -> Option<u64> {
    if limit.number <= 0 {
        return None;
    }

    let unit_seconds = match limit.unit {
        5 => 60,
        3 => 60 * 60,
        1 => 24 * 60 * 60,
        _ => return None,
    };

    Some(
        (limit.number as u64)
            .saturating_mul(unit_seconds)
            .saturating_mul(1000),
    )
}
