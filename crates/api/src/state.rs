use sqlx::PgPool;

use crate::auth::{AuthConfig, JwtService, OAuthService};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    jwt_service: JwtService,
    oauth_service: OAuthService,
}

impl AppState {
    pub fn new(db: PgPool) -> anyhow::Result<Self> {
        let auth_config = AuthConfig::from_env()?;
        let jwt_service = JwtService::new(&auth_config);
        let oauth_service = OAuthService::new(auth_config);

        Ok(Self {
            db,
            jwt_service,
            oauth_service,
        })
    }

    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    pub fn oauth_service(&self) -> &OAuthService {
        &self.oauth_service
    }
}