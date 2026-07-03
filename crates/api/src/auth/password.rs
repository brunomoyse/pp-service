use std::sync::LazyLock;

use bcrypt::{hash, verify, DEFAULT_COST};

use crate::error::AppError;

/// A real bcrypt hash of a fixed throwaway value, computed once. Verifying an
/// attempt against it burns the same time as a genuine check.
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| hash("timing-equalizer", DEFAULT_COST).expect("bcrypt dummy hash"));

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

    /// Burn a bcrypt verification against a dummy hash so "unknown email" and
    /// "wrong password" take indistinguishable time (anti-enumeration).
    pub fn verify_dummy(password: &str) {
        let _ = verify(password, &DUMMY_HASH);
    }

    pub fn validate_password_strength(password: &str) -> Result<(), AppError> {
        if password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        // bcrypt only hashes the first 72 bytes and silently ignores the rest,
        // so a longer password is weaker than the user thinks. Reject rather
        // than truncate, so what they typed is what protects the account.
        if password.len() > 72 {
            return Err(AppError::BadRequest(
                "Password must be at most 72 characters long".to_string(),
            ));
        }

        let has_letter = password.chars().any(|c| c.is_alphabetic());
        let has_digit = password.chars().any(|c| c.is_numeric());

        if !has_letter || !has_digit {
            return Err(AppError::BadRequest(
                "Password must contain at least one letter and one number".to_string(),
            ));
        }

        // Block the handful of passwords that pass the rules above but are the
        // first things an attacker tries.
        const COMMON: &[&str] = &[
            "password",
            "password1",
            "password123",
            "12345678",
            "123456789",
            "1234567890",
            "qwerty123",
            "abc12345",
            "letmein1",
            "iloveyou1",
            "admin123",
            "welcome1",
        ];
        if COMMON.contains(&password.to_ascii_lowercase().as_str()) {
            return Err(AppError::BadRequest(
                "This password is too common; please choose a less guessable one".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_weak_and_accepts_strong() {
        assert!(PasswordService::validate_password_strength("short1").is_err()); // too short
        assert!(PasswordService::validate_password_strength("alphabetical").is_err()); // no digit
        assert!(PasswordService::validate_password_strength("12345678").is_err()); // common + no letter
        assert!(PasswordService::validate_password_strength("Password123").is_err()); // common
        assert!(PasswordService::validate_password_strength(&"a1".repeat(40)).is_err()); // > 72 bytes
        assert!(PasswordService::validate_password_strength("Tr0ubad0ur42").is_ok());
    }
}
