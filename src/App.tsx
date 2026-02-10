import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { getCurrentWindow, PhysicalSize, currentMonitor } from "@tauri-apps/api/window"
import { getVersion } from "@tauri-apps/api/app"
import { resolveResource } from "@tauri-apps/api/path"
import { openUrl } from "@tauri-apps/plugin-opener"
import { TrayIcon } from "@tauri-apps/api/tray"
import { SideNav, type ActiveView } from "@/components/side-nav"
import { PanelFooter } from "@/components/panel-footer"
import { OverviewPage } from "@/pages/overview"
import { ProviderDetailPage } from "@/pages/provider-detail"
import { SettingsPage } from "@/pages/settings"
import type { ProviderMeta, ProviderOutput } from "@/lib/provider-types"
import {
  clearAccountCredentials,
  cancelClaudeOAuth,
  cancelCodexOAuth,
  cancelCopilotOAuth,
  createAccount,
  deleteAccount,
  finishClaudeOAuth,
  finishCodexOAuth,
  finishCopilotOAuth,
  hasAccountCredentials,
  listAccounts,
  listProviders,
  setAccountCredentials,
  startClaudeOAuth,
  startCodexOAuth,
  startCopilotOAuth,
  updateAccount,
  type OAuthStartResponse,
  type AccountRecord,
  type ProviderAuthStrategy,
  type ProviderDescriptor,
} from "@/lib/accounts"
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
  loadAccountOrderByProvider,
  loadDisplayMode,
  loadProviderSettings,
  loadTrayShowPercentage,
  loadTrayIconStyle,
  loadThemeMode,
  normalizeProviderSettings,
  saveAutoUpdateInterval,
  saveAccountOrderByProvider,
  saveDisplayMode,
  saveProviderSettings,
  saveTrayShowPercentage,
  saveTrayIconStyle,
  saveThemeMode,
  type AccountOrderByProvider,
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

type AccountOAuthSession = {
  status: "pending" | "error"
  providerId: string
  requestId: string
  url?: string
  userCode?: string | null
  message?: string
}

