// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod account_store;
mod error;
mod models;
mod providers;
mod secrets;
mod utils;

use account_store::AccountStore;
use models::{AccountRecord, CreateAccountInput, UpdateAccountInput};
use providers::ProviderDescriptor;
use tauri::{Manager, State};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn list_providers() -> Vec<ProviderDescriptor> {
    providers::all_provider_descriptors()
}

#[tauri::command]
fn list_accounts(store: State<'_, AccountStore>) -> Result<Vec<AccountRecord>, String> {
    store.list_accounts().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_account(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<Option<AccountRecord>, String> {
    store
        .get_account(&account_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn create_account(
    store: State<'_, AccountStore>,
    input: CreateAccountInput,
) -> Result<AccountRecord, String> {
    store.create_account(input).map_err(|err| err.to_string())
}

#[tauri::command]
fn update_account(
    store: State<'_, AccountStore>,
    account_id: String,
    input: UpdateAccountInput,
) -> Result<AccountRecord, String> {
    store
        .update_account(&account_id, input)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn delete_account(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<Option<AccountRecord>, String> {
    store
        .delete_account(&account_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_account_credentials(
    app: tauri::AppHandle,
    store: State<'_, AccountStore>,
    account_id: String,
    credentials: serde_json::Value,
) -> Result<(), String> {
    secrets::set_account_credentials(&app, store.inner(), &account_id, &credentials)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn has_account_credentials(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<bool, String> {
    secrets::has_account_credentials(store.inner(), &account_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_account_credentials(
    store: State<'_, AccountStore>,
    account_id: String,
) -> Result<(), String> {
    secrets::clear_account_credentials(store.inner(), &account_id).map_err(|err| err.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_keyring::init())
        .setup(|app| {
            let store = AccountStore::load(app.handle())
                .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;
            app.manage(store);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            list_providers,
            list_accounts,
            get_account,
            create_account,
            update_account,
            delete_account,
            set_account_credentials,
            has_account_credentials,
            clear_account_credentials
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
