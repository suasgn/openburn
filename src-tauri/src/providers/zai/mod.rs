pub mod client;
pub mod probe;

use super::contract::{api_key_provider_contract, ProviderContract};

pub const CONTRACT: ProviderContract = api_key_provider_contract("zai", "Z.ai");
