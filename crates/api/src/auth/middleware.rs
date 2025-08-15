use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};
use crate::error::AppError;
use crate::state::AppState;

pub struct AuthMiddleware;

impl AuthMiddleware {
    pub async fn jwt_auth(
        State(state): State<AppState>,
        mut request: Request,
        next: Next,
    ) -> Result<Response, AppError> {
        let jwt_service = state.jwt_service();

        let auth_header = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid authorization header format".to_string()))?;

        let claims = jwt_service.verify_token(token)?;
        
        // Add user information to request extensions for use in handlers
        request.extensions_mut().insert(claims);
        
        Ok(next.run(request).await)
    }

    pub async fn optional_jwt_auth(
        State(state): State<AppState>,
        mut request: Request,
        next: Next,
    ) -> Response {
        let jwt_service = state.jwt_service();

        if let Some(auth_header) = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
        {
            if let Some(token) = auth_header.strip_prefix("Bearer ") {
                if let Ok(claims) = jwt_service.verify_token(token) {
                    request.extensions_mut().insert(claims);
                }
            }
        }
        
        next.run(request).await
    }
}