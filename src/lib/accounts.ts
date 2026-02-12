import { invoke } from "@tauri-apps/api/core"

export type ProviderAuthStrategy = {
  id: string
  label: string
}

export type ProviderDescriptor = {
  id: string
  name: string
  defaultAuthStrategyId: string
  authStrategies: ProviderAuthStrategy[]
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

type AccountRecordWire = Partial<AccountRecord> & {
  provider_id?: string
  auth_strategy_id?: string | null
  created_at?: string
  updated_at?: string
  last_fetch_at?: string | null
  last_error?: string | null
}

function normalizeAccountRecord(record: AccountRecordWire): AccountRecord {
  return {
    id: String(record.id ?? ""),
    providerId: String(record.providerId ?? record.provider_id ?? ""),
    authStrategyId:
      record.authStrategyId !== undefined
        ? record.authStrategyId
        : record.auth_strategy_id !== undefined
          ? record.auth_strategy_id
          : null,
    label: String(record.label ?? ""),
    settings: record.settings ?? {},
    createdAt: String(record.createdAt ?? record.created_at ?? ""),
    updatedAt: String(record.updatedAt ?? record.updated_at ?? ""),
    lastFetchAt:
      record.lastFetchAt !== undefined
        ? record.lastFetchAt
        : record.last_fetch_at !== undefined
          ? record.last_fetch_at
          : null,
    lastError:
      record.lastError !== undefined
        ? record.lastError
        : record.last_error !== undefined
          ? record.last_error
          : null,
  }
}

export type OAuthStartResponse = {
  requestId: string
  url: string
  redirectUri: string
  userCode?: string | null
}

export type OAuthResult = {
  accountId: string
  expiresAt: number
}

export async function listProviders(): Promise<ProviderDescriptor[]> {
  return invoke<ProviderDescriptor[]>("list_providers")
}

export async function listAccounts(): Promise<AccountRecord[]> {
  const rows = await invoke<AccountRecordWire[]>("list_accounts")
  return rows
    .map(normalizeAccountRecord)
    .filter((record) => record.id.length > 0 && record.providerId.length > 0)
}

export async function createAccount(input: CreateAccountInput): Promise<AccountRecord> {
  const record = await invoke<AccountRecordWire>("create_account", { input })
  return normalizeAccountRecord(record)
}

export async function updateAccount(
  accountId: string,
  input: UpdateAccountInput,
): Promise<AccountRecord> {
  const record = await invoke<AccountRecordWire>("update_account", { accountId, input })
  return normalizeAccountRecord(record)
}

export async function deleteAccount(accountId: string): Promise<AccountRecord | null> {
  const record = await invoke<AccountRecordWire | null>("delete_account", { accountId })
  return record ? normalizeAccountRecord(record) : null
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

export async function startCodexOAuth(accountId: string): Promise<OAuthStartResponse> {
  return invoke<OAuthStartResponse>("start_codex_oauth", { accountId })
}

export async function finishCodexOAuth(
  requestId: string,
  timeoutMs?: number,
): Promise<OAuthResult> {
  return invoke<OAuthResult>("finish_codex_oauth", { requestId, timeoutMs })
}

export async function cancelCodexOAuth(requestId: string): Promise<boolean> {
  return invoke<boolean>("cancel_codex_oauth", { requestId })
}

export async function startAntigravityOAuth(accountId: string): Promise<OAuthStartResponse> {
  return invoke<OAuthStartResponse>("start_antigravity_oauth", { accountId })
}

export async function finishAntigravityOAuth(
  requestId: string,
  timeoutMs?: number,
): Promise<OAuthResult> {
  return invoke<OAuthResult>("finish_antigravity_oauth", { requestId, timeoutMs })
}

export async function cancelAntigravityOAuth(requestId: string): Promise<boolean> {
  return invoke<boolean>("cancel_antigravity_oauth", { requestId })
}

export async function startClaudeOAuth(accountId: string): Promise<OAuthStartResponse> {
  return invoke<OAuthStartResponse>("start_claude_oauth", { accountId })
}

export async function finishClaudeOAuth(
  requestId: string,
  timeoutMs?: number,
): Promise<OAuthResult> {
  return invoke<OAuthResult>("finish_claude_oauth", { requestId, timeoutMs })
}

export async function cancelClaudeOAuth(requestId: string): Promise<boolean> {
  return invoke<boolean>("cancel_claude_oauth", { requestId })
}

export async function startCopilotOAuth(accountId: string): Promise<OAuthStartResponse> {
  return invoke<OAuthStartResponse>("start_copilot_oauth", { accountId })
}

export async function finishCopilotOAuth(
  requestId: string,
  timeoutMs?: number,
): Promise<OAuthResult> {
  return invoke<OAuthResult>("finish_copilot_oauth", { requestId, timeoutMs })
}

export async function cancelCopilotOAuth(requestId: string): Promise<boolean> {
  return invoke<boolean>("cancel_copilot_oauth", { requestId })
}

export async function startOpencodeOAuth(accountId: string): Promise<OAuthStartResponse> {
  return invoke<OAuthStartResponse>("start_opencode_oauth", { accountId })
}

export async function finishOpencodeOAuth(
  requestId: string,
  timeoutMs?: number,
): Promise<OAuthResult> {
  return invoke<OAuthResult>("finish_opencode_oauth", { requestId, timeoutMs })
}

export async function cancelOpencodeOAuth(requestId: string): Promise<boolean> {
  return invoke<boolean>("cancel_opencode_oauth", { requestId })
}
