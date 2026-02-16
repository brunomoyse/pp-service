use anyhow::Result;
use std::env;

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub access_token_expiration_minutes: u64,
    pub refresh_token_expiration_days: u64,
    pub cookie_domain: Option<String>,
    pub cookie_secure: bool,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub redirect_base_url: String,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            access_token_expiration_minutes: env::var("ACCESS_TOKEN_EXPIRATION_MINUTES")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .unwrap_or(15),
            refresh_token_expiration_days: env::var("REFRESH_TOKEN_EXPIRATION_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .unwrap_or(7),
            cookie_domain: env::var("COOKIE_DOMAIN").ok(),
            cookie_secure: env::var("COOKIE_SECURE")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            google_client_id: env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            redirect_base_url: env::var("REDIRECT_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        })
    }
}
