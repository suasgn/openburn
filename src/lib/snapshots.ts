import { LazyStore } from "@tauri-apps/plugin-store"
import type { AccountRecord } from "@/lib/accounts"
import {
  ACCOUNT_LABEL_DELIMITER,
  ACCOUNT_META_DELIMITER,
  splitAccountScopedLabel,
} from "@/lib/account-scoped-label"
import type { MetricLine, ProviderMeta, ProviderOutput } from "@/lib/provider-types"

const SNAPSHOT_STORE_PATH = "snapshots.json"
const SNAPSHOT_SCHEMA_KEY = "schemaVersion"
const SNAPSHOT_DATA_KEY = "snapshots"
const SNAPSHOT_SCHEMA_VERSION = 1

const store = new LazyStore(SNAPSHOT_STORE_PATH)

export type AccountSnapshot = {
  accountId: string
  providerId: string
  displayName: string
  plan?: string
  lines: MetricLine[]
  iconUrl: string
  updatedAt: string
}

export type AccountSnapshotsById = Record<string, AccountSnapshot>

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

function normalizeSnapshotMap(raw: unknown): AccountSnapshotsById {
  if (!isRecord(raw)) return {}

  const normalized: AccountSnapshotsById = {}
  for (const [accountId, value] of Object.entries(raw)) {
    if (!isRecord(value)) continue

    const providerId = typeof value.providerId === "string"
      ? value.providerId.trim().toLowerCase()
      : ""
    if (!providerId) continue

    const snapshotAccountId = typeof value.accountId === "string"
      ? value.accountId.trim()
      : ""
    const resolvedAccountId = snapshotAccountId || accountId.trim()
    if (!resolvedAccountId) continue

    const lines = Array.isArray(value.lines) ? value.lines as MetricLine[] : []
    const displayName = typeof value.displayName === "string"
      ? value.displayName
      : providerId
    const iconUrl = typeof value.iconUrl === "string"
      ? value.iconUrl
      : "/vite.svg"
    const plan = typeof value.plan === "string" ? value.plan : undefined
    const updatedAt = typeof value.updatedAt === "string"
      ? value.updatedAt
      : new Date(0).toISOString()

    normalized[resolvedAccountId] = {
      accountId: resolvedAccountId,
      providerId,
      displayName,
      plan,
      lines,
      iconUrl,
      updatedAt,
    }
  }

  return normalized
}

export async function loadAccountSnapshots(): Promise<AccountSnapshotsById> {
  const schemaVersion = await store.get<number>(SNAPSHOT_SCHEMA_KEY)
  if (schemaVersion !== undefined && schemaVersion !== SNAPSHOT_SCHEMA_VERSION) {
    return {}
  }

  const raw = await store.get<unknown>(SNAPSHOT_DATA_KEY)
  return normalizeSnapshotMap(raw)
}

export async function saveAccountSnapshots(snapshots: AccountSnapshotsById): Promise<void> {
  await store.set(SNAPSHOT_SCHEMA_KEY, SNAPSHOT_SCHEMA_VERSION)
  await store.set(SNAPSHOT_DATA_KEY, snapshots)
  await store.save()
}

export function pruneSnapshotsForAccounts(
  snapshots: AccountSnapshotsById,
  accounts: AccountRecord[],
): { snapshots: AccountSnapshotsById; pruned: boolean } {
  const accountIds = new Set(accounts.map((account) => account.id))
  const next: AccountSnapshotsById = {}
  let pruned = false

  for (const [accountId, snapshot] of Object.entries(snapshots)) {
    if (!accountIds.has(accountId)) {
      pruned = true
      continue
    }
    next[accountId] = snapshot
  }

  return { snapshots: next, pruned }
}

function isErrorLine(line: MetricLine): boolean {
  return line.type === "badge" && line.label.trim() === "Error"
}

function prefixAccountLabel(accountLabel: string, accountId: string, metricLabel: string): string {
  return `${accountLabel.trim()}${ACCOUNT_META_DELIMITER}${accountId.trim()}${ACCOUNT_LABEL_DELIMITER}${metricLabel.trim()}`
}

function toUnscopedMetricLine(line: MetricLine): MetricLine {
  const metricLabel = splitAccountScopedLabel(line.label).metricLabel
  if (line.type === "progress") {
    return { ...line, label: metricLabel }
  }
  if (line.type === "text") {
    return { ...line, label: metricLabel }
  }
  return { ...line, label: metricLabel }
}

function toScopedMetricLine(line: MetricLine, accountLabel: string, accountId: string): MetricLine {
  const scopedLabel = prefixAccountLabel(accountLabel, accountId, line.label)
  if (line.type === "progress") {
    return { ...line, label: scopedLabel }
  }
  if (line.type === "text") {
    return { ...line, label: scopedLabel }
  }
  return { ...line, label: scopedLabel }
}

function normalizeAccountLabel(label: string, accountId: string): string {
  const trimmed = label.trim()
  if (trimmed.length > 0) return trimmed
  const shortId = accountId.slice(0, 8)
  return shortId ? `Account ${shortId}` : "Account"
}

