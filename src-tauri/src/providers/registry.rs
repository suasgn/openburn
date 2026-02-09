use super::contract::{AuthStrategyContract, AuthStrategyKind, ProviderContract, SettingsContract};
use super::descriptor::ProviderDescriptor;

const OPENAI_AUTH_STRATEGIES: [AuthStrategyContract; 2] = [
    AuthStrategyContract {
        id: "oauth",
        label: "OAuth",
        kind: AuthStrategyKind::OAuth,
    },
    AuthStrategyContract {
        id: "apiKey",
        label: "API Key",
        kind: AuthStrategyKind::ApiKey,
    },
];

const ZAI_AUTH_STRATEGIES: [AuthStrategyContract; 1] = [AuthStrategyContract {
    id: "apiKey",
    label: "API Key",
    kind: AuthStrategyKind::ApiKey,
}];

const PROVIDERS: [ProviderContract; 2] = [
    ProviderContract {
        id: "openai",
        name: "OpenAI",
        default_auth_strategy_id: "oauth",
        auth_strategies: &OPENAI_AUTH_STRATEGIES,
        settings: SettingsContract {
            required_keys: &[],
            allow_additional_keys: true,
        },
    },
    ProviderContract {
        id: "zai",
        name: "Z.ai",
        default_auth_strategy_id: "apiKey",
        auth_strategies: &ZAI_AUTH_STRATEGIES,
        settings: SettingsContract {
            required_keys: &[],
            allow_additional_keys: true,
        },
    },
];

pub fn all_provider_descriptors() -> Vec<ProviderDescriptor> {
    PROVIDERS.iter().map(ProviderContract::descriptor).collect()
}

pub fn find_provider_contract(provider_id: &str) -> Option<&'static ProviderContract> {
    let provider_id = provider_id.trim().to_ascii_lowercase();
    PROVIDERS.iter().find(|provider| provider.id == provider_id)
}
