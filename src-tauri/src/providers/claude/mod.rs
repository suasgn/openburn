pub mod client;
pub mod probe;

use crate::models::AccountRecord;

use super::contract::{oauth_provider_contract, ProviderContract};
use super::runtime::{ManifestLineSpec, ProbeFuture, ProviderRuntime};

pub const CONTRACT: ProviderContract = oauth_provider_contract("claude", "Claude");

const LINES: [ManifestLineSpec; 4] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Session",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Weekly",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Sonnet",
        scope: "detail",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Extra usage",
        scope: "detail",
    },
];

const PRIMARY_CANDIDATES: [&str; 1] = ["Session"];

#[derive(Debug, Clone, Copy)]
pub struct ClaudeRuntime;

pub const RUNTIME: ClaudeRuntime = ClaudeRuntime;

impl ProviderRuntime for ClaudeRuntime {
    fn id(&self) -> &'static str {
        CONTRACT.id
    }

    fn name(&self) -> &'static str {
        CONTRACT.name
    }

    fn icon_url(&self) -> &'static str {
        "/providers/claude.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#DE7356")
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
