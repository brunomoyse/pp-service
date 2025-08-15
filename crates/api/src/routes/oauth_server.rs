use axum::{
    extract::{Query, State, Form},
    response::{Html, IntoResponse, Redirect},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::custom_oauth::{
    AuthorizeRequest, CustomOAuthService, ErrorResponse, TokenRequest, TokenResponse,
};
use crate::auth::password::PasswordService;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub username: Option<String>,
}

#[derive(Serialize)]
pub struct UserRegistrationResponse {
    pub user_id: String,
    pub message: String,
}

/// OAuth 2.0 Authorization Endpoint
/// GET /oauth/authorize?response_type=code&client_id=...&redirect_uri=...&scope=...&state=...
pub async fn authorize(
    State(state): State<AppState>,
    Query(params): Query<AuthorizeRequest>,
) -> Result<axum::response::Response, AppError> {
    // Validate response_type
    if params.response_type != "code" {
        return Ok(redirect_with_error(
            &params.redirect_uri,
            "unsupported_response_type",
            Some("Only 'code' response type is supported"),
            params.state.as_deref(),
        )?.into_response());
    }

    // Get and validate client
    let client = match CustomOAuthService::get_client_by_id(&state, &params.client_id).await? {
        Some(client) => client,
        None => {
            return Ok(redirect_with_error(
                &params.redirect_uri,
                "invalid_client",
                Some("Client not found"),
                params.state.as_deref(),
            )?.into_response());
        }
    };

    // Validate redirect URI
    if !CustomOAuthService::validate_redirect_uri(&client, &params.redirect_uri).await? {
        return Err(AppError::BadRequest("Invalid redirect URI".to_string()));
    }

    // Validate scopes
    let requested_scopes = CustomOAuthService::parse_scopes(params.scope.clone());
    if !CustomOAuthService::validate_scopes(&requested_scopes, &client.scopes) {
        return Ok(redirect_with_error(
            &params.redirect_uri,
            "invalid_scope",
            Some("Requested scope is not valid"),
            params.state.as_deref(),
        )?.into_response());
    }

    // Return login form
    let login_form = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Login - {}</title>
            <style>
                body {{ font-family: Arial, sans-serif; max-width: 400px; margin: 50px auto; padding: 20px; }}
                .form-group {{ margin-bottom: 15px; }}
                label {{ display: block; margin-bottom: 5px; }}
                input {{ width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; }}
                button {{ background: #007bff; color: white; padding: 10px 20px; border: none; border-radius: 4px; cursor: pointer; }}
                button:hover {{ background: #0056b3; }}
                .register-link {{ margin-top: 20px; text-align: center; }}
            </style>
        </head>
        <body>
            <h2>Login to {}</h2>
            <form method="post" action="/oauth/login">
                <input type="hidden" name="client_id" value="{}">
                <input type="hidden" name="redirect_uri" value="{}">
                <input type="hidden" name="scope" value="{}">
                <input type="hidden" name="state" value="{}">
                
                <div class="form-group">
                    <label for="email">Email:</label>
                    <input type="email" id="email" name="email" required>
                </div>
                
                <div class="form-group">
                    <label for="password">Password:</label>
                    <input type="password" id="password" name="password" required>
                </div>
                
                <button type="submit">Login</button>
            </form>
            
            <div class="register-link">
                <p>Don't have an account? <a href="/oauth/register">Register here</a></p>
            </div>
        </body>
        </html>
        "#,
        client.name,
        client.name,
        params.client_id,
        params.redirect_uri,
        params.scope.unwrap_or_else(|| "read".to_string()),
        params.state.unwrap_or_default()
    );

    Ok(Html(login_form).into_response())
}

/// OAuth 2.0 Login Handler
/// POST /oauth/login
pub async fn login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<axum::response::Response, AppError> {
    // Get and validate client
    let client = match CustomOAuthService::get_client_by_id(&state, &form.client_id).await? {
        Some(client) => client,
        None => {
            return Ok(redirect_with_error(
                &form.redirect_uri,
                "invalid_client",
                Some("Client not found"),
                form.state.as_deref(),
            )?.into_response());
        }
    };

    // Find user by email
    let user_row = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE email = $1",
        form.email
    )
    .fetch_optional(&state.db)
    .await?;

    let user_id = match user_row {
        Some(row) => {
            // Verify password
            if let Some(ref password_hash) = row.password_hash {
                if !PasswordService::verify_password(&form.password, &password_hash)? {
                    return Ok(redirect_with_error(
                        &form.redirect_uri,
                        "access_denied",
                        Some("Invalid credentials"),
                        form.state.as_deref(),
                    )?.into_response());
                }
            } else {
                return Ok(redirect_with_error(
                    &form.redirect_uri,
                    "access_denied",
                    Some("User has no password set"),
                    form.state.as_deref(),
                )?.into_response());
            }
            row.id
        }
        None => {
            return Ok(redirect_with_error(
                &form.redirect_uri,
                "access_denied",
                Some("Invalid credentials"),
                form.state.as_deref(),
            )?.into_response());
        }
    };

    // Create authorization code
    let scopes = CustomOAuthService::parse_scopes(form.scope);
    let auth_code = CustomOAuthService::create_authorization_code(
        &state,
        client.id,
        user_id,
        form.redirect_uri.clone(),
        scopes,
        None, // PKCE challenge not implemented in this simple version
        None, // PKCE challenge method
    )
    .await?;

    // Redirect back to client with authorization code
    let mut redirect_url = format!("{}?code={}", form.redirect_uri, auth_code.code);
    if let Some(state_param) = form.state {
        redirect_url.push_str(&format!("&state={}", state_param));
    }

    Ok(Redirect::to(&redirect_url).into_response())
}

/// OAuth 2.0 Token Endpoint
/// POST /oauth/token
pub async fn token(
    State(state): State<AppState>,
    Form(form): Form<TokenRequest>,
) -> Result<axum::response::Response, AppError> {
    match form.grant_type.as_str() {
        "authorization_code" => handle_authorization_code_grant(state, form).await,
        "refresh_token" => handle_refresh_token_grant(state, form).await,
        _ => Ok(Json(ErrorResponse {
            error: "unsupported_grant_type".to_string(),
            error_description: Some("Only 'authorization_code' and 'refresh_token' grant types are supported".to_string()),
        }).into_response()),
    }
}

async fn handle_authorization_code_grant(
    state: AppState,
    form: TokenRequest,
) -> Result<axum::response::Response, AppError> {
    let code = form.code.ok_or_else(|| AppError::BadRequest("Missing authorization code".to_string()))?;
    let redirect_uri = form.redirect_uri.ok_or_else(|| AppError::BadRequest("Missing redirect_uri".to_string()))?;

    // Get and validate client
    let client = match CustomOAuthService::get_client_by_id(&state, &form.client_id).await? {
        Some(client) => client,
        None => {
            return Ok(Json(ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: Some("Client not found".to_string()),
            }).into_response());
        }
    };

    // Verify client secret
    if let Some(client_secret) = form.client_secret {
        if client_secret != client.client_secret {
            return Ok(Json(ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: Some("Invalid client secret".to_string()),
            }).into_response());
        }
    }

    // Get authorization code
    let auth_code = match CustomOAuthService::get_authorization_code(&state, &code).await? {
        Some(auth_code) => auth_code,
        None => {
            return Ok(Json(ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: Some("Invalid or expired authorization code".to_string()),
            }).into_response());
        }
    };

    // Validate redirect URI
    if auth_code.redirect_uri != redirect_uri {
        return Ok(Json(ErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: Some("Redirect URI mismatch".to_string()),
        }).into_response());
    }

    // Validate client
    if auth_code.client_id != client.id {
        return Ok(Json(ErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: Some("Client mismatch".to_string()),
        }).into_response());
    }

    // Delete used authorization code
    CustomOAuthService::delete_authorization_code(&state, &code).await?;

    // Create access and refresh tokens
    let (access_token, refresh_token) = CustomOAuthService::create_access_token(
        &state,
        auth_code.client_id,
        auth_code.user_id,
        auth_code.scopes.clone(),
    ).await?;

    let response = TokenResponse {
        access_token: access_token.token,
        token_type: "Bearer".to_string(),
        expires_in: 3600, // 1 hour
        refresh_token: Some(refresh_token.token),
        scope: auth_code.scopes.join(" "),
    };

    Ok(Json(response).into_response())
}

