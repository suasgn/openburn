use std::future::Future;
use std::pin::Pin;

use serde::Serialize;

use crate::error::Result;
use crate::models::AccountRecord;

use super::usage::ProbeSuccess;

pub type ProbeFuture<'a> = Pin<Box<dyn Future<Output = Result<ProbeSuccess>> + Send + 'a>>;

#[derive(Debug, Clone, Copy)]
pub struct ManifestLineSpec {
    pub line_type: &'static str,
    pub label: &'static str,
    pub scope: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLineDto {
    #[serde(rename = "type")]
    pub line_type: String,
    pub label: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMeta {
    pub id: String,
    pub name: String,
    pub icon_url: String,
    pub brand_color: Option<String>,
    pub lines: Vec<ManifestLineDto>,
    pub primary_candidates: Vec<String>,
}

pub trait ProviderRuntime: Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn icon_url(&self) -> &'static str;
    fn brand_color(&self) -> Option<&'static str>;
    fn lines(&self) -> &'static [ManifestLineSpec];
    fn primary_candidates(&self) -> &'static [&'static str];
    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a>;
}

const CODEX_LINES: [ManifestLineSpec; 4] = [
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
        label: "Reviews",
        scope: "detail",
    },
    ManifestLineSpec {
        line_type: "progress",
        label: "Credits",
        scope: "detail",
    },
];

const CLAUDE_LINES: [ManifestLineSpec; 4] = [
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

const COPILOT_LINES: [ManifestLineSpec; 3] = [
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

const OPENCODE_LINES: [ManifestLineSpec; 4] = [
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

const ZAI_LINES: [ManifestLineSpec; 2] = [
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

const ANTIGRAVITY_LINES: [ManifestLineSpec; 4] = [
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

const ANTIGRAVITY_PRIMARY_CANDIDATES: [&str; 5] = [
    "Gemini 3 Pro",
    "Gemini 3 Flash",
    "Claude Opus 4.5",
    "Claude Sonnet 4.5",
    "GPT-OSS 120B",
];
const CODEX_PRIMARY_CANDIDATES: [&str; 1] = ["Session"];
const COPILOT_PRIMARY_CANDIDATES: [&str; 2] = ["Premium", "Chat"];
const CLAUDE_PRIMARY_CANDIDATES: [&str; 1] = ["Session"];
const OPENCODE_PRIMARY_CANDIDATES: [&str; 1] = ["Session"];
const ZAI_PRIMARY_CANDIDATES: [&str; 2] = ["Token Usage", "Utility Usage"];

#[derive(Debug, Clone, Copy)]
struct AntigravityRuntime;

impl ProviderRuntime for AntigravityRuntime {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn name(&self) -> &'static str {
        "Antigravity"
    }

    fn icon_url(&self) -> &'static str {
        "/providers/antigravity.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#4285F4")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &ANTIGRAVITY_LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &ANTIGRAVITY_PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(super::antigravity::probe::probe(account, credentials))
    }
}

#[derive(Debug, Clone, Copy)]
struct CodexRuntime;

impl ProviderRuntime for CodexRuntime {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn icon_url(&self) -> &'static str {
        "/providers/codex.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#74AA9C")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &CODEX_LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &CODEX_PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(super::codex::probe::probe(account, credentials))
    }
}

#[derive(Debug, Clone, Copy)]
struct CopilotRuntime;

impl ProviderRuntime for CopilotRuntime {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn name(&self) -> &'static str {
        "Copilot"
    }

    fn icon_url(&self) -> &'static str {
        "/providers/copilot.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#A855F7")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &COPILOT_LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &COPILOT_PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(super::copilot::probe::probe(account, credentials))
    }
}

#[derive(Debug, Clone, Copy)]
struct ClaudeRuntime;

impl ProviderRuntime for ClaudeRuntime {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn name(&self) -> &'static str {
        "Claude"
    }

    fn icon_url(&self) -> &'static str {
        "/providers/claude.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#DE7356")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &CLAUDE_LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &CLAUDE_PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(super::claude::probe::probe(account, credentials))
    }
}

#[derive(Debug, Clone, Copy)]
struct ZaiRuntime;

impl ProviderRuntime for ZaiRuntime {
    fn id(&self) -> &'static str {
        "zai"
    }

    fn name(&self) -> &'static str {
        "Z.ai"
    }

    fn icon_url(&self) -> &'static str {
        "/providers/zai.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#2D2D2D")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &ZAI_LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &ZAI_PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(super::zai::probe::probe(account, credentials))
    }
}

#[derive(Debug, Clone, Copy)]
struct OpencodeRuntime;

impl ProviderRuntime for OpencodeRuntime {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn name(&self) -> &'static str {
        "OpenCode"
    }

    fn icon_url(&self) -> &'static str {
        "/providers/opencode.svg"
    }

    fn brand_color(&self) -> Option<&'static str> {
        Some("#3B82F6")
    }

    fn lines(&self) -> &'static [ManifestLineSpec] {
        &OPENCODE_LINES
    }

    fn primary_candidates(&self) -> &'static [&'static str] {
        &OPENCODE_PRIMARY_CANDIDATES
    }

    fn probe<'a>(
        &self,
        account: &'a AccountRecord,
        credentials: serde_json::Value,
    ) -> ProbeFuture<'a> {
        Box::pin(super::opencode::probe::probe(account, credentials))
    }
}

const ANTIGRAVITY_RUNTIME: AntigravityRuntime = AntigravityRuntime;
const CODEX_RUNTIME: CodexRuntime = CodexRuntime;
const COPILOT_RUNTIME: CopilotRuntime = CopilotRuntime;
const CLAUDE_RUNTIME: ClaudeRuntime = ClaudeRuntime;
const OPENCODE_RUNTIME: OpencodeRuntime = OpencodeRuntime;
const ZAI_RUNTIME: ZaiRuntime = ZaiRuntime;

const RUNTIMES: [&dyn ProviderRuntime; 6] = [
    &ANTIGRAVITY_RUNTIME,
    &CODEX_RUNTIME,
    &COPILOT_RUNTIME,
    &CLAUDE_RUNTIME,
    &OPENCODE_RUNTIME,
    &ZAI_RUNTIME,
];

pub fn all_provider_meta() -> Vec<ProviderMeta> {
    RUNTIMES
        .iter()
        .map(|runtime| ProviderMeta {
            id: runtime.id().to_string(),
            name: runtime.name().to_string(),
            icon_url: runtime.icon_url().to_string(),
            brand_color: runtime.brand_color().map(|value| value.to_string()),
            lines: runtime
                .lines()
                .iter()
                .map(|line| ManifestLineDto {
                    line_type: line.line_type.to_string(),
                    label: line.label.to_string(),
                    scope: line.scope.to_string(),
                })
                .collect(),
            primary_candidates: runtime
                .primary_candidates()
                .iter()
                .map(|label| label.to_string())
                .collect(),
        })
        .collect()
}

pub fn all_provider_ids() -> Vec<String> {
    RUNTIMES
        .iter()
        .map(|runtime| runtime.id().to_string())
        .collect()
}

pub fn find_provider_runtime(provider_id: &str) -> Option<&'static dyn ProviderRuntime> {
    let provider_id = provider_id.trim().to_ascii_lowercase();
    RUNTIMES
        .iter()
        .copied()
        .find(|runtime| runtime.id() == provider_id.as_str())
}
