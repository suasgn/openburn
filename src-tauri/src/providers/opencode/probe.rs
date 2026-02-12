use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::providers::usage::{
    plan_label, read_json_string, status_line, unix_to_rfc3339, MetricLine, ProbeSuccess,
    PERIOD_5_HOURS_MS, PERIOD_7_DAYS_MS,
};
use crate::utils::now_unix_ms;

use super::client as opencode;

pub async fn probe(
    account: &AccountRecord,
    credentials_value: serde_json::Value,
) -> Result<ProbeSuccess> {
    let mut credentials = serde_json::from_value::<opencode::OpenCodeCredentials>(
        credentials_value,
    )
    .map_err(|err| BackendError::Provider(format!("Invalid OpenCode credentials: {err}")))?;

    let mut updated = false;
    if credentials.kind.as_deref() != Some("cookie") {
        credentials.kind = Some("cookie".to_string());
        updated = true;
    }

    if credentials.cookie_header.trim().is_empty() {
        if let Some(value) = read_json_string(
            &account.settings,
            &["cookieHeader", "cookie_header", "cookie", "session"],
        ) {
            credentials.cookie_header = value;
            updated = true;
        }
    }

    if credentials.cookie_header.trim().is_empty() {
        return Err(BackendError::Provider(
            "OpenCode session cookie is invalid or expired.".to_string(),
        ));
    }

    let workspace_override = read_json_string(
        &account.settings,
        &["workspaceId", "workspace_id", "workspace"],
    )
    .and_then(|value| opencode::normalize_workspace_id(Some(&value)))
    .ok_or_else(|| {
        BackendError::Provider(
            "OpenCode workspaceId is missing in account settings. Reconnect OpenCode.".to_string(),
        )
    })?;

    let snapshot =
        opencode::fetch_usage(&credentials.cookie_header, Some(&workspace_override)).await?;

    let now_sec = now_unix_ms() / 1000;
    let rolling_resets_at = snapshot
        .rolling_reset_in_sec
        .map(|value| unix_to_rfc3339(now_sec.saturating_add(value)))
        .flatten();
    let weekly_resets_at = snapshot
        .weekly_reset_in_sec
        .map(|value| unix_to_rfc3339(now_sec.saturating_add(value)))
        .flatten();

    let mut lines = Vec::new();

    if let Some(rolling_usage_percent) = snapshot.rolling_usage_percent {
        lines.push(crate::providers::usage::progress_percent_line(
            "Session",
            rolling_usage_percent.clamp(0.0, 100.0),
            rolling_resets_at,
            Some(PERIOD_5_HOURS_MS),
        ));
    }

    if let Some(weekly_usage_percent) = snapshot.weekly_usage_percent {
        lines.push(crate::providers::usage::progress_percent_line(
            "Weekly",
            weekly_usage_percent.clamp(0.0, 100.0),
            weekly_resets_at,
            Some(PERIOD_7_DAYS_MS),
        ));
    }

    if let Some(total_cost_usd) = snapshot.monthly_total_cost_usd {
        lines.push(MetricLine::Text {
            label: "Monthly Cost".to_string(),
            value: format!("${:.2}", total_cost_usd.max(0.0)),
            color: None,
            subtitle: None,
        });
    }

    if let Some(usage_rows) = snapshot.usage_rows {
        lines.push(MetricLine::Badge {
            label: "Usage Rows".to_string(),
            text: usage_rows.to_string(),
            color: None,
            subtitle: None,
        });
    }

    if let Some(api_keys) = snapshot.api_keys {
        lines.push(MetricLine::Badge {
            label: "API Keys".to_string(),
            text: api_keys.to_string(),
            color: None,
            subtitle: None,
        });
    }

    if let Some(models) = snapshot.models {
        lines.push(MetricLine::Badge {
            label: "Models".to_string(),
            text: models.to_string(),
            color: None,
            subtitle: None,
        });
    }

    if let Some(subscription_rows) = snapshot.subscription_rows {
        lines.push(MetricLine::Badge {
            label: "Subscription Rows".to_string(),
            text: subscription_rows.to_string(),
            color: None,
            subtitle: None,
        });
    }

    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    let plan = snapshot
        .plan
        .as_deref()
        .map(plan_label)
        .filter(|v| !v.is_empty());

    let updated_credentials = if updated {
        Some(
            serde_json::to_value(credentials.with_kind()).map_err(|err| {
                BackendError::Provider(format!("Invalid OpenCode credentials: {err}"))
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
