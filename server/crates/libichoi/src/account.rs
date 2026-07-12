//! Account identity: `AccountId` is the string `uuid@domain.tld` (§8). The domain is part
//! of the identity because Ichoi trusts users from other LinkKeys domains.

use crate::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRef {
    pub uuid: String,
    pub domain: String,
}

impl AccountRef {
    /// Parse `uuid@domain.tld`. The uuid is split on the *last* `@` so the uuid portion
    /// may itself be any non-empty token; the domain must be non-empty.
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        let (uuid, domain) = s
            .rsplit_once('@')
            .ok_or_else(|| DomainError::InvalidAccountId(s.to_string()))?;
        if uuid.is_empty() || domain.is_empty() {
            return Err(DomainError::InvalidAccountId(s.to_string()));
        }
        Ok(AccountRef {
            uuid: uuid.to_string(),
            domain: domain.to_string(),
        })
    }

    pub fn as_id(&self) -> String {
        format!("{}@{}", self.uuid, self.domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uuid_at_domain() {
        let a = AccountRef::parse("018f-uuid@example.com").unwrap();
        assert_eq!(a.uuid, "018f-uuid");
        assert_eq!(a.domain, "example.com");
        assert_eq!(a.as_id(), "018f-uuid@example.com");
    }

    #[test]
    fn rejects_missing_domain() {
        assert!(AccountRef::parse("nobody").is_err());
        assert!(AccountRef::parse("uuid@").is_err());
        assert!(AccountRef::parse("@domain").is_err());
    }
}
