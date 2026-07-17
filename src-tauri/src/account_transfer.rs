use serde::{Deserialize, Serialize};

use crate::account_store::AccountStore;
use crate::error::{BackendError, Result};
use crate::models::{AccountRecord, CreateAccountInput, normalize_string};
use crate::plugin_engine::manifest::LoadedPlugin;
use crate::secrets;

const TRANSFER_FORMAT: &str = "openburn.account-transfer";
const TRANSFER_VERSION: u32 = 2;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountTransferBundle {
    format: String,
    version: u32,
    accounts: Vec<AccountTransferAccount>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountTransferAccount {
    plugin_id: String,
    auth_strategy_id: String,
    label: String,
    settings: serde_json::Value,
    #[serde(default)]
    credentials: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountTransferBundleRef {
    format: &'static str,
    version: u32,
    accounts: Vec<AccountTransferAccount>,
}

pub fn export_account_transfer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &AccountStore,
    plugins: &[LoadedPlugin],
) -> Result<String> {
    let accounts = store.list_accounts()?;
    if accounts.is_empty() {
        return Err(BackendError::Validation(
            "no accounts to export".to_string(),
        ));
    }

    let mut transfer_accounts = Vec::with_capacity(accounts.len());
    for account in accounts {
        let strategy_id = export_strategy_id(&account, plugins)?;
        let credentials = secrets::get_account_credentials(app, store, &account.id)?;
        if let Some(credentials) = &credentials {
            if !credentials.is_object() {
                return Err(BackendError::Validation(format!(
                    "credentials for account '{}' must be a JSON object",
                    account.label
                )));
            }
        }
        transfer_accounts.push(AccountTransferAccount {
            plugin_id: account.plugin_id,
            auth_strategy_id: strategy_id,
            label: account.label,
            settings: account.settings,
            credentials,
        });
    }

    Ok(serde_json::to_string(&AccountTransferBundleRef {
        format: TRANSFER_FORMAT,
        version: TRANSFER_VERSION,
        accounts: transfer_accounts,
    })?)
}

fn export_strategy_id(account: &AccountRecord, plugins: &[LoadedPlugin]) -> Result<String> {
    if let Some(strategy_id) = account.auth_strategy_id.as_deref() {
        return Ok(strategy_id.to_string());
    }

    let plugin = plugins
        .iter()
        .find(|plugin| plugin.manifest.id == account.plugin_id)
        .ok_or_else(|| {
            BackendError::Validation(format!(
                "plugin '{}' for account '{}' is not available and its auth strategy is missing",
                account.plugin_id, account.label
            ))
        })?;
    plugin
        .manifest
        .auth
        .as_ref()
        .and_then(|auth| auth.default_strategy_id.as_deref())
        .map(str::to_string)
        .ok_or_else(|| {
            BackendError::Validation(format!(
                "auth strategy missing for account '{}', plugin '{}'",
                account.label, account.plugin_id
            ))
        })
}

pub fn import_account_transfer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &AccountStore,
    plugins: &[LoadedPlugin],
    payload: &str,
) -> Result<Vec<AccountRecord>> {
    let transfer: AccountTransferBundle = serde_json::from_str(payload.trim()).map_err(|_| {
        BackendError::Validation("not a valid OpenBurn account QR code".to_string())
    })?;
    if transfer.format != TRANSFER_FORMAT || transfer.version != TRANSFER_VERSION {
        return Err(BackendError::Validation(
            "unsupported OpenBurn account QR code".to_string(),
        ));
    }
    if transfer.accounts.is_empty() {
        return Err(BackendError::Validation(
            "QR account bundle is empty".to_string(),
        ));
    }

    for account in &transfer.accounts {
        let resolved_plugin_id =
            resolve_import_plugin_id(&account.plugin_id, plugins).ok_or_else(|| {
                BackendError::Validation(format!(
                    "plugin '{}' from QR is not available on this device",
                    account.plugin_id
                ))
            })?;
        let plugin = plugins
            .iter()
            .find(|plugin| plugin.manifest.id == resolved_plugin_id)
            .ok_or_else(|| {
                BackendError::Validation(format!(
                    "plugin '{}' from QR is not available on this device",
                    account.plugin_id
                ))
            })?;
        if !plugin.manifest.auth.as_ref().is_some_and(|auth| {
            auth.strategies
                .iter()
                .any(|strategy| strategy.id == account.auth_strategy_id)
        }) {
            return Err(BackendError::Validation(format!(
                "auth strategy '{}' for plugin '{}' is not supported",
                account.auth_strategy_id, account.plugin_id
            )));
        }
        if !account.settings.is_object()
            || account
                .credentials
                .as_ref()
                .is_some_and(|credentials| !credentials.is_object())
        {
            return Err(BackendError::Validation(format!(
                "QR data for account '{}' is malformed",
                account.label
            )));
        }
        normalize_string(&account.label)
            .ok_or_else(|| BackendError::Validation("QR account label is missing".to_string()))?;
    }

    let mut imported: Vec<AccountRecord> = Vec::with_capacity(transfer.accounts.len());
    for account in transfer.accounts {
        let plugin_id = resolve_import_plugin_id(&account.plugin_id, plugins).ok_or_else(|| {
            BackendError::Validation(format!(
                "plugin '{}' from QR is not available on this device",
                account.plugin_id
            ))
        })?;
        let created = store.create_account(CreateAccountInput {
            plugin_id,
            auth_strategy_id: Some(account.auth_strategy_id),
            label: Some(account.label),
            settings: Some(account.settings),
        })?;
        if let Some(credentials) = account.credentials {
            if let Err(error) =
                secrets::set_account_credentials(app, store, &created.id, &credentials)
            {
                let _ = store.delete_account(&created.id);
                for previous in &imported {
                    let _ = store.delete_account(&previous.id);
                }
                return Err(error);
            }
        }
        imported.push(created);
    }

    imported
        .into_iter()
        .map(|account| {
            store
                .get_account(&account.id)?
                .ok_or(BackendError::AccountNotFound)
        })
        .collect()
}

fn resolve_import_plugin_id(plugin_id: &str, plugins: &[LoadedPlugin]) -> Option<String> {
    if plugins.iter().any(|plugin| plugin.manifest.id == plugin_id) {
        return Some(plugin_id.to_string());
    }
    if plugin_id == "opencode-go"
        && plugins
            .iter()
            .any(|plugin| plugin.manifest.id == "opencode")
    {
        return Some("opencode".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_transfer_payloads() {
        let result = serde_json::from_str::<AccountTransferBundle>(
            r#"{"format":"other","version":1,"accounts":[]}"#,
        );
        assert!(result.is_ok());
        assert_ne!(result.unwrap().format, TRANSFER_FORMAT);
    }

    #[test]
    fn exports_stored_strategy_without_loaded_plugin() {
        let account = AccountRecord {
            id: "account-1".to_string(),
            plugin_id: "opencode-go".to_string(),
            enabled: true,
            auth_strategy_id: Some("apiKey".to_string()),
            label: "opencode-go".to_string(),
            settings: serde_json::json!({}),
            credentials: None,
            created_at: String::new(),
            updated_at: String::new(),
            last_fetch_at: None,
            last_error: None,
        };
        assert_eq!(export_strategy_id(&account, &[]).unwrap(), "apiKey");
    }
}
