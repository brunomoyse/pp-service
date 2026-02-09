use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthConfig;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub email: String,
    pub role: String,
    pub iat: i64, // Issued at
    pub exp: i64, // Expiration
}

impl Claims {
    pub fn new(user_id: Uuid, email: String, role: String, expiration_minutes: u64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::minutes(expiration_minutes as i64);

        Self {
            sub: user_id.to_string(),
            email,
            role,
            iat: now.timestamp(),
            exp: exp.timestamp(),
        }
    }
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiration_minutes: u64,
}

impl JwtService {
    pub fn new(config: &AuthConfig) -> Self {
        let secret = config.jwt_secret.as_bytes();
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            expiration_minutes: config.access_token_expiration_minutes,
        }
    }

    pub fn create_token(
        &self,
        user_id: Uuid,
        email: String,
        role: String,
    ) -> Result<String, AppError> {
        let claims = Claims::new(user_id, email, role, self.expiration_minutes);
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| AppError::Internal(format!("Invalid token: {}", e)))?;

        Ok(token_data.claims)
    }
}
