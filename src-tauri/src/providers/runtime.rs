use std::future::Future;
use std::pin::Pin;

use serde::Serialize;

use crate::error::Result;
use crate::models::AccountRecord;

use super::usage::ProbeSuccess;
use super::{antigravity, claude, codex, copilot, opencode, zai};

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

const RUNTIMES: [&dyn ProviderRuntime; 6] = [
    &antigravity::RUNTIME,
    &codex::RUNTIME,
    &copilot::RUNTIME,
    &claude::RUNTIME,
    &opencode::RUNTIME,
    &zai::RUNTIME,
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
