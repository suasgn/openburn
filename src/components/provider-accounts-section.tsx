import { useEffect, useMemo, useRef, useState } from "react"
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core"
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { AlertCircle, CheckCircle2, Copy, Plus, RefreshCw, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

type ProviderConfig = {
  id: string
  name: string
  enabled: boolean
}

export type ProviderAuthStrategyOption = {
  id: string
  label: string
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

export type AccountOAuthSession = {
  status: "pending" | "error"
  requestId?: string
  url?: string
  userCode?: string | null
  message?: string
}

type ToastState =
  | { kind: "success"; text: string }
  | { kind: "error"; text: string }
  | null

interface ProviderAccountsSectionProps {
  providers: ProviderConfig[]
  accountsByProvider: AccountsByProvider
  providerAuthStrategiesByProvider: Record<string, ProviderAuthStrategyOption[]>
  loading: boolean
  onReorderAccounts: (providerId: string, orderedAccountIds: string[]) => void
  onReloadAccounts: () => Promise<void>
  onToggleProvider: (providerId: string) => void
  onCreateAccount: (providerId: string, authStrategyId: string) => Promise<void>
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
  oauthSessionByAccount: Record<string, AccountOAuthSession | undefined>
  onStartAccountOAuth: (providerId: string, accountId: string) => Promise<void>
  onCancelAccountOAuth: (providerId: string, accountId: string) => Promise<void>
}

function supportsNativeOAuth(providerId: string): boolean {
  return providerId === "codex" || providerId === "claude" || providerId === "copilot"
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

type ProviderFetchStatus = {
  dotClass: string
  label: string
  detail: string
}

function resolveAccountFetchStatus(account: ProviderAccountSummary): ProviderFetchStatus {
  if (account.lastError && account.lastError.trim().length > 0) {
    return {
      dotClass: "bg-red-500",
      label: "Fetch error",
      detail: account.lastError,
    }
  }

  if (account.lastFetchAt) {
    const formatted = timestampLabel(account.lastFetchAt)
    return {
      dotClass: "bg-emerald-500",
      label: "Fetched",
      detail: formatted ? `Last fetch: ${formatted}` : "Fetched successfully",
    }
  }

  if (account.hasCredentials) {
    return {
      dotClass: "bg-amber-500",
      label: "Waiting",
      detail: "Credentials set, waiting for first fetch",
    }
  }

  return {
    dotClass: "bg-zinc-400",
    label: "Missing credentials",
    detail: "Credentials are not configured yet",
  }
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

type SortableAccountCardRenderProps = {
  dragAttributes: Record<string, unknown>
  dragListeners: Record<string, unknown>
}

function SortableAccountCard({
  accountId,
  className,
  children,
}: {
  accountId: string
  className: string
  children: (props: SortableAccountCardRenderProps) => React.ReactNode
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: accountId,
  })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  }

  return (
    <div ref={setNodeRef} style={style} className={cn(className, isDragging && "opacity-70")}> 
      {children({
        dragAttributes: attributes as unknown as Record<string, unknown>,
        dragListeners: listeners as unknown as Record<string, unknown>,
      })}
    </div>
  )
}

export function ProviderAccountsSection({
  providers,
  accountsByProvider,
  providerAuthStrategiesByProvider,
  loading,
  onReorderAccounts,
  onReloadAccounts,
  onToggleProvider,
  onCreateAccount,
  onUpdateAccountLabel,
  onDeleteAccount,
  onSaveAccountCredentials,
  onClearAccountCredentials,
  oauthSessionByAccount,
  onStartAccountOAuth,
  onCancelAccountOAuth,
}: ProviderAccountsSectionProps) {
  const [toast, setToast] = useState<ToastState>(null)
  const [activeAction, setActiveAction] = useState<string | null>(null)
  const [createPickerProviderId, setCreatePickerProviderId] = useState<string | null>(null)
  const [labelDraftByAccount, setLabelDraftByAccount] = useState<Record<string, string>>({})
  const [credentialsDraftByAccount, setCredentialsDraftByAccount] = useState<
    Record<string, string>
  >({})
  const [zaiApiKeyDraftByAccount, setZaiApiKeyDraftByAccount] = useState<Record<string, string>>(
    {},
  )
  const [zaiRegionDraftByAccount, setZaiRegionDraftByAccount] = useState<Record<string, string>>(
    {},
  )
  const labelSaveTimerByAccountRef = useRef<Record<string, number>>({})
  const toastTimerRef = useRef<number | null>(null)

  const accountReorderSensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 8 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

  useEffect(() => {
    return () => {
      for (const timerId of Object.values(labelSaveTimerByAccountRef.current)) {
        window.clearTimeout(timerId)
      }
      if (toastTimerRef.current !== null) {
        window.clearTimeout(toastTimerRef.current)
      }
    }
  }, [])

  const providersWithAccounts = useMemo(
    () =>
      providers.map((provider) => ({
        provider,
        accounts: accountsByProvider[provider.id] ?? [],
      })),
    [accountsByProvider, providers],
  )

  const showToast = (kind: "success" | "error", text: string) => {
    if (toastTimerRef.current !== null) {
      window.clearTimeout(toastTimerRef.current)
    }
    setToast({ kind, text })
    toastTimerRef.current = window.setTimeout(() => {
      setToast(null)
      toastTimerRef.current = null
    }, 3200)
  }

  const runAction = async (actionId: string, task: () => Promise<void>) => {
    setActiveAction(actionId)
    try {
      await task()
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : String(error))
    } finally {
      setActiveAction(null)
    }
  }

  const handleAccountDragEnd = (
    providerId: string,
    orderedAccountIds: string[],
    event: DragEndEvent,
  ) => {
    const { active, over } = event
    if (!over || active.id === over.id) return

    const sourceIndex = orderedAccountIds.indexOf(String(active.id))
    const targetIndex = orderedAccountIds.indexOf(String(over.id))
    if (sourceIndex < 0 || targetIndex < 0) return

    const nextIds = arrayMove(orderedAccountIds, sourceIndex, targetIndex)
    onReorderAccounts(providerId, nextIds)
  }

  return (
    <section>
      <div className="flex items-start justify-between gap-2">
        <div>
          <h3 className="text-lg font-semibold mb-0">Accounts</h3>
          <p className="text-sm text-muted-foreground mb-2">
            Manage accounts and credentials.
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

      <div className="space-y-2">
        {providersWithAccounts.map(({ provider, accounts }) => {
          const createActionId = `create:${provider.id}`
          const orderedAccountIds = accounts.map((account) => account.id)
          const availableAuthStrategies = providerAuthStrategiesByProvider[provider.id] ?? []
          const createPickerOpen = createPickerProviderId === provider.id

          return (
            <div key={provider.id} className="rounded-lg border bg-muted/50 p-2 space-y-2">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <p className="text-sm font-medium leading-none">{provider.name}</p>
                </div>
                <div className="flex items-center gap-1">
                  <label className="inline-flex items-center gap-1.5 text-xs text-muted-foreground select-none pr-1">
                    <Checkbox
                      key={`${provider.id}-${provider.enabled}`}
                      checked={provider.enabled}
                      disabled={loading || activeAction !== null}
                      onCheckedChange={() => onToggleProvider(provider.id)}
                    />
                    Enabled
                  </label>
                  <Button
                    type="button"
                    size="xs"
                    disabled={loading || activeAction !== null}
                    onClick={() => {
                      setCreatePickerProviderId((previous) =>
                        previous === provider.id ? null : provider.id,
                      )
                    }}
                  >
                    <Plus className="size-3" />
                    Add
                  </Button>
                </div>
              </div>

              {createPickerOpen && (
                <div className="rounded-md border border-dashed bg-background/70 p-2 space-y-2">
                  <p className="text-xs text-muted-foreground">Choose auth for the new account.</p>
                  {availableAuthStrategies.length === 0 ? (
                    <p className="text-xs text-destructive">No auth strategies are available.</p>
                  ) : (
                    <div className="flex flex-wrap items-center gap-1">
                      {availableAuthStrategies.map((strategy) => (
                        <Button
                          key={strategy.id}
                          type="button"
                          size="xs"
                          variant="outline"
                          disabled={loading || activeAction !== null}
                          onClick={() =>
                            runAction(`${createActionId}:${strategy.id}`, async () => {
                              await onCreateAccount(provider.id, strategy.id)
                              setCreatePickerProviderId(null)
                              showToast(
                                "success",
                                `${provider.name} account created with ${strategy.label}`,
                              )
                            })
                          }
                        >
                          {strategy.label}
                        </Button>
                      ))}
                    </div>
                  )}
                  <div className="flex items-center justify-end">
                    <Button
                      type="button"
                      size="xs"
                      variant="ghost"
                      disabled={loading || activeAction !== null}
                      onClick={() => setCreatePickerProviderId(null)}
                    >
                      Cancel
                    </Button>
                  </div>
                </div>
              )}

              {accounts.length === 0 ? (
                <p className="text-xs text-muted-foreground px-1 py-1">
                  No account configured yet.
                </p>
              ) : (
                <DndContext
                  sensors={accountReorderSensors}
                  collisionDetection={closestCenter}
                  onDragEnd={(event) => handleAccountDragEnd(provider.id, orderedAccountIds, event)}
                >
                  <SortableContext items={orderedAccountIds} strategy={verticalListSortingStrategy}>
                    <div className="space-y-2">
                      {accounts.map((account) => {
                    const labelValue =
                      labelDraftByAccount[account.id] === undefined
                        ? account.label
                        : labelDraftByAccount[account.id]
                    const clearCredentialsActionId = `clear-creds:${account.id}`
                    const startOauthActionId = `oauth:start:${account.id}`
                    const cancelOauthActionId = `oauth:cancel:${account.id}`
                    const copyOauthUrlActionId = `oauth:copy:${account.id}`
                    const deleteActionId = `delete:${account.id}`
                    const oauthSession = oauthSessionByAccount[account.id]
                    const canOAuth = supportsNativeOAuth(provider.id)
                    const supportsManualCredentials = !canOAuth
                    const supportsZaiCredentialForm = provider.id === "zai"
                    const supportsJsonCredentials = supportsManualCredentials && !supportsZaiCredentialForm
                    const oauthPending = oauthSession?.status === "pending"
                    const oauthError = oauthSession?.status === "error"
                    const accountFetchStatus = resolveAccountFetchStatus(account)
                    const zaiApiKeyValue = zaiApiKeyDraftByAccount[account.id] ?? ""
                    const zaiRegionValue = zaiRegionDraftByAccount[account.id] ?? "global"

                    return (
                      <SortableAccountCard
                        key={account.id}
                        accountId={account.id}
                        className="rounded-md border bg-card p-2 space-y-2"
                      >
                        {({ dragAttributes, dragListeners }) => {
                          const dragHandleProps = {
                            ...dragAttributes,
                            ...dragListeners,
                          } as unknown as Record<string, unknown>

                          return (
                        <>
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0">
                            <div className="flex items-center gap-1.5">
                              <Tooltip>
                                <TooltipTrigger
                                  render={(props) => (
                                    <p
                                      {...props}
                                      className="text-sm font-medium truncate cursor-grab active:cursor-grabbing"
                                      {...(dragHandleProps as Record<string, unknown>)}
                                    >
                                      {account.label}
                                    </p>
                                  )}
                                />
                                <TooltipContent side="top" className="text-xs">
                                  Account ID: {account.id}
                                </TooltipContent>
                              </Tooltip>
                              <Tooltip>
                                <TooltipTrigger
                                  render={(props) => (
                                    <span
                                      {...props}
                                      className={`inline-block size-2 rounded-full ${accountFetchStatus.dotClass}`}
                                      aria-label={accountFetchStatus.label}
                                    />
                                  )}
                                />
                                <TooltipContent side="top" className="text-xs">
                                  <div className="font-medium">{accountFetchStatus.label}</div>
                                  <div className="opacity-80">{accountFetchStatus.detail}</div>
                                </TooltipContent>
                              </Tooltip>
                            </div>
                            <p className="text-xs text-muted-foreground mt-0.5">
                              {authLabel(account.authStrategyId)}
                            </p>
                          </div>
                          <div className="flex items-center gap-1">
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-xs"
                              disabled={loading || activeAction !== null}
                              onClick={() =>
                                runAction(deleteActionId, async () => {
                                  await onDeleteAccount(provider.id, account.id)
                                  showToast("success", `${provider.name} account removed`)
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

                              const existingTimer = labelSaveTimerByAccountRef.current[account.id]
                              if (existingTimer) {
                                window.clearTimeout(existingTimer)
                                delete labelSaveTimerByAccountRef.current[account.id]
                              }

                              const trimmed = value.trim()
                              if (!trimmed || trimmed === account.label) {
                                return
                              }

                              labelSaveTimerByAccountRef.current[account.id] = window.setTimeout(() => {
                                delete labelSaveTimerByAccountRef.current[account.id]
                                void onUpdateAccountLabel(provider.id, account.id, trimmed).catch((error) => {
                                  showToast(
                                    "error",
                                    error instanceof Error ? error.message : String(error),
                                  )
                                })
                              }, 450)
                            }}
                            onBlur={() => {
                              const value = (labelDraftByAccount[account.id] ?? account.label).trim()
                              if (!value) {
                                setLabelDraftByAccount((previous) => ({
                                  ...previous,
                                  [account.id]: account.label,
                                }))
                              }
                            }}
                            className="h-8 flex-1 rounded-md border border-input bg-background px-2 text-xs"
                            placeholder="Account label"
                          />
                        </div>

                        {supportsZaiCredentialForm && (
                          <div className="space-y-2">
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                              <div className="space-y-1">
                                <label className="block text-xs text-muted-foreground">API key</label>
                                <input
                                  type="password"
                                  value={zaiApiKeyValue}
                                  onChange={(event) => {
                                    const value = event.target.value
                                    setZaiApiKeyDraftByAccount((previous) => ({
                                      ...previous,
                                      [account.id]: value,
                                    }))
                                  }}
                                  className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs"
                                  placeholder="Enter Z.ai API key"
                                  autoComplete="off"
                                />
                              </div>
                              <div className="space-y-1">
                                <label className="block text-xs text-muted-foreground">Region</label>
                                <Tabs
                                  value={zaiRegionValue}
                                  onValueChange={(value) => {
                                    setZaiRegionDraftByAccount((previous) => ({
                                      ...previous,
                                      [account.id]: value,
                                    }))
                                  }}
                                >
                                  <TabsList className="h-8 w-full">
                                    <TabsTrigger
                                      value="global"
                                      className="text-xs"
                                      disabled={loading || activeAction !== null}
                                    >
                                      Global
                                    </TabsTrigger>
                                    <TabsTrigger
                                      value="cn"
                                      className="text-xs"
                                      disabled={loading || activeAction !== null}
                                    >
                                      China
                                    </TabsTrigger>
                                  </TabsList>
                                </Tabs>
                              </div>
                            </div>
                            <p className="text-[11px] text-muted-foreground">
                              Region selects endpoint: Global uses api.z.ai, China uses open.bigmodel.cn.
                            </p>
                          </div>
                        )}

                        {supportsJsonCredentials && (
                          <textarea
                            value={credentialsDraftByAccount[account.id] ?? ""}
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
                        )}

                        <div className="flex items-center gap-1">
                          {canOAuth && (
                            <>
                              <Button
                                type="button"
                                size="xs"
                                variant={oauthError ? "destructive" : "outline"}
                                disabled={loading || activeAction !== null || oauthPending}
                                onClick={() =>
                                  runAction(startOauthActionId, async () => {
                                    await onStartAccountOAuth(provider.id, account.id)
                                    showToast(
                                      "success",
                                      oauthError
                                        ? `${provider.name} OAuth restarted`
                                        : `${provider.name} OAuth started`,
                                    )
                                  })
                                }
                              >
                                {oauthError ? "Retry OAuth" : "Connect OAuth"}
                              </Button>
                              {oauthPending && (
                                <Button
                                  type="button"
                                  size="xs"
                                  variant="outline"
                                  disabled={loading || activeAction !== null}
                                  onClick={() =>
                                    runAction(cancelOauthActionId, async () => {
                                      await onCancelAccountOAuth(provider.id, account.id)
                                      showToast("success", `${provider.name} OAuth cancelled`)
                                    })
                                  }
                                >
                                  Cancel OAuth
                                </Button>
                              )}
                            </>
                          )}
                          {supportsZaiCredentialForm && (
                            <Button
                              type="button"
                              size="xs"
                              disabled={loading || activeAction !== null}
                              onClick={() =>
                                runAction(`save-zai-creds:${account.id}`, async () => {
                                  const apiKey = (zaiApiKeyDraftByAccount[account.id] ?? "").trim()
                                  if (!apiKey) {
                                    throw new Error("API key is required")
                                  }
                                  const region = zaiRegionDraftByAccount[account.id] ?? "global"
                                  const credentials: Record<string, unknown> = {
                                    type: "apiKey",
                                    apiKey,
                                  }
                                  if (region === "cn") {
                                    credentials.apiRegion = "cn"
                                  }
                                  await onSaveAccountCredentials(provider.id, account.id, credentials)
                                  setZaiApiKeyDraftByAccount((previous) => ({
                                    ...previous,
                                    [account.id]: "",
                                  }))
                                  showToast("success", `${provider.name} credentials saved`)
                                })
                              }
                            >
                              Save credentials
                            </Button>
                          )}
                          {supportsJsonCredentials && (
                            <Button
                              type="button"
                              size="xs"
                              disabled={loading || activeAction !== null}
                              onClick={() =>
                                runAction(`save-creds:${account.id}`, async () => {
                                  const parsed = parseCredentialsInput(
                                    credentialsDraftByAccount[account.id] ?? "",
                                  )
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
                                  showToast("success", `${provider.name} credentials saved`)
                                })
                              }
                            >
                              Save credentials
                            </Button>
                          )}
                          {account.hasCredentials && (
                            <Button
                              type="button"
                              size="xs"
                              variant="outline"
                              disabled={loading || activeAction !== null}
                              onClick={() =>
                                runAction(clearCredentialsActionId, async () => {
                                  await onClearAccountCredentials(provider.id, account.id)
                                  showToast("success", `${provider.name} credentials cleared`)
                                })
                              }
                            >
                              Clear credentials
                            </Button>
                          )}
                        </div>

                        {canOAuth && oauthSession?.url && (
                          <div className="rounded-md border border-dashed p-2 text-xs">
                            <p className="font-medium text-foreground">
                              {oauthPending ? "OAuth in progress" : "OAuth status"}
                            </p>
                            {oauthSession.userCode && (
                              <p className="text-muted-foreground mt-1">
                                Enter code: <span className="font-mono">{oauthSession.userCode}</span>
                              </p>
                            )}
                            <p className="text-muted-foreground mt-1 break-all">{oauthSession.url}</p>
                            <div className="mt-1 flex items-center gap-1">
                              <Button
                                type="button"
                                size="xs"
                                variant="outline"
                                disabled={loading || activeAction !== null}
                                onClick={() =>
                                  runAction(copyOauthUrlActionId, async () => {
                                    if (!oauthSession.url) {
                                      throw new Error("OAuth URL is not available")
                                    }
                                    if (!navigator.clipboard?.writeText) {
                                      throw new Error("Clipboard is unavailable")
                                    }
                                    await navigator.clipboard.writeText(oauthSession.url)
                                    showToast("success", "OAuth URL copied")
                                  })
                                }
                              >
                                <Copy className="size-3" />
                                Copy URL
                              </Button>
                            </div>
                            {oauthError && oauthSession.message && (
                              <p className="text-destructive mt-1 break-words">{oauthSession.message}</p>
                            )}
                          </div>
                        )}
                        </>
                          )
                        }}
                      </SortableAccountCard>
                    )
                  })}
                    </div>
                  </SortableContext>
                </DndContext>
              )}
            </div>
          )
        })}
      </div>

      {toast && (
        <div className="pointer-events-none fixed right-8 bottom-14 z-50 flex justify-end">
          <div
            role={toast.kind === "error" ? "alert" : "status"}
            aria-live="polite"
            className={cn(
              "pointer-events-auto flex max-w-[20rem] items-start gap-2 rounded-md border px-3 py-2 text-xs shadow-xl backdrop-blur-sm",
              toast.kind === "error"
                ? "border-rose-300/70 bg-rose-600/95 text-white"
                : "border-emerald-300/70 bg-emerald-600/95 text-white",
            )}
          >
            {toast.kind === "error" ? (
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
            ) : (
              <CheckCircle2 className="mt-0.5 size-3.5 shrink-0" />
            )}
            <p className="leading-5">{toast.text}</p>
          </div>
        </div>
      )}
    </section>
  )
}
