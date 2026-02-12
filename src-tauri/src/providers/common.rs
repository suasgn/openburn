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

pub fn format_status_error(status: reqwest::StatusCode, body: &str) -> String {
    let body = shorten_body(body);
    if body.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status} - {body}")
    }
}

pub fn format_http_error(context: &str, status: reqwest::StatusCode, body: &str) -> String {
    format!("{context}: {}", format_status_error(status, body))
}
