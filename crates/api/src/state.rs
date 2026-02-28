use sqlx::PgPool;
use tracing::{info, warn};

use crate::auth::{AuthConfig, JwtService, OAuthService};
use crate::services::{EmailConfig, EmailService};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    auth_config: AuthConfig,
    jwt_service: JwtService,
    oauth_service: OAuthService,
    email_service: Option<EmailService>,
}

impl AppState {
    pub fn new(db: PgPool) -> anyhow::Result<Self> {
        let auth_config = AuthConfig::from_env()?;
        let jwt_service = JwtService::new(&auth_config);
        let oauth_service = OAuthService::new(auth_config.clone());

        let email_service = match EmailConfig::from_env() {
            Some(config) => {
                info!("Email service configured (Scaleway Transactional Email)");
                Some(EmailService::new(config))
            }
            None => {
                warn!("Email service not configured: missing SCW_SECRET_KEY, SCW_DEFAULT_PROJECT_ID, or SCW_SENDER_EMAIL");
                None
            }
        };

        Ok(Self {
            db,
            auth_config,
            jwt_service,
            oauth_service,
            email_service,
        })
    }

    pub fn auth_config(&self) -> &AuthConfig {
        &self.auth_config
    }

    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    pub fn oauth_service(&self) -> &OAuthService {
        &self.oauth_service
    }

    pub fn email_service(&self) -> Option<&EmailService> {
        self.email_service.as_ref()
    }
}
