import { useMemo, useState } from "react"
import { AlertCircle, Plus, RefreshCw, Save, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Alert, AlertDescription } from "@/components/ui/alert"

type ProviderConfig = {
  id: string
  name: string
  enabled: boolean
}

export type ProviderAccountSummary = {
  id: string
  providerId: string
  authStrategyId?: string | null
  label: string
  hasCredentials: boolean
  lastFetchAt?: string | null
  lastError?: string | null
}

type AccountsByProvider = Record<string, ProviderAccountSummary[]>

type NoticeState =
  | { kind: "success"; text: string }
  | { kind: "error"; text: string }
  | null

interface ProviderAccountsSectionProps {
  providers: ProviderConfig[]
  accountsByProvider: AccountsByProvider
  defaultAuthStrategyByProvider: Record<string, string>
  loading: boolean
  onReloadAccounts: () => Promise<void>
  onCreateAccount: (providerId: string) => Promise<void>
  onUpdateAccountLabel: (
    providerId: string,
    accountId: string,
    label: string,
  ) => Promise<void>
  onDeleteAccount: (providerId: string, accountId: string) => Promise<void>
  onSaveAccountCredentials: (
    providerId: string,
    accountId: string,
    credentials: Record<string, unknown>,
  ) => Promise<void>
  onClearAccountCredentials: (providerId: string, accountId: string) => Promise<void>
}

function authLabel(value?: string | null): string {
  if (!value) return "Default"
  if (value === "oauth") return "OAuth"
  if (value === "apiKey") return "API Key"
  return value
}

function timestampLabel(value?: string | null): string | null {
  if (!value) return null
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return null
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date)
}

function credentialTemplate(providerId: string): string {
  if (providerId === "codex") {
    return `{
  "type": "oauth",
  "access_token": "",
  "refresh_token": "",
  "expires_at": 0,
  "account_id": ""
}`
  }

  if (providerId === "claude") {
    return `{
  "type": "oauth",
  "access_token": "",
  "refresh_token": "",
  "expires_at": 0,
  "subscriptionType": ""
}`
  }

  if (providerId === "copilot") {
    return `{
  "type": "oauth",
  "access_token": ""
}`
  }

  if (providerId === "zai") {
    return `{
  "type": "apiKey",
  "apiKey": "",
  "apiHost": ""
}`
  }

  return `{
  "type": "",
  "token": ""
}`
}

function parseCredentialsInput(
  text: string,
): { value: Record<string, unknown> | null; error: string | null } {
  const trimmed = text.trim()
  if (!trimmed) {
    return { value: null, error: "Credential JSON is required" }
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return { value: null, error: "Credentials must be a JSON object" }
    }
    return { value: parsed as Record<string, unknown>, error: null }
  } catch {
    return { value: null, error: "Credential JSON is invalid" }
  }
}

