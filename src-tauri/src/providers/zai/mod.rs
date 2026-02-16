pub mod client;
pub mod probe;

use crate::models::AccountRecord;

use super::contract::{api_key_provider_contract, ProviderContract};
use super::runtime::{ManifestLineSpec, ProbeFuture, ProviderRuntime};

pub const CONTRACT: ProviderContract = api_key_provider_contract("zai", "Z.ai");

const LINES: [ManifestLineSpec; 2] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Token Usage",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Utility Usage",
        scope: "overview",
    },
];

const PRIMARY_CANDIDATES: [&str; 2] = ["Token Usage", "Utility Usage"];

#[derive(Debug, Clone, Copy)]
pub struct ZaiRuntime;

pub const RUNTIME: ZaiRuntime = ZaiRuntime;

impl ProviderRuntime for ZaiRuntime {
    fn id(&self) -> &'static str {
        CONTRACT.id
    }

    fn name(&self) -> &'static str {
        CONTRACT.name
    }

    fn icon_url(&self) -> &'static str {
        "/providers/zai.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#2D2D2D")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(probe::probe(account, credentials))
    }
}
