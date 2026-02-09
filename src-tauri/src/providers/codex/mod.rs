use super::contract::{AuthStrategyContract, AuthStrategyKind, ProviderContract, SettingsContract};

const AUTH_STRATEGIES: [AuthStrategyContract; 1] = [AuthStrategyContract {
    id: "oauth",
    label: "OAuth",
    kind: AuthStrategyKind::OAuth,
}];

pub const CONTRACT: ProviderContract = ProviderContract {
    id: "codex",
    name: "Codex",
    default_auth_strategy_id: "oauth",
    auth_strategies: &AUTH_STRATEGIES,
    settings: SettingsContract {
        required_keys: &[],
        allow_additional_keys: true,
    },
};
