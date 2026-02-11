# Providers

Openburn providers are implemented directly in Rust.

## Provider model

- Providers are registered in `src-tauri/src/providers/registry.rs`.
- Each provider has a stable `providerId`.
- Accounts reference providers using `providerId`.
- Provider execution logic lives in Rust, not JavaScript.

## Provider contract modules

- `src-tauri/src/providers/contract.rs` defines full provider contract types.
- `src-tauri/src/providers/registry.rs` defines provider contracts and descriptors.
- `src-tauri/src/providers/validation.rs` enforces provider-specific account validation.

Provider list command:

- `list_providers() -> ProviderDescriptor[]`

## Runtime contract

- `providerId` must match `^[a-z0-9][a-z0-9._-]{1,63}$`.
- `providerId` must exist in the provider registry.
- `authStrategyId` must match `^[a-zA-Z][a-zA-Z0-9._-]{1,63}$` when provided.
- `authStrategyId` must be supported by the selected provider.

## Account store

Account records are persisted in `<app_data_dir>/accounts.json`.

See `docs/accounts/accounts.md` for the account schema and credential encryption behavior.

## Usage snapshots cache

Latest successful provider outputs are cached in `<app_data_dir>/snapshots.json`.

Behavior:

- Each `accountId` stores one latest successful snapshot for that account.
- Cache writes are best-effort and happen after probe results are emitted.
- Account-level `Error` lines are ignored and do not overwrite existing snapshots.
- On app startup, cached snapshots are loaded before fresh probes begin so the UI can render last-known data instead of skeletons while loading.

Stored shape:

```json
{
  "schemaVersion": 1,
  "snapshots": {
    "4120a8d0-e885-408f-8a3a-a180cab20312": {
      "accountId": "4120a8d0-e885-408f-8a3a-a180cab20312",
      "providerId": "codex",
      "displayName": "Codex",
      "plan": "Plus",
      "lines": [],
      "iconUrl": "/providers/codex.svg",
      "updatedAt": "2026-02-11T10:45:00.000Z"
    }
  }
}
```
