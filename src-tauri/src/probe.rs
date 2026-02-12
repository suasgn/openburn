use serde::Serialize;
use tauri::AppHandle;

use crate::account_store::AccountStore;
use crate::error::{BackendError, Result};
use crate::providers;
use crate::providers::usage::{error_line, status_line};
use crate::providers::{MetricLine, ProbeSuccess};
use crate::secrets;

pub use crate::providers::ProviderMeta;

const ACCOUNT_META_DELIMITER: &str = " @@ ";
const ACCOUNT_LABEL_DELIMITER: &str = " :: ";

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
struct AccountScope {
    label: String,
    id: String,
}

pub fn all_provider_meta() -> Vec<ProviderMeta> {
    providers::all_provider_meta()
}

pub fn all_provider_ids() -> Vec<String> {
    providers::all_provider_ids()
}

pub fn build_error_output(provider_id: &str, message: impl Into<String>) -> ProviderOutput {
    let message = message.into();
    let runtime = providers::find_provider_runtime(provider_id);
    ProviderOutput {
        provider_id: provider_id.to_string(),
        display_name: runtime
            .map(|provider| provider.name().to_string())
            .unwrap_or_else(|| provider_id.to_string()),
        plan: None,
        lines: vec![error_line(message)],
        icon_url: runtime
            .map(|provider| provider.icon_url().to_string())
            .unwrap_or_else(|| "/vite.svg".to_string()),
    }
}

pub async fn probe_provider(
    app: &AppHandle,
    store: &AccountStore,
    provider_id: &str,
) -> Result<ProviderOutput> {
    let runtime = providers::find_provider_runtime(provider_id).ok_or_else(|| {
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
            runtime.name()
        )));
    }

    let mut had_credentials = false;
    let mut last_error: Option<BackendError> = None;
    let mut successes: Vec<(AccountScope, ProbeSuccess)> = Vec::new();
    let mut account_errors: Vec<(AccountScope, String)> = Vec::new();
    let has_multiple_accounts = accounts.len() > 1;

    // Keep account probing sequential per provider to avoid account-level burst rate limits.
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

        let result = runtime.probe(&account, credentials).await;

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
            runtime.name()
        )));
    }

    if successes.is_empty() {
        return Err(last_error.unwrap_or_else(|| {
            BackendError::Provider(format!("Failed to fetch {} usage", runtime.name()))
        }));
    }

    if !has_multiple_accounts && account_errors.is_empty() {
        if let Some((_, success)) = successes.first() {
            return Ok(ProviderOutput {
                provider_id: provider_id.to_string(),
                display_name: runtime.name().to_string(),
                plan: success.plan.clone(),
                lines: success.lines.clone(),
                icon_url: runtime.icon_url().to_string(),
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
        display_name: runtime.name().to_string(),
        plan: None,
        lines,
        icon_url: runtime.icon_url().to_string(),
    })
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
