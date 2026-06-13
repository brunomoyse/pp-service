use async_graphql::{InputObject, SimpleObject, ID};

use crate::gql::types::User;

#[derive(SimpleObject, Clone)]
pub struct AuthPayload {
    pub token: String,
    pub user: User,
    /// Raw refresh token, returned **only** to native clients that opt in via
    /// `native_client: true`. Web clients receive `None` and rely on the
    /// HttpOnly refresh cookie instead. Native clients persist this in the
    /// device keychain and replay it on `/auth/refresh` via `X-Refresh-Token`.
    pub refresh_token: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct OAuthUrlResponse {
    pub auth_url: String,
    pub csrf_token: String,
}

#[derive(InputObject)]
pub struct OAuthCallbackInput {
    pub provider: String,
    pub code: String,
    pub csrf_token: String,
}

#[derive(SimpleObject, Clone)]
pub struct OAuthClient {
    pub id: ID,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub is_active: bool,
}

#[derive(InputObject)]
pub struct CreateOAuthClientInput {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
}

#[derive(SimpleObject)]
pub struct CreateOAuthClientResponse {
    pub client: OAuthClient,
    pub client_secret: String,
}

#[derive(InputObject)]
pub struct UserRegistrationInput {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub username: Option<String>,
}

#[derive(InputObject)]
pub struct UserLoginInput {
    pub email: String,
    pub password: String,
    /// When true, the refresh cookie persists across browser sessions.
    #[graphql(default = false)]
    pub remember_me: bool,
    /// When true, the caller is a native app (no cookie jar): the raw refresh
    /// token is returned in `AuthPayload.refresh_token` so it can be stored in
    /// the device keychain. Web clients omit this and keep the HttpOnly cookie.
    #[graphql(default = false)]
    pub native_client: bool,
}

#[derive(InputObject)]
pub struct RequestPasswordResetInput {
    pub email: String,
    /// Optional locale for the email (e.g. "en", "fr", "nl"). Defaults to "en".
    pub locale: Option<String>,
}

#[derive(SimpleObject)]
pub struct RequestPasswordResetResponse {
    pub success: bool,
    pub message: String,
}

#[derive(InputObject)]
pub struct ResetPasswordInput {
    pub token: String,
    pub new_password: String,
}

#[derive(SimpleObject)]
pub struct ResetPasswordResponse {
    pub success: bool,
    pub message: String,
}
