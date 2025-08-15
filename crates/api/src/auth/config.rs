use anyhow::Result;
use std::env;

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub redirect_base_url: String,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-secret-key".to_string()),
            jwt_expiration_hours: env::var("JWT_EXPIRATION_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
            google_client_id: env::var("GOOGLE_CLIENT_ID")
                .unwrap_or_default(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .unwrap_or_default(),
            redirect_base_url: env::var("REDIRECT_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        })
    }
}