use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::providers::common::normalize_percent;
use crate::providers::usage::{
    normalize_resets_at, plan_label, progress_percent_line, status_line, MetricLine, ProbeSuccess,
    PERIOD_30_DAYS_MS,
};

use super::client as copilot;

pub async fn probe(
    _account: &AccountRecord,
    credentials: serde_json::Value,
) -> Result<ProbeSuccess> {
    let mut credentials = serde_json::from_value::<copilot::CopilotCredentials>(credentials)
        .map_err(|err| BackendError::Provider(format!("Invalid Copilot credentials: {err}")))?;

    if credentials.access_token.trim().is_empty() {
        return Err(BackendError::Provider(
            "Copilot OAuth credentials are incomplete".to_string(),
        ));
    }

    let mut updated_credentials = None;
    if credentials.kind.as_deref() != Some("oauth") {
        credentials.kind = Some("oauth".to_string());
        updated_credentials = Some(
            serde_json::to_value(credentials.clone().with_kind()).map_err(|err| {
                BackendError::Provider(format!("Invalid Copilot credentials: {err}"))
            })?,
        );
    }

    let usage = copilot::fetch_usage(&credentials.access_token).await?;
    let mut lines = Vec::new();

    if let Some(snapshots) = usage.quota_snapshots.as_ref() {
        if let Some(line) = build_copilot_quota_line(
            "Premium",
            snapshots
                .premium_interactions
                .as_ref()
                .and_then(|snapshot| snapshot.percent_remaining),
            usage.quota_reset_date.clone(),
        ) {
            lines.push(line);
        }

        if let Some(line) = build_copilot_quota_line(
            "Chat",
            snapshots
                .chat
                .as_ref()
                .and_then(|snapshot| snapshot.percent_remaining),
            usage.quota_reset_date.clone(),
        ) {
            lines.push(line);
        }
    }

    if let (Some(limited), Some(monthly)) = (
        usage.limited_user_quotas.as_ref(),
        usage.monthly_quotas.as_ref(),
    ) {
        if let Some(line) = build_copilot_limited_line(
            "Chat",
            limited.chat,
            monthly.chat,
            usage.limited_user_reset_date.clone(),
        ) {
            lines.push(line);
        }

        if let Some(line) = build_copilot_limited_line(
            "Completions",
            limited.completions,
            monthly.completions,
            usage.limited_user_reset_date,
        ) {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    let plan = usage
        .copilot_plan
        .as_deref()
        .map(plan_label)
        .filter(|value| !value.is_empty());

    Ok(ProbeSuccess {
        plan,
        lines,
        updated_credentials,
    })
}

fn build_copilot_quota_line(
    label: &str,
    percent_remaining: Option<f64>,
    resets_at: Option<String>,
) -> Option<MetricLine> {
    let percent_remaining = percent_remaining?;
    let percent_remaining = normalize_percent(percent_remaining).clamp(0.0, 100.0);
    let used = (100.0 - percent_remaining).clamp(0.0, 100.0);
    Some(progress_percent_line(
        label,
        used,
        normalize_resets_at(resets_at),
        Some(PERIOD_30_DAYS_MS),
    ))
}

fn build_copilot_limited_line(
    label: &str,
    remaining: Option<f64>,
    total: Option<f64>,
    resets_at: Option<String>,
) -> Option<MetricLine> {
    let remaining = remaining?;
    let total = total?;
    if total <= 0.0 {
        return None;
    }
    let used_percent = ((total - remaining) / total * 100.0).clamp(0.0, 100.0);
    Some(progress_percent_line(
        label,
        used_percent,
        normalize_resets_at(resets_at),
        Some(PERIOD_30_DAYS_MS),
    ))
}
