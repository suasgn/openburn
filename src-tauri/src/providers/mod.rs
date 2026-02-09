mod claude;
mod codex;
mod contract;
mod copilot;
mod descriptor;
mod registry;
mod validation;
mod zai;

pub use descriptor::ProviderDescriptor;
pub use registry::{all_provider_descriptors, find_provider_contract};
pub use validation::{validate_auth_strategy_for_provider, validate_provider_settings};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_is_resolved() {
        let provider = find_provider_contract("codex").expect("provider should exist");
        assert_eq!(provider.id, "codex");
        assert!(find_provider_contract(" CODEX ").is_some());
        assert!(find_provider_contract("unknown").is_none());
    }

    #[test]
    fn descriptors_are_exposed() {
        let providers = all_provider_descriptors();
        assert!(providers.iter().any(|provider| provider.id == "codex"));
        assert!(providers.iter().any(|provider| provider.id == "copilot"));
        assert!(providers.iter().any(|provider| provider.id == "claude"));
        assert!(providers.iter().any(|provider| provider.id == "zai"));
    }
}
