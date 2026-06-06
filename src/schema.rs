//! Username validation and normalization.

/// Validates and normalizes a username: strips leading @, checks 1-15 chars, [a-zA-Z0-9_].
/// Returns the normalized username (without @).
pub fn validate_username(username: &str) -> Result<&str, Box<dyn std::error::Error + Send + Sync>> {
    let clean = username.strip_prefix('@').unwrap_or(username);
    if clean.is_empty() || clean.len() > 15 {
        return Err(format!("username must be 1-15 characters, got '{}'", username).into());
    }
    if !clean.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "username must be alphanumeric or underscore, got '{}'",
            username
        )
        .into());
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_username_valid() {
        assert_eq!(validate_username("elonmusk").expect("test"), "elonmusk");
        assert_eq!(validate_username("a").expect("test"), "a");
        assert_eq!(
            validate_username("user_name_123").expect("test"),
            "user_name_123"
        );
        assert_eq!(validate_username("A_B_C").expect("test"), "A_B_C");
    }

    #[test]
    fn validate_username_strips_at() {
        assert_eq!(validate_username("@elonmusk").expect("test"), "elonmusk");
    }

    #[test]
    fn validate_username_empty() {
        assert!(validate_username("").is_err());
        assert!(validate_username("@").is_err());
    }

    #[test]
    fn validate_username_too_long() {
        assert!(validate_username("abcdefghijklmnop").is_err()); // 16 chars
    }

    #[test]
    fn validate_username_invalid_chars() {
        assert!(validate_username("user-name").is_err());
        assert!(validate_username("user.name").is_err());
        assert!(validate_username("user name").is_err());
        assert!(validate_username("user@name").is_err());
    }
}
