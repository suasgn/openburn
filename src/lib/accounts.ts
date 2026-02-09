import { invoke } from "@tauri-apps/api/core"

export type ProviderDescriptor = {
  id: string
  name: string
  defaultAuthStrategyId: string
}

export type AccountRecord = {
  id: string
  providerId: string
  authStrategyId?: string | null
  label: string
  settings: unknown
  createdAt: string
  updatedAt: string
  lastFetchAt?: string | null
  lastError?: string | null
}

export type CreateAccountInput = {
  providerId: string
  authStrategyId?: string
  label?: string
  settings?: unknown
}

export type UpdateAccountInput = {
  authStrategyId?: string
  label?: string
  settings?: unknown
  clearLastError?: boolean
}

export async function listProviders(): Promise<ProviderDescriptor[]> {
  return invoke<ProviderDescriptor[]>("list_providers")
}

export async function listAccounts(): Promise<AccountRecord[]> {
  return invoke<AccountRecord[]>("list_accounts")
}

export async function createAccount(input: CreateAccountInput): Promise<AccountRecord> {
  return invoke<AccountRecord>("create_account", { input })
}

export async function updateAccount(
  accountId: string,
  input: UpdateAccountInput,
): Promise<AccountRecord> {
  return invoke<AccountRecord>("update_account", { accountId, input })
}

export async function deleteAccount(accountId: string): Promise<AccountRecord | null> {
  return invoke<AccountRecord | null>("delete_account", { accountId })
}

export async function hasAccountCredentials(accountId: string): Promise<boolean> {
  return invoke<boolean>("has_account_credentials", { accountId })
}

export async function setAccountCredentials(
  accountId: string,
  credentials: Record<string, unknown>,
): Promise<void> {
  await invoke("set_account_credentials", { accountId, credentials })
}

export async function clearAccountCredentials(accountId: string): Promise<void> {
  await invoke("clear_account_credentials", { accountId })
}
