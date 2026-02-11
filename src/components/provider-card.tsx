import { useMemo } from "react"
import { RefreshCw } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { Separator } from "@/components/ui/separator"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SkeletonLines } from "@/components/skeleton-lines"
import { ProviderError } from "@/components/provider-error"
import { useNowTicker } from "@/hooks/use-now-ticker"
import { type DisplayMode } from "@/lib/settings"
import type { ManifestLine, MetricLine } from "@/lib/provider-types"
import { clamp01 } from "@/lib/utils"
import { calculatePaceStatus, type PaceStatus } from "@/lib/pace-status"
import { buildPaceDetailText, formatCompactDuration, getPaceStatusText } from "@/lib/pace-tooltip"
import { getBaseMetricLabel, splitAccountScopedLabel } from "@/lib/account-scoped-label"

interface ProviderCardProps {
  name: string
  plan?: string
  accountOrder?: string[]
  showSeparator?: boolean
  loading?: boolean
  error?: string | null
  lines?: MetricLine[]
  skeletonLines?: ManifestLine[]
  onRetry?: () => void
  scopeFilter?: "overview" | "all"
  displayMode: DisplayMode
}

export function formatNumber(value: number) {
  if (Number.isNaN(value)) return "0"
  const fractionDigits = Number.isInteger(value) ? 0 : 2
  return new Intl.NumberFormat("en-US", {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(value)
}

function formatCount(value: number) {
  if (!Number.isFinite(value)) return "0"
  const maximumFractionDigits = Number.isInteger(value) ? 0 : 2
  return new Intl.NumberFormat("en-US", { maximumFractionDigits }).format(value)
}

function formatPercentValue(value: number, displayMode: DisplayMode): string {
  if (!Number.isFinite(value)) return "0"
  const clamped = Math.max(0, Math.min(100, value))
  if (clamped === 0 || clamped === 100) {
    return `${clamped}`
  }

  const adjusted = displayMode === "left"
    ? Math.floor(clamped * 10) / 10
    : Math.ceil(clamped * 10) / 10

  const bounded = Math.max(0.1, Math.min(99.9, adjusted))
  return Number.isInteger(bounded) ? `${bounded}` : bounded.toFixed(1)
}

function formatResetIn(nowMs: number, resetsAtIso: string): string | null {
  const resetsAtMs = Date.parse(resetsAtIso)
  if (!Number.isFinite(resetsAtMs)) return null
  const deltaMs = resetsAtMs - nowMs
  if (deltaMs <= 0) return "Resets now"
  const durationText = formatCompactDuration(deltaMs)
  return durationText ? `Resets in ${durationText}` : "Resets in <1m"
}

type AccountLineGroup = {
  accountLabel: string
  accountId: string | null
  plan: string | null
  lines: MetricLine[]
}

function removeAccountPrefix(line: MetricLine): {
  accountLabel: string | null
  accountId: string | null
  line: MetricLine
} {
  const { accountLabel, accountId, metricLabel } = splitAccountScopedLabel(line.label)
  if (!accountLabel) {
    return { accountLabel: null, accountId: null, line }
  }

  if (line.type === "progress") {
    return {
      accountLabel,
      accountId,
      line: {
        ...line,
        label: metricLabel,
      },
    }
  }

  if (line.type === "text") {
    return {
      accountLabel,
      accountId,
      line: {
        ...line,
        label: metricLabel,
      },
    }
  }

  return {
    accountLabel,
    accountId,
    line: {
      ...line,
      label: metricLabel,
    },
  }
}

/** Colored dot indicator showing pace status */
function PaceIndicator({
  status,
  detailText,
  isLimitReached,
}: {
  status: PaceStatus
  detailText?: string | null
  isLimitReached?: boolean
}) {
  const colorClass =
    status === "ahead"
      ? "bg-green-500"
      : status === "on-track"
        ? "bg-yellow-500"
        : "bg-red-500"

  const statusText = getPaceStatusText(status)

  return (
    <Tooltip>
      <TooltipTrigger
        render={(props) => (
          <span
            {...props}
            className={`inline-block w-2 h-2 rounded-full ${colorClass}`}
            aria-label={isLimitReached ? "Limit reached" : statusText}
          />
        )}
      />
      <TooltipContent side="top" className="text-xs text-center">
        {isLimitReached ? (
          "Limit reached"
        ) : (
          <>
            <div>{statusText}</div>
            {detailText && <div className="text-[10px] opacity-60">{detailText}</div>}
          </>
        )}
      </TooltipContent>
    </Tooltip>
  )
}

export function ProviderCard({
  name,
  plan,
  accountOrder = [],
  showSeparator = true,
  loading = false,
  error = null,
  lines = [],
  skeletonLines = [],
  onRetry,
  scopeFilter = "all",
  displayMode,
}: ProviderCardProps) {
  // Filter lines based on scope - match by label since runtime lines can differ from manifest
  const overviewLabels = new Set(
    skeletonLines
      .filter(line => line.scope === "overview")
      .map(line => line.label)
  )
  const filteredSkeletonLines = scopeFilter === "all"
    ? skeletonLines
    : skeletonLines.filter(line => line.scope === "overview")
  const filteredLines = scopeFilter === "all"
    ? lines
    : lines.filter((line) => line.type !== "progress" || overviewLabels.has(getBaseMetricLabel(line.label)))
  const hasVisibleData = filteredLines.length > 0
  const showSkeleton = loading && !error && !hasVisibleData

  const groupedLines = useMemo(() => {
    const ungrouped: MetricLine[] = []
    const groups: AccountLineGroup[] = []
    const byAccount = new Map<string, AccountLineGroup>()

    for (const line of filteredLines) {
      const scoped = removeAccountPrefix(line)
      if (!scoped.accountLabel) {
        ungrouped.push(scoped.line)
        continue
      }

      const groupKey = `${scoped.accountLabel}::${scoped.accountId ?? ""}`
      let group = byAccount.get(groupKey)
      if (!group) {
        group = {
          accountLabel: scoped.accountLabel,
          accountId: scoped.accountId,
          plan: null,
          lines: [],
        }
        byAccount.set(groupKey, group)
        groups.push(group)
      }

      if (scoped.line.type === "badge" && scoped.line.label === "Plan") {
        group.plan = scoped.line.text
        continue
      }

      group.lines.push(scoped.line)
    }

    if (accountOrder.length > 0 && groups.length > 1) {
      const orderIndexById = new Map(accountOrder.map((accountId, index) => [accountId, index]))
      groups.sort((left, right) => {
        const leftOrder = left.accountId ? orderIndexById.get(left.accountId) : undefined
        const rightOrder = right.accountId ? orderIndexById.get(right.accountId) : undefined
        if (leftOrder === undefined && rightOrder === undefined) return 0
        if (leftOrder === undefined) return 1
        if (rightOrder === undefined) return -1
        return leftOrder - rightOrder
      })
    }

    return { ungrouped, groups }
  }, [accountOrder, filteredLines])

  const hasResetCountdown = filteredLines.some(
    (line) => line.type === "progress" && Boolean(line.resetsAt)
  )

  const now = useNowTicker({
    enabled: hasResetCountdown,
    intervalMs: 30_000,
    stopAfterMs: null,
  })

  return (
    <div>
      <div className="py-3">
        <div className="flex items-center justify-between mb-2">
          <div className="relative flex items-center">
            <h2 className="text-lg font-semibold" style={{ transform: "translateZ(0)" }}>{name}</h2>
            {onRetry && (
              loading ? (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="ml-1 pointer-events-none opacity-50"
                  style={{ transform: "translateZ(0)", backfaceVisibility: "hidden" }}
                  tabIndex={-1}
                >
                  <RefreshCw className="h-3 w-3 animate-spin" />
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label="Retry"
                  onClick={(e) => {
                    e.currentTarget.blur()
                    onRetry()
                  }}
                  className="ml-1 opacity-0 hover:opacity-100 focus-visible:opacity-100"
                  style={{ transform: "translateZ(0)", backfaceVisibility: "hidden" }}
                >
                  <RefreshCw className="h-3 w-3" />
                </Button>
              )
            )}
          </div>
          {plan && (
            <Badge
              variant="outline"
              className="truncate min-w-0 max-w-[40%]"
              title={plan}
            >
              {plan}
            </Badge>
          )}
        </div>
        {error && <ProviderError message={error} />}

        {showSkeleton && (
          <SkeletonLines lines={filteredSkeletonLines} />
        )}

        {!showSkeleton && !error && (
          <div className="space-y-4">
            {groupedLines.ungrouped.map((line, index) => (
              <MetricLineRenderer
                key={`plain-${line.label}-${index}`}
                line={line}
                displayMode={displayMode}
                now={now}
              />
            ))}

            {groupedLines.groups.map((group) => (
              <div key={`${group.accountLabel}:${group.accountId ?? ""}`} className="rounded-md border bg-muted/40 p-2">
                <div className="flex items-center justify-between gap-2 mb-2">
                  {group.accountId ? (
                    <Tooltip>
                      <TooltipTrigger
                        render={(props) => (
                          <p {...props} className="text-xs text-muted-foreground font-medium">
                            {group.accountLabel}
                          </p>
                        )}
                      />
                      <TooltipContent side="top" className="text-xs">
                        Account ID: {group.accountId}
                      </TooltipContent>
                    </Tooltip>
                  ) : (
                    <p className="text-xs text-muted-foreground font-medium">{group.accountLabel}</p>
                  )}
                  {group.plan && (
                    <Badge variant="outline" className="truncate min-w-0 max-w-[60%]" title={group.plan}>
                      {group.plan}
                    </Badge>
                  )}
                </div>
                <div className="space-y-4">
                  {group.lines.map((line, index) => (
                    <MetricLineRenderer
                      key={`${group.accountLabel}-${line.label}-${index}`}
                      line={line}
                      displayMode={displayMode}
                      now={now}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
      {showSeparator && <Separator />}
    </div>
  )
}

function MetricLineRenderer({
  line,
  displayMode,
  now,
}: {
  line: MetricLine
  displayMode: DisplayMode
  now: number
}) {
  if (line.type === "text") {
    return (
      <div>
        <div className="flex justify-between items-center h-[22px]">
          <span className="text-sm text-muted-foreground flex-shrink-0">{line.label}</span>
          <span
            className="text-sm text-muted-foreground truncate min-w-0 max-w-[60%] text-right"
            style={line.color ? { color: line.color } : undefined}
            title={line.value}
          >
            {line.value}
          </span>
        </div>
        {line.subtitle && (
          <div className="text-xs text-muted-foreground text-right -mt-0.5">{line.subtitle}</div>
        )}
      </div>
    )
  }

  if (line.type === "badge") {
    return (
      <div>
        <div className="flex justify-between items-center h-[22px]">
          <span className="text-sm text-muted-foreground flex-shrink-0">{line.label}</span>
          <Badge
            variant="outline"
            className="truncate min-w-0 max-w-[60%]"
            style={
              line.color
                ? { color: line.color, borderColor: line.color }
                : undefined
            }
            title={line.text}
          >
            {line.text}
          </Badge>
        </div>
        {line.subtitle && (
          <div className="text-xs text-muted-foreground text-right -mt-0.5">{line.subtitle}</div>
        )}
      </div>
    )
  }

  if (line.type === "progress") {
    const resetsAtMs = line.resetsAt ? Date.parse(line.resetsAt) : Number.NaN
    const hasPaceContext = Number.isFinite(resetsAtMs) && Number.isFinite(line.periodDurationMs)
    const shownAmount =
      displayMode === "used"
        ? line.used
        : Math.max(0, line.limit - line.used)
    const percent = Math.round(clamp01(shownAmount / line.limit) * 10000) / 100
    const leftSuffix = displayMode === "left" ? " left" : ""

    const primaryText =
      line.format.kind === "percent"
        ? `${formatPercentValue(shownAmount, displayMode)}%${leftSuffix}`
        : line.format.kind === "dollars"
          ? `$${formatNumber(shownAmount)}${leftSuffix}`
          : `${formatCount(shownAmount)} ${line.format.suffix}${leftSuffix}`

    const secondaryText =
      line.resetsAt
        ? formatResetIn(now, line.resetsAt)
        : line.format.kind === "percent"
          ? `${line.limit}% cap`
          : line.format.kind === "dollars"
            ? `$${formatNumber(line.limit)} limit`
            : `${formatCount(line.limit)} ${line.format.suffix}`

    // Calculate pace status if we have reset time and period duration
    const paceResult = hasPaceContext
      ? calculatePaceStatus(line.used, line.limit, resetsAtMs, line.periodDurationMs!, now)
      : null
    const paceStatus = paceResult?.status ?? null
    const isLimitReached = line.used >= line.limit
    const paceDetailText =
      hasPaceContext && !isLimitReached
        ? buildPaceDetailText({
            paceResult,
            used: line.used,
            limit: line.limit,
            periodDurationMs: line.periodDurationMs!,
            resetsAtMs,
            nowMs: now,
            displayMode,
          })
        : null

    return (
      <div>
        <div className="text-sm font-medium mb-1.5 flex items-center gap-1.5">
          {line.label}
          {paceStatus && (
            <PaceIndicator status={paceStatus} detailText={paceDetailText} isLimitReached={isLimitReached} />
          )}
        </div>
        <Progress
          value={percent}
          indicatorColor={line.color}
        />
        <div className="flex justify-between items-center mt-1.5">
          <span className="text-xs text-muted-foreground tabular-nums">
            {primaryText}
          </span>
          {secondaryText && (
            <span className="text-xs text-muted-foreground">
              {secondaryText}
            </span>
          )}
        </div>
      </div>
    )
  }

  return null
}
