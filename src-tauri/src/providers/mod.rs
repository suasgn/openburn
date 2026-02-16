mod antigravity;
mod claude;
mod codex;
pub mod common;
mod contract;
mod copilot;
mod descriptor;
mod opencode;
mod registry;
mod runtime;
pub mod usage;
mod validation;
mod zai;

#[allow(unused_imports)]
pub mod clients {
    pub use super::antigravity::client as antigravity;
    pub use super::claude::client as claude;
    pub use super::codex::client as codex;
    pub use super::copilot::client as copilot;
    pub use super::opencode::client as opencode;
    pub use super::zai::client as zai;
}

pub use descriptor::ProviderDescriptor;
pub use registry::{all_provider_descriptors, find_provider_contract};
pub use runtime::{all_provider_ids, all_provider_meta, find_provider_runtime, ProviderMeta};
pub use usage::{MetricLine, ProbeSuccess};
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
        assert!(providers
            .iter()
            .any(|provider| provider.id == "antigravity"));
        assert!(providers.iter().any(|provider| provider.id == "codex"));
        assert!(providers.iter().any(|provider| provider.id == "copilot"));
        assert!(providers.iter().any(|provider| provider.id == "claude"));
        assert!(providers.iter().any(|provider| provider.id == "opencode"));
        assert!(providers.iter().any(|provider| provider.id == "zai"));
    }

    #[test]
    fn runtime_meta_ids_match_provider_registry() {
        let runtime_ids = all_provider_meta()
            .into_iter()
            .map(|provider| provider.id)
            .collect::<Vec<_>>();
        let descriptor_ids = all_provider_descriptors()
            .into_iter()
            .map(|provider| provider.id.to_string())
            .collect::<Vec<_>>();

        let runtime_set = runtime_ids.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(runtime_set.len(), runtime_ids.len());

        let descriptor_set = descriptor_ids
            .iter()
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(runtime_set, descriptor_set);
    }
}
