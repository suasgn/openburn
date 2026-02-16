pub mod client;
pub mod probe;

use crate::models::AccountRecord;

use super::contract::{cookie_provider_contract, ProviderContract};
use super::runtime::{ManifestLineSpec, ProbeFuture, ProviderRuntime};

pub const CONTRACT: ProviderContract = cookie_provider_contract("opencode", "OpenCode");

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
        line_type: "text",
        label: "Monthly Cost",
        scope: "overview",
    },
    ManifestLineSpec {
        line_type: "badge",
        label: "Subscription Rows",
        scope: "detail",
    },
];

const PRIMARY_CANDIDATES: [&str; 1] = ["Session"];

#[derive(Debug, Clone, Copy)]
pub struct OpencodeRuntime;

pub const RUNTIME: OpencodeRuntime = OpencodeRuntime;

impl ProviderRuntime for OpencodeRuntime {
    fn id(&self) -> &'static str {
        CONTRACT.id
    }

    fn name(&self) -> &'static str {
        CONTRACT.name
    }

    fn icon_url(&self) -> &'static str {
        "/providers/opencode.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#211E1E")
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
