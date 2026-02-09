use sqlx::PgPool;

use crate::auth::{AuthConfig, JwtService, OAuthService};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    auth_config: AuthConfig,
    jwt_service: JwtService,
    oauth_service: OAuthService,
}

impl AppState {
    pub fn new(db: PgPool) -> anyhow::Result<Self> {
        let auth_config = AuthConfig::from_env()?;
        let jwt_service = JwtService::new(&auth_config);
        let oauth_service = OAuthService::new(auth_config.clone());

        Ok(Self {
            db,
            auth_config,
            jwt_service,
            oauth_service,
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
}
