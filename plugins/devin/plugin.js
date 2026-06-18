(function () {
  var CLOUD_SERVICE = "exa.seat_management_pb.SeatManagementService"
  var DEFAULT_API_SERVER_URL = "https://server.codeium.com"
  var CLOUD_COMPAT_VERSION = "1.108.2"
  var LOGIN_HINT = "Add your Devin API key in Settings."
  var QUOTA_HINT = "Devin quota data unavailable. Try again later."
  var DAY_MS = 24 * 60 * 60 * 1000
  var WEEK_MS = 7 * DAY_MS

  function readFiniteNumber(value) {
    if (typeof value === "number") return Number.isFinite(value) ? value : null
    if (typeof value !== "string") return null
    var trimmed = value.trim()
    if (!trimmed) return null
    var parsed = Number(trimmed)
    return Number.isFinite(parsed) ? parsed : null
  }

  function readString(value) {
    if (typeof value !== "string") return null
    var trimmed = value.trim()
    return trimmed ? trimmed : null
  }

  function clampPercent(value) {
    if (!Number.isFinite(value)) return 0
    if (value < 0) return 0
    if (value > 100) return 100
    return value
  }

  function cleanApiServerUrl(value) {
    if (typeof value !== "string") return null
    var trimmed = value.trim().replace(/\/+$/, "")
    if (!trimmed) return null
    if (!/^https:\/\//.test(trimmed)) return null
    return trimmed
  }

  function effectiveApiServerUrl(auth) {
    return (auth && auth.apiServerUrl) || DEFAULT_API_SERVER_URL
  }

  function hasOwn(obj, key) {
    return Boolean(obj && Object.prototype.hasOwnProperty.call(obj, key))
  }

  function readHost(value) {
    if (typeof value !== "string") return null
    var match = /^https?:\/\/([^/]+)/.exec(value.trim())
    return match ? match[1] : null
  }

  function valueOrMissing(value) {
    return value === null || value === undefined || value === "" ? "missing" : String(value)
  }

  function logQuotaDiagnostics(ctx, auth, userStatus) {
    var planStatus = (userStatus && userStatus.planStatus) || {}
    var planInfo = planStatus.planInfo || {}
    var devinInfo = planInfo.devinInfo || {}
    var apiServerHost = readHost(auth.apiServerUrl || DEFAULT_API_SERVER_URL)
    var webappHost = readHost(devinInfo.webappHost) || devinInfo.webappHost || null
    var devinApiHost = readHost(devinInfo.apiUrl)

    ctx.host.log.info(
      "Devin quota diagnostics" +
        " apiServerHost=" + valueOrMissing(apiServerHost) +
        " planName=" + valueOrMissing(planInfo.planName) +
        " teamsTier=" + valueOrMissing(userStatus && userStatus.teamsTier) +
        " planTeamsTier=" + valueOrMissing(planInfo.teamsTier) +
        " billingStrategy=" + valueOrMissing(planInfo.billingStrategy) +
        " isDevin=" + String(planInfo.isDevin === true) +
        " hideDailyQuota=" + String(planInfo.hideDailyQuota === true) +
        " hasDailyQuotaPercent=" + String(hasOwn(planStatus, "dailyQuotaRemainingPercent")) +
        " hasWeeklyQuotaPercent=" + String(hasOwn(planStatus, "weeklyQuotaRemainingPercent")) +
        " hasOverageBalance=" + String(hasOwn(planStatus, "overageBalanceMicros")) +
        " hasDailyReset=" + String(hasOwn(planStatus, "dailyQuotaResetAtUnix")) +
        " hasWeeklyReset=" + String(hasOwn(planStatus, "weeklyQuotaResetAtUnix")) +
        " hasTopUpStatus=" + String(hasOwn(planStatus, "topUpStatus")) +
        " availablePromptCredits=" + valueOrMissing(planStatus.availablePromptCredits) +
        " canUseCli=" + String(devinInfo.canUseCli === true) +
        " canUseCascade=" + String(devinInfo.canUseCascade === true) +
        " devinReviewEnabled=" + String(devinInfo.devinReviewEnabled === true) +
        " webappHost=" + valueOrMissing(webappHost) +
        " devinApiHost=" + valueOrMissing(devinApiHost)
    )
  }

  function loadCredentials(ctx) {
    var credentials = ctx.credentials && typeof ctx.credentials === "object" ? ctx.credentials : null
    var apiKey = readString(credentials && credentials.apiKey)
    if (!apiKey) return null
    return {
      apiKey: apiKey,
      apiServerUrl: cleanApiServerUrl(readString(credentials && credentials.apiServerUrl)),
      source: "account",
    }
  }

  function callCloud(ctx, auth) {
    var apiServerUrl = effectiveApiServerUrl(auth)
    try {
      var resp = ctx.host.http.request({
        method: "POST",
        url: apiServerUrl + "/" + CLOUD_SERVICE + "/GetUserStatus",
        headers: {
          "Content-Type": "application/json",
          "Connect-Protocol-Version": "1",
        },
        bodyText: JSON.stringify({
          metadata: {
            apiKey: auth.apiKey,
            ideName: "devin",
            ideVersion: CLOUD_COMPAT_VERSION,
            extensionName: "devin",
            extensionVersion: CLOUD_COMPAT_VERSION,
            locale: "en",
          },
        }),
        timeoutMs: 15000,
      })
      if (resp.status < 200 || resp.status >= 300) {
        ctx.host.log.warn("cloud request returned status " + resp.status)
        if (ctx.util && typeof ctx.util.isAuthStatus === "function" && ctx.util.isAuthStatus(resp.status)) {
          return { __openusageAuthError: true }
        }
        return null
      }
      return ctx.util.tryParseJson(resp.bodyText)
    } catch (e) {
      ctx.host.log.warn("cloud request failed: " + String(e))
      return null
    }
  }

  function tryAuth(ctx, auth) {
    var data = callCloud(ctx, auth)
    if (data && data.__openusageAuthError) {
      return { authFailure: true }
    }
    if (!data || !data.userStatus) return {}

    try {
      logQuotaDiagnostics(ctx, auth, data.userStatus)
      return { output: buildOutput(ctx, data.userStatus) }
    } catch (e) {
      if (e === QUOTA_HINT) {
        ctx.host.log.warn("quota contract unavailable")
        return {}
      }
      throw e
    }
  }

  function unixSecondsToIso(ctx, value) {
    var seconds = readFiniteNumber(value)
    if (seconds === null) return null
    return ctx.util.toIso(seconds * 1000)
  }

  function formatDollarsFromMicros(value) {
    var micros = readFiniteNumber(value)
    if (micros === null) return null
    if (!Number.isFinite(micros)) return null
    if (micros < 0) micros = 0
    return "$" + (micros / 1000000).toFixed(2)
  }

  function buildQuotaLine(ctx, label, remaining, resetsAt, periodDurationMs) {
    if (remaining === null) return null
    return buildUsedQuotaLine(ctx, label, 100 - remaining, resetsAt, periodDurationMs)
  }

  function buildUsedQuotaLine(ctx, label, used, resetsAt, periodDurationMs) {
    if (used === null) return null
    var line = {
      label: label,
      used: clampPercent(used),
      limit: 100,
      format: { kind: "percent" },
      periodDurationMs: periodDurationMs,
    }
    if (resetsAt) line.resetsAt = resetsAt
    return ctx.line.progress(line)
  }

  function buildOutput(ctx, userStatus) {
    var planStatus = (userStatus && userStatus.planStatus) || {}

    var planInfo = planStatus.planInfo || {}
    var planName = typeof planInfo.planName === "string" && planInfo.planName.trim()
      ? planInfo.planName.trim()
      : "Unknown"

    var hideDailyQuota = planInfo.hideDailyQuota === true
    var dailyRemaining = readFiniteNumber(planStatus.dailyQuotaRemainingPercent)
    var weeklyRemaining = readFiniteNumber(planStatus.weeklyQuotaRemainingPercent)
    var dailyReset = !hideDailyQuota ? unixSecondsToIso(ctx, planStatus.dailyQuotaResetAtUnix) : null
    var weeklyReset = unixSecondsToIso(ctx, planStatus.weeklyQuotaResetAtUnix)
    var extraUsageBalance = formatDollarsFromMicros(planStatus.overageBalanceMicros)

    var dailyLine = !hideDailyQuota
      ? buildQuotaLine(ctx, "Daily quota", dailyRemaining, dailyReset, DAY_MS)
      : null
    var weeklyLine = weeklyRemaining !== null
      ? buildQuotaLine(ctx, "Weekly quota", weeklyRemaining, weeklyReset, WEEK_MS)
      : hideDailyQuota
        ? buildUsedQuotaLine(ctx, "Weekly quota", dailyRemaining, weeklyReset, WEEK_MS)
        : null

    var lines = []
    if (dailyLine) lines.push(dailyLine)
    if (weeklyLine) lines.push(weeklyLine)
    if (extraUsageBalance) {
      lines.push(ctx.line.text({ label: "Extra usage balance", value: extraUsageBalance }))
    }

    if (!lines.length) throw QUOTA_HINT

    return {
      plan: planName,
      lines: lines,
    }
  }

  function probe(ctx) {
    var auth = loadCredentials(ctx)
    if (!auth) throw LOGIN_HINT

    var attempt = tryAuth(ctx, auth)
    if (attempt.output) return attempt.output
    if (attempt.authFailure) throw LOGIN_HINT
    throw QUOTA_HINT
  }

  globalThis.__openusage_plugin = { id: "devin", probe: probe }
})()
