use sqlx::PgPool;

use crate::auth::{AuthConfig, JwtService, OAuthService, SessionService};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    jwt_service: JwtService,
    oauth_service: OAuthService,
    session_service: SessionService,
}

impl AppState {
    pub fn new(db: PgPool) -> anyhow::Result<Self> {
        let auth_config = AuthConfig::from_env()?;
        let jwt_service = JwtService::new(&auth_config);
        let oauth_service = OAuthService::new(auth_config);
        let session_service = SessionService::new();

        Ok(Self {
            db,
            jwt_service,
            oauth_service,
            session_service,
        })
    }

    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    pub fn oauth_service(&self) -> &OAuthService {
        &self.oauth_service
    }

    pub fn session_service(&self) -> &SessionService {
        &self.session_service
    }
}