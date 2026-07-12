//! Client-satellite naming (§6.4): a client that enables satellite mode becomes a shared
//! target named `"<Handle>'s <suffix>"`. The `"<Handle>'s "` prefix is fixed; the suffix
//! is user-settable (uniqueness is enforced per server by the store, not here).

use crate::error::DomainError;

pub const MAX_SUFFIX_LEN: usize = 48;
pub const DEFAULT_SUFFIX: &str = "Device";

/// Build the shared-target display name. The prefix is not configurable.
pub fn share_name(handle: &str, suffix: &str) -> String {
    format!("{handle}'s {suffix}")
}

/// Validate a user-supplied suffix. Uniqueness is a store concern, checked elsewhere.
pub fn validate_suffix(suffix: &str) -> Result<(), DomainError> {
    let trimmed = suffix.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_SUFFIX_LEN {
        return Err(DomainError::InvalidSuffix(suffix.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_name_with_fixed_prefix() {
        assert_eq!(share_name("Ann", "Device"), "Ann's Device");
        assert_eq!(share_name("Ann", "Kitchen Tablet"), "Ann's Kitchen Tablet");
    }

    #[test]
    fn validates_suffix() {
        assert!(validate_suffix("Device").is_ok());
        assert!(validate_suffix("").is_err());
        assert!(validate_suffix("   ").is_err());
        assert!(validate_suffix(&"x".repeat(MAX_SUFFIX_LEN + 1)).is_err());
    }
}
