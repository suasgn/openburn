# Account

Account records are persisted in JSON at:

- `<app_data_dir>/accounts.json`

## Stored account shape

```json
{
  "id": "uuid",
  "providerId": "codex",
  "authStrategyId": "oauth",
  "label": "Codex Personal",
  "settings": {},
  "credentials": {
    "alg": "xchacha20poly1305",
    "keyVersion": 1,
    "nonce": "...",
    "ciphertext": "..."
  },
  "createdAt": "2026-02-09T12:00:00Z",
  "updatedAt": "2026-02-09T12:00:00Z",
  "lastFetchAt": null,
  "lastError": null
}
```

## Tauri commands

Supported `providerId` values:

- `codex`
- `copilot`
- `claude`
- `zai`

- `list_accounts() -> AccountRecord[]`
- `get_account(accountId) -> AccountRecord | null`
- `create_account(input) -> AccountRecord`
- `update_account(accountId, input) -> AccountRecord`
- `delete_account(accountId) -> AccountRecord | null`
- `set_account_credentials(accountId, credentials) -> void`
- `has_account_credentials(accountId) -> boolean`
- `clear_account_credentials(accountId) -> void`

## Credentials vault

Credentials are encrypted before being written to disk:

- Envelope encryption: per-account key derived from a master key (HKDF-SHA256)
- Cipher support: `xchacha20poly1305` and `chacha20poly1305`
- Master key storage: OS keychain via Tauri keyring plugin

Encrypted credential blobs are stored directly on each account record in `accounts.json`.

## Input contracts

`create_account` input:

```json
{
  "providerId": "codex",
  "authStrategyId": "oauth",
  "label": "Codex Personal",
  "settings": {}
}
```

`update_account` input (partial):

```json
{
  "label": "New label",
  "authStrategyId": "apiKey",
  "settings": {},
  "clearLastError": true
}
```

Validation rules:

- `providerId` must match `^[a-z0-9][a-z0-9._-]{1,63}$`
- `providerId` must exist in the provider registry
- `authStrategyId` must match `^[a-zA-Z][a-zA-Z0-9._-]{1,63}$` when provided
- `authStrategyId` must be supported by the selected provider when provided
- `settings` must be a JSON object
