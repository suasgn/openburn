import { Checkbox } from "@/components/ui/checkbox";
import { Button } from "@/components/ui/button";
import {
  ProviderAccountsSection,
  type AccountOAuthSession,
  type ProviderAuthStrategyOption,
  type ProviderAccountSummary,
} from "@/components/provider-accounts-section";
import {
  AUTO_UPDATE_OPTIONS,
  DISPLAY_MODE_OPTIONS,
  TRAY_ICON_STYLE_OPTIONS,
  THEME_OPTIONS,
  isTrayPercentageMandatory,
  type AutoUpdateIntervalMinutes,
  type DisplayMode,
  type ThemeMode,
  type TrayIconStyle,
} from "@/lib/settings";
import { cn } from "@/lib/utils";

interface ProviderConfig {
  id: string;
  name: string;
  enabled: boolean;
}

const PREVIEW_BAR_TRACK_PX = 20;
function getPreviewMinVisibleRemainderPx(trackW: number): number {
  return Math.max(4, Math.round(trackW * 0.2));
}

function getPreviewVisualBarFraction(fraction: number): number {
  const clamped = Math.max(0, Math.min(1, fraction));
  if (clamped > 0.7 && clamped < 1) {
    const remainder = 1 - clamped;
    const quantizedRemainder = Math.min(1, Math.ceil(remainder / 0.15) * 0.15);
    return Math.max(0, 1 - quantizedRemainder);
  }
  return clamped;
}

function getPreviewBarLayout(fraction: number): { fillPercent: number; remainderPercent: number } {
  if (!Number.isFinite(fraction) || fraction <= 0) return { fillPercent: 0, remainderPercent: 0 };
  const visual = getPreviewVisualBarFraction(fraction);
  if (visual >= 1) return { fillPercent: 100, remainderPercent: 0 };

  const minFillW = 1;
  const minVisibleRemainderPx = getPreviewMinVisibleRemainderPx(PREVIEW_BAR_TRACK_PX);
  const maxFillW = Math.max(minFillW, PREVIEW_BAR_TRACK_PX - minVisibleRemainderPx);
  const fillW = Math.max(minFillW, Math.min(maxFillW, Math.round(PREVIEW_BAR_TRACK_PX * visual)));
  const trueRemainderW = PREVIEW_BAR_TRACK_PX - fillW;
  const remainderDrawW = Math.min(
    PREVIEW_BAR_TRACK_PX - 1,
    Math.max(trueRemainderW, minVisibleRemainderPx)
  );
  return {
    fillPercent: (fillW / PREVIEW_BAR_TRACK_PX) * 100,
    remainderPercent: (remainderDrawW / PREVIEW_BAR_TRACK_PX) * 100,
  };
}

