use super::descriptor::ProviderDescriptor;

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
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsContract {
    pub required_keys: &'static [&'static str],
    pub allow_additional_keys: bool,
}