function App() {
  const [activeView, setActiveView] = useState<ActiveView>("home");
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollDown, setCanScrollDown] = useState(false);
  const [providerStates, setProviderStates] = useState<Record<string, ProviderState>>({})
  const [providersMeta, setProvidersMeta] = useState<ProviderMeta[]>([])
  const [providerDescriptors, setProviderDescriptors] = useState<ProviderDescriptor[]>([])
  const [accounts, setAccounts] = useState<AccountRecord[]>([])
  const [accountOrderByProvider, setAccountOrderByProvider] =
    useState<AccountOrderByProvider>({})
  const [accountCredentialsById, setAccountCredentialsById] = useState<Record<string, boolean>>({})
  const [accountOAuthSessionById, setAccountOAuthSessionById] = useState<
    Record<string, AccountOAuthSession | undefined>
  >({})
  const [accountsLoading, setAccountsLoading] = useState(false)
  const [providerSettings, setProviderSettings] = useState<ProviderSettings | null>(null)
  const [autoUpdateInterval, setAutoUpdateInterval] = useState<AutoUpdateIntervalMinutes>(
    DEFAULT_AUTO_UPDATE_INTERVAL
  )
  const [autoUpdateNextAt, setAutoUpdateNextAt] = useState<number | null>(null)
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
  const isMountedRef = useRef(true)

  useEffect(() => {
    isMountedRef.current = true
    return () => {
      isMountedRef.current = false
    }
  }, [])

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
      renderTrayBarsIcon({ bars, sizePx, style, percentText })
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

  const reloadAccounts = useCallback(async () => {
    if (isMountedRef.current) {
      setAccountsLoading(true)
    }
    try {
      const nextAccounts = await listAccounts()
      const credentialsEntries = await Promise.all(
        nextAccounts.map(async (account) => {
          try {
            const hasCredentials = await hasAccountCredentials(account.id)
            return [account.id, hasCredentials] as const
          } catch {
            return [account.id, false] as const
          }
        }),
      )

      if (!isMountedRef.current) return

      setAccounts(nextAccounts)
      setAccountCredentialsById(Object.fromEntries(credentialsEntries))
    } finally {
      if (isMountedRef.current) {
        setAccountsLoading(false)
      }
    }
  }, [])

  const triggerProviderProbe = useCallback(
    (providerId: string) => {
      if (!providerSettings) return
      if (providerSettings.disabled.includes(providerId)) return

      setLoadingForProviders([providerId])
      startBatch([providerId]).catch((error) => {
        console.error("Failed to start probe for provider account change:", error)
        setErrorForProviders([providerId], "Failed to start probe")
      })
    },
    [providerSettings, setLoadingForProviders, setErrorForProviders, startBatch],
  )

  const providerAuthStrategiesByProvider = useMemo(() => {
    const byProvider: Record<string, ProviderAuthStrategy[]> = {}
    for (const provider of providerDescriptors) {
      byProvider[provider.id] = provider.authStrategies
    }
    return byProvider
  }, [providerDescriptors])

  const accountsByProvider = useMemo(() => {
    const grouped = accounts.reduce<
      Record<
        string,
        Array<{
          id: string
          providerId: string
          authStrategyId?: string | null
          label: string
          hasCredentials: boolean
          lastFetchAt?: string | null
          lastError?: string | null
        }>
      >
    >((result, account) => {
      if (!result[account.providerId]) {
        result[account.providerId] = []
      }

      result[account.providerId].push({
        id: account.id,
        providerId: account.providerId,
        authStrategyId: account.authStrategyId,
        label: account.label,
        hasCredentials: Boolean(accountCredentialsById[account.id]),
        lastFetchAt: account.lastFetchAt,
        lastError: account.lastError,
      })

      return result
    }, {})

    for (const [providerId, providerAccounts] of Object.entries(grouped)) {
      const order = accountOrderByProvider[providerId] ?? []
      if (order.length === 0 || providerAccounts.length <= 1) {
        continue
      }
      const orderIndex = new Map(order.map((accountId, index) => [accountId, index]))
      providerAccounts.sort((left, right) => {
        const leftIndex = orderIndex.get(left.id)
        const rightIndex = orderIndex.get(right.id)
        if (leftIndex === undefined && rightIndex === undefined) return 0
        if (leftIndex === undefined) return 1
        if (rightIndex === undefined) return -1
        return leftIndex - rightIndex
      })
    }

    return grouped
  }, [accounts, accountCredentialsById, accountOrderByProvider])

  const accountIdsByProvider = useMemo(() => {
    return Object.fromEntries(
      Object.entries(accountsByProvider).map(([providerId, providerAccounts]) => [
        providerId,
        providerAccounts.map((account) => account.id),
      ]),
    ) as Record<string, string[]>
  }, [accountsByProvider])

  const handleReorderProviderAccounts = useCallback(
    (providerId: string, orderedAccountIds: string[]) => {
      if (orderedAccountIds.length <= 1) return

      setAccountOrderByProvider((previous) => {
        const current = previous[providerId] ?? []
        const unchanged =
          current.length === orderedAccountIds.length &&
          current.every((accountId, index) => accountId === orderedAccountIds[index])
        if (unchanged) {
          return previous
        }

        const next: AccountOrderByProvider = {
          ...previous,
          [providerId]: orderedAccountIds,
        }

        track("accounts_reordered", { provider_id: providerId, count: orderedAccountIds.length })
        void saveAccountOrderByProvider(next).catch((error) => {
          console.error("Failed to save account order:", error)
        })

        return next
      })
    },
    [],
  )

  const handleCreateProviderAccount = useCallback(
    async (providerId: string, authStrategyId: string) => {
      const descriptor = providerDescriptors.find((provider) => provider.id === providerId)
      if (!descriptor) {
        throw new Error(`Unknown provider: ${providerId}`)
      }

      const strategySupported = descriptor.authStrategies.some(
        (strategy) => strategy.id === authStrategyId,
      )
      if (!strategySupported) {
        throw new Error(`Unsupported auth strategy for provider ${providerId}: ${authStrategyId}`)
      }

      await createAccount({
        providerId,
        authStrategyId,
        settings: {},
      })

      await reloadAccounts()
    },
    [providerDescriptors, reloadAccounts],
  )

  const handleUpdateProviderAccountLabel = useCallback(
    async (providerId: string, accountId: string, label: string) => {
      await updateAccount(accountId, { label })
      await reloadAccounts()
      triggerProviderProbe(providerId)
    },
    [reloadAccounts, triggerProviderProbe],
  )

  const handleDeleteProviderAccount = useCallback(
    async (providerId: string, accountId: string) => {
      await deleteAccount(accountId)
      await reloadAccounts()
      triggerProviderProbe(providerId)
    },
    [reloadAccounts, triggerProviderProbe],
  )

  const handleSaveProviderAccountCredentials = useCallback(
    async (
      providerId: string,
      accountId: string,
      credentials: Record<string, unknown>,
    ) => {
      await setAccountCredentials(accountId, credentials)
      await reloadAccounts()
      triggerProviderProbe(providerId)
    },
    [reloadAccounts, triggerProviderProbe],
  )

  const handleClearProviderAccountCredentials = useCallback(
    async (providerId: string, accountId: string) => {
      await clearAccountCredentials(accountId)
      await reloadAccounts()
      triggerProviderProbe(providerId)
    },
    [reloadAccounts, triggerProviderProbe],
  )

  const startProviderOAuth = useCallback(
    async (providerId: string, accountId: string): Promise<OAuthStartResponse> => {
      if (providerId === "codex") {
        return startCodexOAuth(accountId)
      }
      if (providerId === "claude") {
        return startClaudeOAuth(accountId)
      }
      if (providerId === "copilot") {
        return startCopilotOAuth(accountId)
      }
      throw new Error(`Provider does not support native OAuth: ${providerId}`)
    },
    [],
  )

  const finishProviderOAuth = useCallback(
    async (providerId: string, requestId: string) => {
      if (providerId === "codex") {
        return finishCodexOAuth(requestId, 180_000)
      }
      if (providerId === "claude") {
        return finishClaudeOAuth(requestId, 180_000)
      }
      if (providerId === "copilot") {
        return finishCopilotOAuth(requestId, 180_000)
      }
      throw new Error(`Provider does not support native OAuth: ${providerId}`)
    },
    [],
  )

  const cancelProviderOAuth = useCallback(
    async (providerId: string, requestId: string) => {
      if (providerId === "codex") {
        await cancelCodexOAuth(requestId)
        return
      }
      if (providerId === "claude") {
        await cancelClaudeOAuth(requestId)
        return
      }
      if (providerId === "copilot") {
        await cancelCopilotOAuth(requestId)
        return
      }
      throw new Error(`Provider does not support native OAuth: ${providerId}`)
    },
    [],
  )

  const handleStartAccountOAuth = useCallback(
    async (providerId: string, accountId: string) => {
      const started = await startProviderOAuth(providerId, accountId)
      if (!isMountedRef.current) return

      setAccountOAuthSessionById((previous) => ({
        ...previous,
        [accountId]: {
          status: "pending",
          providerId,
          requestId: started.requestId,
          url: started.url,
          userCode: started.userCode,
        },
      }))

      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        navigator.clipboard.writeText(started.url).catch((error) => {
          console.error("Failed to copy OAuth URL:", error)
        })
      }

      openUrl(started.url).catch((error) => {
        console.error("Failed to open OAuth URL:", error)
      })

      void (async () => {
        try {
          await finishProviderOAuth(providerId, started.requestId)
          if (!isMountedRef.current) return
          setAccountOAuthSessionById((previous) => ({
            ...previous,
            [accountId]: undefined,
          }))
          await reloadAccounts()
          triggerProviderProbe(providerId)
        } catch (error) {
          if (!isMountedRef.current) return
          const message = error instanceof Error ? error.message : String(error)
          const isTimeoutError = /timed out|timeout/i.test(message)
          setAccountOAuthSessionById((previous) => ({
            ...previous,
            [accountId]: isTimeoutError
              ? undefined
              : {
                  status: "error",
                  providerId,
                  requestId: started.requestId,
                  message,
                },
          }))
        }
      })()
    },
    [finishProviderOAuth, reloadAccounts, startProviderOAuth, triggerProviderProbe],
  )

  const handleCancelAccountOAuth = useCallback(
    async (providerId: string, accountId: string) => {
      const session = accountOAuthSessionById[accountId]
      if (!session?.requestId) {
        return
      }

      await cancelProviderOAuth(providerId, session.requestId)
      if (!isMountedRef.current) return
      setAccountOAuthSessionById((previous) => ({
        ...previous,
        [accountId]: undefined,
      }))
    },
    [accountOAuthSessionById, cancelProviderOAuth],
  )

  useEffect(() => {
    let isMounted = true

    const loadSettings = async () => {
      try {
        const [availableProviders, descriptors] = await Promise.all([
          invoke<ProviderMeta[]>("list_providers_meta"),
          listProviders(),
        ])
        if (!isMounted) return
        setProvidersMeta(availableProviders)
        setProviderDescriptors(descriptors)

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

        let storedAccountOrderByProvider: AccountOrderByProvider = {}
        try {
          storedAccountOrderByProvider = await loadAccountOrderByProvider()
        } catch (error) {
          console.error("Failed to load account order:", error)
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
          setAccountOrderByProvider(storedAccountOrderByProvider)

          try {
            await reloadAccounts()
          } catch (error) {
            console.error("Failed to load accounts:", error)
          }

          const enabledIds = getEnabledProviderIds(normalized)
          setLoadingForProviders(enabledIds)
          if (enabledIds.length > 0) {
            startBatch(enabledIds).catch((error) => {
              console.error("Failed to start probe batch:", error)
              if (isMounted) {
                setErrorForProviders(enabledIds, "Failed to start probe")
              }
            })
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
  }, [setLoadingForProviders, setErrorForProviders, startBatch, reloadAccounts])

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

  const handleRetryProvider = useCallback(
    (id: string) => {
      track("provider_refreshed", { provider_id: id })
      // Mark as manual refresh
      manualRefreshIdsRef.current.add(id)
      setLoadingForProviders([id])
      startBatch([id]).catch((error) => {
        console.error("Failed to retry provider:", error)
        setErrorForProviders([id], "Failed to start probe")
      })
    },
    [setLoadingForProviders, setErrorForProviders, startBatch]
  )

  const handleManualRefreshAll = useCallback(() => {
    if (!providerSettings) return
    const enabledIds = getEnabledProviderIds(providerSettings)
    if (enabledIds.length === 0) return

    track("providers_refreshed", { count: enabledIds.length })
    for (const id of enabledIds) {
      manualRefreshIdsRef.current.add(id)
    }
    setLoadingForProviders(enabledIds)
    startBatch(enabledIds).catch((error) => {
      console.error("Failed to manually refresh providers:", error)
      setErrorForProviders(enabledIds, "Failed to start probe")
    })
  }, [providerSettings, setLoadingForProviders, setErrorForProviders, startBatch])

  const hasEnabledProviders = useMemo(() => {
    if (!providerSettings) return false
    return getEnabledProviderIds(providerSettings).length > 0
  }, [providerSettings])

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

  const handleNavProviderReorder = useCallback(
    (orderedEnabledIds: string[]) => {
      if (!providerSettings) return
      if (orderedEnabledIds.length <= 1) return

      const disabledSet = new Set(providerSettings.disabled)
      const currentEnabledIds = providerSettings.order.filter((id) => !disabledSet.has(id))
      if (currentEnabledIds.length !== orderedEnabledIds.length) {
        return
      }

      const orderedSet = new Set(orderedEnabledIds)
      if (currentEnabledIds.some((id) => !orderedSet.has(id))) {
        return
      }

      let index = 0
      const nextOrder = providerSettings.order.map((id) => {
        if (disabledSet.has(id)) {
          return id
        }
        const replacement = orderedEnabledIds[index]
        index += 1
        return replacement ?? id
      })

      handleReorder(nextOrder)
    },
    [handleReorder, providerSettings],
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
          accountOrderByProvider={accountIdsByProvider}
          onRetryProvider={handleRetryProvider}
          displayMode={displayMode}
        />
      )
    }
    if (activeView === "settings") {
      return (
        <SettingsPage
          providers={settingsProviders}
          accountsByProvider={accountsByProvider}
          providerAuthStrategiesByProvider={providerAuthStrategiesByProvider}
          accountsLoading={accountsLoading}
          onReorderAccounts={handleReorderProviderAccounts}
          onToggleProvider={handleToggle}
          onReloadAccounts={reloadAccounts}
          onCreateAccount={handleCreateProviderAccount}
          onUpdateAccountLabel={handleUpdateProviderAccountLabel}
          onDeleteAccount={handleDeleteProviderAccount}
          onSaveAccountCredentials={handleSaveProviderAccountCredentials}
          onClearAccountCredentials={handleClearProviderAccountCredentials}
          accountOAuthSessionById={accountOAuthSessionById}
          onStartAccountOAuth={handleStartAccountOAuth}
          onCancelAccountOAuth={handleCancelAccountOAuth}
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
        accountOrder={selectedProvider ? (accountIdsByProvider[selectedProvider.meta.id] ?? []) : []}
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
            onReorderProviders={handleNavProviderReorder}
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
              onManualRefreshAll={hasEnabledProviders ? handleManualRefreshAll : undefined}
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
