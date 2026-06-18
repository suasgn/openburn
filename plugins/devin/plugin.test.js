import { beforeEach, describe, expect, it, vi } from "vitest"
import { makeCtx } from "../test-helpers.js"

const DEFAULT_API_SERVER_URL = "https://server.codeium.com"
const CLOUD_COMPAT_VERSION = "1.108.2"

const loadPlugin = async () => {
  await import("./plugin.js")
  return globalThis.__openusage_plugin
}

function setCredentials(ctx, credentials) {
  ctx.credentials = credentials
}

function makeQuotaResponse(overrides = {}) {
  const base = {
    userStatus: {
      planStatus: {
        planInfo: {
          planName: "Max",
          billingStrategy: "BILLING_STRATEGY_QUOTA",
        },
        dailyQuotaRemainingPercent: 100,
        weeklyQuotaRemainingPercent: 40,
        overageBalanceMicros: "964220000",
        dailyQuotaResetAtUnix: "1774080000",
        weeklyQuotaResetAtUnix: "1774166400",
      },
    },
  }

  base.userStatus.planStatus = {
    ...base.userStatus.planStatus,
    ...overrides,
    planInfo: {
      ...base.userStatus.planStatus.planInfo,
      ...(overrides.planInfo || {}),
    },
  }

  return base
}

describe("devin plugin", () => {
  beforeEach(() => {
    delete globalThis.__openusage_plugin
    vi.resetModules()
  })

  it("loads account credentials and renders quota lines", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, {
      apiKey: "devin-session-token",
      apiServerUrl: "https://server.codeium.test",
    })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(makeQuotaResponse()),
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(plugin.id).toBe("devin")
    expect(result.plan).toBe("Max")
    expect(result.lines).toEqual([
      {
        type: "progress",
        label: "Daily quota",
        used: 0,
        limit: 100,
        format: { kind: "percent" },
        resetsAt: "2026-03-21T08:00:00.000Z",
        periodDurationMs: 24 * 60 * 60 * 1000,
      },
      {
        type: "progress",
        label: "Weekly quota",
        used: 60,
        limit: 100,
        format: { kind: "percent" },
        resetsAt: "2026-03-22T08:00:00.000Z",
        periodDurationMs: 7 * 24 * 60 * 60 * 1000,
      },
      {
        type: "text",
        label: "Extra usage balance",
        value: "$964.22",
      },
    ])

    const request = ctx.host.http.request.mock.calls[0][0]
    expect(request.url).toBe(
      "https://server.codeium.test/exa.seat_management_pb.SeatManagementService/GetUserStatus"
    )
    const sentBody = JSON.parse(String(request.bodyText))
    expect(sentBody.metadata.apiKey).toBe("devin-session-token")
    expect(sentBody.metadata.ideName).toBe("devin")
    expect(sentBody.metadata.extensionName).toBe("devin")
    expect(sentBody.metadata.ideVersion).toBe(CLOUD_COMPAT_VERSION)
    expect(sentBody.metadata.extensionVersion).toBe(CLOUD_COMPAT_VERSION)
  })

  it("uses the default API server when apiServerUrl is empty", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token", apiServerUrl: "" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(makeQuotaResponse({ planInfo: { planName: "Pro" } })),
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.plan).toBe("Pro")
    expect(ctx.host.http.request.mock.calls[0][0].url).toBe(
      `${DEFAULT_API_SERVER_URL}/exa.seat_management_pb.SeatManagementService/GetUserStatus`
    )
  })

  it("ignores non-https API server URLs and falls back to default", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token", apiServerUrl: "http://server.codeium.test" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(makeQuotaResponse()),
    })

    const plugin = await loadPlugin()
    plugin.probe(ctx)

    expect(ctx.host.http.request.mock.calls[0][0].url).toBe(
      `${DEFAULT_API_SERVER_URL}/exa.seat_management_pb.SeatManagementService/GetUserStatus`
    )
  })

  it("throws the login hint when no credentials are set", async () => {
    const ctx = makeCtx()
    const plugin = await loadPlugin()

    expect(() => plugin.probe(ctx)).toThrow("Add your Devin API key in Settings.")
    expect(ctx.host.http.request).not.toHaveBeenCalled()
  })

  it("throws the login hint when apiKey is empty", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "", apiServerUrl: "" })
    const plugin = await loadPlugin()

    expect(() => plugin.probe(ctx)).toThrow("Add your Devin API key in Settings.")
    expect(ctx.host.http.request).not.toHaveBeenCalled()
  })

  it("throws the login hint on 401 auth failure", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token" })
    ctx.host.http.request.mockReturnValue({ status: 401, bodyText: "{}" })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("Add your Devin API key in Settings.")
  })

  it("uses Devin's hidden daily quota field as weekly usage when weekly percentage is absent", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(
        makeQuotaResponse({
          planInfo: { hideDailyQuota: true },
          weeklyQuotaRemainingPercent: undefined,
        })
      ),
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.lines.find((line) => line.label === "Daily quota")).toBeUndefined()
    expect(result.lines.find((line) => line.label === "Weekly quota")).toMatchObject({
      type: "progress",
      used: 100,
      limit: 100,
      format: { kind: "percent" },
      resetsAt: "2026-03-22T08:00:00.000Z",
      periodDurationMs: 7 * 24 * 60 * 60 * 1000,
    })
    expect(result.lines.find((line) => line.label === "Extra usage balance")?.value).toBe("$964.22")
  })

  it("renders quota percentages when reset timestamps are absent", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(
        makeQuotaResponse({
          dailyQuotaResetAtUnix: undefined,
          weeklyQuotaResetAtUnix: undefined,
        })
      ),
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    const dailyLine = result.lines.find((line) => line.label === "Daily quota")
    const weeklyLine = result.lines.find((line) => line.label === "Weekly quota")
    expect(dailyLine).toMatchObject({
      type: "progress",
      used: 0,
      limit: 100,
      format: { kind: "percent" },
      periodDurationMs: 24 * 60 * 60 * 1000,
    })
    expect(weeklyLine).toMatchObject({
      type: "progress",
      used: 60,
      limit: 100,
      format: { kind: "percent" },
      periodDurationMs: 7 * 24 * 60 * 60 * 1000,
    })
    expect(dailyLine).not.toHaveProperty("resetsAt")
    expect(weeklyLine).not.toHaveProperty("resetsAt")
  })

  it("throws quota unavailable when no displayable fields are present", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(
        makeQuotaResponse({
          dailyQuotaRemainingPercent: undefined,
          weeklyQuotaRemainingPercent: undefined,
          overageBalanceMicros: undefined,
        })
      ),
    })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("Devin quota data unavailable. Try again later.")
  })

  it("omits daily quota when Devin marks it hidden", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(
        makeQuotaResponse({
          planInfo: { hideDailyQuota: true },
          dailyQuotaRemainingPercent: undefined,
          dailyQuotaResetAtUnix: undefined,
        })
      ),
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.lines.find((line) => line.label === "Daily quota")).toBeUndefined()
    expect(result.lines.find((line) => line.label === "Weekly quota")?.used).toBe(60)
  })

  it("renders quota lines when Devin omits extra usage balance", async () => {
    const ctx = makeCtx()
    setCredentials(ctx, { apiKey: "devin-session-token" })
    ctx.host.http.request.mockReturnValue({
      status: 200,
      bodyText: JSON.stringify(makeQuotaResponse({ overageBalanceMicros: undefined })),
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.lines).toHaveLength(2)
    expect(result.lines.find((line) => line.label === "Extra usage balance")).toBeUndefined()
  })
})
