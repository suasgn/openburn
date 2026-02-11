import { AlertTriangle, UserRound } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

type ProviderErrorProps = {
  message: string
  hasCachedData?: boolean
  contextLabel?: string
  contextAccountId?: string | null
}

function formatMessage(message: string) {
  const parts = message.split(/`([^`]+)`/)
  return parts.map((part, index) =>
    index % 2 === 1 ? (
      <code
        key={`code-${index}`}
        className="rounded bg-muted px-1 font-mono text-[0.75rem] leading-tight"
      >
        {part}
      </code>
    ) : (
      part
    )
  )
}

function classifyError(message: string): { title: string; hint: string } {
  const normalized = message.toLowerCase()

  if (/no credentials configured/.test(normalized)) {
    return {
      title: "Credentials missing",
      hint: "Connect this account in Settings, then refresh.",
    }
  }

  if (/no .* account configured/.test(normalized)) {
    return {
      title: "Account not configured",
      hint: "Add an account for this provider in Settings.",
    }
  }

  if (/unauthorized|forbidden|http\s+401|http\s+403/.test(normalized)) {
    return {
      title: "Authentication failed",
      hint: "Reconnect this account in Settings, then refresh.",
    }
  }

  if (/rate\s*limit|http\s+429/.test(normalized)) {
    return {
      title: "Rate limited",
      hint: "Try again in a moment.",
    }
  }

  if (/timed out|timeout/.test(normalized)) {
    return {
      title: "Request timed out",
      hint: "The provider took too long to respond. Try again.",
    }
  }

  if (/http\s+5\d\d|service unavailable|bad gateway|provider unavailable/.test(normalized)) {
    return {
      title: "Provider unavailable",
      hint: "The service may be temporarily down.",
    }
  }

  return {
    title: "Couldn\'t refresh usage",
    hint: "Try refreshing again in a moment.",
  }
}

export function ProviderError({
  message,
  hasCachedData = false,
  contextLabel,
  contextAccountId,
}: ProviderErrorProps) {
  const details = classifyError(message)
  const accountContext = (
    <span className="inline-flex min-w-0 max-w-[14rem] items-center gap-1.5 rounded-md border border-destructive/25 bg-destructive/5 px-1.5 py-0.5 text-[11px] text-muted-foreground">
      <UserRound className="h-3 w-3 shrink-0 text-destructive/75" />
      <span className="truncate">{contextLabel}</span>
    </span>
  )

  return (
    <div className="rounded-lg border border-destructive/25 bg-gradient-to-br from-destructive/10 via-destructive/5 to-transparent p-3">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 inline-flex h-6 w-6 flex-none items-center justify-center rounded-full border border-destructive/30 bg-destructive/10">
          <AlertTriangle className="h-3.5 w-3.5 text-destructive" />
        </span>
        <div className="min-w-0 space-y-2">
          <p className="text-sm font-medium leading-5 text-foreground">{details.title}</p>
          {contextLabel ? (
            <div className="min-w-0">
              {contextAccountId ? (
                <Tooltip>
                  <TooltipTrigger
                    render={(props) => (
                      <span {...props} className="inline-flex cursor-help">
                        {accountContext}
                      </span>
                    )}
                  />
                  <TooltipContent side="top" className="text-xs">
                    Account ID: <code className="font-mono text-[11px]">{contextAccountId}</code>
                  </TooltipContent>
                </Tooltip>
              ) : accountContext}
            </div>
          ) : null}
          <p className="text-xs leading-relaxed text-muted-foreground">
            {details.hint}
            {hasCachedData ? " Showing your last successful snapshot." : ""}
          </p>
          <p className="text-[11px] leading-relaxed text-muted-foreground/90 break-words">
            {formatMessage(message)}
          </p>
        </div>
      </div>
    </div>
  )
}
