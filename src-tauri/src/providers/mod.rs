mod contract;
mod descriptor;
mod registry;
mod validation;

pub use descriptor::ProviderDescriptor;
pub use registry::{all_provider_descriptors, find_provider_contract};
pub use validation::{validate_auth_strategy_for_provider, validate_provider_settings};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_is_resolved() {
        let provider = find_provider_contract("openai").expect("provider should exist");
        assert_eq!(provider.id, "openai");
        assert!(find_provider_contract(" OpenAI ").is_some());
        assert!(find_provider_contract("unknown").is_none());
    }

    #[test]
    fn descriptors_are_exposed() {
        let providers = all_provider_descriptors();
        assert!(providers.iter().any(|provider| provider.id == "openai"));
        assert!(providers.iter().any(|provider| provider.id == "zai"));
    }
}
