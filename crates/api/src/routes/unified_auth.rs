use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
};
use serde::Deserialize;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct AuthChoiceQuery {
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
}

/// Unified Authentication Choice Page
/// GET /auth/choose - Shows users choice between Google OAuth and Custom OAuth
pub async fn auth_choice(
    State(_state): State<AppState>,
    Query(params): Query<AuthChoiceQuery>,
) -> Result<impl IntoResponse, AppError> {
    let choice_page = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Choose Authentication Method</title>
            <style>
                body {{
                    font-family: Arial, sans-serif;
                    max-width: 600px;
                    margin: 50px auto;
                    padding: 20px;
                    background-color: #f5f5f5;
                }}
                .auth-container {{
                    background: white;
                    padding: 40px;
                    border-radius: 8px;
                    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
                }}
                .auth-option {{
                    border: 2px solid #e0e0e0;
                    border-radius: 8px;
                    padding: 30px;
                    margin: 20px 0;
                    text-align: center;
                    transition: all 0.3s ease;
                    cursor: pointer;
                    text-decoration: none;
                    display: block;
                    color: inherit;
                }}
                .auth-option:hover {{
                    border-color: #007bff;
                    transform: translateY(-2px);
                    box-shadow: 0 4px 12px rgba(0,123,255,0.15);
                }}
                .auth-option h3 {{
                    margin: 0 0 10px 0;
                    color: #333;
                }}
                .auth-option p {{
                    margin: 0;
                    color: #666;
                    font-size: 14px;
                }}
                .google-auth {{
                    background: linear-gradient(135deg, #4285f4 0%, #34a853 100%);
                    color: white;
                }}
                .google-auth:hover {{
                    border-color: #4285f4;
                    color: white;
                }}
                .google-auth h3, .google-auth p {{
                    color: white;
                }}
                .custom-auth {{
                    background: linear-gradient(135deg, #6c5ce7 0%, #a29bfe 100%);
                    color: white;
                }}
                .custom-auth:hover {{
                    border-color: #6c5ce7;
                    color: white;
                }}
                .custom-auth h3, .custom-auth p {{
                    color: white;
                }}
                .title {{
                    text-align: center;
                    margin-bottom: 30px;
                    color: #333;
                }}
                .or-divider {{
                    text-align: center;
                    margin: 20px 0;
                    color: #999;
                    position: relative;
                }}
                .or-divider::before {{
                    content: '';
                    position: absolute;
                    top: 50%;
                    left: 0;
                    right: 0;
                    height: 1px;
                    background: #e0e0e0;
                    z-index: 1;
                }}
                .or-divider span {{
                    background: white;
                    padding: 0 20px;
                    position: relative;
                    z-index: 2;
                }}
            </style>
        </head>
        <body>
            <div class="auth-container">
                <h1 class="title">Choose How to Sign In</h1>
                
                <a href="/auth/google/authorize?{}" class="auth-option google-auth">
                    <h3>🔐 Continue with Google</h3>
                    <p>Sign in using your Google account</p>
                </a>
                
                <div class="or-divider">
                    <span>OR</span>
                </div>
                
                <a href="/oauth/authorize?{}" class="auth-option custom-auth">
                    <h3>👤 Use Your Account</h3>
                    <p>Sign in with your existing account or create a new one</p>
                </a>
            </div>
        </body>
        </html>
        "#,
        build_query_string(&params),
        build_oauth_query_string(&params)
    );

    Ok(Html(choice_page).into_response())
}

fn build_query_string(params: &AuthChoiceQuery) -> String {
    let mut query_parts = Vec::new();
    
    if let Some(ref redirect_uri) = params.redirect_uri {
        query_parts.push(format!("redirect_uri={}", urlencoding::encode(redirect_uri)));
    }
    if let Some(ref scope) = params.scope {
        query_parts.push(format!("scope={}", urlencoding::encode(scope)));
    }
    if let Some(ref state) = params.state {
        query_parts.push(format!("state={}", urlencoding::encode(state)));
    }
    
    query_parts.join("&")
}

fn build_oauth_query_string(params: &AuthChoiceQuery) -> String {
    let mut query_parts = vec!["response_type=code".to_string()];
    
    if let Some(ref client_id) = params.client_id {
        query_parts.push(format!("client_id={}", urlencoding::encode(client_id)));
    }
    if let Some(ref redirect_uri) = params.redirect_uri {
        query_parts.push(format!("redirect_uri={}", urlencoding::encode(redirect_uri)));
    }
    if let Some(ref scope) = params.scope {
        query_parts.push(format!("scope={}", urlencoding::encode(scope)));
    }
    if let Some(ref state) = params.state {
        query_parts.push(format!("state={}", urlencoding::encode(state)));
    }
    
    query_parts.join("&")
}