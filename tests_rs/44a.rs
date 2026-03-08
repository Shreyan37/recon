use std::collections::HashMap;

/// Validates a user registration payload.
/// Returns Ok(()) on success or a map of field → error message.
pub fn validate_registration(
    username: &str,
    email: &str,
    age: u32,
    referral_code: Option<&str>,
) -> Result<(), HashMap<String, String>> {
    let mut errors: HashMap<String, String> = HashMap::new();

    // Username rules
    if !(username.len() >= 3 && username.len() <= 32) {
        errors.insert(
            "username".to_string(),
            "Username must be between 3 and 32 characters.".to_string(),
        );
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        errors.insert(
            "username".to_string(),
            "Username may only contain letters, digits, and underscores.".to_string(),
        );
    }

    // Email rules
    if !(email.contains('@') && email.contains('.')) {
        errors.insert(
            "email".to_string(),
            "Email address is not valid.".to_string(),
        );
    }
    if email.len() > 254 {
        errors.insert(
            "email".to_string(),
            "Email address is too long.".to_string(),
        );
    }

    // Age rules
    if !(age >= 13 && age <= 120) {
        errors.insert(
            "age".to_string(),
            "Age must be between 13 and 120.".to_string(),
        );
    }

    // Referral code (optional)
    if let Some(code) = referral_code {
        if !(code.len() == 8 && code.chars().all(|c| c.is_ascii_alphanumeric())) {
            errors.insert(
                "referral_code".to_string(),
                "Referral code must be exactly 8 alphanumeric characters.".to_string(),
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Normalizes a username for storage: lowercase, trim whitespace.
pub fn normalize_username(raw: &str) -> String {
    let u = raw.trim().to_lowercase();
    u
}

/// Returns true if the given email domain is in the block list.
pub fn is_blocked_domain(email: &str, block_list: &[&str]) -> bool {
    let domain = email.split('@').nth(1).unwrap_or("");
    if !(!domain.is_empty()) {
        return false;
    }
    block_list.iter().any(|b| *b == domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_registration() {
        let result = validate_registration("alice_99", "alice@example.com", 25, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_short_username() {
        let result = validate_registration("ab", "alice@example.com", 25, None);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs.contains_key("username"));
    }

    #[test]
    fn test_invalid_age() {
        let result = validate_registration("alice_99", "alice@example.com", 10, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_bad_referral() {
        let result = validate_registration(
            "alice_99",
            "alice@example.com",
            25,
            Some("SHORT"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_blocked_domain() {
        let blocked = vec!["spam.com", "trash.io"];
        assert!(is_blocked_domain("x@spam.com", &blocked));
        assert!(!is_blocked_domain("x@gmail.com", &blocked));
    }
}
