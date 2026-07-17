use std::sync::Mutex;

use crate::error::{BackendError, Result};

use super::croc_protocol;

const MAX_TRANSFER_BYTES: usize = 10 * 1024 * 1024;

pub struct CrocTransferState {
    pending: Mutex<Option<PendingTransfer>>,
}

struct PendingTransfer {
    code: String,
    payload: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrocTransferStart {
    pub code: String,
    pub account_count: usize,
}

impl Default for CrocTransferState {
    fn default() -> Self {
        Self {
            pending: Mutex::new(None),
        }
    }
}

pub fn start(payload: String, state: &CrocTransferState) -> Result<CrocTransferStart> {
    validate_payload(&payload)?;
    let account_count = count_accounts(&payload)?;
    let code = croc_protocol::generate_code();
    let mut pending = state.pending.lock().map_err(|error| {
        BackendError::Validation(format!("could not prepare croc transfer: {error}"))
    })?;
    *pending = Some(PendingTransfer {
        code: code.clone(),
        payload,
    });
    Ok(CrocTransferStart {
        code,
        account_count,
    })
}

pub async fn finish(state: &CrocTransferState) -> Result<()> {
    let pending = state
        .pending
        .lock()
        .map_err(|error| {
            BackendError::Validation(format!("could not read croc transfer: {error}"))
        })?
        .take()
        .ok_or_else(|| BackendError::Validation("no croc export is waiting".to_string()))?;
    croc_protocol::send_payload(&pending.code, &pending.payload)
        .await
        .map_err(|error| BackendError::Validation(format!("croc transfer failed: {error}")))
}

pub async fn receive(code: &str) -> Result<String> {
    croc_protocol::receive_payload(code)
        .await
        .map_err(|error| BackendError::Validation(format!("croc transfer failed: {error}")))
}

fn validate_payload(payload: &str) -> Result<()> {
    if payload.len() > MAX_TRANSFER_BYTES {
        return Err(BackendError::Validation(
            "account transfer is too large".to_string(),
        ));
    }
    Ok(())
}

fn count_accounts(payload: &str) -> Result<usize> {
    let value: serde_json::Value = serde_json::from_str(payload)?;
    value
        .get("accounts")
        .and_then(serde_json::Value::as_array)
        .filter(|accounts| !accounts.is_empty())
        .map(Vec::len)
        .ok_or_else(|| BackendError::Validation("account transfer has no accounts".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_all_accounts() {
        let payload = r#"{"accounts":[{},{}]}"#;
        assert_eq!(count_accounts(payload).unwrap(), 2);
    }

    #[test]
    fn rejects_empty_accounts() {
        let error = count_accounts(r#"{"accounts":[]}"#).unwrap_err();
        assert!(error.to_string().contains("no accounts"));
    }

    #[test]
    fn rejects_oversized_payload() {
        let error = validate_payload(&"x".repeat(MAX_TRANSFER_BYTES + 1)).unwrap_err();
        assert!(error.to_string().contains("too large"));
    }
}
