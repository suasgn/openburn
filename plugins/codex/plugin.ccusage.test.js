import { beforeEach, describe, expect, it, vi } from "vitest"
import { makeCtx as makeBaseCtx } from "../test-helpers.js"

const loadPlugin = async () => {
  await import("./plugin.js")
  return globalThis.__openusage_plugin
}

const setAccountCredentials = (ctx, auth) => {
  if (!auth || typeof auth !== "object") return
  if (auth.OPENAI_API_KEY) {
    ctx.credentials = { type: "apiKey", apiKey: auth.OPENAI_API_KEY }
    return
  }
  const tokens = auth.tokens || {}
  if (!tokens.access_token && !tokens.refresh_token && !tokens.id_token) return
  ctx.credentials = {
    type: "oauth",
    accessToken: tokens.access_token || "",
    refreshToken: tokens.refresh_token || "",
    idToken: tokens.id_token || "",
    accountId: tokens.account_id || null,
    expiresAt: tokens.expires_at || null,
    lastRefresh: auth.last_refresh || new Date().toISOString(),
  }
}

function makeCtx() {
  const ctx = makeBaseCtx()
  const writeText = ctx.host.fs.writeText
  ctx.host.fs.writeText = vi.fn((path, text) => {
    if (String(path).endsWith("/auth.json")) {
      const parsed = ctx.util.tryParseJson(text)
      setAccountCredentials(ctx, parsed)
    }
    return writeText(path, text)
  })
  return ctx
}

function dayKey(daysAgo) {
  const d = new Date()
  d.setDate(d.getDate() - daysAgo)
  const year = d.getFullYear()
  const month = String(d.getMonth() + 1).padStart(2, "0")
  const day = String(d.getDate()).padStart(2, "0")
  return year + "-" + month + "-" + day
}

describe("codex plugin ccusage usage trend", () => {
  beforeEach(() => {
    delete globalThis.__openusage_plugin
    vi.resetModules()
  })

  it("adds model percentage text lines and a usage chart from codex ccusage", async () => {
    const ctx = makeCtx()
    ctx.host.fs.writeText("~/.codex/auth.json", JSON.stringify({
      tokens: { access_token: "token" },
      last_refresh: new Date().toISOString(),
    }))
    ctx.host.http.request.mockReturnValue({
      status: 200,
      headers: { "x-codex-primary-used-percent": "10" },
      bodyText: JSON.stringify({}),
    })
    ctx.host.ccusage.query.mockReturnValue({
      status: "ok",
      data: {
        daily: [
          {
            date: dayKey(0),
            totalTokens: 300,
            models: {
              "gpt-5.5": { totalTokens: 200 },
              "gpt-5": { totalTokens: 100 },
            },
          },
          {
            date: dayKey(1),
            totalTokens: 150,
            models: {
              "gpt-5": { inputTokens: 30, cachedInputTokens: 20, outputTokens: 50 },
            },
          },
        ],
      },
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    const chart = result.lines.find((line) => line.label === "Usage Trend")
    expect(chart).toMatchObject({
      type: "barChart",
      note: "Estimated from local Codex logs for the selected account.",
    })
    expect(chart.points.map((point) => point.value)).toEqual([150, 300])

    const gpt55 = result.lines.find((line) => line.label === "gpt-5.5")
    const gpt5 = result.lines.find((line) => line.label === "gpt-5")
    expect(gpt55).toMatchObject({
      type: "text",
      value: "50%",
    })
    expect(gpt5).toMatchObject({
      type: "text",
      value: "50%",
    })
  })
})
