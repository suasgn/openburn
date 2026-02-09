use super::contract::{AuthStrategyContract, AuthStrategyKind, ProviderContract, SettingsContract};

const AUTH_STRATEGIES: [AuthStrategyContract; 1] = [AuthStrategyContract {
    id: "apiKey",
    label: "API Key",
    kind: AuthStrategyKind::ApiKey,
}];

pub const CONTRACT: ProviderContract = ProviderContract {
    id: "zai",
    name: "Z.ai",
    default_auth_strategy_id: "apiKey",
    auth_strategies: &AUTH_STRATEGIES,
    settings: SettingsContract {
        required_keys: &[],
        allow_additional_keys: true,
    },
};