async fn handle_refresh_token_grant(
    _state: AppState,
    _form: TokenRequest,
) -> Result<axum::response::Response, AppError> {
    // TODO: Implement refresh token grant
    Ok(Json(ErrorResponse {
        error: "unsupported_grant_type".to_string(),
        error_description: Some("Refresh token grant not yet implemented".to_string()),
    }).into_response())
}

/// User Registration Endpoint
/// GET /oauth/register
pub async fn register_form() -> Html<String> {
    let form = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Register</title>
            <style>
                body { font-family: Arial, sans-serif; max-width: 400px; margin: 50px auto; padding: 20px; }
                .form-group { margin-bottom: 15px; }
                label { display: block; margin-bottom: 5px; }
                input { width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; }
                button { background: #28a745; color: white; padding: 10px 20px; border: none; border-radius: 4px; cursor: pointer; }
                button:hover { background: #218838; }
                .login-link { margin-top: 20px; text-align: center; }
            </style>
        </head>
        <body>
            <h2>Create Account</h2>
            <form method="post" action="/oauth/register">
                <div class="form-group">
                    <label for="email">Email:</label>
                    <input type="email" id="email" name="email" required>
                </div>
                
                <div class="form-group">
                    <label for="password">Password:</label>
                    <input type="password" id="password" name="password" required>
                </div>
                
                <div class="form-group">
                    <label for="first_name">First Name:</label>
                    <input type="text" id="first_name" name="first_name" required>
                </div>
                
                <div class="form-group">
                    <label for="last_name">Last Name:</label>
                    <input type="text" id="last_name" name="last_name" required>
                </div>
                
                <div class="form-group">
                    <label for="username">Username (optional):</label>
                    <input type="text" id="username" name="username">
                </div>
                
                <button type="submit">Register</button>
            </form>
            
            <div class="login-link">
                <p>Already have an account? <a href="/oauth/authorize">Login here</a></p>
            </div>
        </body>
        </html>
    "#;
    
    Html(form.to_string())
}

/// User Registration Handler
/// POST /oauth/register
pub async fn register(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> Result<impl IntoResponse, AppError> {
    // Validate password strength
    PasswordService::validate_password_strength(&form.password)?;

    // Hash password
    let password_hash = PasswordService::hash_password(&form.password)?;

    // Check if user already exists
    let existing_user = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        form.email
    )
    .fetch_optional(&state.db)
    .await?;

    if existing_user.is_some() {
        return Err(AppError::BadRequest("User with this email already exists".to_string()));
    }

    // Create user
    let row = sqlx::query!(
        r#"
        INSERT INTO users (email, first_name, last_name, username, password_hash)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
        form.email,
        form.first_name,
        form.last_name,
        form.username,
        password_hash
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(UserRegistrationResponse {
        user_id: row.id.to_string(),
        message: "User registered successfully".to_string(),
    }))
}

fn redirect_with_error(
    redirect_uri: &str,
    error: &str,
    description: Option<&str>,
    state: Option<&str>,
) -> Result<Redirect, AppError> {
    let mut url = format!("{}?error={}", redirect_uri, error);
    
    if let Some(desc) = description {
        url.push_str(&format!("&error_description={}", urlencoding::encode(desc)));
    }
    
    if let Some(state_param) = state {
        url.push_str(&format!("&state={}", state_param));
    }
    
    Ok(Redirect::to(&url))
}