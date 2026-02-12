pub mod client;
pub mod probe;

use super::contract::{oauth_provider_contract, ProviderContract};

pub const CONTRACT: ProviderContract = oauth_provider_contract("copilot", "Copilot");
