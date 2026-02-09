import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { getCurrentWindow, PhysicalSize, currentMonitor } from "@tauri-apps/api/window"
import { getVersion } from "@tauri-apps/api/app"
import { resolveResource } from "@tauri-apps/api/path"
import { TrayIcon } from "@tauri-apps/api/tray"
import { SideNav, type ActiveView } from "@/components/side-nav"
import { PanelFooter } from "@/components/panel-footer"
import { OverviewPage } from "@/pages/overview"
import { ProviderDetailPage } from "@/pages/provider-detail"
import { SettingsPage } from "@/pages/settings"
import type { ProviderMeta, ProviderOutput } from "@/lib/provider-types"
import { track } from "@/lib/analytics"
import { getTrayIconSizePx, renderTrayBarsIcon } from "@/lib/tray-bars-icon"
import { getTrayPrimaryBars } from "@/lib/tray-primary-progress"
import { useProbeEvents } from "@/hooks/use-probe-events"
import { useAppUpdate } from "@/hooks/use-app-update"
import {
  areProviderSettingsEqual,
  DEFAULT_AUTO_UPDATE_INTERVAL,
  DEFAULT_DISPLAY_MODE,
  DEFAULT_TRAY_ICON_STYLE,
  DEFAULT_TRAY_SHOW_PERCENTAGE,
  DEFAULT_THEME_MODE,
  getEnabledProviderIds,
  isTrayPercentageMandatory,
  loadAutoUpdateInterval,
  loadDisplayMode,
  loadProviderSettings,
  loadTrayShowPercentage,
  loadTrayIconStyle,
  loadThemeMode,
  normalizeProviderSettings,
  saveAutoUpdateInterval,
  saveDisplayMode,
  saveProviderSettings,
  saveTrayShowPercentage,
  saveTrayIconStyle,
  saveThemeMode,
  type AutoUpdateIntervalMinutes,
  type DisplayMode,
  type ProviderSettings,
  type TrayIconStyle,
  type ThemeMode,
} from "@/lib/settings"

const PANEL_WIDTH = 400;
const MAX_HEIGHT_FALLBACK_PX = 600;
const MAX_HEIGHT_FRACTION_OF_MONITOR = 0.8;
const ARROW_OVERHEAD_PX = 37; // .tray-arrow (7px) + wrapper pt-1.5 (6px) + bottom p-6 (24px)
const TRAY_SETTINGS_DEBOUNCE_MS = 2000;
const TRAY_PROBE_DEBOUNCE_MS = 500;

type ProviderState = {
  data: ProviderOutput | null
  loading: boolean
  error: string | null
  lastManualRefreshAt: number | null
}

