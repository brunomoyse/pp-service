use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

use crate::auth::Claims;
use crate::error::AppError;
use crate::state::AppState;

/// JWT middleware that extracts and validates JWT tokens from Authorization header
/// and adds claims to the request extensions for GraphQL context.
/// If a token is present but invalid, logs the error but allows the request to proceed.
/// Individual resolvers must enforce authentication as needed.
pub async fn jwt_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Try to extract Authorization header
    if let Some(auth_header) = request.headers().get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            // Check if it's a Bearer token
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                // Verify the token
                match state.jwt_service().verify_token(token) {
                    Ok(claims) => {
                        // Add claims to request extensions so GraphQL can access them
                        request.extensions_mut().insert::<Claims>(claims);
                    }
                    Err(e) => {
                        // Log the error but continue - let GraphQL resolvers enforce auth
                        tracing::debug!("JWT validation failed: {}", e);
                    }
                }
            }
        }
    }

    // Continue to the next middleware/handler (with or without claims)
    Ok(next.run(request).await)
}
