import { ProviderCard } from "@/components/provider-card"
import type { ProviderDisplayState } from "@/lib/provider-types"
import type { DisplayMode } from "@/lib/settings"

interface ProviderDetailPageProps {
  provider: ProviderDisplayState | null
  accountOrder?: string[]
  onRetry?: () => void
  displayMode: DisplayMode
}

export function ProviderDetailPage({
  provider,
  accountOrder = [],
  onRetry,
  displayMode,
}: ProviderDetailPageProps) {
  if (!provider) {
    return (
      <div className="text-center text-muted-foreground py-8">
        Provider not found
      </div>
    )
  }

  return (
    <ProviderCard
      name={provider.meta.name}
      plan={provider.data?.plan}
      showSeparator={false}
      loading={provider.loading}
      error={provider.error}
      lines={provider.data?.lines ?? []}
      skeletonLines={provider.meta.lines}
      accountOrder={accountOrder}
      onRetry={onRetry}
      scopeFilter="all"
      displayMode={displayMode}
    />
  )
}