function App() {
  const [activeView, setActiveView] = useState<ActiveView>("home");
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollDown, setCanScrollDown] = useState(false);
  const [providerStates, setProviderStates] = useState<Record<string, ProviderState>>({})
  const [providersMeta, setProvidersMeta] = useState<ProviderMeta[]>([])
  const [providerSettings, setProviderSettings] = useState<ProviderSettings | null>(null)
  const [autoUpdateInterval, setAutoUpdateInterval] = useState<AutoUpdateIntervalMinutes>(
    DEFAULT_AUTO_UPDATE_INTERVAL
  )
  const [autoUpdateNextAt, setAutoUpdateNextAt] = useState<number | null>(null)
  const [autoUpdateResetToken, setAutoUpdateResetToken] = useState(0)
  const [themeMode, setThemeMode] = useState<ThemeMode>(DEFAULT_THEME_MODE)
  const [displayMode, setDisplayMode] = useState<DisplayMode>(DEFAULT_DISPLAY_MODE)
  const [trayIconStyle, setTrayIconStyle] = useState<TrayIconStyle>(DEFAULT_TRAY_ICON_STYLE)
  const [trayShowPercentage, setTrayShowPercentage] = useState(DEFAULT_TRAY_SHOW_PERCENTAGE)
  const [maxPanelHeightPx, setMaxPanelHeightPx] = useState<number | null>(null)
  const maxPanelHeightPxRef = useRef<number | null>(null)
  const [appVersion, setAppVersion] = useState("...")

  const { updateStatus, triggerInstall } = useAppUpdate()
  const [showAbout, setShowAbout] = useState(false)

  const trayRef = useRef<TrayIcon | null>(null)
  const trayGaugeIconPathRef = useRef<string | null>(null)
  const trayUpdateTimerRef = useRef<number | null>(null)
  const trayUpdatePendingRef = useRef(false)
  const [trayReady, setTrayReady] = useState(false)

  // Store state in refs so scheduleTrayIconUpdate can read current values without recreating the callback
  const providersMetaRef = useRef(providersMeta)
  const providerSettingsRef = useRef(providerSettings)
  const providerStatesRef = useRef(providerStates)
  const displayModeRef = useRef(displayMode)
  const trayIconStyleRef = useRef(trayIconStyle)
  const trayShowPercentageRef = useRef(trayShowPercentage)
  useEffect(() => { providersMetaRef.current = providersMeta }, [providersMeta])
  useEffect(() => { providerSettingsRef.current = providerSettings }, [providerSettings])
  useEffect(() => { providerStatesRef.current = providerStates }, [providerStates])
  useEffect(() => { displayModeRef.current = displayMode }, [displayMode])
  useEffect(() => { trayIconStyleRef.current = trayIconStyle }, [trayIconStyle])
  useEffect(() => { trayShowPercentageRef.current = trayShowPercentage }, [trayShowPercentage])

  // Fetch app version on mount
  useEffect(() => {
    getVersion().then(setAppVersion)
  }, [])

  // Stable callback that reads from refs - never recreated, so debounce works correctly
  const scheduleTrayIconUpdate = useCallback((_reason: "probe" | "settings" | "init", delayMs = 0) => {
    if (trayUpdateTimerRef.current !== null) {
      window.clearTimeout(trayUpdateTimerRef.current)
      trayUpdateTimerRef.current = null
    }

    trayUpdateTimerRef.current = window.setTimeout(() => {
      trayUpdateTimerRef.current = null
      if (trayUpdatePendingRef.current) return
      trayUpdatePendingRef.current = true

      const tray = trayRef.current
      if (!tray) {
        trayUpdatePendingRef.current = false
        return
      }

      const style = trayIconStyleRef.current
      const maxBars = style === "bars" ? 4 : 1
      const bars = getTrayPrimaryBars({
        providersMeta: providersMetaRef.current,
        providerSettings: providerSettingsRef.current,
        providerStates: providerStatesRef.current,
        maxBars,
        displayMode: displayModeRef.current,
      })

      // 0 bars: revert to the packaged gauge tray icon.
      if (bars.length === 0) {
        const gaugePath = trayGaugeIconPathRef.current
        if (gaugePath) {
          Promise.all([
            tray.setIcon(gaugePath),
            tray.setIconAsTemplate(true),
          ])
            .catch((e) => {
              console.error("Failed to restore tray gauge icon:", e)
            })
            .finally(() => {
              trayUpdatePendingRef.current = false
            })
        } else {
          trayUpdatePendingRef.current = false
        }
        return
      }

      const percentageMandatory = isTrayPercentageMandatory(style)

      let percentText: string | undefined
      if (percentageMandatory || trayShowPercentageRef.current) {
        const firstFraction = bars[0]?.fraction
        if (typeof firstFraction === "number" && Number.isFinite(firstFraction)) {
          const clamped = Math.max(0, Math.min(1, firstFraction))
          const rounded = Math.round(clamped * 100)
          percentText = `${rounded}%`
        }
      }

      if (style === "textOnly" && !percentText) {
        const gaugePath = trayGaugeIconPathRef.current
        if (gaugePath) {
          Promise.all([
            tray.setIcon(gaugePath),
            tray.setIconAsTemplate(true),
          ])
            .catch((e) => {
              console.error("Failed to restore tray gauge icon:", e)
            })
            .finally(() => {
              trayUpdatePendingRef.current = false
            })
        } else {
          trayUpdatePendingRef.current = false
        }
        return
      }

      const sizePx = getTrayIconSizePx(window.devicePixelRatio)
      const firstProviderId = bars[0]?.id
      const providerIconUrl =
        style === "provider"
          ? providersMetaRef.current.find((provider) => provider.id === firstProviderId)?.iconUrl
          : undefined

      renderTrayBarsIcon({ bars, sizePx, style, percentText, providerIconUrl })
        .then(async (img) => {
          await tray.setIcon(img)
          await tray.setIconAsTemplate(true)
        })
        .catch((e) => {
          console.error("Failed to update tray icon:", e)
        })
        .finally(() => {
          trayUpdatePendingRef.current = false
        })
    }, delayMs)
  }, [])

  // Initialize tray handle once (separate from tray updates)
  const trayInitializedRef = useRef(false)
  useEffect(() => {
    if (trayInitializedRef.current) return
    let cancelled = false
    ;(async () => {
      try {
        const tray = await TrayIcon.getById("tray")
        if (cancelled) return
        trayRef.current = tray
        trayInitializedRef.current = true
        setTrayReady(true)
        try {
          trayGaugeIconPathRef.current = await resolveResource("icons/tray-icon.png")
        } catch (e) {
          console.error("Failed to resolve tray gauge icon resource:", e)
        }
      } catch (e) {
        console.error("Failed to load tray icon handle:", e)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  // Trigger tray update once tray + provider metadata/settings are available.
  // This prevents missing the first paint if probe results arrive before the tray handle resolves.
  useEffect(() => {
    if (!trayReady) return
    if (!providerSettings) return
    if (providersMeta.length === 0) return
    scheduleTrayIconUpdate("init", 0)
  }, [providersMeta.length, providerSettings, scheduleTrayIconUpdate, trayReady])


  const displayProviders = useMemo(() => {
    if (!providerSettings) return []
    const disabledSet = new Set(providerSettings.disabled)
    const metaById = new Map(providersMeta.map((provider) => [provider.id, provider]))
    return providerSettings.order
      .filter((id) => !disabledSet.has(id))
      .map((id) => {
        const meta = metaById.get(id)
        if (!meta) return null
        const state = providerStates[id] ?? { data: null, loading: false, error: null, lastManualRefreshAt: null }
        return { meta, ...state }
      })
      .filter((provider): provider is { meta: ProviderMeta } & ProviderState => Boolean(provider))
  }, [providerSettings, providerStates, providersMeta])

  // Derive enabled provider list for nav icons
  const navProviders = useMemo(() => {
    if (!providerSettings) return []
    const disabledSet = new Set(providerSettings.disabled)
    const metaById = new Map(providersMeta.map((p) => [p.id, p]))
    return providerSettings.order
      .filter((id) => !disabledSet.has(id))
      .map((id) => metaById.get(id))
      .filter((p): p is ProviderMeta => Boolean(p))
      .map((p) => ({ id: p.id, name: p.name, iconUrl: p.iconUrl, brandColor: p.brandColor }))
  }, [providerSettings, providersMeta])

  // Track page views
  useEffect(() => {
    const page =
      activeView === "home" ? "overview"
        : activeView === "settings" ? "settings"
          : "provider_detail"
    const props: Record<string, string> =
      activeView !== "home" && activeView !== "settings"
        ? { page, provider_id: activeView }
        : { page }
    track("page_viewed", props)
  }, [activeView])

  // If active view is a provider that got disabled, switch to home
  useEffect(() => {
    if (activeView === "home" || activeView === "settings") return
    const isStillEnabled = navProviders.some((p) => p.id === activeView)
    if (!isStillEnabled) {
      setActiveView("home")
    }
  }, [activeView, navProviders])

  // Get the selected provider for detail view
  const selectedProvider = useMemo(() => {
    if (activeView === "home" || activeView === "settings") return null
    return displayProviders.find((p) => p.meta.id === activeView) ?? null
  }, [activeView, displayProviders])


  // Initialize panel on mount
  useEffect(() => {
    invoke("init_panel").catch(console.error);
  }, []);

  // Hide panel on Escape key (unless about dialog is open - it handles its own Escape)
  useEffect(() => {
    if (!isTauri()) return
    if (showAbout) return // Let dialog handle its own Escape

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        invoke("hide_panel")
      }
    }
    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [showAbout])

  // Listen for tray menu events
  useEffect(() => {
    if (!isTauri()) return
    let cancelled = false
    const unlisteners: (() => void)[] = []

    async function setup() {
      const u1 = await listen<string>("tray:navigate", (event) => {
        setActiveView(event.payload as ActiveView)
      })
      if (cancelled) { u1(); return }
      unlisteners.push(u1)

      const u2 = await listen("tray:show-about", () => {
        setShowAbout(true)
      })
      if (cancelled) { u2(); return }
      unlisteners.push(u2)
    }
    void setup()

    return () => {
      cancelled = true
      for (const fn of unlisteners) fn()
    }
  }, [])

  // Auto-resize window to fit content using ResizeObserver
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const resizeWindow = async () => {
      const factor = window.devicePixelRatio;

      const width = Math.ceil(PANEL_WIDTH * factor);
      const desiredHeightLogical = Math.max(1, container.scrollHeight);

      let maxHeightPhysical: number | null = null;
      let maxHeightLogical: number | null = null;
      try {
        const monitor = await currentMonitor();
        if (monitor) {
          maxHeightPhysical = Math.floor(monitor.size.height * MAX_HEIGHT_FRACTION_OF_MONITOR);
          maxHeightLogical = Math.floor(maxHeightPhysical / factor);
        }
      } catch {
        // fall through to fallback
      }

      if (maxHeightLogical === null) {
        const screenAvailHeight = Number(window.screen?.availHeight) || MAX_HEIGHT_FALLBACK_PX;
        maxHeightLogical = Math.floor(screenAvailHeight * MAX_HEIGHT_FRACTION_OF_MONITOR);
        maxHeightPhysical = Math.floor(maxHeightLogical * factor);
      }

      if (maxPanelHeightPxRef.current !== maxHeightLogical) {
        maxPanelHeightPxRef.current = maxHeightLogical;
        setMaxPanelHeightPx(maxHeightLogical);
      }

      const desiredHeightPhysical = Math.ceil(desiredHeightLogical * factor);
      const height = Math.ceil(Math.min(desiredHeightPhysical, maxHeightPhysical!));

      try {
        const currentWindow = getCurrentWindow();
        await currentWindow.setSize(new PhysicalSize(width, height));
      } catch (e) {
        console.error("Failed to resize window:", e);
      }
    };

    // Initial resize
    resizeWindow();

    // Observe size changes
    const observer = new ResizeObserver(() => {
      resizeWindow();
    });
    observer.observe(container);

    return () => observer.disconnect();
  }, [activeView, displayProviders]);

  const getErrorMessage = useCallback((output: ProviderOutput) => {
    if (output.lines.length !== 1) return null
    const line = output.lines[0]
    if (line.type === "badge" && line.label === "Error") {
      return line.text || "Couldn't update data. Try again?"
    }
    return null
  }, [])

  const setLoadingForProviders = useCallback((ids: string[]) => {
    setProviderStates((prev) => {
      const next = { ...prev }
      for (const id of ids) {
        const existing = prev[id]
        next[id] = { data: null, loading: true, error: null, lastManualRefreshAt: existing?.lastManualRefreshAt ?? null }
      }
      return next
    })
  }, [])

  const setErrorForProviders = useCallback((ids: string[], error: string) => {
    setProviderStates((prev) => {
      const next = { ...prev }
      for (const id of ids) {
        const existing = prev[id]
        next[id] = { data: null, loading: false, error, lastManualRefreshAt: existing?.lastManualRefreshAt ?? null }
      }
      return next
    })
  }, [])

  // Track which provider IDs are being manually refreshed (vs initial load / enable toggle)
  const manualRefreshIdsRef = useRef<Set<string>>(new Set())

  const handleProbeResult = useCallback(
    (output: ProviderOutput) => {
      const errorMessage = getErrorMessage(output)
      if (errorMessage) {
        track("provider_fetch_error", {
          provider_id: output.providerId,
          error: errorMessage.slice(0, 200),
        })
      }
      const isManual = manualRefreshIdsRef.current.has(output.providerId)
      if (isManual) {
        manualRefreshIdsRef.current.delete(output.providerId)
      }
      setProviderStates((prev) => ({
        ...prev,
        [output.providerId]: {
          data: errorMessage ? null : output,
          loading: false,
          error: errorMessage,
          // Only set cooldown timestamp for successful manual refreshes
          lastManualRefreshAt: (!errorMessage && isManual)
            ? Date.now()
            : (prev[output.providerId]?.lastManualRefreshAt ?? null),
        },
      }))

      // Regenerate tray icon on every probe result (debounced to avoid churn).
      scheduleTrayIconUpdate("probe", TRAY_PROBE_DEBOUNCE_MS)
    },
    [getErrorMessage, scheduleTrayIconUpdate]
  )

  const handleBatchComplete = useCallback(() => {}, [])

  const { startBatch } = useProbeEvents({
    onResult: handleProbeResult,
    onBatchComplete: handleBatchComplete,
  })

  useEffect(() => {
    let isMounted = true

    const loadSettings = async () => {
      try {
        const availableProviders = await invoke<ProviderMeta[]>("list_providers_meta")
        if (!isMounted) return
        setProvidersMeta(availableProviders)

        const storedSettings = await loadProviderSettings()
        const normalized = normalizeProviderSettings(
          storedSettings,
          availableProviders
        )

        if (!areProviderSettingsEqual(storedSettings, normalized)) {
          await saveProviderSettings(normalized)
        }

        let storedInterval = DEFAULT_AUTO_UPDATE_INTERVAL
        try {
          storedInterval = await loadAutoUpdateInterval()
        } catch (error) {
          console.error("Failed to load auto-update interval:", error)
        }

        let storedThemeMode = DEFAULT_THEME_MODE
        try {
          storedThemeMode = await loadThemeMode()
        } catch (error) {
          console.error("Failed to load theme mode:", error)
        }

        let storedDisplayMode = DEFAULT_DISPLAY_MODE
        try {
          storedDisplayMode = await loadDisplayMode()
        } catch (error) {
          console.error("Failed to load display mode:", error)
        }

        let storedTrayIconStyle = DEFAULT_TRAY_ICON_STYLE
        try {
          storedTrayIconStyle = await loadTrayIconStyle()
        } catch (error) {
          console.error("Failed to load tray icon style:", error)
        }

        let storedTrayShowPercentage = DEFAULT_TRAY_SHOW_PERCENTAGE
        try {
          storedTrayShowPercentage = await loadTrayShowPercentage()
        } catch (error) {
          console.error("Failed to load tray show percentage:", error)
        }

        const normalizedTrayShowPercentage = isTrayPercentageMandatory(storedTrayIconStyle)
          ? true
          : storedTrayShowPercentage

        if (isMounted) {
          setProviderSettings(normalized)
          setAutoUpdateInterval(storedInterval)
          setThemeMode(storedThemeMode)
          setDisplayMode(storedDisplayMode)
          setTrayIconStyle(storedTrayIconStyle)
          setTrayShowPercentage(normalizedTrayShowPercentage)
          const enabledIds = getEnabledProviderIds(normalized)
          setLoadingForProviders(enabledIds)
          try {
            await startBatch(enabledIds)
          } catch (error) {
            console.error("Failed to start probe batch:", error)
            if (isMounted) {
              setErrorForProviders(enabledIds, "Failed to start probe")
            }
          }
        }

        if (
          isTrayPercentageMandatory(storedTrayIconStyle) &&
          storedTrayShowPercentage !== true
        ) {
          void saveTrayShowPercentage(true).catch((error) => {
            console.error("Failed to save tray show percentage:", error)
          })
        }
      } catch (e) {
        console.error("Failed to load provider settings:", e)
      }
    }

    loadSettings()

    return () => {
      isMounted = false
    }
  }, [setLoadingForProviders, setErrorForProviders, startBatch])

  useEffect(() => {
    if (!providerSettings) {
      setAutoUpdateNextAt(null)
      return
    }
    const enabledIds = getEnabledProviderIds(providerSettings)
    if (enabledIds.length === 0) {
      setAutoUpdateNextAt(null)
      return
    }
    const intervalMs = autoUpdateInterval * 60_000
    const scheduleNext = () => setAutoUpdateNextAt(Date.now() + intervalMs)
    scheduleNext()
    const interval = setInterval(() => {
      setLoadingForProviders(enabledIds)
      startBatch(enabledIds).catch((error) => {
        console.error("Failed to start auto-update batch:", error)
        setErrorForProviders(enabledIds, "Failed to start probe")
      })
      scheduleNext()
    }, intervalMs)
    return () => clearInterval(interval)
  }, [
    autoUpdateInterval,
    autoUpdateResetToken,
    providerSettings,
    setLoadingForProviders,
    setErrorForProviders,
    startBatch,
  ])

  // Apply theme mode to document
  useEffect(() => {
    const root = document.documentElement
    const apply = (dark: boolean) => {
      root.classList.toggle("dark", dark)
    }

    if (themeMode === "light") {
      apply(false)
      return
    }
    if (themeMode === "dark") {
      apply(true)
      return
    }

    // "system" — follow OS preference
    const mq = window.matchMedia("(prefers-color-scheme: dark)")
    apply(mq.matches)
    const handler = (e: MediaQueryListEvent) => apply(e.matches)
    mq.addEventListener("change", handler)
    return () => mq.removeEventListener("change", handler)
  }, [themeMode])

  const resetAutoUpdateSchedule = useCallback(() => {
    if (!providerSettings) return
    const enabledIds = getEnabledProviderIds(providerSettings)
    // Defensive: retry only possible for enabled providers, so this branch is unreachable in normal use
    /* v8 ignore start */
    if (enabledIds.length === 0) {
      setAutoUpdateNextAt(null)
      return
    }
    /* v8 ignore stop */
    setAutoUpdateNextAt(Date.now() + autoUpdateInterval * 60_000)
    setAutoUpdateResetToken((value) => value + 1)
  }, [autoUpdateInterval, providerSettings])

  const handleRetryProvider = useCallback(
    (id: string) => {
      track("provider_refreshed", { provider_id: id })
      resetAutoUpdateSchedule()
      // Mark as manual refresh
      manualRefreshIdsRef.current.add(id)
      setLoadingForProviders([id])
      startBatch([id]).catch((error) => {
        console.error("Failed to retry provider:", error)
        setErrorForProviders([id], "Failed to start probe")
      })
    },
    [resetAutoUpdateSchedule, setLoadingForProviders, setErrorForProviders, startBatch]
  )

  const handleThemeModeChange = useCallback((mode: ThemeMode) => {
    track("setting_changed", { setting: "theme", value: mode })
    setThemeMode(mode)
    void saveThemeMode(mode).catch((error) => {
      console.error("Failed to save theme mode:", error)
    })
  }, [])

  const handleDisplayModeChange = useCallback((mode: DisplayMode) => {
    track("setting_changed", { setting: "display_mode", value: mode })
    setDisplayMode(mode)
    // Display mode is a direct user-facing toggle; update tray immediately.
    scheduleTrayIconUpdate("settings", 0)
    void saveDisplayMode(mode).catch((error) => {
      console.error("Failed to save display mode:", error)
    })
  }, [scheduleTrayIconUpdate])

  const handleTrayIconStyleChange = useCallback((style: TrayIconStyle) => {
    track("setting_changed", { setting: "tray_icon_style", value: style })
    const mandatory = isTrayPercentageMandatory(style)
    if (mandatory && trayShowPercentageRef.current !== true) {
      trayShowPercentageRef.current = true
      setTrayShowPercentage(true)
      void saveTrayShowPercentage(true).catch((error) => {
        console.error("Failed to save tray show percentage:", error)
      })
    }

    trayIconStyleRef.current = style
    setTrayIconStyle(style)
    // Tray icon style is a direct user-facing toggle; update tray immediately.
    scheduleTrayIconUpdate("settings", 0)
    void saveTrayIconStyle(style).catch((error) => {
      console.error("Failed to save tray icon style:", error)
    })
  }, [scheduleTrayIconUpdate])

  const handleTrayShowPercentageChange = useCallback((value: boolean) => {
    track("setting_changed", { setting: "tray_show_percentage", value: value ? "true" : "false" })
    trayShowPercentageRef.current = value
    setTrayShowPercentage(value)
    // Tray icon text visibility is a direct user-facing toggle; update tray immediately.
    scheduleTrayIconUpdate("settings", 0)
    void saveTrayShowPercentage(value).catch((error) => {
      console.error("Failed to save tray show percentage:", error)
    })
  }, [scheduleTrayIconUpdate])

  const handleAutoUpdateIntervalChange = useCallback((value: AutoUpdateIntervalMinutes) => {
    track("setting_changed", { setting: "auto_refresh", value: String(value) })
    setAutoUpdateInterval(value)
    if (providerSettings) {
      const enabledIds = getEnabledProviderIds(providerSettings)
      if (enabledIds.length > 0) {
        setAutoUpdateNextAt(Date.now() + value * 60_000)
      } else {
        setAutoUpdateNextAt(null)
      }
    }
    void saveAutoUpdateInterval(value).catch((error) => {
      console.error("Failed to save auto-update interval:", error)
    })
  }, [providerSettings])

  const settingsProviders = useMemo(() => {
    if (!providerSettings) return []
    const providerMap = new Map(providersMeta.map((provider) => [provider.id, provider]))
    return providerSettings.order
      .map((id) => {
        const meta = providerMap.get(id)
        if (!meta) return null
        return {
          id,
          name: meta.name,
          enabled: !providerSettings.disabled.includes(id),
        }
      })
      .filter((provider): provider is { id: string; name: string; enabled: boolean } =>
        Boolean(provider)
      )
  }, [providerSettings, providersMeta])

  const handleReorder = useCallback(
    (orderedIds: string[]) => {
      if (!providerSettings) return
      track("providers_reordered", { count: orderedIds.length })
      const nextSettings: ProviderSettings = {
        ...providerSettings,
        order: orderedIds,
      }
      setProviderSettings(nextSettings)
      scheduleTrayIconUpdate("settings", TRAY_SETTINGS_DEBOUNCE_MS)
      void saveProviderSettings(nextSettings).catch((error) => {
        console.error("Failed to save provider order:", error)
      })
    },
    [providerSettings, scheduleTrayIconUpdate]
  )

  const handleToggle = useCallback(
    (id: string) => {
      if (!providerSettings) return
      const wasDisabled = providerSettings.disabled.includes(id)
      track("provider_toggled", { provider_id: id, enabled: wasDisabled ? "true" : "false" })
      const disabled = new Set(providerSettings.disabled)

      if (wasDisabled) {
        disabled.delete(id)
        setLoadingForProviders([id])
        startBatch([id]).catch((error) => {
          console.error("Failed to start probe for enabled provider:", error)
          setErrorForProviders([id], "Failed to start probe")
        })
      } else {
        disabled.add(id)
        // No probe needed for disable
      }

      const nextSettings: ProviderSettings = {
        ...providerSettings,
        disabled: Array.from(disabled),
      }
      setProviderSettings(nextSettings)
      scheduleTrayIconUpdate("settings", TRAY_SETTINGS_DEBOUNCE_MS)
      void saveProviderSettings(nextSettings).catch((error) => {
        console.error("Failed to save provider toggle:", error)
      })
    },
    [providerSettings, setLoadingForProviders, setErrorForProviders, startBatch, scheduleTrayIconUpdate]
  )

  // Detect whether the scroll area has overflow below
  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const check = () => {
      setCanScrollDown(el.scrollHeight - el.scrollTop - el.clientHeight > 1)
    }
    check()
    el.addEventListener("scroll", check, { passive: true })
    const ro = new ResizeObserver(check)
    ro.observe(el)
    // Re-check when child content changes (async data loads)
    const mo = new MutationObserver(check)
    mo.observe(el, { childList: true, subtree: true })
    return () => {
      el.removeEventListener("scroll", check)
      ro.disconnect()
      mo.disconnect()
    }
  }, [activeView])

  // Render content based on active view
  const renderContent = () => {
    if (activeView === "home") {
      return (
        <OverviewPage
          providers={displayProviders}
          onRetryProvider={handleRetryProvider}
          displayMode={displayMode}
        />
      )
    }
    if (activeView === "settings") {
      return (
        <SettingsPage
          providers={settingsProviders}
          onReorder={handleReorder}
          onToggle={handleToggle}
          autoUpdateInterval={autoUpdateInterval}
          onAutoUpdateIntervalChange={handleAutoUpdateIntervalChange}
          themeMode={themeMode}
          onThemeModeChange={handleThemeModeChange}
          displayMode={displayMode}
          onDisplayModeChange={handleDisplayModeChange}
          trayIconStyle={trayIconStyle}
          onTrayIconStyleChange={handleTrayIconStyleChange}
          trayShowPercentage={trayShowPercentage}
          onTrayShowPercentageChange={handleTrayShowPercentageChange}
          providerIconUrl={navProviders[0]?.iconUrl}
        />
      )
    }
    // Provider detail view
    const handleRetry = selectedProvider
      ? () => handleRetryProvider(selectedProvider.meta.id)
      : /* v8 ignore next */ undefined
    return (
      <ProviderDetailPage
        provider={selectedProvider}
        onRetry={handleRetry}
        displayMode={displayMode}
      />
    )
  }

  return (
    <div ref={containerRef} className="flex flex-col items-center p-6 pt-1.5 bg-transparent">
      <div className="tray-arrow" />
      <div
        className="relative bg-card rounded-xl overflow-hidden select-none w-full border shadow-lg flex flex-col"
        style={maxPanelHeightPx ? { maxHeight: `${maxPanelHeightPx - ARROW_OVERHEAD_PX}px` } : undefined}
      >
        <div className="flex flex-1 min-h-0 flex-row">
          <SideNav
            activeView={activeView}
            onViewChange={setActiveView}
            providers={navProviders}
          />
          <div className="flex-1 flex flex-col px-3 pt-2 pb-1.5 min-w-0 bg-card dark:bg-muted/50">
            <div className="relative flex-1 min-h-0">
              <div ref={scrollRef} className="h-full overflow-y-auto scrollbar-none">
                {renderContent()}
              </div>
              <div className={`pointer-events-none absolute inset-x-0 bottom-0 h-14 bg-gradient-to-t from-card dark:from-muted/50 to-transparent transition-opacity duration-200 ${canScrollDown ? "opacity-100" : "opacity-0"}`} />
            </div>
            <PanelFooter
              version={appVersion}
              autoUpdateNextAt={autoUpdateNextAt}
              updateStatus={updateStatus}
              onUpdateInstall={triggerInstall}
              showAbout={showAbout}
              onShowAbout={() => setShowAbout(true)}
              onCloseAbout={() => setShowAbout(false)}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

export { App };