function TrayIconStylePreview({
  style,
  isActive,
  appIconUrl,
}: {
  style: TrayIconStyle;
  isActive: boolean;
  appIconUrl?: string;
}) {
  const trackClass = isActive ? "bg-primary-foreground/30" : "bg-foreground/15";
  const remainderClass = isActive ? "bg-primary-foreground/55" : "bg-foreground/25";
  const fillClass = isActive ? "bg-primary-foreground" : "bg-foreground";
  const textClass = isActive ? "text-primary-foreground" : "text-foreground";

  if (style === "bars") {
    const fractions = [0.83, 0.7, 0.56];
    return (
      <div className="flex items-center">
        <div className="flex flex-col gap-0.5 w-5">
          {fractions.map((fraction, i) => {
            const { fillPercent, remainderPercent } = getPreviewBarLayout(fraction);
            return (
              <div key={i} className={`relative h-1 rounded-sm ${trackClass}`}>
                {remainderPercent > 0 && (
                  <span
                    aria-hidden
                    className={remainderClass}
                    style={{
                      position: "absolute",
                      right: 0,
                      top: 0,
                      bottom: 0,
                      width: `${remainderPercent}%`,
                      borderRadius: "1px 2px 2px 1px",
                    }}
                  />
                )}
                <div
                  className={`h-1 ${fillClass}`}
                  style={{ width: `${fillPercent}%`, borderRadius: "2px 1px 1px 2px" }}
                />
              </div>
            );
          })}
        </div>
      </div>
    );
  }

  if (style === "circle") {
    return (
      <div className="flex items-center">
        <svg viewBox="0 0 26 26" aria-hidden className="shrink-0 size-5">
          <circle
            cx="13"
            cy="13"
            r="9"
            fill="none"
            stroke="currentColor"
            strokeWidth="4"
            opacity={isActive ? 0.35 : 0.2}
            className={textClass}
          />
          <circle
            cx="13"
            cy="13"
            r="9"
            fill="none"
            stroke="currentColor"
            strokeWidth="4"
            strokeLinecap="butt"
            pathLength="100"
            strokeDasharray="83 100"
            transform="rotate(-90 13 13)"
            className={textClass}
          />
        </svg>
      </div>
    );
  }

  if (style === "app") {
    if (appIconUrl) {
      return (
        <div
          aria-hidden
          className={cn(
            "w-[18px] h-[18px] shrink-0",
            isActive ? "bg-primary-foreground" : "bg-foreground"
          )}
          style={{
            WebkitMaskImage: `url(${appIconUrl})`,
            WebkitMaskSize: "contain",
            WebkitMaskRepeat: "no-repeat",
            WebkitMaskPosition: "center",
            maskImage: `url(${appIconUrl})`,
            maskSize: "contain",
            maskRepeat: "no-repeat",
            maskPosition: "center",
          }}
        />
      );
    }
    // Fallback: app icon mark
    return (
      <svg
        aria-hidden
        viewBox="0 0 100 100"
        className={cn("shrink-0 size-[18px]", textClass)}
      >
        <path
          d="M25.7146 63.2153L41.4393 54.3917L41.7025 53.6226L41.4393 53.1976H40.6705L38.0394 53.0359L29.054 52.7929L21.2624 52.4691L13.7134 52.0644L11.8111 51.6594L10.0303 49.3118L10.2123 48.138L11.8111 47.0657L14.0981 47.2681L19.1574 47.6119L26.7467 48.138L32.2516 48.4618L40.4073 49.3118H41.7025L41.8846 48.7857L41.4393 48.4618L41.0955 48.138L33.243 42.8155L24.7432 37.1894L20.2909 33.9513L17.8824 32.3119L16.6684 30.774L16.1422 27.4147L18.328 25.0062L21.2624 25.2088L22.0112 25.4112L24.9861 27.6979L31.3407 32.616L39.6381 38.7273L40.8525 39.7391L41.3381 39.395L41.399 39.1523L40.8525 38.2415L36.3394 30.0858L31.5227 21.7883L29.3775 18.3478L28.811 16.2837C28.6087 15.4334 28.4669 14.7252 28.4669 13.8549L30.9563 10.4753L32.3321 10.0303L35.6515 10.4756L37.0479 11.6897L39.112 16.4052L42.4513 23.8327L47.6321 33.9313L49.15 36.9265L49.9594 39.6991L50.2632 40.5491H50.7894V40.0632L51.2141 34.3766L52.0035 27.3944L52.7726 18.4087L53.0358 15.8793L54.2905 12.8435L56.7795 11.2041L58.7224 12.135L60.3212 14.422L60.0986 15.899L59.1474 22.0718L57.2857 31.7458L56.0713 38.2218H56.7795L57.5892 37.4121L60.8677 33.061L66.3723 26.18L68.801 23.448L71.6342 20.4325L73.4556 18.9957H76.8962L79.4255 22.7601L78.2926 26.6456L74.7509 31.1384L71.8163 34.943L67.607 40.6097L64.9758 45.1431L65.2188 45.5072L65.8464 45.4466L75.358 43.4228L80.4984 42.4917L86.6304 41.4393L89.4033 42.7346L89.7065 44.0502L88.6135 46.7419L82.0566 48.3607L74.3662 49.8989L62.9118 52.6109L62.77 52.7121L62.9321 52.9144L68.0925 53.4L70.2987 53.5214H75.7021L85.7601 54.2702L88.3912 56.0108L89.9697 58.1358L89.7065 59.7545L85.6589 61.8189L80.1949 60.5236L67.4452 57.4881L63.0735 56.3952H62.4665V56.7596L66.1093 60.3213L72.7877 66.3523L81.1461 74.1236L81.5707 76.0462L80.4984 77.5638L79.3649 77.4021L72.0186 71.8772L69.1854 69.3879L62.77 63.9844H62.3453V64.5509L63.8223 66.7164L71.6342 78.4544L72.0389 82.0567L71.4725 83.2308L69.4487 83.939L67.2222 83.534L62.6485 77.1189L57.9333 69.8937L54.1284 63.4177L53.6631 63.6809L51.4167 87.8651L50.3644 89.0995L47.9356 90.0303L45.9121 88.4924L44.8392 86.0031L45.9118 81.0852L47.2071 74.6701L48.2594 69.5699L49.2106 63.2356L49.7773 61.131L49.7367 60.9892L49.2715 61.0498L44.4954 67.607L37.23 77.4224L31.4825 83.5746L30.1063 84.1211L27.7181 82.8864L27.9408 80.6805L29.2763 78.7177L37.2297 68.5988L42.026 62.3248L45.1227 58.7025L45.1024 58.176H44.9204L23.7917 71.8975L20.0274 72.3831L18.4083 70.8655L18.6106 68.3761L19.3798 67.5664L25.7343 63.195L25.7146 63.2153Z"
          fill="currentColor"
        />
      </svg>
    );
  }

  return (
    <span className={cn("text-[13px] font-bold tabular-nums leading-none", textClass)}>
      %
    </span>
  );
}

