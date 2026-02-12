pub mod client;
pub mod probe;

use super::contract::{cookie_provider_contract, ProviderContract};

pub const CONTRACT: ProviderContract = cookie_provider_contract("opencode", "OpenCode");
