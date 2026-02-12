use super::descriptor::{AuthStrategyDescriptor, ProviderDescriptor};

pub const OAUTH_AUTH_STRATEGY: AuthStrategyContract = AuthStrategyContract {
    id: "oauth",
    label: "OAuth",
    kind: AuthStrategyKind::OAuth,
};

pub const API_KEY_AUTH_STRATEGY: AuthStrategyContract = AuthStrategyContract {
    id: "apiKey",
    label: "API Key",
    kind: AuthStrategyKind::ApiKey,
};

pub const COOKIE_AUTH_STRATEGY: AuthStrategyContract = AuthStrategyContract {
    id: "cookie",
    label: "Cookie",
    kind: AuthStrategyKind::Cookie,
};

pub const OAUTH_AUTH_STRATEGIES: &[AuthStrategyContract] = &[OAUTH_AUTH_STRATEGY];
pub const API_KEY_AUTH_STRATEGIES: &[AuthStrategyContract] = &[API_KEY_AUTH_STRATEGY];
pub const COOKIE_AUTH_STRATEGIES: &[AuthStrategyContract] = &[COOKIE_AUTH_STRATEGY];

pub const OPEN_SETTINGS: SettingsContract = SettingsContract {
    required_keys: &[],
    allow_additional_keys: true,
};

#[derive(Debug, Clone, Copy)]
pub struct ProviderContract {
    pub id: &'static str,
    pub name: &'static str,
    pub default_auth_strategy_id: &'static str,
    pub auth_strategies: &'static [AuthStrategyContract],
    pub settings: SettingsContract,
}

impl ProviderContract {
    pub fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id,
            name: self.name,
            default_auth_strategy_id: self.default_auth_strategy_id,
            auth_strategies: self
                .auth_strategies
                .iter()
                .map(|strategy| AuthStrategyDescriptor {
                    id: strategy.id,
                    label: strategy.label,
                })
                .collect(),
        }
    }

    pub fn supports_auth_strategy(&self, auth_strategy_id: &str) -> bool {
        self.auth_strategies
            .iter()
            .any(|strategy| strategy.id == auth_strategy_id)
    }
}

// TODO(openburn): Use auth strategy label/kind in runtime UI + richer validation.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct AuthStrategyContract {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: AuthStrategyKind,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStrategyKind {
    OAuth,
    ApiKey,
    Cookie,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsContract {
    pub required_keys: &'static [&'static str],
    pub allow_additional_keys: bool,
}

pub const fn provider_contract(
    id: &'static str,
    name: &'static str,
    default_auth_strategy_id: &'static str,
    auth_strategies: &'static [AuthStrategyContract],
    settings: SettingsContract,
) -> ProviderContract {
    ProviderContract {
        id,
        name,
        default_auth_strategy_id,
        auth_strategies,
        settings,
    }
}

pub const fn oauth_provider_contract(id: &'static str, name: &'static str) -> ProviderContract {
    provider_contract(id, name, "oauth", OAUTH_AUTH_STRATEGIES, OPEN_SETTINGS)
}

pub const fn api_key_provider_contract(id: &'static str, name: &'static str) -> ProviderContract {
    provider_contract(id, name, "apiKey", API_KEY_AUTH_STRATEGIES, OPEN_SETTINGS)
}

pub const fn cookie_provider_contract(id: &'static str, name: &'static str) -> ProviderContract {
    provider_contract(id, name, "cookie", COOKIE_AUTH_STRATEGIES, OPEN_SETTINGS)
}
