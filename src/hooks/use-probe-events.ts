import { useCallback, useEffect, useRef } from "react"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import type { PluginOutput } from "@/lib/plugin-types"

type ProbeResult = {
  batchId: string
  output: PluginOutput
}

type ProbeBatchComplete = {
  batchId: string
}

type ProbeBatchStarted = {
  batchId: string
  pluginIds: string[]
}

type UseProbeEventsOptions = {
  onResult: (output: PluginOutput) => void
  onBatchComplete: (missingPluginIds: string[]) => void
}

type ActiveBatch = {
  pluginIds: string[]
  resultPluginIds: Set<string>
}

export function useProbeEvents({ onResult, onBatchComplete }: UseProbeEventsOptions) {
  const activeBatches = useRef<Map<string, ActiveBatch>>(new Map())
  const unlisteners = useRef<UnlistenFn[]>([])
  const listenersReadyRef = useRef<Promise<void> | null>(null)
  const listenersReadyResolveRef = useRef<(() => void) | null>(null)

  useEffect(() => {
    let cancelled = false

    // Create the promise that will resolve when listeners are ready
    listenersReadyRef.current = new Promise<void>((resolve) => {
      listenersReadyResolveRef.current = resolve
    })

    const setup = async () => {
      const resultUnlisten = await listen<ProbeResult>("probe:result", (event) => {
        const batch = activeBatches.current.get(event.payload.batchId)
        if (batch) {
          batch.resultPluginIds.add(event.payload.output.providerId)
          onResult(event.payload.output)
        }
      })

      if (cancelled) {
        resultUnlisten()
        return
      }

      const completeUnlisten = await listen<ProbeBatchComplete>(
        "probe:batch-complete",
        (event) => {
          const batch = activeBatches.current.get(event.payload.batchId)
          if (batch) {
            activeBatches.current.delete(event.payload.batchId)
            onBatchComplete(
              batch.pluginIds.filter((pluginId) => !batch.resultPluginIds.has(pluginId))
            )
          }
        }
      )

      if (cancelled) {
        resultUnlisten()
        completeUnlisten()
        return
      }

      unlisteners.current.push(resultUnlisten, completeUnlisten)

      // Signal that listeners are ready
      listenersReadyResolveRef.current?.()
    }

    void setup()

    return () => {
      cancelled = true
      activeBatches.current.clear()
      unlisteners.current.forEach((unlisten) => unlisten())
      unlisteners.current = []
      listenersReadyRef.current = null
      listenersReadyResolveRef.current = null
    }
  }, [onBatchComplete, onResult])

  const startBatch = useCallback(async (pluginIds?: string[]) => {
    // Wait for listeners to be ready before starting the batch
    if (listenersReadyRef.current) {
      await listenersReadyRef.current
    }

    const batchId =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `batch-${Date.now()}-${Math.random().toString(16).slice(2)}`

    activeBatches.current.set(batchId, {
      pluginIds: pluginIds ? [...new Set(pluginIds)] : [],
      resultPluginIds: new Set(),
    })
    const args = pluginIds
      ? { batchId, pluginIds }
      : { batchId }
    try {
      const result = await invoke<ProbeBatchStarted>("start_probe_batch", args)
      const activeBatch = activeBatches.current.get(batchId)
      if (activeBatch) activeBatch.pluginIds = result.pluginIds
      return result.pluginIds
    } catch (error) {
      activeBatches.current.delete(batchId)
      throw error
    }
  }, [])

  return { startBatch }
}
