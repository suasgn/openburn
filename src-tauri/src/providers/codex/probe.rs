use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::providers::common::normalize_percent;
use crate::providers::usage::{
    duration_ms_from_seconds, parse_number, plan_label, progress_percent_line, status_line,
    unix_to_rfc3339, MetricLine, ProbeSuccess, ProgressFormat, PERIOD_5_HOURS_MS, PERIOD_7_DAYS_MS,
};

use super::client as codex;

pub async fn probe(
    _account: &AccountRecord,
    credentials: serde_json::Value,
) -> Result<ProbeSuccess> {
    let mut credentials = serde_json::from_value::<codex::CodexCredentials>(credentials)
        .map_err(|err| BackendError::Provider(format!("Invalid Codex credentials: {err}")))?;

    if credentials.access_token.trim().is_empty() || credentials.refresh_token.trim().is_empty() {
        return Err(BackendError::Provider(
            "Codex OAuth credentials are incomplete".to_string(),
        ));
    }

    let mut updated_credentials = None;
    if credentials.kind.as_deref() != Some("oauth") {
        credentials.kind = Some("oauth".to_string());
        updated_credentials =
            Some(serde_json::to_value(credentials.clone()).map_err(|err| {
                BackendError::Provider(format!("Invalid Codex credentials: {err}"))
            })?);
    }

    if credentials.is_expired() {
        credentials = codex::refresh_credentials(
            &credentials.refresh_token,
            credentials.account_id.as_deref(),
        )
        .await?;
        updated_credentials = Some(
            serde_json::to_value(credentials.clone().with_kind()).map_err(|err| {
                BackendError::Provider(format!("Invalid Codex credentials: {err}"))
            })?,
        );
    }

    let usage =
        codex::fetch_usage(&credentials.access_token, credentials.account_id.as_deref()).await?;
    let mut lines = Vec::new();

    if let Some(primary) = usage
        .rate_limit
        .as_ref()
        .and_then(|value| value.primary_window.as_ref())
    {
        if let Some(used_percent) = primary.used_percent {
            lines.push(progress_percent_line(
                "Session",
                normalize_percent(used_percent).clamp(0.0, 100.0),
                primary.reset_at.and_then(unix_to_rfc3339),
                duration_ms_from_seconds(primary.limit_window_seconds).or(Some(PERIOD_5_HOURS_MS)),
            ));
        }
    }

    if let Some(secondary) = usage
        .rate_limit
        .as_ref()
        .and_then(|value| value.secondary_window.as_ref())
    {
        if let Some(used_percent) = secondary.used_percent {
            lines.push(progress_percent_line(
                "Weekly",
                normalize_percent(used_percent).clamp(0.0, 100.0),
                secondary.reset_at.and_then(unix_to_rfc3339),
                duration_ms_from_seconds(secondary.limit_window_seconds).or(Some(PERIOD_7_DAYS_MS)),
            ));
        }
    }

    if let Some(review) = usage
        .code_review_rate_limit
        .as_ref()
        .and_then(|value| value.primary_window.as_ref())
    {
        if let Some(used_percent) = review.used_percent {
            lines.push(progress_percent_line(
                "Reviews",
                normalize_percent(used_percent).clamp(0.0, 100.0),
                review.reset_at.and_then(unix_to_rfc3339),
                duration_ms_from_seconds(review.limit_window_seconds).or(Some(PERIOD_7_DAYS_MS)),
            ));
        }
    }

    if let Some(credits) = usage.credits.as_ref() {
        if credits.has_credits.unwrap_or(false) && !credits.unlimited.unwrap_or(false) {
            if let Some(balance) = credits.balance.as_deref().and_then(parse_number) {
                let limit = 1000.0;
                let used = (limit - balance).clamp(0.0, limit);
                lines.push(MetricLine::Progress {
                    label: "Credits".to_string(),
                    used,
                    limit,
                    format: ProgressFormat::Count {
                        suffix: "credits".to_string(),
                    },
                    resets_at: None,
                    period_duration_ms: None,
                    color: None,
                });
            }
        }
    }

    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    let plan = usage
        .plan_type
        .as_deref()
        .map(plan_label)
        .filter(|value| !value.is_empty());

    Ok(ProbeSuccess {
        plan,
        lines,
        updated_credentials,
    })
}
