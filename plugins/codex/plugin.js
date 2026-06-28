(function () {
  const CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann"
  const REFRESH_URL = "https://auth.openai.com/oauth/token"
  const USAGE_URL = "https://chatgpt.com/backend-api/wham/usage"
  const RESET_CREDITS_URL = "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits"
  const CREDIT_USD_RATE = 0.04
  const DAY_MS = 24 * 60 * 60 * 1000
  const REFRESH_AGE_MS = 7 * DAY_MS
  const SHORT_EXPIRY_REFRESH_BUFFER_MS = 5 * 60 * 1000
  const LONG_EXPIRY_REFRESH_BUFFER_MS = DAY_MS

  function extractAccountIdFromClaims(claims) {
    if (!claims || typeof claims !== "object") return null
    if (claims.chatgpt_account_id) return claims.chatgpt_account_id
    const openaiAuth = claims["https://api.openai.com/auth"]
    if (openaiAuth && openaiAuth.chatgpt_account_id) return openaiAuth.chatgpt_account_id
    if (Array.isArray(claims.organizations) && claims.organizations.length > 0) {
      const first = claims.organizations[0]
      if (first && first.id) return first.id
    }
    return null
  }

  function extractAccountId(ctx, accessToken, idToken) {
    if (ctx.jwt && typeof ctx.jwt.decodePayload === "function") {
      const idClaims = idToken ? ctx.jwt.decodePayload(idToken) : null
      const idAccount = extractAccountIdFromClaims(idClaims)
      if (idAccount) return idAccount
      const accessClaims = accessToken ? ctx.jwt.decodePayload(accessToken) : null
      return extractAccountIdFromClaims(accessClaims)
    }
    return null
  }

  function loadAccountAuth(ctx) {
    const creds = ctx.credentials
    if (!creds || typeof creds !== "object") return null
    if (creds.accessToken || creds.refreshToken || creds.idToken) {
      const accountId = creds.accountId || extractAccountId(ctx, creds.accessToken, creds.idToken)
      return {
        auth: {
          last_refresh: creds.lastRefresh || new Date().toISOString(),
          tokens: {
            access_token: creds.accessToken || "",
            refresh_token: creds.refreshToken || "",
            id_token: creds.idToken || "",
            account_id: accountId || null,
            expires_at: creds.expiresAt || null,
          },
        },
        source: "account",
      }
    }
    if (creds.apiKey) {
      return { auth: { OPENAI_API_KEY: creds.apiKey }, source: "account" }
    }
    return null
  }

  function loadAuth(ctx) {
    return loadAccountAuth(ctx)
  }

  function needsRefresh(ctx, auth, nowMs) {
    if (auth.tokens && auth.tokens.expires_at) {
      const expiresAt = Number(auth.tokens.expires_at) * 1000
      if (Number.isFinite(expiresAt)) {
        const lastMs = auth.last_refresh ? ctx.util.parseDateMs(auth.last_refresh) : null
        const lifetimeMs = lastMs === null ? null : expiresAt - lastMs
        const bufferMs = lifetimeMs !== null && lifetimeMs > DAY_MS
          ? LONG_EXPIRY_REFRESH_BUFFER_MS
          : SHORT_EXPIRY_REFRESH_BUFFER_MS
        if (nowMs + bufferMs >= expiresAt) return true
      }
    }
    if (!auth.last_refresh) return true
    const lastMs = ctx.util.parseDateMs(auth.last_refresh)
    if (lastMs === null) return true
    return nowMs - lastMs > REFRESH_AGE_MS
  }

  function refreshToken(ctx, authState) {
    const auth = authState.auth
    if (!auth.tokens || !auth.tokens.refresh_token) {
      ctx.host.log.warn("refresh skipped: no refresh token")
      return null
    }

    ctx.host.log.info("attempting token refresh")
    try {
      const resp = ctx.util.request({
        method: "POST",
        url: REFRESH_URL,
        headers: { "Content-Type": "application/json" },
        bodyText: JSON.stringify({
          grant_type: "refresh_token",
          client_id: CLIENT_ID,
          refresh_token: auth.tokens.refresh_token,
        }),
        timeoutMs: 15000,
      })

      if (resp.status === 400 || resp.status === 401) {
        let code = null
        const body = ctx.util.tryParseJson(resp.bodyText)
        if (body) {
          code = body.error?.code || body.error || body.code
        }
        ctx.host.log.error("refresh failed: status=" + resp.status + " code=" + String(code))
        if (code === "refresh_token_expired") {
          throw "Session expired. Run `codex` to log in again."
        }
        if (code === "refresh_token_reused") {
          throw "Token conflict. Run `codex` to log in again."
        }
        if (code === "refresh_token_invalidated") {
          throw "Token revoked. Run `codex` to log in again."
        }
        throw "Token expired. Run `codex` to log in again."
      }
      if (resp.status < 200 || resp.status >= 300) {
        ctx.host.log.warn("refresh returned unexpected status: " + resp.status)
        return null
      }

      const body = ctx.util.tryParseJson(resp.bodyText)
      if (!body) {
        ctx.host.log.warn("refresh response not valid JSON")
        return null
      }
      const newAccessToken = body.access_token
      if (!newAccessToken) {
        ctx.host.log.warn("refresh response missing access_token")
        return null
      }

      auth.tokens.access_token = newAccessToken
      if (body.refresh_token) auth.tokens.refresh_token = body.refresh_token
      if (body.id_token) auth.tokens.id_token = body.id_token
      if (typeof body.expires_in === "number") {
        auth.tokens.expires_at = Math.floor(Date.now() / 1000) + Math.max(1, body.expires_in)
      }
      const accountId = extractAccountId(ctx, auth.tokens.access_token, auth.tokens.id_token)
      if (accountId) auth.tokens.account_id = accountId
      auth.last_refresh = new Date().toISOString()

      return newAccessToken
    } catch (e) {
      if (typeof e === "string") throw e
      ctx.host.log.error("refresh exception: " + String(e))
      return null
    }
  }

  function fetchUsage(ctx, accessToken, accountId) {
    const headers = {
      Authorization: "Bearer " + accessToken,
      Accept: "application/json",
      "User-Agent": "openburn",
    }
    if (accountId) {
      headers["ChatGPT-Account-Id"] = accountId
    }
    return ctx.util.request({
      method: "GET",
      url: USAGE_URL,
      headers,
      timeoutMs: 10000,
    })
  }

  function fetchRateLimitResetCredits(ctx, accessToken, accountId) {
    const headers = {
      Authorization: "Bearer " + accessToken,
      Accept: "application/json",
      "User-Agent": "openburn",
    }
    if (accountId) {
      headers["ChatGPT-Account-Id"] = accountId
    }
    return ctx.util.request({
      method: "GET",
      url: RESET_CREDITS_URL,
      headers,
      timeoutMs: 10000,
    })
  }

  function formatDate(isoStr) {
    if (!isoStr) return null
    const d = new Date(isoStr)
    if (isNaN(d.getTime())) return null
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    return months[d.getMonth()] + " " + d.getDate() + ", " + d.getFullYear()
  }

  function buildResetCreditsTooltip(credits) {
    if (!Array.isArray(credits) || credits.length === 0) return null
    const lines = []
    for (let i = 0; i < credits.length; i++) {
      const c = credits[i]
      if (!c || c.status !== "available") continue
      const granted = formatDate(c.granted_at)
      const expires = formatDate(c.expires_at)
      if (granted && expires) {
        lines.push("Given: " + granted + "  Expires: " + expires)
      }
    }
    return lines.length > 0 ? lines.join("\n") : null
  }

  function readPercent(value) {
    const n = Number(value)
    return Number.isFinite(n) ? n : null
  }

  function readNumber(value) {
    const n = Number(value)
    return Number.isFinite(n) ? n : null
  }

  function readCreditsRemaining(resp, data) {
    const credits = data && data.credits && typeof data.credits === "object" ? data.credits : null
    if (credits) {
      const bodyBalance = readNumber(credits.balance)
      if (bodyBalance !== null) return bodyBalance
      if (credits.has_credits === false) return 0
    }

    return readNumber(resp.headers["x-codex-credits-balance"])
  }

  function formatCodexPlan(ctx, planType) {
    const rawPlan = typeof planType === "string" ? planType.trim() : ""
    if (!rawPlan) return null
    if (rawPlan.toLowerCase() === "prolite") return "Pro 5x"
    if (rawPlan.toLowerCase() === "pro") return "Pro 20x"
    return ctx.fmt.planLabel(rawPlan) || null
  }

  function getResetsAtIso(ctx, nowSec, window) {
    if (!window) return null
    if (typeof window.reset_at === "number") {
      return ctx.util.toIso(window.reset_at)
    }
    if (typeof window.reset_after_seconds === "number") {
      return ctx.util.toIso(nowSec + window.reset_after_seconds)
    }
    return null
  }

  // Period durations in milliseconds
  var PERIOD_SESSION_MS = 5 * 60 * 60 * 1000    // 5 hours
  var PERIOD_WEEKLY_MS = 7 * 24 * 60 * 60 * 1000 // 7 days

  function queryTokenUsage(ctx) {
    if (!ctx.host.ccusage || typeof ctx.host.ccusage.query !== "function") {
      return { status: "no_runner", data: null }
    }

    const since = new Date()
    // Inclusive range: today + previous 30 days = 31 calendar days.
    since.setDate(since.getDate() - 30)
    const y = since.getFullYear()
    const m = since.getMonth() + 1
    const d = since.getDate()
    const sinceStr = "" + y + (m < 10 ? "0" : "") + m + (d < 10 ? "0" : "") + d
    const queryOpts = { provider: "codex", since: sinceStr }

    const result = ctx.host.ccusage.query(queryOpts)
    if (!result || typeof result !== "object" || typeof result.status !== "string") {
      return { status: "runner_failed", data: null }
    }
    if (result.status !== "ok") {
      return { status: result.status, data: null }
    }
    if (!result.data || !Array.isArray(result.data.daily)) {
      return { status: "runner_failed", data: null }
    }
    return { status: "ok", data: result.data }
  }

  function fmtTokens(n) {
    const abs = Math.abs(n)
    const sign = n < 0 ? "-" : ""
    const units = [
      { threshold: 1e9, divisor: 1e9, suffix: "B" },
      { threshold: 1e6, divisor: 1e6, suffix: "M" },
      { threshold: 1e3, divisor: 1e3, suffix: "K" },
    ]
    for (let i = 0; i < units.length; i++) {
      const unit = units[i]
      if (abs >= unit.threshold) {
        const scaled = abs / unit.divisor
        const formatted = scaled >= 10
          ? Math.round(scaled).toString()
          : scaled.toFixed(1).replace(/\.0$/, "")
        return sign + formatted + unit.suffix
      }
    }
    return sign + Math.round(abs).toString()
  }

  function dayKeyFromDate(date) {
    const year = date.getFullYear()
    const month = date.getMonth() + 1
    const day = date.getDate()
    return year + "-" + (month < 10 ? "0" : "") + month + "-" + (day < 10 ? "0" : "") + day
  }

  function dayKeyFromUsageDate(rawDate) {
    if (typeof rawDate !== "string") return null
    const value = rawDate.trim()
    if (!value) return null

    const isoMatch = value.match(/^(\d{4})-(\d{2})-(\d{2})$/)
    if (isoMatch) {
      return isoMatch[1] + "-" + isoMatch[2] + "-" + isoMatch[3]
    }

    const isoDatePrefixMatch = value.match(/^(\d{4})-(\d{2})-(\d{2})(?:[Tt\s]|$)/)
    if (isoDatePrefixMatch) {
      return isoDatePrefixMatch[1] + "-" + isoDatePrefixMatch[2] + "-" + isoDatePrefixMatch[3]
    }

    const compactMatch = value.match(/^(\d{4})(\d{2})(\d{2})$/)
    if (compactMatch) {
      return compactMatch[1] + "-" + compactMatch[2] + "-" + compactMatch[3]
    }

    const ms = Date.parse(value)
    if (!Number.isFinite(ms)) return null
    return dayKeyFromDate(new Date(ms))
  }

  function usageCostUsd(day) {
    if (!day || typeof day !== "object") return null

    if (day.totalCost != null) {
      const totalCost = Number(day.totalCost)
      if (Number.isFinite(totalCost)) return totalCost
    }

    if (day.costUSD != null) {
      const costUSD = Number(day.costUSD)
      if (Number.isFinite(costUSD)) return costUSD
    }

    return null
  }

  function costAndTokensLabel(data, opts) {
    const includeZeroTokens = !!(opts && opts.includeZeroTokens)
    const parts = []
    if (data.costUSD != null) parts.push("$" + data.costUSD.toFixed(2))
    if (data.tokens > 0 || (includeZeroTokens && data.tokens === 0)) {
      parts.push(fmtTokens(data.tokens) + " tokens")
    }
    return parts.join(" · ")
  }

  function modelTokenCount(modelUsage) {
    if (!modelUsage || typeof modelUsage !== "object") return 0
    const total = Number(modelUsage.totalTokens)
    if (Number.isFinite(total) && total > 0) return total

    const fields = [
      "inputTokens",
      "cachedInputTokens",
      "cacheCreationTokens",
      "cacheReadTokens",
      "outputTokens",
      "reasoningOutputTokens",
    ]
    let sum = 0
    for (let i = 0; i < fields.length; i++) {
      const n = Number(modelUsage[fields[i]])
      if (Number.isFinite(n) && n > 0) sum += n
    }
    return sum
  }

  function collectModelUsage(daily) {
    const totals = {}
    let totalTokens = 0
    for (let i = 0; i < daily.length; i++) {
      const day = daily[i]
      const models = day && day.models
      if (models && typeof models === "object") {
        const names = Object.keys(models)
        for (let j = 0; j < names.length; j++) {
          const name = names[j]
          const tokens = modelTokenCount(models[name])
          if (tokens <= 0) continue
          totals[name] = (totals[name] || 0) + tokens
          totalTokens += tokens
        }
      }

      const breakdowns = day && day.modelBreakdowns
      if (Array.isArray(breakdowns)) {
        for (let j = 0; j < breakdowns.length; j++) {
          const breakdown = breakdowns[j]
          const name = String(
            (breakdown && (breakdown.modelName || breakdown.name || breakdown.model)) || ""
          ).trim()
          if (!name) continue
          const tokens = modelTokenCount(breakdown)
          if (tokens <= 0) continue
          totals[name] = (totals[name] || 0) + tokens
          totalTokens += tokens
        }
      }
    }

    if (totalTokens <= 0) return []
    return Object.keys(totals)
      .map((name) => ({ name, tokens: totals[name], percent: (totals[name] / totalTokens) * 100 }))
      .sort((a, b) => b.tokens - a.tokens || a.name.localeCompare(b.name))
  }

  function percentLabel(value) {
    if (value > 0 && value < 0.1) return "<0.1%"
    const rounded = Math.round(value * 10) / 10
    return (rounded % 1 === 0 ? String(Math.round(rounded)) : String(rounded)) + "%"
  }

  function pushModelUsageLines(lines, ctx, daily) {
    const models = collectModelUsage(daily)
    for (let i = 0; i < models.length; i++) {
      const model = models[i]
      lines.push(ctx.line.text({
        label: model.name,
        value: percentLabel(model.percent),
      }))
    }
  }

  function usageDayLabel(rawDate) {
    const key = dayKeyFromUsageDate(rawDate)
    if (!key) return String(rawDate || "").slice(0, 10) || "Usage"
    const month = Number(key.slice(5, 7))
    const day = Number(key.slice(8, 10))
    return month + "/" + day
  }

  function collectUsageChartPoints(daily) {
    const points = []
    for (let i = 0; i < daily.length; i++) {
      const day = daily[i]
      const tokens = Number(day && day.totalTokens)
      if (!Number.isFinite(tokens) || tokens < 0) continue
      const key = dayKeyFromUsageDate(day.date)
      if (!key) continue
      points.push({
        key: key,
        label: usageDayLabel(day.date),
        value: tokens,
        valueLabel: fmtTokens(tokens) + " tokens",
      })
    }
    return points
      .sort((a, b) => a.key.localeCompare(b.key))
      .slice(-31)
      .map((point) => ({
        label: point.label,
        value: point.value,
        valueLabel: point.valueLabel,
      }))
  }

  function pushUsageChartLine(lines, ctx, daily) {
    const points = collectUsageChartPoints(daily)
    if (points.length === 0) return
    lines.push(ctx.line.barChart({
      label: "Usage Trend",
      points: points,
      note: "Estimated from local Codex logs for the selected account.",
      color: "#74AA9C",
    }))
  }

  function pushDayUsageLine(lines, ctx, label, dayEntry) {
    const tokens = Number(dayEntry && dayEntry.totalTokens) || 0
    const cost = usageCostUsd(dayEntry)
    if (tokens > 0) {
      lines.push(ctx.line.text({
        label: label,
        value: costAndTokensLabel({ tokens: tokens, costUSD: cost })
      }))
      return
    }

    lines.push(ctx.line.text({
      label: label,
      value: costAndTokensLabel({ tokens: 0, costUSD: 0 }, { includeZeroTokens: true })
    }))
  }

  function probe(ctx) {
    const authState = loadAuth(ctx)
    if (!authState || !authState.auth) {
      ctx.host.log.error("probe failed: not logged in")
      throw "Not logged in. Run `codex` to authenticate."
    }
    const auth = authState.auth

    if (auth.tokens && auth.tokens.access_token) {
      const nowMs = Date.now()
      let accessToken = auth.tokens.access_token
      let accountId = auth.tokens.account_id

      if (needsRefresh(ctx, auth, nowMs)) {
        ctx.host.log.info("token needs refresh")
        const refreshed = refreshToken(ctx, authState)
        if (refreshed) {
          accessToken = refreshed
          accountId = auth.tokens.account_id
        } else {
          ctx.host.log.warn("proactive refresh failed, trying with existing token")
        }
      }

      let resp
      let didRefresh = false
      try {
        resp = ctx.util.retryOnceOnAuth({
          request: (token) => {
            try {
              return fetchUsage(ctx, token || accessToken, accountId)
            } catch (e) {
              ctx.host.log.error("usage request exception: " + String(e))
              if (didRefresh) {
                throw "Usage request failed after refresh. Try again."
              }
              throw "Usage request failed. Check your connection."
            }
          },
          refresh: () => {
            ctx.host.log.info("usage returned 401, attempting refresh")
            didRefresh = true
            const refreshed = refreshToken(ctx, authState)
            accountId = auth.tokens.account_id
            return refreshed
          },
        })
      } catch (e) {
        if (typeof e === "string") throw e
        ctx.host.log.error("usage request failed: " + String(e))
        throw "Usage request failed. Check your connection."
      }

      if (ctx.util.isAuthStatus(resp.status)) {
        ctx.host.log.error("usage returned auth error after all retries: status=" + resp.status)
        throw "Token expired. Run `codex` to log in again."
      }

      if (resp.status < 200 || resp.status >= 300) {
        ctx.host.log.error("usage returned error: status=" + resp.status)
        throw "Usage request failed (HTTP " + String(resp.status) + "). Try again later."
      }

      ctx.host.log.info("usage fetch succeeded")

      const data = ctx.util.tryParseJson(resp.bodyText)
      if (data === null) {
        throw "Usage response invalid. Try again later."
      }

      let resetCreditsDetail = null
      try {
        const creditsResp = fetchRateLimitResetCredits(ctx, accessToken, accountId)
        if (creditsResp.status >= 200 && creditsResp.status < 300) {
          const creditsData = ctx.util.tryParseJson(creditsResp.bodyText)
          if (creditsData && Array.isArray(creditsData.credits)) {
            resetCreditsDetail = creditsData.credits
          }
        }
      } catch (e) {
        ctx.host.log.warn("reset credits fetch failed: " + String(e))
      }

      const lines = []
      const nowSec = Math.floor(Date.now() / 1000)
      const rateLimit = data.rate_limit || null
      const primaryWindow = rateLimit && rateLimit.primary_window ? rateLimit.primary_window : null
      const secondaryWindow = rateLimit && rateLimit.secondary_window ? rateLimit.secondary_window : null
      const reviewWindow =
        data.code_review_rate_limit && data.code_review_rate_limit.primary_window
          ? data.code_review_rate_limit.primary_window
          : null

      const headerPrimary = readPercent(resp.headers["x-codex-primary-used-percent"])
      const headerSecondary = readPercent(resp.headers["x-codex-secondary-used-percent"])

      if (headerPrimary !== null) {
        lines.push(ctx.line.progress({
          label: "Session",
          used: headerPrimary,
          limit: 100,
          format: { kind: "percent" },
          resetsAt: getResetsAtIso(ctx, nowSec, primaryWindow),
          periodDurationMs: PERIOD_SESSION_MS
        }))
      }
      if (headerSecondary !== null) {
        lines.push(ctx.line.progress({
          label: "Weekly",
          used: headerSecondary,
          limit: 100,
          format: { kind: "percent" },
          resetsAt: getResetsAtIso(ctx, nowSec, secondaryWindow),
          periodDurationMs: PERIOD_WEEKLY_MS
        }))
      }

      if (lines.length === 0 && data.rate_limit) {
        if (data.rate_limit.primary_window && typeof data.rate_limit.primary_window.used_percent === "number") {
          lines.push(ctx.line.progress({
            label: "Session",
            used: data.rate_limit.primary_window.used_percent,
            limit: 100,
            format: { kind: "percent" },
            resetsAt: getResetsAtIso(ctx, nowSec, primaryWindow),
            periodDurationMs: PERIOD_SESSION_MS
          }))
        }
        if (data.rate_limit.secondary_window && typeof data.rate_limit.secondary_window.used_percent === "number") {
          lines.push(ctx.line.progress({
            label: "Weekly",
            used: data.rate_limit.secondary_window.used_percent,
            limit: 100,
            format: { kind: "percent" },
            resetsAt: getResetsAtIso(ctx, nowSec, secondaryWindow),
            periodDurationMs: PERIOD_WEEKLY_MS
          }))
        }
      }

      if (Array.isArray(data.additional_rate_limits)) {
        for (const entry of data.additional_rate_limits) {
          if (!entry || !entry.rate_limit) continue
          const name = typeof entry.limit_name === "string" ? entry.limit_name : ""
          let shortName = name.replace(/^GPT-[\d.]+-Codex-/, "")
          if (!shortName) shortName = name || "Model"
          const rl = entry.rate_limit
          if (rl.primary_window && typeof rl.primary_window.used_percent === "number") {
            lines.push(ctx.line.progress({
              label: shortName,
              used: rl.primary_window.used_percent,
              limit: 100,
              format: { kind: "percent" },
              resetsAt: getResetsAtIso(ctx, nowSec, rl.primary_window),
              periodDurationMs: typeof rl.primary_window.limit_window_seconds === "number"
                ? rl.primary_window.limit_window_seconds * 1000
                : PERIOD_SESSION_MS
            }))
          }
          if (rl.secondary_window && typeof rl.secondary_window.used_percent === "number") {
            lines.push(ctx.line.progress({
              label: shortName + " Weekly",
              used: rl.secondary_window.used_percent,
              limit: 100,
              format: { kind: "percent" },
              resetsAt: getResetsAtIso(ctx, nowSec, rl.secondary_window),
              periodDurationMs: typeof rl.secondary_window.limit_window_seconds === "number"
                ? rl.secondary_window.limit_window_seconds * 1000
                : PERIOD_WEEKLY_MS
            }))
          }
        }
      }

      if (reviewWindow) {
        const used = reviewWindow.used_percent
        if (typeof used === "number") {
          lines.push(ctx.line.progress({
            label: "Reviews",
            used: used,
            limit: 100,
            format: { kind: "percent" },
            resetsAt: getResetsAtIso(ctx, nowSec, reviewWindow),
            periodDurationMs: PERIOD_WEEKLY_MS // code_review_rate_limit is a 7-day window
          }))
        }
      }

      const resetCredits =
        data.rate_limit_reset_credits &&
        typeof data.rate_limit_reset_credits === "object" &&
        data.rate_limit_reset_credits.available_count != null
          ? readNumber(data.rate_limit_reset_credits.available_count)
          : null
      if (resetCredits !== null && resetCredits >= 0) {
        const tooltip = buildResetCreditsTooltip(resetCreditsDetail)
        const lineOpts = {
          label: "Rate Limit Resets",
          value: Math.floor(resetCredits) + " available",
        }
        if (tooltip) lineOpts.tooltip = tooltip
        lines.push(ctx.line.text(lineOpts))
      }

      const creditsRemaining = readCreditsRemaining(resp, data)
      if (creditsRemaining !== null) {
        const remaining = Math.max(0, Math.floor(creditsRemaining))
        const usdValue = (remaining * CREDIT_USD_RATE).toFixed(2)
        lines.push(ctx.line.text({
          label: "Credits",
          value: "$" + usdValue + " · " + remaining + " credits",
        }))
      }

      let plan = null
      if (data.plan_type) {
        const planLabel = formatCodexPlan(ctx, data.plan_type)
        if (planLabel) {
          plan = planLabel
        }
      }

      const tokenUsageResult = queryTokenUsage(ctx)
      if (tokenUsageResult.status === "ok") {
        const tokenUsage = tokenUsageResult.data
        const now = new Date()
        const todayKey = dayKeyFromDate(now)
        const yesterday = new Date(now.getTime())
        yesterday.setDate(yesterday.getDate() - 1)
        const yesterdayKey = dayKeyFromDate(yesterday)

        let todayEntry = null
        let yesterdayEntry = null
        for (let i = 0; i < tokenUsage.daily.length; i++) {
          const usageDayKey = dayKeyFromUsageDate(tokenUsage.daily[i].date)
          if (usageDayKey === todayKey) {
            todayEntry = tokenUsage.daily[i]
            continue
          }
          if (usageDayKey === yesterdayKey) {
            yesterdayEntry = tokenUsage.daily[i]
          }
        }

        pushDayUsageLine(lines, ctx, "Today", todayEntry)
        pushDayUsageLine(lines, ctx, "Yesterday", yesterdayEntry)

        let totalTokens = 0
        let totalCostNanos = 0
        let hasCost = false
        for (let i = 0; i < tokenUsage.daily.length; i++) {
          const day = tokenUsage.daily[i]
          const dayTokens = Number(day.totalTokens)
          if (Number.isFinite(dayTokens)) {
            totalTokens += dayTokens
          }

          const dayCost = usageCostUsd(day)
          if (dayCost != null) {
            totalCostNanos += Math.round(dayCost * 1e9)
            hasCost = true
          }
        }

        if (totalTokens > 0) {
          lines.push(ctx.line.text({
            label: "Last 30 Days",
            value: costAndTokensLabel({ tokens: totalTokens, costUSD: hasCost ? totalCostNanos / 1e9 : null })
          }))
        }

        pushUsageChartLine(lines, ctx, tokenUsage.daily)
        pushModelUsageLines(lines, ctx, tokenUsage.daily)
      }

      if (lines.length === 0) {
        lines.push(ctx.line.badge({ label: "Status", text: "No usage data", color: "#a3a3a3" }))
      }

      const result = { plan: plan, lines: lines }
      result.updatedCredentialsJson = JSON.stringify({
        type: "oauth",
        accessToken: auth.tokens.access_token || "",
        refreshToken: auth.tokens.refresh_token || "",
        idToken: auth.tokens.id_token || "",
        accountId: auth.tokens.account_id || null,
        expiresAt: auth.tokens.expires_at || null,
        lastRefresh: auth.last_refresh || new Date().toISOString(),
      })
      return result
    }

    if (auth.OPENAI_API_KEY) {
      throw "Usage not available for API key."
    }

    throw "Not logged in. Run `codex` to authenticate."
  }

  globalThis.__openusage_plugin = { id: "codex", probe }
})()
