import type { ProviderMeta, ProviderOutput } from "@/lib/provider-types"
import type { ProviderSettings } from "@/lib/settings"
import { DEFAULT_DISPLAY_MODE, type DisplayMode } from "@/lib/settings"
import { clamp01 } from "@/lib/utils"
import { getBaseMetricLabel } from "@/lib/account-scoped-label"

type ProviderState = {
  data: ProviderOutput | null
  loading: boolean
  error: string | null
}

export type TrayPrimaryBar = {
  id: string
  fraction?: number
}

type ProgressLine = Extract<
  ProviderOutput["lines"][number],
  { type: "progress"; label: string; used: number; limit: number }
>

function isProgressLine(line: ProviderOutput["lines"][number]): line is ProgressLine {
  return line.type === "progress"
}

export function getTrayPrimaryBars(args: {
  providersMeta: ProviderMeta[]
  providerSettings: ProviderSettings | null
  providerStates: Record<string, ProviderState | undefined>
  maxBars?: number
  displayMode?: DisplayMode
}): TrayPrimaryBar[] {
  const { providersMeta, providerSettings, providerStates, maxBars = 4, displayMode = DEFAULT_DISPLAY_MODE } = args
  if (!providerSettings) return []

  const metaById = new Map(providersMeta.map((p) => [p.id, p]))
  const disabled = new Set(providerSettings.disabled)

  const out: TrayPrimaryBar[] = []
  for (const id of providerSettings.order) {
    if (disabled.has(id)) continue
    const meta = metaById.get(id)
    if (!meta) continue
    
    // Skip if no primary candidates defined
    if (!meta.primaryCandidates || meta.primaryCandidates.length === 0) continue

    const state = providerStates[id]
    const data = state?.data ?? null

    let fraction: number | undefined
    if (data) {
      // Find first candidate that exists in runtime data
      const primaryLabel = meta.primaryCandidates.find((label) =>
        data.lines.some((line) => isProgressLine(line) && getBaseMetricLabel(line.label) === label)
      )
      if (primaryLabel) {
        const primaryLine = data.lines.find(
          (line): line is ProgressLine =>
            isProgressLine(line) && getBaseMetricLabel(line.label) === primaryLabel
        )
        if (primaryLine && primaryLine.limit > 0) {
          const shownAmount =
            displayMode === "used"
              ? primaryLine.used
              : primaryLine.limit - primaryLine.used
          fraction = clamp01(shownAmount / primaryLine.limit)
        }
      }
    }

    out.push({ id, fraction })
    if (out.length >= maxBars) break
  }

  return out
}
