use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const PERIOD_5_HOURS_MS: u64 = 5 * 60 * 60 * 1000;
pub const PERIOD_7_DAYS_MS: u64 = 7 * 24 * 60 * 60 * 1000;
pub const PERIOD_30_DAYS_MS: u64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProgressFormat {
    Percent,
    Dollars,
    Count { suffix: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MetricLine {
    Text {
        label: String,
        value: String,
        color: Option<String>,
        subtitle: Option<String>,
    },
    Progress {
        label: String,
        used: f64,
        limit: f64,
        format: ProgressFormat,
        #[serde(rename = "resetsAt")]
        resets_at: Option<String>,
        #[serde(rename = "periodDurationMs")]
        period_duration_ms: Option<u64>,
        color: Option<String>,
    },
    Badge {
        label: String,
        text: String,
        color: Option<String>,
        subtitle: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ProbeSuccess {
    pub plan: Option<String>,
    pub lines: Vec<MetricLine>,
    pub updated_credentials: Option<serde_json::Value>,
}

pub fn progress_percent_line(
    label: &str,
    used: f64,
    resets_at: Option<String>,
    period_duration_ms: Option<u64>,
) -> MetricLine {
    MetricLine::Progress {
        label: label.to_string(),
        used,
        limit: 100.0,
        format: ProgressFormat::Percent,
        resets_at,
        period_duration_ms,
        color: None,
    }
}

pub fn status_line(text: &str) -> MetricLine {
    MetricLine::Badge {
        label: "Status".to_string(),
        text: text.to_string(),
        color: Some("#a3a3a3".to_string()),
        subtitle: None,
    }
}

pub fn error_line(message: String) -> MetricLine {
    MetricLine::Badge {
        label: "Error".to_string(),
        text: message,
        color: Some("#ef4444".to_string()),
        subtitle: None,
    }
}

pub fn plan_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    trimmed
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().to_string();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_number(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(int_value) = trimmed.parse::<i64>() {
        return Some(int_value as f64);
    }
    trimmed.parse::<f64>().ok()
}

pub fn unix_to_rfc3339(value: i64) -> Option<String> {
    if value <= 0 {
        return None;
    }
    let seconds = if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    };
    let timestamp = OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    timestamp.format(&Rfc3339).ok()
}

pub fn duration_ms_from_seconds(seconds: Option<i64>) -> Option<u64> {
    seconds
        .filter(|value| *value > 0)
        .map(|value| value as u64)
        .map(|value| value.saturating_mul(1000))
}

pub fn normalize_resets_at(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub fn dollars_from_cents(value: f64) -> f64 {
    (value / 100.0 * 100.0).round() / 100.0
}

pub fn read_json_string(settings: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let object = settings.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key) {
            if let Some(text) = value.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}
