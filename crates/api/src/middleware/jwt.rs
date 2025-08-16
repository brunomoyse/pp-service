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
/// and adds claims to the request extensions for GraphQL context
pub async fn jwt_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Try to extract Authorization header
    if let Some(auth_header) = request.headers().get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            // Check if it's a Bearer token
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..]; // Remove "Bearer " prefix
                
                // Verify the token
                match state.jwt_service().verify_token(token) {
                    Ok(claims) => {
                        // Add claims to request extensions so GraphQL can access them
                        request.extensions_mut().insert(claims);
                    }
                    Err(_) => {
                        // Invalid token - we don't return an error here,
                        // just don't add claims to the request
                        // This allows unauthenticated requests to proceed
                    }
                }
            }
        }
    }
    
    // Continue to the next middleware/handler
    Ok(next.run(request).await)
}