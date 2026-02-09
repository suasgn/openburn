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
