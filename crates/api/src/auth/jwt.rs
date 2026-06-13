use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthConfig;
use crate::error::AppError;

/// Issuer / audience stamped into every access token and verified on the way in.
/// Pinning these scopes the secret to this service+client so a token minted for
/// (or leaked from) another system sharing the secret can't be replayed here.
pub const TOKEN_ISSUER: &str = "pocketpair-api";
pub const TOKEN_AUDIENCE: &str = "pocketpair-clients";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub email: String,
    pub role: String,
    pub iss: String, // Issuer
    pub aud: String, // Audience
    pub iat: i64,    // Issued at
    pub exp: i64,    // Expiration
}

impl Claims {
    pub fn new(user_id: Uuid, email: String, role: String, expiration_minutes: u64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::minutes(expiration_minutes as i64);

        Self {
            sub: user_id.to_string(),
            email,
            role,
            iss: TOKEN_ISSUER.to_string(),
            aud: TOKEN_AUDIENCE.to_string(),
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
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[TOKEN_ISSUER]);
        validation.set_audience(&[TOKEN_AUDIENCE]);

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|e| AppError::Internal(format!("Invalid token: {}", e)))?;

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::{Claims, JwtService, TOKEN_AUDIENCE, TOKEN_ISSUER};
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
    use uuid::Uuid;

    const SECRET: &str = "unit-test-secret-please-ignore";

    // Build a service directly (bypassing AuthConfig/env) so the test is pure.
    fn service(secret: &str) -> JwtService {
        JwtService {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            expiration_minutes: 15,
        }
    }

    fn claims_with(iss: &str, aud: &str) -> Claims {
        let now = Utc::now();
        Claims {
            sub: Uuid::new_v4().to_string(),
            email: "u@test.dev".into(),
            role: "player".into(),
            iss: iss.into(),
            aud: aud.into(),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(15)).timestamp(),
        }
    }

    fn sign(secret: &str, claims: &Claims) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn round_trips_a_valid_token() {
        let svc = service(SECRET);
        let token = svc
            .create_token(Uuid::new_v4(), "u@test.dev".into(), "player".into())
            .unwrap();
        let claims = svc.verify_token(&token).expect("valid token verifies");
        assert_eq!(claims.iss, TOKEN_ISSUER);
        assert_eq!(claims.aud, TOKEN_AUDIENCE);
        assert_eq!(claims.role, "player");
    }

    #[test]
    fn rejects_wrong_audience() {
        // Correct issuer + signature, but minted for a different audience —
        // e.g. a token leaked from another client sharing the secret.
        let token = sign(SECRET, &claims_with(TOKEN_ISSUER, "someone-elses-clients"));
        assert!(service(SECRET).verify_token(&token).is_err());
    }

    #[test]
    fn rejects_wrong_issuer() {
        let token = sign(SECRET, &claims_with("another-service", TOKEN_AUDIENCE));
        assert!(service(SECRET).verify_token(&token).is_err());
    }

    #[test]
    fn rejects_foreign_signature() {
        // Right claims, wrong signing key: the signature check must fail.
        let token = sign(
            "a-totally-different-secret",
            &claims_with(TOKEN_ISSUER, TOKEN_AUDIENCE),
        );
        assert!(service(SECRET).verify_token(&token).is_err());
    }

    #[test]
    fn rejects_tampered_token() {
        let svc = service(SECRET);
        let mut token = svc
            .create_token(Uuid::new_v4(), "u@test.dev".into(), "player".into())
            .unwrap();
        token.push('x'); // corrupt the trailing signature segment
        assert!(svc.verify_token(&token).is_err());
    }
}
