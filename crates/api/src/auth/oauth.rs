use anyhow::Result;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

use crate::auth::AuthConfig;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub enum OAuthProvider {
    Google,
    Custom,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Custom => "custom",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    pub given_name: String,
    pub family_name: String,
    pub picture: String,
}


#[derive(Debug, Clone)]
pub struct OAuthUserInfo {
    pub provider_id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<GoogleUserInfo> for OAuthUserInfo {
    fn from(google_user: GoogleUserInfo) -> Self {
        Self {
            provider_id: google_user.id,
            email: google_user.email,
            first_name: google_user.given_name,
            last_name: Some(google_user.family_name),
            username: None,
            avatar_url: Some(google_user.picture),
        }
    }
}


#[derive(Clone)]
pub struct OAuthService {
    config: AuthConfig,
    http_client: HttpClient,
}

impl OAuthService {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            http_client: HttpClient::new(),
        }
    }

    pub fn get_authorize_url(&self, provider: OAuthProvider) -> Result<(String, String), AppError> {
        match provider {
            OAuthProvider::Custom => {
                // For custom OAuth, return our own authorization URL
                let csrf_token = general_purpose::STANDARD.encode(rand::random::<[u8; 32]>());
                let auth_url = format!("{}/oauth/authorize", self.config.redirect_base_url);
                Ok((auth_url, csrf_token))
            }
            _ => {
                let client = self.create_oauth_client(provider.clone())?;
                let (auth_url, csrf_token) = client
                    .authorize_url(CsrfToken::new_random)
                    .add_scope(self.get_scopes(provider))
                    .url();

                Ok((auth_url.to_string(), csrf_token.secret().clone()))
            }
        }
    }

    pub async fn exchange_code_for_user_info(
        &self,
        provider: OAuthProvider,
        code: String,
    ) -> Result<OAuthUserInfo, AppError> {
        match provider {
            OAuthProvider::Custom => {
                // For custom OAuth, the code should contain user info directly
                // This is a simplified approach - in practice you'd exchange the code for user info
                Err(AppError::Internal("Custom OAuth code exchange should be handled by custom OAuth service".to_string()))
            }
            _ => {
                let client = self.create_oauth_client(provider.clone())?;
                
                let token = client
                    .exchange_code(AuthorizationCode::new(code))
                    .request_async(async_http_client)
                    .await
                    .map_err(|e| AppError::Internal(format!("Token exchange failed: {}", e)))?;

                let access_token = token.access_token().secret();
                
                match provider {
                    OAuthProvider::Google => {
                        let user_info = self.get_google_user_info(access_token).await?;
                        Ok(user_info.into())
                    }
                    OAuthProvider::Custom => unreachable!(), // Already handled above
                }
            }
        }
    }

    fn create_oauth_client(&self, provider: OAuthProvider) -> Result<BasicClient, AppError> {
        let redirect_url = format!("{}/auth/{}/callback", self.config.redirect_base_url, provider.as_str());

        let client = match provider {
            OAuthProvider::Google => {
                BasicClient::new(
                    ClientId::new(self.config.google_client_id.clone()),
                    Some(ClientSecret::new(self.config.google_client_secret.clone())),
                    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                        .map_err(|e| AppError::Internal(format!("Invalid auth URL: {}", e)))?,
                    Some(
                        TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string())
                            .map_err(|e| AppError::Internal(format!("Invalid token URL: {}", e)))?,
                    ),
                )
            }
            OAuthProvider::Custom => {
                return Err(AppError::Internal("Custom OAuth should not use external OAuth client".to_string()));
            }
        };

        let client = client.set_redirect_uri(
            RedirectUrl::new(redirect_url)
                .map_err(|e| AppError::Internal(format!("Invalid redirect URL: {}", e)))?,
        );

        Ok(client)
    }

    fn get_scopes(&self, provider: OAuthProvider) -> Scope {
        match provider {
            OAuthProvider::Google => Scope::new("openid email profile".to_string()),
            OAuthProvider::Custom => Scope::new("read".to_string()), // Default scope for custom OAuth
        }
    }

    async fn get_google_user_info(&self, access_token: &str) -> Result<GoogleUserInfo, AppError> {
        let url = "https://www.googleapis.com/oauth2/v2/userinfo";
        let response = self
            .http_client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to fetch user info: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "Failed to fetch user info: {}",
                response.status()
            )));
        }

        response
            .json::<GoogleUserInfo>()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse user info: {}", e)))
    }

}