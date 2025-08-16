use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthConfig;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub email: String,
    pub iat: i64, // Issued at
    pub exp: i64, // Expiration
}

impl Claims {
    pub fn new(user_id: Uuid, email: String, expiration_hours: u64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::hours(expiration_hours as i64);

        Self {
            sub: user_id.to_string(),
            email,
            iat: now.timestamp(),
            exp: exp.timestamp(),
        }
    }
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    expiration_hours: u64,
}

impl JwtService {
    pub fn new(config: &AuthConfig) -> Self {
        let secret = config.jwt_secret.as_bytes();
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            expiration_hours: config.jwt_expiration_hours,
        }
    }

    pub fn create_token(&self, user_id: Uuid, email: String) -> Result<String, AppError> {
        let claims = Claims::new(user_id, email, self.expiration_hours);
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::Internal(e.to_string()))
    }

}