export function extractAccountSnapshotsFromProviderOutput(args: {
  output: ProviderOutput
  accounts: AccountRecord[]
}): AccountSnapshotsById {
  const { output, accounts } = args
  const providerId = output.providerId.trim().toLowerCase()
  if (!providerId) return {}

  const providerAccounts = accounts.filter((account) => account.providerId === providerId)
  if (providerAccounts.length === 0) return {}

  const accountById = new Map(providerAccounts.map((account) => [account.id, account]))
  const groupedByAccount = new Map<string, { plan?: string; lines: MetricLine[] }>()

  for (const line of output.lines) {
    const { accountId, metricLabel } = splitAccountScopedLabel(line.label)
    if (!accountId) continue
    if (!accountById.has(accountId)) continue

    let group = groupedByAccount.get(accountId)
    if (!group) {
      group = { lines: [] }
      groupedByAccount.set(accountId, group)
    }

    if (line.type === "badge" && metricLabel === "Plan") {
      if (line.text.trim()) {
        group.plan = line.text
      }
      continue
    }

    if (line.type === "badge" && metricLabel === "Error") {
      continue
    }

    group.lines.push(toUnscopedMetricLine(line))
  }

  const updatedAt = new Date().toISOString()

  if (groupedByAccount.size > 0) {
    const out: AccountSnapshotsById = {}
    for (const [accountId, group] of groupedByAccount.entries()) {
      if (group.lines.length === 0 && !group.plan) continue
      out[accountId] = {
        accountId,
        providerId,
        displayName: output.displayName,
        plan: group.plan,
        lines: group.lines,
        iconUrl: output.iconUrl,
        updatedAt,
      }
    }
    return out
  }

  if (providerAccounts.length !== 1) {
    return {}
  }

  const account = providerAccounts[0]
  const lines = output.lines.filter((line) => !isErrorLine(line))
  const plan = output.plan?.trim()
  if (lines.length === 0 && !plan) {
    return {}
  }

  return {
    [account.id]: {
      accountId: account.id,
      providerId,
      displayName: output.displayName,
      plan: plan || undefined,
      lines,
      iconUrl: output.iconUrl,
      updatedAt,
    },
  }
}

export function buildProviderOutputsFromSnapshots(args: {
  providersMeta: ProviderMeta[]
  accounts: AccountRecord[]
  snapshotsByAccountId: AccountSnapshotsById
}): Record<string, ProviderOutput> {
  const { providersMeta, accounts, snapshotsByAccountId } = args

  const metaById = new Map(providersMeta.map((provider) => [provider.id, provider]))
  const accountsByProvider = accounts.reduce<Record<string, AccountRecord[]>>((acc, account) => {
    if (!acc[account.providerId]) {
      acc[account.providerId] = []
    }
    acc[account.providerId].push(account)
    return acc
  }, {})

  for (const providerAccounts of Object.values(accountsByProvider)) {
    providerAccounts.sort((left, right) => {
      const leftLabel = left.label.toLowerCase()
      const rightLabel = right.label.toLowerCase()
      if (leftLabel !== rightLabel) {
        return leftLabel.localeCompare(rightLabel)
      }
      return left.id.localeCompare(right.id)
    })
  }

  const outputs: Record<string, ProviderOutput> = {}

  for (const [providerId, providerAccounts] of Object.entries(accountsByProvider)) {
    const meta = metaById.get(providerId)
    if (!meta) continue

    const accountEntries = providerAccounts
      .map((account) => ({ account, snapshot: snapshotsByAccountId[account.id] }))
      .filter(
        (entry): entry is { account: AccountRecord; snapshot: AccountSnapshot } =>
          Boolean(entry.snapshot) && entry.snapshot.providerId === providerId,
      )

    if (accountEntries.length === 0) continue

    if (providerAccounts.length === 1 && accountEntries.length === 1) {
      const snapshot = accountEntries[0].snapshot
      outputs[providerId] = {
        providerId,
        displayName: snapshot.displayName || meta.name,
        plan: snapshot.plan,
        lines: snapshot.lines,
        iconUrl: snapshot.iconUrl || meta.iconUrl,
      }
      continue
    }

    const lines: MetricLine[] = []
    for (const { account, snapshot } of accountEntries) {
      const accountLabel = normalizeAccountLabel(account.label, account.id)
      if (snapshot.plan && snapshot.plan.trim()) {
        lines.push({
          type: "badge",
          label: prefixAccountLabel(accountLabel, account.id, "Plan"),
          text: snapshot.plan,
          color: undefined,
          subtitle: undefined,
        })
      }
      for (const line of snapshot.lines) {
        lines.push(toScopedMetricLine(line, accountLabel, account.id))
      }
    }

    if (lines.length === 0) continue

    outputs[providerId] = {
      providerId,
      displayName: meta.name,
      lines,
      iconUrl: meta.iconUrl,
    }
  }

  return outputs
}
