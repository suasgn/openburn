import { ProviderCard } from "@/components/provider-card"
import type { ProviderDisplayState } from "@/lib/provider-types"
import type { DisplayMode } from "@/lib/settings"

interface OverviewPageProps {
  providers: ProviderDisplayState[]
  onRetryProvider?: (providerId: string) => void
  displayMode: DisplayMode
}

export function OverviewPage({ providers, onRetryProvider, displayMode }: OverviewPageProps) {
  if (providers.length === 0) {
    return (
      <div className="text-center text-muted-foreground py-8">
        No providers enabled
      </div>
    )
  }

  return (
    <div>
      {providers.map((provider, index) => (
        <ProviderCard
          key={provider.meta.id}
          name={provider.meta.name}
          plan={provider.data?.plan}
          showSeparator={index < providers.length - 1}
          loading={provider.loading}
          error={provider.error}
          lines={provider.data?.lines ?? []}
          skeletonLines={provider.meta.lines}
          lastManualRefreshAt={provider.lastManualRefreshAt}
          onRetry={onRetryProvider ? () => onRetryProvider(provider.meta.id) : undefined}
          scopeFilter="overview"
          displayMode={displayMode}
        />
      ))}
    </div>
  )
}
