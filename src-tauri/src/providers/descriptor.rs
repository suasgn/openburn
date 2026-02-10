use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStrategyDescriptor {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub default_auth_strategy_id: &'static str,
    pub auth_strategies: Vec<AuthStrategyDescriptor>,
}
