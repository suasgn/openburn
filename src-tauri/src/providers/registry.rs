use super::contract::ProviderContract;
use super::descriptor::ProviderDescriptor;
use super::{antigravity, claude, codex, copilot, zai};

const PROVIDERS: [ProviderContract; 5] = [
    antigravity::CONTRACT,
    codex::CONTRACT,
    copilot::CONTRACT,
    claude::CONTRACT,
    zai::CONTRACT,
];

pub fn all_provider_descriptors() -> Vec<ProviderDescriptor> {
    PROVIDERS.iter().map(ProviderContract::descriptor).collect()
}

pub fn find_provider_contract(provider_id: &str) -> Option<&'static ProviderContract> {
    let provider_id = provider_id.trim().to_ascii_lowercase();
    PROVIDERS.iter().find(|provider| provider.id == provider_id)
}
