use bcrypt::{hash, verify, DEFAULT_COST};

use crate::error::AppError;

pub struct PasswordService;

impl PasswordService {
    pub fn hash_password(password: &str) -> Result<String, AppError> {
        hash(password, DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
        verify(password, hash)
            .map_err(|e| AppError::Internal(format!("Failed to verify password: {}", e)))
    }

    pub fn validate_password_strength(password: &str) -> Result<(), AppError> {
        if password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        let has_letter = password.chars().any(|c| c.is_alphabetic());
        let has_digit = password.chars().any(|c| c.is_numeric());

        if !has_letter || !has_digit {
            return Err(AppError::BadRequest(
                "Password must contain at least one letter and one number".to_string(),
            ));
        }

        Ok(())
    }
}
