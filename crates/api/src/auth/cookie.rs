pub fn build_refresh_cookie(
    raw_token: &str,
    max_age_secs: Option<u64>,
    cookie_domain: &Option<String>,
    secure: bool,
) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    let same_site = if secure { "Strict" } else { "Lax" };

    let mut cookie = format!(
        "refresh_token={}; HttpOnly{}; SameSite={}; Path=/auth",
        raw_token, secure_flag, same_site
    );

    // When max_age is Some, the cookie persists across browser sessions ("remember me").
    // When None, it becomes a session cookie that expires when the browser closes.
    if let Some(secs) = max_age_secs {
        cookie.push_str(&format!("; Max-Age={}", secs));
    }

    if let Some(domain) = cookie_domain {
        cookie.push_str(&format!("; Domain={}", domain));
    }

    cookie
}

pub fn build_clear_cookie(cookie_domain: &Option<String>, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    let same_site = if secure { "Strict" } else { "Lax" };

    let mut cookie = format!(
        "refresh_token=; HttpOnly{}; SameSite={}; Path=/auth; Max-Age=0",
        secure_flag, same_site
    );

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