interface SettingsPageProps {
  providers: ProviderConfig[];
  accountsByProvider: Record<string, ProviderAccountSummary[]>;
  providerAuthStrategiesByProvider: Record<string, ProviderAuthStrategyOption[]>;
  accountsLoading: boolean;
  onReorderAccounts: (providerId: string, orderedAccountIds: string[]) => void;
  onToggleProvider: (id: string) => void;
  onReloadAccounts: () => Promise<void>;
  onCreateAccount: (providerId: string, authStrategyId: string) => Promise<void>;
  onUpdateAccountLabel: (
    providerId: string,
    accountId: string,
    label: string
  ) => Promise<void>;
  onDeleteAccount: (providerId: string, accountId: string) => Promise<void>;
  onSaveAccountCredentials: (
    providerId: string,
    accountId: string,
    credentials: Record<string, unknown>
  ) => Promise<void>;
  onClearAccountCredentials: (providerId: string, accountId: string) => Promise<void>;
  accountOAuthSessionById: Record<string, AccountOAuthSession | undefined>;
  onStartAccountOAuth: (providerId: string, accountId: string) => Promise<void>;
  onCancelAccountOAuth: (providerId: string, accountId: string) => Promise<void>;
  autoUpdateInterval: AutoUpdateIntervalMinutes;
  onAutoUpdateIntervalChange: (value: AutoUpdateIntervalMinutes) => void;
  themeMode: ThemeMode;
  onThemeModeChange: (value: ThemeMode) => void;
  displayMode: DisplayMode;
  onDisplayModeChange: (value: DisplayMode) => void;
  trayIconStyle: TrayIconStyle;
  onTrayIconStyleChange: (value: TrayIconStyle) => void;
  trayShowPercentage: boolean;
  onTrayShowPercentageChange: (value: boolean) => void;
}

