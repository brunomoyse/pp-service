use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct Session {
    pub user_id: Uuid,
    pub email: String,
    pub csrf_token: Option<String>,
}

#[derive(Clone)]
pub struct SessionService {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionService {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_session(&self, session_id: String, user_id: Uuid, email: String) {
        let session = Session {
            user_id,
            email,
            csrf_token: None,
        };
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session);
    }

    pub async fn store_csrf_token(&self, session_id: String, csrf_token: String) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.csrf_token = Some(csrf_token);
        }
    }

    pub async fn verify_csrf_token(&self, session_id: &str, csrf_token: &str) -> Result<(), AppError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AppError::Unauthorized("Invalid session".to_string()))?;

        match &session.csrf_token {
            Some(stored_token) if stored_token == csrf_token => Ok(()),
            _ => Err(AppError::Unauthorized("Invalid CSRF token".to_string())),
        }
    }

    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    pub async fn cleanup_expired_sessions(&self) {
        // In a real implementation, you'd check expiration times
        // For now, this is a placeholder for future enhancement
    }
}