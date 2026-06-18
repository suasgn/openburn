import { beforeEach, describe, expect, it, vi } from "vitest"
import { makeCtx as makeBaseCtx } from "../test-helpers.js"

const loadPlugin = async () => {
  await import("./plugin.js")
  return globalThis.__openusage_plugin
}

const USAGE_RESPONSE = JSON.stringify({
  five_hour: { utilization: 30, resets_at: "2099-01-01T00:00:00.000Z" },
  seven_day: { utilization: 50, resets_at: "2099-01-01T00:00:00.000Z" },
})

function setClaudeCredentials(ctx, oauth) {
  ctx.credentials = {
    type: "oauth",
    accessToken: oauth.accessToken || "",
    refreshToken: oauth.refreshToken || "",
    expiresAt: oauth.expiresAt || null,
    subscriptionType: oauth.subscriptionType || null,
    rateLimitTier: oauth.rateLimitTier || null,
  }
}

function makeProbeCtx({ ccusageResult = { status: "runner_failed" } } = {}) {
  const ctx = makeBaseCtx()
  setClaudeCredentials(ctx, { accessToken: "tok", subscriptionType: "pro" })
  ctx.host.http.request.mockReturnValue({ status: 200, bodyText: USAGE_RESPONSE })
  ctx.host.ccusage.query = vi.fn(() => ccusageResult)
  return ctx
}

const okUsage = (daily) => ({ status: "ok", data: { daily } })

function localDayKey(date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, "0")
  const day = String(date.getDate()).padStart(2, "0")
  return year + "-" + month + "-" + day
}

describe("claude plugin ccusage usage trend", () => {
  beforeEach(() => {
    delete globalThis.__openusage_plugin
    vi.resetModules()
  })

  it("adds model percentage text lines and a usage chart from claude ccusage", async () => {
    const todayKey = localDayKey(new Date())
    const yesterdayKey = localDayKey(new Date(Date.now() - 24 * 60 * 60 * 1000))
    const ctx = makeProbeCtx({
      ccusageResult: okUsage([
        {
          date: todayKey,
          totalTokens: 300,
          totalCost: 1,
          modelBreakdowns: [
            { modelName: "claude-sonnet-4-20250514", totalTokens: 200 },
            { modelName: "claude-opus-4-1-20250805", totalTokens: 100 },
          ],
        },
        {
          date: yesterdayKey,
          totalTokens: 150,
          totalCost: 1,
          modelBreakdowns: [
            {
              modelName: "claude-opus-4-1-20250805",
              inputTokens: 30,
              cacheCreationTokens: 20,
              cacheReadTokens: 20,
              outputTokens: 30,
            },
          ],
        },
      ]),
    })
    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    const chart = result.lines.find((line) => line.label === "Usage Trend")
    expect(chart).toMatchObject({
      type: "barChart",
      note: "Estimated from local Claude logs at API rates.",
    })
    expect(chart.points.map((point) => point.value)).toEqual([150, 300])

    const sonnet = result.lines.find((line) => line.label === "claude-sonnet-4-20250514")
    const opus = result.lines.find((line) => line.label === "claude-opus-4-1-20250805")
    expect(sonnet).toMatchObject({
      type: "text",
      value: "50%",
    })
    expect(opus).toMatchObject({
      type: "text",
      value: "50%",
    })
  })
})
