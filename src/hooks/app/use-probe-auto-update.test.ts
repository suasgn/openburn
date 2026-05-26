import { act, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const { getEnabledPluginIdsMock } = vi.hoisted(() => ({
  getEnabledPluginIdsMock: vi.fn(),
}))

vi.mock("@/lib/settings", () => ({
  getEnabledPluginIds: getEnabledPluginIdsMock,
}))

import { useProbeAutoUpdate } from "@/hooks/app/use-probe-auto-update"

describe("useProbeAutoUpdate", () => {
  beforeEach(() => {
    getEnabledPluginIdsMock.mockReset()
    getEnabledPluginIdsMock.mockImplementation((settings: { order: string[] }) => settings.order)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("keeps auto-update cleared when plugin settings are missing", () => {
    const { result } = renderHook(() =>
      useProbeAutoUpdate({
        pluginSettings: null,
        autoUpdateInterval: 15,
        setLoadingForPlugins: vi.fn(),
        setErrorForPlugins: vi.fn(),
        isPluginLoading: vi.fn(),
        startBatch: vi.fn(),
      })
    )

    act(() => {
      result.current.resetAutoUpdateSchedule()
    })

    expect(result.current.autoUpdateNextAt).toBeNull()
  })

  it("resets the schedule when enabled plugins are present", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(10_000)

    const { result } = renderHook(() =>
      useProbeAutoUpdate({
        pluginSettings: { order: ["codex"], disabled: [] },
        autoUpdateInterval: 15,
        setLoadingForPlugins: vi.fn(),
        setErrorForPlugins: vi.fn(),
        isPluginLoading: vi.fn(),
        startBatch: vi.fn(),
      })
    )

    act(() => {
      result.current.resetAutoUpdateSchedule()
    })

    expect(result.current.autoUpdateNextAt).toBe(910_000)
    nowSpy.mockRestore()
  })

  it("skips plugins that are already loading", () => {
    vi.useFakeTimers()
    const setLoadingForPlugins = vi.fn()
    const startBatch = vi.fn().mockResolvedValue(["codex"])

    renderHook(() =>
      useProbeAutoUpdate({
        pluginSettings: { order: ["claude", "codex"], disabled: [] },
        autoUpdateInterval: 5,
        setLoadingForPlugins,
        setErrorForPlugins: vi.fn(),
        isPluginLoading: (id: string) => id === "claude",
        startBatch,
      })
    )

    act(() => {
      vi.advanceTimersByTime(300_000)
    })

    expect(setLoadingForPlugins).toHaveBeenCalledWith(["codex"])
    expect(startBatch).toHaveBeenCalledWith(["codex"])

  })
})
