import { act, renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const {
  trackMock,
  saveDisplayModeMock,
  saveMenubarIconStyleMock,
  saveResetTimerDisplayModeMock,
  saveThemeModeMock,
  saveTimeFormatModeMock,
} = vi.hoisted(() => ({
  trackMock: vi.fn(),
  saveThemeModeMock: vi.fn(),
  saveDisplayModeMock: vi.fn(),
  saveMenubarIconStyleMock: vi.fn(),
  saveResetTimerDisplayModeMock: vi.fn(),
  saveTimeFormatModeMock: vi.fn(),
}))

vi.mock("@/lib/analytics", () => ({
  track: trackMock,
}))

vi.mock("@/lib/settings", () => ({
  saveThemeMode: saveThemeModeMock,
  saveDisplayMode: saveDisplayModeMock,
  saveMenubarIconStyle: saveMenubarIconStyleMock,
  saveResetTimerDisplayMode: saveResetTimerDisplayModeMock,
  saveTimeFormatMode: saveTimeFormatModeMock,
}))

import { useSettingsDisplayActions } from "@/hooks/app/use-settings-display-actions"

describe("useSettingsDisplayActions", () => {
  beforeEach(() => {
    trackMock.mockReset()
    saveThemeModeMock.mockReset()
    saveDisplayModeMock.mockReset()
    saveMenubarIconStyleMock.mockReset()
    saveResetTimerDisplayModeMock.mockReset()
    saveTimeFormatModeMock.mockReset()
    saveThemeModeMock.mockResolvedValue(undefined)
    saveDisplayModeMock.mockResolvedValue(undefined)
    saveMenubarIconStyleMock.mockResolvedValue(undefined)
    saveResetTimerDisplayModeMock.mockResolvedValue(undefined)
    saveTimeFormatModeMock.mockResolvedValue(undefined)
  })

  it("tracks and applies display-related setting changes", () => {
    const setThemeMode = vi.fn()
    const setDisplayMode = vi.fn()
    const setResetTimerDisplayMode = vi.fn()
    const setTimeFormatMode = vi.fn()
    const setMenubarIconStyle = vi.fn()
    const scheduleTrayIconUpdate = vi.fn()

    const { result } = renderHook(() =>
      useSettingsDisplayActions({
        setThemeMode,
        setDisplayMode,
        resetTimerDisplayMode: "relative",
        setResetTimerDisplayMode,
        setTimeFormatMode,
        setMenubarIconStyle,
        scheduleTrayIconUpdate,
      })
    )

    act(() => {
      result.current.handleThemeModeChange("dark")
      result.current.handleDisplayModeChange("used")
      result.current.handleResetTimerDisplayModeChange("absolute")
      result.current.handleTimeFormatModeChange("24h")
      result.current.handleMenubarIconStyleChange("bars")
    })

    expect(trackMock).toHaveBeenCalledWith("setting_changed", { setting: "theme", value: "dark" })
    expect(trackMock).toHaveBeenCalledWith("setting_changed", {
      setting: "display_mode",
      value: "used",
    })
    expect(trackMock).toHaveBeenCalledWith("setting_changed", {
      setting: "reset_timer_display_mode",
      value: "absolute",
    })
    expect(trackMock).toHaveBeenCalledWith("setting_changed", {
      setting: "time_format_mode",
      value: "24h",
    })
    expect(trackMock).toHaveBeenCalledWith("setting_changed", {
      setting: "menubar_icon_style",
      value: "bars",
    })

    expect(setThemeMode).toHaveBeenCalledWith("dark")
    expect(setDisplayMode).toHaveBeenCalledWith("used")
    expect(setResetTimerDisplayMode).toHaveBeenCalledWith("absolute")
    expect(setTimeFormatMode).toHaveBeenCalledWith("24h")
    expect(setMenubarIconStyle).toHaveBeenCalledWith("bars")
    expect(scheduleTrayIconUpdate).toHaveBeenCalledWith("settings", 0)

    expect(saveThemeModeMock).toHaveBeenCalledWith("dark")
    expect(saveDisplayModeMock).toHaveBeenCalledWith("used")
    expect(saveResetTimerDisplayModeMock).toHaveBeenCalledWith("absolute")
    expect(saveTimeFormatModeMock).toHaveBeenCalledWith("24h")
    expect(saveMenubarIconStyleMock).toHaveBeenCalledWith("bars")
  })

  it("toggles reset timer mode in both directions", () => {
    const setResetTimerDisplayMode = vi.fn()

    const { result, rerender } = renderHook(
      ({ mode }: { mode: "relative" | "absolute" }) =>
        useSettingsDisplayActions({
          setThemeMode: vi.fn(),
          setDisplayMode: vi.fn(),
          resetTimerDisplayMode: mode,
          setResetTimerDisplayMode,
          setTimeFormatMode: vi.fn(),
          setMenubarIconStyle: vi.fn(),
          scheduleTrayIconUpdate: vi.fn(),
        }),
      { initialProps: { mode: "relative" as "relative" | "absolute" } }
    )

    act(() => {
      result.current.handleResetTimerDisplayModeToggle()
    })
    expect(setResetTimerDisplayMode).toHaveBeenCalledWith("absolute")

    rerender({ mode: "absolute" })
    act(() => {
      result.current.handleResetTimerDisplayModeToggle()
    })
    expect(setResetTimerDisplayMode).toHaveBeenCalledWith("relative")
  })

  it("logs persistence failures", async () => {
    const themeError = new Error("theme failed")
    const displayError = new Error("display failed")
    const resetError = new Error("reset failed")
    const timeFormatError = new Error("time format failed")
    const menubarError = new Error("menubar failed")
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {})
    saveThemeModeMock.mockRejectedValueOnce(themeError)
    saveDisplayModeMock.mockRejectedValueOnce(displayError)
    saveResetTimerDisplayModeMock.mockRejectedValueOnce(resetError)
    saveTimeFormatModeMock.mockRejectedValueOnce(timeFormatError)
    saveMenubarIconStyleMock.mockRejectedValueOnce(menubarError)

    const { result } = renderHook(() =>
      useSettingsDisplayActions({
        setThemeMode: vi.fn(),
        setDisplayMode: vi.fn(),
        resetTimerDisplayMode: "relative",
        setResetTimerDisplayMode: vi.fn(),
        setTimeFormatMode: vi.fn(),
        setMenubarIconStyle: vi.fn(),
        scheduleTrayIconUpdate: vi.fn(),
      })
    )

    act(() => {
      result.current.handleThemeModeChange("light")
      result.current.handleDisplayModeChange("left")
      result.current.handleResetTimerDisplayModeChange("relative")
      result.current.handleTimeFormatModeChange("12h")
      result.current.handleMenubarIconStyleChange("donut")
    })

    await waitFor(() => {
      expect(errorSpy).toHaveBeenCalledWith("Failed to save theme mode:", themeError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save display mode:", displayError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save reset timer display mode:", resetError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save time format mode:", timeFormatError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save menubar icon style:", menubarError)
    })

    errorSpy.mockRestore()
  })
})
