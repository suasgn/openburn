use serde::Serialize;
use tauri::AppHandle;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::account_store::AccountStore;
use crate::error::{BackendError, Result};
use crate::models::AccountRecord;
use crate::provider_clients::normalize_percent;
use crate::provider_clients::{claude, codex, copilot, zai};
use crate::secrets;

const PERIOD_5_HOURS_MS: u64 = 5 * 60 * 60 * 1000;
const PERIOD_7_DAYS_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const PERIOD_30_DAYS_MS: u64 = 30 * 24 * 60 * 60 * 1000;
const ACCOUNT_META_DELIMITER: &str = " @@ ";
const ACCOUNT_LABEL_DELIMITER: &str = " :: ";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProgressFormat {
    Percent,
    Dollars,
    Count { suffix: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MetricLine {
    Text {
        label: String,
        value: String,
        color: Option<String>,
        subtitle: Option<String>,
    },
    Progress {
        label: String,
        used: f64,
        limit: f64,
        format: ProgressFormat,
        #[serde(rename = "resetsAt")]
        resets_at: Option<String>,
        #[serde(rename = "periodDurationMs")]
        period_duration_ms: Option<u64>,
        color: Option<String>,
    },
    Badge {
        label: String,
        text: String,
        color: Option<String>,
        subtitle: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOutput {
    pub provider_id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub lines: Vec<MetricLine>,
    pub icon_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLineDto {
    #[serde(rename = "type")]
    pub line_type: String,
    pub label: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMeta {
    pub id: String,
    pub name: String,
    pub icon_url: String,
    pub brand_color: Option<String>,
    pub lines: Vec<ManifestLineDto>,
    pub primary_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBatchStarted {
    pub batch_id: String,
    pub provider_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResultEvent {
    pub batch_id: String,
    pub output: ProviderOutput,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBatchCompleteEvent {
    pub batch_id: String,
}

#[derive(Debug, Clone)]
struct ProbeSuccess {
    plan: Option<String>,
    lines: Vec<MetricLine>,
    updated_credentials: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct AccountScope {
    label: String,
    id: String,
}

#[derive(Debug, Clone, Copy)]
struct ManifestLineSpec {
    line_type: &'static str,
    label: &'static str,
    scope: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct ProviderSpec {
    id: &'static str,
    name: &'static str,
    icon_url: &'static str,
    brand_color: &'static str,
    lines: &'static [ManifestLineSpec],
    primary_candidates: &'static [&'static str],
}

const CODEX_LINES: [ManifestLineSpec; 4] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Session",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Weekly",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Reviews",
        scope: "detail",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Credits",
        scope: "detail",
    },
];

const CLAUDE_LINES: [ManifestLineSpec; 4] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Session",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Weekly",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Sonnet",
        scope: "detail",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Extra usage",
        scope: "detail",
    },
];

const COPILOT_LINES: [ManifestLineSpec; 3] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Premium",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Chat",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Completions",
        scope: "overview",
    },
];

const ZAI_LINES: [ManifestLineSpec; 2] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Token Usage",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Utility Usage",
        scope: "overview",
    },
];

const PROVIDER_SPECS: [ProviderSpec; 4] = [
    ProviderSpec {
        id: "codex",
        name: "Codex",
        icon_url: "/providers/codex.svg",
        brand_color: "#74AA9C",
        lines: &CODEX_LINES,
        primary_candidates: &["Session"],
    },
    ProviderSpec {
        id: "copilot",
        name: "Copilot",
        icon_url: "/providers/copilot.svg",
        brand_color: "#A855F7",
        lines: &COPILOT_LINES,
        primary_candidates: &["Premium", "Chat"],
    },
    ProviderSpec {
        id: "claude",
        name: "Claude",
        icon_url: "/providers/claude.svg",
        brand_color: "#DE7356",
        lines: &CLAUDE_LINES,
        primary_candidates: &["Session"],
    },
    ProviderSpec {
        id: "zai",
        name: "Z.ai",
        icon_url: "/providers/zai.svg",
        brand_color: "#2D2D2D",
        lines: &ZAI_LINES,
        primary_candidates: &["Token Usage", "Utility Usage"],
    },
];

pub fn all_provider_meta() -> Vec<ProviderMeta> {
    PROVIDER_SPECS
        .iter()
        .map(|spec| ProviderMeta {
            id: spec.id.to_string(),
            name: spec.name.to_string(),
            icon_url: spec.icon_url.to_string(),
            brand_color: Some(spec.brand_color.to_string()),
            lines: spec
                .lines
                .iter()
                .map(|line| ManifestLineDto {
                    line_type: line.line_type.to_string(),
                    label: line.label.to_string(),
                    scope: line.scope.to_string(),
                })
                .collect(),
            primary_candidates: spec
                .primary_candidates
                .iter()
                .map(|label| label.to_string())
                .collect(),
        })
        .collect()
}

pub fn all_provider_ids() -> Vec<String> {
    PROVIDER_SPECS
        .iter()
        .map(|provider| provider.id.to_string())
        .collect()
}

pub fn build_error_output(provider_id: &str, message: impl Into<String>) -> ProviderOutput {
    let message = message.into();
    let spec = provider_spec(provider_id);
    ProviderOutput {
        provider_id: provider_id.to_string(),
        display_name: spec
            .map(|provider| provider.name.to_string())
            .unwrap_or_else(|| provider_id.to_string()),
        plan: None,
        lines: vec![error_line(message)],
        icon_url: spec
            .map(|provider| provider.icon_url.to_string())
            .unwrap_or_else(|| "/vite.svg".to_string()),
    }
}

pub async fn probe_provider(
    app: &AppHandle,
    store: &AccountStore,
    provider_id: &str,
) -> Result<ProviderOutput> {
    let spec = provider_spec(provider_id).ok_or_else(|| {
        BackendError::Provider(format!("provider '{}' is not registered", provider_id))
    })?;

    let mut accounts = store
        .list_accounts()?
        .into_iter()
        .filter(|account| account.provider_id == provider_id)
        .collect::<Vec<_>>();

    accounts.sort_by(|left, right| {
        let left_key = left.label.to_ascii_lowercase();
        let right_key = right.label.to_ascii_lowercase();
        left_key
            .cmp(&right_key)
            .then_with(|| left.id.cmp(&right.id))
    });

    if accounts.is_empty() {
        return Err(BackendError::Provider(format!(
            "No {} account configured",
            spec.name
        )));
    }

    let mut had_credentials = false;
    let mut last_error: Option<BackendError> = None;
    let mut successes: Vec<(AccountScope, ProbeSuccess)> = Vec::new();
    let mut account_errors: Vec<(AccountScope, String)> = Vec::new();
    let has_multiple_accounts = accounts.len() > 1;

    for account in accounts {
        let account_scope = AccountScope {
            label: normalized_account_label(&account.label, &account.id),
            id: account.id.clone(),
        };
        let credentials = match secrets::get_account_credentials(app, store, &account.id)? {
            Some(value) => {
                had_credentials = true;
                value
            }
            None => continue,
        };

        let result = match provider_id {
            "codex" => probe_codex(credentials).await,
            "copilot" => probe_copilot(credentials).await,
            "claude" => probe_claude(credentials).await,
            "zai" => probe_zai(&account, credentials).await,
            _ => Err(BackendError::Provider(format!(
                "provider '{}' is not supported",
                provider_id
            ))),
        };

        match result {
            Ok(success) => {
                if let Some(updated) = success.updated_credentials.clone() {
                    let _ = secrets::set_account_credentials(app, store, &account.id, &updated);
                }
                let _ = store.record_probe_success(&account.id);
                successes.push((account_scope, success));
            }
            Err(err) => {
                let message = err.to_string();
                let _ = store.record_probe_error(&account.id, &message);
                account_errors.push((account_scope, message));
                last_error = Some(err);
            }
        }
    }

    if !had_credentials {
        return Err(BackendError::Provider(format!(
            "No credentials configured for {}",
            spec.name
        )));
    }

    if successes.is_empty() {
        return Err(last_error.unwrap_or_else(|| {
            BackendError::Provider(format!("Failed to fetch {} usage", spec.name))
        }));
    }

    if !has_multiple_accounts && account_errors.is_empty() {
        if let Some((_, success)) = successes.first() {
            return Ok(ProviderOutput {
                provider_id: provider_id.to_string(),
                display_name: spec.name.to_string(),
                plan: success.plan.clone(),
                lines: success.lines.clone(),
                icon_url: spec.icon_url.to_string(),
            });
        }
    }

    let mut lines: Vec<MetricLine> = Vec::new();

    for (account_scope, success) in successes {
        if let Some(plan) = success.plan.as_ref().map(|value| value.trim()) {
            if !plan.is_empty() {
                lines.push(prefix_metric_line(
                    MetricLine::Badge {
                        label: "Plan".to_string(),
                        text: plan.to_string(),
                        color: None,
                        subtitle: None,
                    },
                    &account_scope,
                ));
            }
        }

        for line in success.lines {
            lines.push(prefix_metric_line(line, &account_scope));
        }
    }

    for (account_scope, error_message) in account_errors {
        lines.push(MetricLine::Badge {
            label: account_scoped_label(&account_scope, "Error"),
            text: error_message,
            color: Some("#ef4444".to_string()),
            subtitle: None,
        });
    }

    if lines.is_empty() {
        lines.push(status_line("No usage data"));
    }

    Ok(ProviderOutput {
        provider_id: provider_id.to_string(),
        display_name: spec.name.to_string(),
        plan: None,
        lines,
        icon_url: spec.icon_url.to_string(),
    })
}

async fn probe_codex(credentials: serde_json::Value) -> Result<ProbeSuccess> {
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

async fn probe_claude(credentials: serde_json::Value) -> Result<ProbeSuccess> {
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

async fn probe_copilot(credentials: serde_json::Value) -> Result<ProbeSuccess> {
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

async fn probe_zai(
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

fn provider_spec(provider_id: &str) -> Option<&'static ProviderSpec> {
    PROVIDER_SPECS
        .iter()
        .find(|provider| provider.id == provider_id)
}

fn progress_percent_line(
    label: &str,
    used: f64,
    resets_at: Option<String>,
    period_duration_ms: Option<u64>,
) -> MetricLine {
    MetricLine::Progress {
        label: label.to_string(),
        used,
        limit: 100.0,
        format: ProgressFormat::Percent,
        resets_at,
        period_duration_ms,
        color: None,
    }
}

fn status_line(text: &str) -> MetricLine {
    MetricLine::Badge {
        label: "Status".to_string(),
        text: text.to_string(),
        color: Some("#a3a3a3".to_string()),
        subtitle: None,
    }
}

fn error_line(message: String) -> MetricLine {
    MetricLine::Badge {
        label: "Error".to_string(),
        text: message,
        color: Some("#ef4444".to_string()),
        subtitle: None,
    }
}

fn normalized_account_label(label: &str, account_id: &str) -> String {
    let trimmed = label.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    let short_id = account_id.chars().take(8).collect::<String>();
    if short_id.is_empty() {
        "Account".to_string()
    } else {
        format!("Account {}", short_id)
    }
}

fn account_scoped_label(account_scope: &AccountScope, line_label: &str) -> String {
    format!(
        "{}{}{}{}{}",
        account_scope.label.trim(),
        ACCOUNT_META_DELIMITER,
        account_scope.id.trim(),
        ACCOUNT_LABEL_DELIMITER,
        line_label.trim()
    )
}

fn prefix_metric_line(line: MetricLine, account_scope: &AccountScope) -> MetricLine {
    match line {
        MetricLine::Text {
            label,
            value,
            color,
            subtitle,
        } => MetricLine::Text {
            label: account_scoped_label(account_scope, &label),
            value,
            color,
            subtitle,
        },
        MetricLine::Progress {
            label,
            used,
            limit,
            format,
            resets_at,
            period_duration_ms,
            color,
        } => MetricLine::Progress {
            label: account_scoped_label(account_scope, &label),
            used,
            limit,
            format,
            resets_at,
            period_duration_ms,
            color,
        },
        MetricLine::Badge {
            label,
            text,
            color,
            subtitle,
        } => MetricLine::Badge {
            label: account_scoped_label(account_scope, &label),
            text,
            color,
            subtitle,
        },
    }
}

fn plan_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    trimmed
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().to_string();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_number(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(int_value) = trimmed.parse::<i64>() {
        return Some(int_value as f64);
    }
    trimmed.parse::<f64>().ok()
}

fn unix_to_rfc3339(value: i64) -> Option<String> {
    if value <= 0 {
        return None;
    }
    let seconds = if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    };
    let timestamp = OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    timestamp.format(&Rfc3339).ok()
}

fn duration_ms_from_seconds(seconds: Option<i64>) -> Option<u64> {
    seconds
        .filter(|value| *value > 0)
        .map(|value| value as u64)
        .map(|value| value.saturating_mul(1000))
}

fn normalize_resets_at(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn dollars_from_cents(value: f64) -> f64 {
    (value / 100.0 * 100.0).round() / 100.0
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

fn read_json_string(settings: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let object = settings.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key) {
            if let Some(text) = value.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
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