export function SettingsPage({
  providers,
  accountsByProvider,
  providerAuthStrategiesByProvider,
  accountsLoading,
  onReorderAccounts,
  onToggleProvider,
  onReloadAccounts,
  onCreateAccount,
  onUpdateAccountLabel,
  onDeleteAccount,
  onSaveAccountCredentials,
  onClearAccountCredentials,
  accountOAuthSessionById,
  onStartAccountOAuth,
  onCancelAccountOAuth,
  autoUpdateInterval,
  onAutoUpdateIntervalChange,
  themeMode,
  onThemeModeChange,
  displayMode,
  onDisplayModeChange,
  trayIconStyle,
  onTrayIconStyleChange,
  trayShowPercentage,
  onTrayShowPercentageChange,
}: SettingsPageProps) {
  const percentageMandatory = isTrayPercentageMandatory(trayIconStyle);
  const trayShowPercentageChecked = percentageMandatory
    ? true
    : trayShowPercentage;

  return (
    <div className="py-3 space-y-4">
      <section>
        <h3 className="text-lg font-semibold mb-0">Auto Refresh</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Configure automatic refresh interval.
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <div className="flex gap-1" role="radiogroup" aria-label="Auto-update interval">
            {AUTO_UPDATE_OPTIONS.map((option) => {
              const isActive = option.value === autoUpdateInterval;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onAutoUpdateIntervalChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </div>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Usage Mode</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Display usage as remaining or consumed.
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <div className="flex gap-1" role="radiogroup" aria-label="Usage display mode">
            {DISPLAY_MODE_OPTIONS.map((option) => {
              const isActive = option.value === displayMode;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onDisplayModeChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </div>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Bar Icon</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Configure menu bar icon and percentage.
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <div className="flex gap-1" role="radiogroup" aria-label="Tray icon style">
            {TRAY_ICON_STYLE_OPTIONS.map((option) => {
              const isActive = option.value === trayIconStyle;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-label={option.label}
                  aria-checked={isActive}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onTrayIconStyleChange(option.value)}
                >
                  <TrayIconStylePreview
                    style={option.value}
                    isActive={isActive}
                    appIconUrl={option.value === "app" ? "/favicon.svg" : undefined}
                  />
                </Button>
              );
            })}
          </div>
        </div>
        <label
          className={cn(
            "mt-2 flex items-center gap-2 text-sm select-none",
            percentageMandatory
              ? "text-muted-foreground cursor-not-allowed"
              : "text-foreground"
          )}
        >
          <Checkbox
            key={`tray-pct-${trayShowPercentageChecked}-${percentageMandatory}`}
            checked={trayShowPercentageChecked}
            disabled={percentageMandatory}
            onCheckedChange={(checked) => {
              if (percentageMandatory) return;
              onTrayShowPercentageChange(checked === true);
            }}
          />
          Show percentage
        </label>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">App Theme</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Set application appearance.
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <div className="flex gap-1" role="radiogroup" aria-label="Theme mode">
            {THEME_OPTIONS.map((option) => {
              const isActive = option.value === themeMode;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onThemeModeChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </div>
        </div>
      </section>
      <ProviderAccountsSection
        providers={providers}
        accountsByProvider={accountsByProvider}
        providerAuthStrategiesByProvider={providerAuthStrategiesByProvider}
        loading={accountsLoading}
        onReorderAccounts={onReorderAccounts}
        onReloadAccounts={onReloadAccounts}
        onToggleProvider={onToggleProvider}
        onCreateAccount={onCreateAccount}
        onUpdateAccountLabel={onUpdateAccountLabel}
        onDeleteAccount={onDeleteAccount}
        onSaveAccountCredentials={onSaveAccountCredentials}
        onClearAccountCredentials={onClearAccountCredentials}
        oauthSessionByAccount={accountOAuthSessionById}
        onStartAccountOAuth={onStartAccountOAuth}
        onCancelAccountOAuth={onCancelAccountOAuth}
      />
    </div>
  );
}
