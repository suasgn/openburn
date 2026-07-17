use std::collections::HashMap;
#[cfg(mobile)]
use std::fs;
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
#[cfg(desktop)]
use keyring::Entry;
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::Sha256;
#[cfg(mobile)]
use tauri::Manager;
use tauri::{AppHandle, Runtime};
#[cfg(desktop)]
use tauri_plugin_keyring_store::KeyringExt;

use crate::account_store::AccountStore;
use crate::error::{BackendError, Result};
use crate::models::{AccountRecord, EncryptedCredentials};

#[cfg(desktop)]
const SERVICE_NAME: &str = "openburn";
const MASTER_KEY_PREFIX: &str = "master-key-v";
const KEY_VERSION: u32 = 1;
const ALGORITHM: &str = "xchacha20poly1305";
const HKDF_SALT: &[u8] = b"openburn-credentials-v1";

static MASTER_KEY_CACHE: OnceLock<Mutex<HashMap<u32, [u8; 32]>>> = OnceLock::new();

fn credential_id(account: &AccountRecord) -> String {
    format!("{}:{}", account.plugin_id, account.id)
}

fn master_key_name(version: u32) -> String {
    format!("{MASTER_KEY_PREFIX}{version}")
}

#[cfg(mobile)]
fn mobile_master_key_path<R: Runtime>(
    app: &AppHandle<R>,
    version: u32,
) -> Result<std::path::PathBuf> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| BackendError::Path(error.to_string()))?;
    Ok(data_dir.join(format!("{}.bin", master_key_name(version))))
}