export function ProviderAccountsSection({
  providers,
  accountsByProvider,
  defaultAuthStrategyByProvider,
  loading,
  onReloadAccounts,
  onCreateAccount,
  onUpdateAccountLabel,
  onDeleteAccount,
  onSaveAccountCredentials,
  onClearAccountCredentials,
}: ProviderAccountsSectionProps) {
  const [notice, setNotice] = useState<NoticeState>(null)
  const [activeAction, setActiveAction] = useState<string | null>(null)
  const [labelDraftByAccount, setLabelDraftByAccount] = useState<Record<string, string>>({})
  const [credentialsDraftByAccount, setCredentialsDraftByAccount] = useState<
    Record<string, string>
  >({})

  const providersWithAccounts = useMemo(
    () =>
      providers.map((provider) => ({
        provider,
        accounts: accountsByProvider[provider.id] ?? [],
      })),
    [accountsByProvider, providers],
  )

  const runAction = async (actionId: string, task: () => Promise<void>) => {
    setActiveAction(actionId)
    setNotice(null)
    try {
      await task()
    } catch (error) {
      setNotice({
        kind: "error",
        text: error instanceof Error ? error.message : String(error),
      })
    } finally {
      setActiveAction(null)
    }
  }

  return (
    <section>
      <div className="flex items-start justify-between gap-2">
        <div>
          <h3 className="text-lg font-semibold mb-0">Accounts</h3>
          <p className="text-sm text-muted-foreground mb-2">
            Add provider accounts and save credentials for probing
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="xs"
          disabled={loading || activeAction !== null}
          onClick={() => runAction("reload-accounts", onReloadAccounts)}
        >
          <RefreshCw className="size-3" />
          Reload
        </Button>
      </div>

      {notice && (
        <Alert
          variant={notice.kind === "error" ? "destructive" : "default"}
          className="mb-2 flex items-center gap-2 [&>svg]:static [&>svg]:translate-y-0 [&>svg~*]:pl-0 [&>svg+div]:translate-y-0"
        >
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>{notice.text}</AlertDescription>
        </Alert>
      )}

      <div className="space-y-2">
        {providersWithAccounts.map(({ provider, accounts }) => {
          const defaultStrategy =
            defaultAuthStrategyByProvider[provider.id] || "(unknown)"
          const createActionId = `create:${provider.id}`

          return (
            <div key={provider.id} className="rounded-lg border bg-muted/50 p-2 space-y-2">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <p className="text-sm font-medium leading-none">{provider.name}</p>
                  <p className="text-xs text-muted-foreground mt-1">
                    Default auth: {authLabel(defaultStrategy)}
                  </p>
                </div>
                <div className="flex items-center gap-1">
                  <Badge variant="outline">{provider.enabled ? "Enabled" : "Disabled"}</Badge>
                  <Button
                    type="button"
                    size="xs"
                    disabled={loading || activeAction !== null}
                    onClick={() =>
                      runAction(createActionId, async () => {
                        await onCreateAccount(provider.id)
                        setNotice({
                          kind: "success",
                          text: `${provider.name} account created`,
                        })
                      })
                    }
                  >
                    <Plus className="size-3" />
                    Add
                  </Button>
                </div>
              </div>

              {accounts.length === 0 ? (
                <p className="text-xs text-muted-foreground px-1 py-1">
                  No account configured yet.
                </p>
              ) : (
                <div className="space-y-2">
                  {accounts.map((account) => {
                    const labelValue =
                      labelDraftByAccount[account.id] === undefined
                        ? account.label
                        : labelDraftByAccount[account.id]
                    const credentialsValue = credentialsDraftByAccount[account.id] ?? ""

                    const saveLabelActionId = `save-label:${account.id}`
                    const saveCredentialsActionId = `save-creds:${account.id}`
                    const clearCredentialsActionId = `clear-creds:${account.id}`
                    const deleteActionId = `delete:${account.id}`

                    return (
                      <div key={account.id} className="rounded-md border bg-card p-2 space-y-2">
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0">
                            <p className="text-sm font-medium truncate">{account.label}</p>
                            <p className="text-xs text-muted-foreground mt-0.5">
                              {authLabel(account.authStrategyId)}
                            </p>
                          </div>
                          <div className="flex items-center gap-1">
                            <Badge
                              variant={account.hasCredentials ? "secondary" : "outline"}
                              className="whitespace-nowrap"
                            >
                              {account.hasCredentials ? "Credentials set" : "Missing credentials"}
                            </Badge>
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-xs"
                              disabled={loading || activeAction !== null}
                              onClick={() =>
                                runAction(deleteActionId, async () => {
                                  await onDeleteAccount(provider.id, account.id)
                                  setNotice({
                                    kind: "success",
                                    text: `${provider.name} account removed`,
                                  })
                                })
                              }
                            >
                              <Trash2 className="size-3" />
                            </Button>
                          </div>
                        </div>

                        <div className="flex gap-1">
                          <input
                            value={labelValue}
                            onChange={(event) => {
                              const value = event.target.value
                              setLabelDraftByAccount((previous) => ({
                                ...previous,
                                [account.id]: value,
                              }))
                            }}
                            className="h-8 flex-1 rounded-md border border-input bg-background px-2 text-xs"
                            placeholder="Account label"
                          />
                          <Button
                            type="button"
                            size="xs"
                            variant="outline"
                            disabled={loading || activeAction !== null}
                            onClick={() =>
                              runAction(saveLabelActionId, async () => {
                                const nextLabel = labelValue.trim()
                                if (!nextLabel) {
                                  throw new Error("Label cannot be empty")
                                }
                                await onUpdateAccountLabel(
                                  provider.id,
                                  account.id,
                                  nextLabel,
                                )
                                setNotice({
                                  kind: "success",
                                  text: `Updated label for ${provider.name}`,
                                })
                              })
                            }
                          >
                            <Save className="size-3" />
                            Save
                          </Button>
                        </div>

                        <textarea
                          value={credentialsValue}
                          onChange={(event) => {
                            const value = event.target.value
                            setCredentialsDraftByAccount((previous) => ({
                              ...previous,
                              [account.id]: value,
                            }))
                          }}
                          className="min-h-28 w-full rounded-md border border-input bg-background px-2 py-2 text-xs font-mono"
                          placeholder={credentialTemplate(provider.id)}
                        />

                        <div className="flex items-center gap-1">
                          <Button
                            type="button"
                            size="xs"
                            disabled={loading || activeAction !== null}
                            onClick={() =>
                              runAction(saveCredentialsActionId, async () => {
                                const parsed = parseCredentialsInput(credentialsValue)
                                if (parsed.error || !parsed.value) {
                                  throw new Error(parsed.error || "Invalid credentials")
                                }
                                await onSaveAccountCredentials(
                                  provider.id,
                                  account.id,
                                  parsed.value,
                                )
                                setCredentialsDraftByAccount((previous) => ({
                                  ...previous,
                                  [account.id]: "",
                                }))
                                setNotice({
                                  kind: "success",
                                  text: `${provider.name} credentials saved`,
                                })
                              })
                            }
                          >
                            Save credentials
                          </Button>
                          <Button
                            type="button"
                            size="xs"
                            variant="outline"
                            disabled={loading || activeAction !== null}
                            onClick={() =>
                              runAction(clearCredentialsActionId, async () => {
                                await onClearAccountCredentials(provider.id, account.id)
                                setNotice({
                                  kind: "success",
                                  text: `${provider.name} credentials cleared`,
                                })
                              })
                            }
                          >
                            Clear credentials
                          </Button>
                        </div>

                        <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
                          <span className="truncate">ID: {account.id}</span>
                          <span>
                            {timestampLabel(account.lastFetchAt)
                              ? `Last fetch: ${timestampLabel(account.lastFetchAt)}`
                              : "Never fetched"}
                          </span>
                        </div>

                        {account.lastError && (
                          <p className="text-xs text-destructive break-words">
                            Last error: {account.lastError}
                          </p>
                        )}
                      </div>
                    )
                  })}
                </div>
              )}
            </div>
          )
        })}
      </div>
    </section>
  )
}
