import { cleanup, render, screen, waitFor } from "@testing-library/react"
import type { ReactNode } from "react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { AccountSettingsSection } from "@/components/account-settings-section"

const mocks = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  createAccount: vi.fn(),
  updateAccount: vi.fn(),
  deleteAccount: vi.fn(),
  startAccountTransferCroc: vi.fn(),
  finishAccountTransferCroc: vi.fn(),
  hasAccountCredentials: vi.fn(),
  importAccountTransferCroc: vi.fn(),
  qrToDataUrl: vi.fn(),
  requestPermissions: vi.fn(),
  scan: vi.fn(),
  setAccountCredentials: vi.fn(),
  clearAccountCredentials: vi.fn(),
  syncAccountToOpencodeAuth: vi.fn(),
  startAccountAuth: vi.fn(),
  finishAccountAuth: vi.fn(),
  cancelAccountAuth: vi.fn(),
  loadAccountOrderByPlugin: vi.fn(),
  saveAccountOrderByPlugin: vi.fn(),
  openUrl: vi.fn(),
}))

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: mocks.openUrl,
}))

vi.mock("@dnd-kit/core", () => ({
  DndContext: ({ children }: { children: ReactNode }) => <div data-testid="dnd-context">{children}</div>,
  closestCenter: vi.fn(),
  PointerSensor: class {},
  KeyboardSensor: class {},
  useSensor: vi.fn((_sensor: unknown, options?: unknown) => ({ sensor: _sensor, options })),
  useSensors: vi.fn((...sensors: unknown[]) => sensors),
}))

vi.mock("@dnd-kit/sortable", () => ({
  arrayMove: (items: unknown[], from: number, to: number) => {
    const next = [...items]
    const [moved] = next.splice(from, 1)
    next.splice(to, 0, moved)
    return next
  },
  SortableContext: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  sortableKeyboardCoordinates: vi.fn(),
  useSortable: () => ({
    attributes: {},
    listeners: {},
    setNodeRef: vi.fn(),
    transform: null,
    transition: undefined,
    isDragging: false,
  }),
  verticalListSortingStrategy: vi.fn(),
}))

vi.mock("@dnd-kit/utilities", () => ({
  CSS: { Transform: { toString: () => "" } },
}))

vi.mock("@/lib/accounts", () => ({
  listAccounts: mocks.listAccounts,
  createAccount: mocks.createAccount,
  updateAccount: mocks.updateAccount,
  deleteAccount: mocks.deleteAccount,
  hasAccountCredentials: mocks.hasAccountCredentials,
  setAccountCredentials: mocks.setAccountCredentials,
  clearAccountCredentials: mocks.clearAccountCredentials,
  syncAccountToOpencodeAuth: mocks.syncAccountToOpencodeAuth,
  startAccountAuth: mocks.startAccountAuth,
  finishAccountAuth: mocks.finishAccountAuth,
  cancelAccountAuth: mocks.cancelAccountAuth,
  startAccountTransferCroc: mocks.startAccountTransferCroc,
  finishAccountTransferCroc: mocks.finishAccountTransferCroc,
  importAccountTransferCroc: mocks.importAccountTransferCroc,
}))

vi.mock("qrcode", () => ({
  default: { toDataURL: mocks.qrToDataUrl },
}))

vi.mock("@tauri-apps/plugin-barcode-scanner", () => ({
  Format: { QRCode: "QR_CODE" },
  requestPermissions: mocks.requestPermissions,
  scan: mocks.scan,
}))

vi.mock("@/lib/settings", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/settings")>()),
  loadAccountOrderByPlugin: mocks.loadAccountOrderByPlugin,
  saveAccountOrderByPlugin: mocks.saveAccountOrderByPlugin,
}))

const regionalApiPlugin = {
  id: "regional-ai",
  name: "Regional AI",
  enabled: true,
  auth: {
    defaultStrategyId: "apiKey",
    strategies: [
      {
        id: "apiKey",
        label: "API Key",
        kind: "apiKey" as const,
        fields: [
          { name: "apiKey", label: "API Key", fieldType: "password" as const, secret: true, required: true },
          {
            name: "region",
            label: "Region",
            description: "Global uses api.example.com. China uses cn.example.com.",
            fieldType: "segmented" as const,
            defaultValue: "global",
            options: [
              { label: "Global", value: "global" },
              { label: "China", value: "cn" },
            ],
          },
        ],
      },
    ],
  },
}

