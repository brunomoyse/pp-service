pub fn build_refresh_cookie(
    raw_token: &str,
    max_age_secs: u64,
    cookie_domain: &Option<String>,
) -> String {
    let mut cookie = format!(
        "refresh_token={}; HttpOnly; Secure; SameSite=Strict; Path=/auth; Max-Age={}",
        raw_token, max_age_secs
    );

    if let Some(domain) = cookie_domain {
        cookie.push_str(&format!("; Domain={}", domain));
    }

    cookie
}

pub fn build_clear_cookie(cookie_domain: &Option<String>) -> String {
    let mut cookie =
        "refresh_token=; HttpOnly; Secure; SameSite=Strict; Path=/auth; Max-Age=0".to_string();

    if let Some(domain) = cookie_domain {
        cookie.push_str(&format!("; Domain={}", domain));
    }

    cookie
}

pub fn extract_refresh_token(cookie_header: &str) -> Option<String> {
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix("refresh_token=") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}
