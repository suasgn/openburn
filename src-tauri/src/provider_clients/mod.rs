pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod zai;

pub fn shorten_body(body: &str) -> String {
    let trimmed = body.replace('\n', " ").trim().to_string();
    if trimmed.len() > 400 {
        format!("{}...", trimmed.chars().take(400).collect::<String>())
    } else {
        trimmed
    }
}

pub fn normalize_percent(value: f64) -> f64 {
    if value <= 1.0 {
        value * 100.0
    } else {
        value
    }
}
