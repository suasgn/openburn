use std::collections::HashMap;

use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::providers::common::normalize_percent;
use crate::providers::usage::{
    plan_label, status_line, unix_to_rfc3339, MetricLine, ProbeSuccess, ProgressFormat,
    PERIOD_30_DAYS_MS, PERIOD_5_HOURS_MS,
};

use super::client as antigravity;

pub async fn probe(
    _account: &AccountRecord,
    credentials: serde_json::Value,
) -> Result<ProbeSuccess> {
    let mut credentials = serde_json::from_value::<antigravity::AntigravityCredentials>(
        credentials,
    )
    .map_err(|err| BackendError::Provider(format!("Invalid Antigravity credentials: {err}")))?;

    if credentials.access_token.trim().is_empty() && credentials.refresh_token.trim().is_empty() {
        return Err(BackendError::Provider(
            "Antigravity OAuth credentials are incomplete".to_string(),
        ));
    }

    let mut updated = false;
    if credentials.kind.as_deref() != Some("oauth") {
        credentials.kind = Some("oauth".to_string());
        updated = true;
    }

    let refresh_parts = antigravity::parse_refresh_token(&credentials.refresh_token);
    let refresh_token = refresh_parts.refresh_token;
    let refresh_project_id = refresh_parts.project_id;
    let refresh_managed_project_id = refresh_parts.managed_project_id;
    if refresh_token != credentials.refresh_token {
        credentials.refresh_token = refresh_token;
        updated = true;
    }

    let has_project_id = credentials
        .project_id
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if !has_project_id && refresh_project_id.is_some() {
        credentials.project_id = refresh_project_id;
        updated = true;
    }

    let has_managed_project_id = credentials
        .managed_project_id
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if !has_managed_project_id && refresh_managed_project_id.is_some() {
        credentials.managed_project_id = refresh_managed_project_id;
        updated = true;
    }

    let should_refresh = credentials.access_token.trim().is_empty() || credentials.is_expired();
    if should_refresh {
        if credentials.refresh_token.trim().is_empty() {
            return Err(BackendError::Provider(
                "Antigravity OAuth credentials are expired and missing refresh token".to_string(),
            ));
        }
        credentials = antigravity::refresh_credentials(
            &credentials.refresh_token,
            credentials.project_id.as_deref(),
            credentials.managed_project_id.as_deref(),
        )
        .await?;
        updated = true;
    }

    if credentials.access_token.trim().is_empty() {
        return Err(BackendError::Provider(
            "Missing Antigravity access token".to_string(),
        ));
    }

    if credentials
        .project_id
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        if let Some(project_id) = antigravity::fetch_project_id(&credentials.access_token).await {
            credentials.project_id = Some(project_id);
            updated = true;
        }
    }

    let effective_project_id = credentials
        .managed_project_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            credentials
                .project_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or(antigravity::DEFAULT_PROJECT_ID);

    let usage = antigravity::fetch_usage(&credentials.access_token, effective_project_id).await?;

    if let Some(project_id) = antigravity::extract_load_project_id(&usage.load) {
        let trimmed = project_id.trim();
        if !trimmed.is_empty() {
            let update_project_id = credentials
                .project_id
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true);
            if update_project_id {
                credentials.project_id = Some(trimmed.to_string());
                updated = true;
            }

            let update_managed_id = credentials
                .managed_project_id
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true);
            if update_managed_id {
                credentials.managed_project_id = Some(trimmed.to_string());
                updated = true;
            }
        }
    }

    let mut lines = build_antigravity_model_lines(&usage.models);
    if let Some(prompt_credits_line) = build_antigravity_prompt_credits_line(&usage.load) {
        lines.push(prompt_credits_line);
    }
    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    let plan = usage
        .load
        .plan_info
        .as_ref()
        .and_then(|plan| plan.plan_type.as_deref())
        .or_else(|| {
            usage
                .load
                .current_tier
                .as_ref()
                .and_then(|tier| tier.id.as_deref())
        })
        .or_else(|| {
            usage
                .load
                .paid_tier
                .as_ref()
                .and_then(|tier| tier.id.as_deref())
        })
        .map(|value| value.replace('_', " ").replace('-', " "))
        .map(|value| plan_label(&value))
        .filter(|value| !value.is_empty());

    let updated_credentials = if updated {
        Some(
            serde_json::to_value(credentials.with_kind()).map_err(|err| {
                BackendError::Provider(format!("Invalid Antigravity credentials: {err}"))
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

#[derive(Debug, Clone)]
struct AntigravityModelLine {
    label: String,
    used: f64,
    resets_at: Option<String>,
}

fn build_antigravity_prompt_credits_line(
    load: &antigravity::AntigravityLoadResponse,
) -> Option<MetricLine> {
    let monthly = load.plan_info.as_ref()?.monthly_prompt_credits?;
    if monthly <= 0.0 {
        return None;
    }

    let available = load.available_prompt_credits?;
    let remaining = available.clamp(0.0, monthly);
    let used = (monthly - remaining).clamp(0.0, monthly);
    Some(MetricLine::Progress {
        label: "Prompt Credits".to_string(),
        used,
        limit: monthly,
        format: ProgressFormat::Count {
            suffix: "credits".to_string(),
        },
        resets_at: None,
        period_duration_ms: Some(PERIOD_30_DAYS_MS),
        color: None,
    })
}

fn build_antigravity_model_lines(
    models: &HashMap<String, antigravity::AntigravityModelInfo>,
) -> Vec<MetricLine> {
    let mut deduped: HashMap<String, AntigravityModelLine> = HashMap::new();

    for (model_key, model) in models {
        if !should_include_antigravity_model(model_key, model) {
            continue;
        }

        let Some(quota) = model.quota_info.as_ref() else {
            continue;
        };

        let remaining_fraction = quota.remaining_fraction.or_else(|| {
            quota
                .is_exhausted
                .and_then(|exhausted| exhausted.then_some(0.0))
        });
        let Some(remaining_fraction) = remaining_fraction else {
            continue;
        };

        let label = normalize_antigravity_label(&antigravity_model_label(model, model_key));
        if label.is_empty() {
            continue;
        }

        let remaining = normalize_percent(remaining_fraction).clamp(0.0, 100.0);
        let used = (100.0 - remaining).clamp(0.0, 100.0);
        let resets_at = parse_antigravity_reset_time(quota.reset_time.as_ref());

        if let Some(existing) = deduped.get_mut(&label) {
            if used > existing.used {
                existing.used = used;
                existing.resets_at = resets_at;
            }
            continue;
        }

        deduped.insert(
            label.clone(),
            AntigravityModelLine {
                label,
                used,
                resets_at,
            },
        );
    }

    let mut lines = deduped.into_values().collect::<Vec<_>>();
    lines.sort_by(|left, right| {
        antigravity_model_rank(&left.label)
            .cmp(&antigravity_model_rank(&right.label))
            .then_with(|| {
                left.label
                    .to_ascii_lowercase()
                    .cmp(&right.label.to_ascii_lowercase())
            })
    });

    lines
        .into_iter()
        .map(|line| MetricLine::Progress {
            label: line.label,
            used: line.used,
            limit: 100.0,
            format: ProgressFormat::Percent,
            resets_at: line.resets_at,
            period_duration_ms: Some(PERIOD_5_HOURS_MS),
            color: None,
        })
        .collect()
}

fn antigravity_model_rank(label: &str) -> u8 {
    let lower = label.to_ascii_lowercase();
    if lower.contains("gemini") && lower.contains("pro") {
        return 0;
    }
    if lower.contains("gemini") {
        return 1;
    }
    if lower.contains("claude") && lower.contains("opus") {
        return 2;
    }
    if lower.contains("claude") {
        return 3;
    }
    4
}

fn antigravity_model_label(model: &antigravity::AntigravityModelInfo, model_key: &str) -> String {
    model
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            model
                .label
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or(model_key)
        .trim()
        .to_string()
}

fn normalize_antigravity_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with(')') {
        if let Some(index) = trimmed.rfind(" (") {
            return trimmed[..index].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn should_include_antigravity_model(
    model_key: &str,
    model: &antigravity::AntigravityModelInfo,
) -> bool {
    if model.quota_info.is_none() {
        return false;
    }
    if model.is_internal.unwrap_or(false) {
        return false;
    }

    let model_id = model.model.as_deref().unwrap_or(model_key).trim();
    if model_id.is_empty() {
        return false;
    }
    if is_blacklisted_antigravity_model(model_id) {
        return false;
    }

    let lower = model_id.to_ascii_lowercase();
    if lower.starts_with("chat_") || lower.starts_with("tab_") || lower.starts_with("rev") {
        return false;
    }
    if lower.contains("image") || lower.contains("mquery") || lower.contains("lite") {
        return false;
    }

    let has_display_name = model
        .display_name
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_label = model
        .label
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    has_display_name || has_label
}

fn is_blacklisted_antigravity_model(model_id: &str) -> bool {
    let upper = model_id.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "MODEL_CHAT_20706"
            | "MODEL_CHAT_23310"
            | "MODEL_GOOGLE_GEMINI_2_5_FLASH"
            | "MODEL_GOOGLE_GEMINI_2_5_FLASH_THINKING"
            | "MODEL_GOOGLE_GEMINI_2_5_FLASH_LITE"
            | "MODEL_GOOGLE_GEMINI_2_5_PRO"
            | "MODEL_PLACEHOLDER_M19"
            | "MODEL_PLACEHOLDER_M9"
    )
}

fn parse_antigravity_reset_time(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(timestamp) = trimmed.parse::<i64>() {
                return unix_to_rfc3339(timestamp).or_else(|| Some(trimmed.to_string()));
            }
            Some(trimmed.to_string())
        }
        serde_json::Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value.round() as i64))
            .and_then(unix_to_rfc3339),
        _ => None,
    }
}
