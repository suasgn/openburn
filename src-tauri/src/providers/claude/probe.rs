use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::providers::common::normalize_percent;
use crate::providers::usage::{
    dollars_from_cents, normalize_resets_at, plan_label, progress_percent_line, status_line,
    MetricLine, ProbeSuccess, ProgressFormat, PERIOD_5_HOURS_MS, PERIOD_7_DAYS_MS,
};

use super::client as claude;

pub async fn probe(
    _account: &AccountRecord,
    credentials: serde_json::Value,
) -> Result<ProbeSuccess> {
    let mut credentials = serde_json::from_value::<claude::ClaudeCredentials>(credentials)
        .map_err(|err| BackendError::Provider(format!("Invalid Claude credentials: {err}")))?;

    if credentials.access_token.trim().is_empty() || credentials.refresh_token.trim().is_empty() {
        return Err(BackendError::Provider(
            "Claude OAuth credentials are incomplete".to_string(),
        ));
    }

    let mut updated_credentials = None;
    if credentials.kind.as_deref() != Some("oauth") {
        credentials.kind = Some("oauth".to_string());
        updated_credentials =
            Some(serde_json::to_value(credentials.clone()).map_err(|err| {
                BackendError::Provider(format!("Invalid Claude credentials: {err}"))
            })?);
    }

    if credentials.is_expired() {
        let mut refreshed = claude::refresh_credentials(&credentials.refresh_token).await?;
        refreshed.subscription_type = credentials.subscription_type.clone();
        credentials = refreshed;
        updated_credentials = Some(
            serde_json::to_value(credentials.clone().with_kind()).map_err(|err| {
                BackendError::Provider(format!("Invalid Claude credentials: {err}"))
            })?,
        );
    }

    let usage = claude::fetch_usage(&credentials.access_token).await?;
    let mut lines = Vec::new();

    if let Some(session) = usage.five_hour {
        if let Some(utilization) = session.utilization {
            lines.push(progress_percent_line(
                "Session",
                normalize_percent(utilization).clamp(0.0, 100.0),
                normalize_resets_at(session.resets_at),
                Some(PERIOD_5_HOURS_MS),
            ));
        }
    }

    if let Some(weekly) = usage.seven_day {
        if let Some(utilization) = weekly.utilization {
            lines.push(progress_percent_line(
                "Weekly",
                normalize_percent(utilization).clamp(0.0, 100.0),
                normalize_resets_at(weekly.resets_at),
                Some(PERIOD_7_DAYS_MS),
            ));
        }
    }

    if let Some(sonnet) = usage.seven_day_sonnet {
        if let Some(utilization) = sonnet.utilization {
            lines.push(progress_percent_line(
                "Sonnet",
                normalize_percent(utilization).clamp(0.0, 100.0),
                normalize_resets_at(sonnet.resets_at),
                Some(PERIOD_7_DAYS_MS),
            ));
        }
    }

    if let Some(extra) = usage.extra_usage {
        if extra.is_enabled.unwrap_or(false) {
            let used = extra.used_credits;
            let limit = extra.monthly_limit;
            if let (Some(used), Some(limit)) = (used, limit) {
                if limit > 0.0 {
                    lines.push(MetricLine::Progress {
                        label: "Extra usage".to_string(),
                        used: dollars_from_cents(used),
                        limit: dollars_from_cents(limit),
                        format: ProgressFormat::Dollars,
                        resets_at: None,
                        period_duration_ms: None,
                        color: None,
                    });
                }
            } else if let Some(used) = used {
                if used > 0.0 {
                    lines.push(MetricLine::Text {
                        label: "Extra usage".to_string(),
                        value: format!("${:.2}", dollars_from_cents(used)),
                        color: None,
                        subtitle: None,
                    });
                }
            }
        }
    }

    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    let plan = credentials
        .subscription_type
        .as_deref()
        .map(plan_label)
        .filter(|value| !value.is_empty());

    Ok(ProbeSuccess {
        plan,
        lines,
        updated_credentials,
    })
}
