use crate::error::{AppError, AppResult};

/// Validate and trim a required string field against a maximum length.
pub fn required_str(field: &str, value: &str, max: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!("{field} must not be empty")));
    }
    if trimmed.chars().count() > max {
        return Err(AppError::bad_request(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// Validate an optional string field; None or blank collapses to None.
pub fn optional_str(field: &str, value: &Option<String>, max: usize) -> AppResult<Option<String>> {
    match value {
        None => Ok(None),
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > max {
                return Err(AppError::bad_request(format!(
                    "{field} must be at most {max} characters"
                )));
            }
            Ok(Some(trimmed.to_string()))
        }
    }
}

/// Basic email validation with a length cap.
pub fn validate_email(value: &str) -> AppResult<String> {
    let email = value.trim().to_lowercase();
    if email.len() > 254 {
        return Err(AppError::bad_request("email is too long"));
    }
    // Minimal structural check: one @, non-empty local and domain, a dot in domain.
    let parts: Vec<&str> = email.split('@').collect();
    let valid = parts.len() == 2
        && !parts[0].is_empty()
        && parts[1].contains('.')
        && !parts[1].starts_with('.')
        && !parts[1].ends_with('.');
    if !valid {
        return Err(AppError::bad_request("invalid email address"));
    }
    Ok(email)
}

pub fn validate_password(value: &str) -> AppResult<()> {
    let len = value.chars().count();
    if len < 8 {
        return Err(AppError::bad_request("password must be at least 8 characters"));
    }
    if len > 256 {
        return Err(AppError::bad_request("password must be at most 256 characters"));
    }
    Ok(())
}

pub fn validate_range(field: &str, value: i32, min: i32, max: i32) -> AppResult<()> {
    if value < min || value > max {
        return Err(AppError::bad_request(format!(
            "{field} must be between {min} and {max}"
        )));
    }
    Ok(())
}
