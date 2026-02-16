pub mod client;
pub mod probe;

use crate::models::AccountRecord;

use super::contract::{oauth_provider_contract, ProviderContract};
use super::runtime::{ManifestLineSpec, ProbeFuture, ProviderRuntime};

pub const CONTRACT: ProviderContract = oauth_provider_contract("copilot", "Copilot");

const LINES: [ManifestLineSpec; 3] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Premium",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Chat",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Completions",
        scope: "overview",
    },
];

const PRIMARY_CANDIDATES: [&str; 2] = ["Premium", "Chat"];

#[derive(Debug, Clone, Copy)]
pub struct CopilotRuntime;

pub const RUNTIME: CopilotRuntime = CopilotRuntime;

impl ProviderRuntime for CopilotRuntime {
    fn id(&self) -> &'static str {
        CONTRACT.id
    }

    fn name(&self) -> &'static str {
        CONTRACT.name
    }

    fn icon_url(&self) -> &'static str {
        "/providers/copilot.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#A855F7")
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