describe("AccountSettingsSection", () => {
  beforeEach(() => {
    mocks.listAccounts.mockResolvedValue([
      {
        id: "acc-1",
        pluginId: "regional-ai",
        enabled: true,
        authStrategyId: "apiKey",
        label: "Personal",
        settings: {},
        createdAt: "2026-01-01T00:00:00.000Z",
        updatedAt: "2026-01-01T00:00:00.000Z",
        lastFetchAt: null,
        lastError: null,
      },
    ])
    mocks.hasAccountCredentials.mockResolvedValue(false)
    mocks.setAccountCredentials.mockResolvedValue(undefined)
    mocks.loadAccountOrderByPlugin.mockResolvedValue({})
    mocks.saveAccountOrderByPlugin.mockResolvedValue(undefined)
    mocks.requestPermissions.mockResolvedValue("granted")
  })

  afterEach(() => {
    cleanup()
    vi.clearAllMocks()
  })

  it("renders manifest-defined credential fields and saves selected options", async () => {
    const onAccountChanged = vi.fn()
    render(
      <AccountSettingsSection
        plugins={[regionalApiPlugin]}
        onAccountChanged={onAccountChanged}
        onPluginEnabledChange={vi.fn()}
      />
    )

    await screen.findByText("Personal")
    await userEvent.click(screen.getByText("Personal"))
    expect(screen.getByText("Global uses api.example.com. China uses cn.example.com.")).toBeInTheDocument()
    await userEvent.type(screen.getByLabelText("API Key"), "zai-key")
    await userEvent.click(screen.getByText("China"))
    await userEvent.click(screen.getByRole("button", { name: "Save credentials" }))

    await waitFor(() => {
      expect(mocks.setAccountCredentials).toHaveBeenCalledWith("acc-1", {
        type: "apiKey",
        apiKey: "zai-key",
        region: "cn",
      })
    })
    expect(onAccountChanged).toHaveBeenCalledWith("regional-ai")
  })

  it("shows a croc QR code for configured credentials", async () => {
    mocks.hasAccountCredentials.mockResolvedValue(true)
    mocks.startAccountTransferCroc.mockResolvedValue({ code: "7-red-blue", accountCount: 2 })
    mocks.finishAccountTransferCroc.mockImplementation(() => new Promise(() => {}))
    mocks.qrToDataUrl.mockResolvedValue("data:image/png;base64,qr")
    render(
      <AccountSettingsSection
        plugins={[regionalApiPlugin]}
        onAccountChanged={vi.fn()}
        onPluginEnabledChange={vi.fn()}
      />
    )

    await screen.findByText("Personal")
    await userEvent.click(screen.getByRole("button", { name: "Export Account" }))

    await waitFor(() => expect(mocks.startAccountTransferCroc).toHaveBeenCalledWith())
    expect(
      await screen.findByRole("dialog", { name: "Account croc transfer" }, { timeout: 10_000 })
    ).toBeInTheDocument()
    expect(screen.getByText("7-red-blue")).toBeInTheDocument()
  })

  it("imports accounts through a scanned croc code on mobile", async () => {
    const originalUserAgent = navigator.userAgent
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: "Android" })
    const imported = {
      id: "acc-2",
      pluginId: "regional-ai",
      enabled: true,
      authStrategyId: "apiKey",
      label: "Imported",
      settings: {},
      createdAt: "2026-01-02T00:00:00.000Z",
      updatedAt: "2026-01-02T00:00:00.000Z",
    }
    mocks.importAccountTransferCroc.mockResolvedValue([imported])
    mocks.scan.mockResolvedValue({ content: "croc-transfer:7-red-blue", format: "QR_CODE" })
    render(
      <AccountSettingsSection
        plugins={[regionalApiPlugin]}
        onAccountChanged={vi.fn()}
        onPluginEnabledChange={vi.fn()}
      />
    )

    await userEvent.click(await screen.findByRole("button", { name: "Import Account" }))

    await waitFor(() => expect(mocks.importAccountTransferCroc).toHaveBeenCalledWith("7-red-blue"))
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: originalUserAgent })
  })

  it("shows structured transfer errors instead of object object", async () => {
    const originalUserAgent = navigator.userAgent
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: "Android" })
    mocks.scan.mockResolvedValue({ content: "croc-transfer:7-red-blue", format: "QR_CODE" })
    mocks.importAccountTransferCroc.mockRejectedValue({ message: "croc transfer failed" })
    render(
      <AccountSettingsSection
        plugins={[regionalApiPlugin]}
        onAccountChanged={vi.fn()}
        onPluginEnabledChange={vi.fn()}
      />
    )

    await userEvent.click(await screen.findByRole("button", { name: "Import Account" }))

    expect((await screen.findAllByText("croc transfer failed")).length).toBeGreaterThan(0)
    expect(screen.queryByText("[object Object]")).not.toBeInTheDocument()
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: originalUserAgent })
  })

  it("explains when Android camera permission is denied", async () => {
    const originalUserAgent = navigator.userAgent
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: "Android" })
    mocks.requestPermissions.mockResolvedValue("denied")
    render(
      <AccountSettingsSection
        plugins={[regionalApiPlugin]}
        onAccountChanged={vi.fn()}
        onPluginEnabledChange={vi.fn()}
      />
    )

    await userEvent.click(await screen.findByRole("button", { name: "Import Account" }))

    expect((await screen.findAllByText("Camera permission is required. Allow camera access in Android Settings and try again.")).length).toBeGreaterThan(0)
    expect(mocks.scan).not.toHaveBeenCalled()
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: originalUserAgent })
  })
})