#[cfg(mobile)]
fn read_mobile_master_key<R: Runtime>(
    app: &AppHandle<R>,
    version: u32,
) -> Result<Option<[u8; 32]>> {
    let path = mobile_master_key_path(app, version)?;
    match fs::read(path) {
        Ok(payload) => payload
            .try_into()
            .map(Some)
            .map_err(|_| BackendError::Crypto("master key length invalid".to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn persist_master_key<R: Runtime>(app: &AppHandle<R>, version: u32, key: &[u8; 32]) -> Result<()> {
    #[cfg(mobile)]
    {
        let path = mobile_master_key_path(app, version)?;
        fs::write(path, key)?;
        return Ok(());
    }

    #[cfg(desktop)]
    {
        app.keyring()
            .store
            .set_bytes(&master_key_name(version), key)
            .map_err(|error| BackendError::Keyring(error.to_string()))
    }
}

fn read_master_key<R: Runtime>(app: &AppHandle<R>, version: u32) -> Result<Option<[u8; 32]>> {
    let cache = MASTER_KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(key) = cache
        .lock()
        .expect("master key cache mutex poisoned")
        .get(&version)
        .copied()
    {
        return Ok(Some(key));
    }

    #[cfg(mobile)]
    let key = read_mobile_master_key(app, version)?;

    #[cfg(desktop)]
    let key = match app.keyring().store.get_bytes(&master_key_name(version)) {
        Ok(Some(payload)) => Some(
            payload
                .try_into()
                .map_err(|_| BackendError::Crypto("master key length invalid".to_string()))?,
        ),
        Ok(None) => {
            let entry = Entry::new(SERVICE_NAME, &master_key_name(version))
                .map_err(|error| BackendError::Keyring(error.to_string()))?;
            let Some(payload) = entry.get_secret().ok() else {
                return Ok(None);
            };
            Some(payload.try_into().map_err(|_| {
                BackendError::Crypto("legacy master key length invalid".to_string())
            })?)
        }
        Err(error) => return Err(BackendError::Keyring(error.to_string())),
    };

    if let Some(key) = key {
        let mut cache = cache.lock().expect("master key cache mutex poisoned");
        cache.insert(version, key);
        Ok(Some(key))
    } else {
        Ok(None)
    }
}

fn get_or_create_master_key<R: Runtime>(app: &AppHandle<R>, version: u32) -> Result<[u8; 32]> {
    if let Some(key) = read_master_key(app, version)? {
        return Ok(key);
    }

    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    persist_master_key(app, version, &key)?;

    let cache = MASTER_KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache
        .lock()
        .expect("master key cache mutex poisoned")
        .insert(version, key);
    Ok(key)
}

fn derive_key(master_key: &[u8; 32], credential_id: &str) -> Result<[u8; 32]> {
    let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), master_key);
    let mut derived = [0u8; 32];
    hkdf.expand(credential_id.as_bytes(), &mut derived)
        .map_err(|_| BackendError::Crypto("key derivation failed".to_string()))?;
    Ok(derived)
}

#[allow(deprecated)]
fn encrypt_credentials<R: Runtime>(
    app: &AppHandle<R>,
    account: &AccountRecord,
    credentials: &serde_json::Value,
) -> Result<EncryptedCredentials> {
    let master_key = get_or_create_master_key(app, KEY_VERSION)?;
    let credential_id = credential_id(account);
    let key = derive_key(&master_key, &credential_id)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| BackendError::Crypto("invalid encryption key".to_string()))?;

    let mut nonce_bytes = [0u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let payload = serde_json::to_vec(credentials)?;
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: &payload,
                aad: credential_id.as_bytes(),
            },
        )
        .map_err(|_| BackendError::Crypto("encryption failed".to_string()))?;

    Ok(EncryptedCredentials {
        alg: ALGORITHM.to_string(),
        key_version: KEY_VERSION,
        nonce: URL_SAFE_NO_PAD.encode(nonce_bytes),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

#[allow(dead_code)]
#[allow(deprecated)]
fn decrypt_credentials<R: Runtime>(
    app: &AppHandle<R>,
    account: &AccountRecord,
    encrypted: &EncryptedCredentials,
) -> Result<serde_json::Value> {
    if encrypted.key_version > KEY_VERSION {
        return Err(BackendError::Crypto(format!(
            "unsupported key version: {}",
            encrypted.key_version
        )));
    }

    let nonce_bytes = URL_SAFE_NO_PAD
        .decode(&encrypted.nonce)
        .map_err(|err| BackendError::Crypto(format!("invalid nonce: {err}")))?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(&encrypted.ciphertext)
        .map_err(|err| BackendError::Crypto(format!("invalid ciphertext: {err}")))?;

    let master_key = read_master_key(app, encrypted.key_version)?.ok_or_else(|| {
        BackendError::Crypto(format!("master key v{} missing", encrypted.key_version))
    })?;

    let credential_id = credential_id(account);
    let key = derive_key(&master_key, &credential_id)?;

    let plaintext = match encrypted.alg.as_str() {
        "xchacha20poly1305" => {
            if nonce_bytes.len() != 24 {
                return Err(BackendError::Crypto("invalid nonce length".to_string()));
            }
            let cipher = XChaCha20Poly1305::new_from_slice(&key)
                .map_err(|_| BackendError::Crypto("invalid decryption key".to_string()))?;
            let nonce = XNonce::from_slice(&nonce_bytes);
            cipher
                .decrypt(
                    nonce,
                    Payload {
                        msg: &ciphertext,
                        aad: credential_id.as_bytes(),
                    },
                )
                .map_err(|_| BackendError::Crypto("decryption failed".to_string()))?
        }
        "chacha20poly1305" => {
            if nonce_bytes.len() != 12 {
                return Err(BackendError::Crypto("invalid nonce length".to_string()));
            }
            let cipher = ChaCha20Poly1305::new_from_slice(&key)
                .map_err(|_| BackendError::Crypto("invalid decryption key".to_string()))?;
            let nonce = Nonce::from_slice(&nonce_bytes);
            cipher
                .decrypt(
                    nonce,
                    Payload {
                        msg: &ciphertext,
                        aad: credential_id.as_bytes(),
                    },
                )
                .map_err(|_| BackendError::Crypto("decryption failed".to_string()))?
        }
        _ => {
            return Err(BackendError::Crypto(format!(
                "unsupported algorithm: {}",
                encrypted.alg
            )));
        }
    };

    let value = serde_json::from_slice(&plaintext)?;
    Ok(value)
}

pub fn set_account_credentials<R: Runtime>(
    app: &AppHandle<R>,
    store: &AccountStore,
    account_id: &str,
    credentials: &serde_json::Value,
) -> Result<()> {
    let account = store
        .get_account(account_id)?
        .ok_or(BackendError::AccountNotFound)?;
    let encrypted = encrypt_credentials(app, &account, credentials)?;
    store.set_credentials_blob(account_id, encrypted)
}

#[allow(dead_code)]
pub fn get_account_credentials<R: Runtime>(
    app: &AppHandle<R>,
    store: &AccountStore,
    account_id: &str,
) -> Result<Option<serde_json::Value>> {
    let account = store
        .get_account(account_id)?
        .ok_or(BackendError::AccountNotFound)?;

    let Some(encrypted) = store.get_credentials_blob(account_id)? else {
        return Ok(None);
    };

    let value = decrypt_credentials(app, &account, &encrypted)?;
    if encrypted.key_version != KEY_VERSION || encrypted.alg != ALGORITHM {
        let updated = encrypt_credentials(app, &account, &value)?;
        store.set_credentials_blob(account_id, updated)?;
    }

    Ok(Some(value))
}

pub fn has_account_credentials(store: &AccountStore, account_id: &str) -> Result<bool> {
    store.has_credentials_blob(account_id)
}

pub fn clear_account_credentials(store: &AccountStore, account_id: &str) -> Result<()> {
    store.delete_credentials_blob(account_id)
}
