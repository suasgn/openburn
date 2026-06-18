# Devin

> Reverse-engineered from Devin CLI/app auth. May change without notice.

## Overview

- **Vendor:** Cognition / Devin
- **Protocol:** Connect RPC v1, JSON over HTTPS
- **Service:** `exa.seat_management_pb.SeatManagementService`
- **Auth:** Devin session token (API key)
- **Quota:** weekly quota percentage
- **Extra:** overage balance in micros
- **Requires:** Devin API key added via Settings

OpenBurn does not use `api.devin.ai` for this provider. Devin's public API usage and consumption endpoints are enterprise/admin APIs and do not expose the same local account quota shown in the app.

## Auth

Add your Devin API key in Settings. The optional API Server URL field defaults to `https://server.codeium.com` when left blank.

## GetUserStatus

### Request

```
POST https://server.codeium.com/exa.seat_management_pb.SeatManagementService/GetUserStatus
Content-Type: application/json
Connect-Protocol-Version: 1
```

```json
{
  "metadata": {
    "apiKey": "devin-session-token$...",
    "ideName": "devin",
    "ideVersion": "1.108.2",
    "extensionName": "devin",
    "extensionVersion": "1.108.2",
    "locale": "en"
  }
}
```

### Response

```jsonc
{
  "userStatus": {
    "planStatus": {
      "planInfo": {
        "planName": "Max",
        "billingStrategy": "BILLING_STRATEGY_QUOTA",
        "hideDailyQuota": false        // when true, daily % maps to weekly line
      },
      "dailyQuotaRemainingPercent": 100,   // 0-100
      "weeklyQuotaRemainingPercent": 40,   // 0-100
      "overageBalanceMicros": "964220000", // USD micros (964.22)
      "dailyQuotaResetAtUnix": "1774080000",
      "weeklyQuotaResetAtUnix": "1774166400"
    }
  }
}
```

Remaining percent is inverted to used percent for display. Overage balance is formatted as USD from micros.
