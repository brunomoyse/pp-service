use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use oauth2::{AsyncHttpClient, HttpClientError, HttpResponse};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use crate::auth::AuthConfig;
use crate::error::AppError;

/// Wrapper around reqwest 0.13 Client that implements oauth2's AsyncHttpClient trait.
/// oauth2 5.0 bundles its own reqwest 0.12 integration, but since we use reqwest 0.13,
/// we need a bridge implementation.
#[derive(Clone)]
struct OAuth2HttpClient(reqwest::Client);

impl<'c> AsyncHttpClient<'c> for OAuth2HttpClient {
    type Error = HttpClientError<reqwest::Error>;
    type Future =
        Pin<Box<dyn Future<Output = Result<HttpResponse, Self::Error>> + Send + Sync + 'c>>;

    fn call(&'c self, request: oauth2::HttpRequest) -> Self::Future {
        Box::pin(async move {
            let method = request.method().clone();
            let url = request.uri().to_string();

            let mut req_builder = self.0.request(method, &url);
            for (name, value) in request.headers().iter() {
                req_builder = req_builder.header(name, value);
            }
            req_builder = req_builder.body(request.into_body());

            let response = req_builder.send().await.map_err(Box::new)?;

            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.map_err(Box::new)?.to_vec();

            let mut builder = axum::http::Response::builder().status(status);
            for (name, value) in headers.iter() {
                builder = builder.header(name, value);
            }

            builder.body(body).map_err(HttpClientError::Http)
        })
    }
}

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
    http_client: reqwest::Client,
    oauth2_client: OAuth2HttpClient,
}

impl OAuthService {
    pub fn new(config: AuthConfig) -> Self {
        let http_client = reqwest::Client::new();
        Self {
            config,
            oauth2_client: OAuth2HttpClient(http_client.clone()),
            http_client,
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
            OAuthProvider::Google => {
                let redirect_url = format!(
                    "{}/auth/{}/callback",
                    self.config.redirect_base_url,
                    provider.as_str()
                );

                let client = BasicClient::new(ClientId::new(self.config.google_client_id.clone()))
                    .set_client_secret(ClientSecret::new(self.config.google_client_secret.clone()))
                    .set_auth_uri(
                        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                            .map_err(|e| AppError::Internal(format!("Invalid auth URL: {}", e)))?,
                    )
                    .set_token_uri(
                        TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string())
                            .map_err(|e| AppError::Internal(format!("Invalid token URL: {}", e)))?,
                    )
                    .set_redirect_uri(
                        RedirectUrl::new(redirect_url).map_err(|e| {
                            AppError::Internal(format!("Invalid redirect URL: {}", e))
                        })?,
                    );

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
                Err(AppError::Internal(
                    "Custom OAuth code exchange should be handled by custom OAuth service"
                        .to_string(),
                ))
            }
            OAuthProvider::Google => {
                let redirect_url = format!(
                    "{}/auth/{}/callback",
                    self.config.redirect_base_url,
                    provider.as_str()
                );

                let client = BasicClient::new(ClientId::new(self.config.google_client_id.clone()))
                    .set_client_secret(ClientSecret::new(self.config.google_client_secret.clone()))
                    .set_auth_uri(
                        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                            .map_err(|e| AppError::Internal(format!("Invalid auth URL: {}", e)))?,
                    )
                    .set_token_uri(
                        TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string())
                            .map_err(|e| AppError::Internal(format!("Invalid token URL: {}", e)))?,
                    )
                    .set_redirect_uri(
                        RedirectUrl::new(redirect_url).map_err(|e| {
                            AppError::Internal(format!("Invalid redirect URL: {}", e))
                        })?,
                    );

                let token = client
                    .exchange_code(AuthorizationCode::new(code))
                    .request_async(&self.oauth2_client)
                    .await
                    .map_err(|e| AppError::Internal(format!("Token exchange failed: {}", e)))?;

                let access_token = token.access_token().secret();
                let user_info = self.get_google_user_info(access_token).await?;
                Ok(user_info.into())
            }
        }
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
