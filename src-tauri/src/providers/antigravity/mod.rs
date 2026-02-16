pub mod client;
pub mod probe;

use crate::models::AccountRecord;

use super::contract::{oauth_provider_contract, ProviderContract};
use super::runtime::{ManifestLineSpec, ProbeFuture, ProviderRuntime};

pub const CONTRACT: ProviderContract = oauth_provider_contract("antigravity", "Antigravity");

const LINES: [ManifestLineSpec; 4] = [
    ManifestLineSpec {
        line_type: "progress",
        label: "Gemini 3 Pro",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Gemini 3 Flash",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Claude Opus 4.5",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Prompt Credits",
        scope: "detail",
    },
];

const PRIMARY_CANDIDATES: [&str; 5] = [
    "Gemini 3 Pro",
    "Gemini 3 Flash",
    "Claude Opus 4.5",
    "Claude Sonnet 4.5",
    "GPT-OSS 120B",
];

#[derive(Debug, Clone, Copy)]
pub struct AntigravityRuntime;

pub const RUNTIME: AntigravityRuntime = AntigravityRuntime;

impl ProviderRuntime for AntigravityRuntime {
    fn id(&self) -> &'static str {
        CONTRACT.id
    }

    fn name(&self) -> &'static str {
        CONTRACT.name
    }

    fn icon_url(&self) -> &'static str {
        "/providers/antigravity.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#4285F4")
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
