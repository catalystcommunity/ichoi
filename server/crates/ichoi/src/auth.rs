//! Sessions and roles (§7). Ichoi holds no passwords. Identity comes from LinkKeys; after a
//! successful assertion Ichoi mints a high-entropy opaque token and stores only its SHA-256.

use rand::RngCore;
use sha2::{Digest, Sha256};

/// A minted session token and the hash we persist. The plaintext is returned to the client
/// once and never stored.
pub struct MintedToken {
    pub token: String,
    pub sha256_hex: String,
}

/// Mint a 256-bit random token, hex-encoded, plus its SHA-256 hash.
pub fn mint_token() -> MintedToken {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    MintedToken {
        sha256_hex: sha256_hex(&token),
        token,
    }
}

/// Hash a presented token for lookup.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleLevel {
    Guest,
    Member,
    Admin,
}

impl RoleLevel {
    pub fn parse(s: &str) -> RoleLevel {
        match s.to_ascii_lowercase().as_str() {
            "admin" => RoleLevel::Admin,
            "guest" => RoleLevel::Guest,
            _ => RoleLevel::Member,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RoleLevel::Guest => "guest",
            RoleLevel::Member => "member",
            RoleLevel::Admin => "admin",
        }
    }

    /// Ordering for capability checks: guest < member < admin.
    pub fn rank(self) -> u8 {
        match self {
            RoleLevel::Guest => 0,
            RoleLevel::Member => 1,
            RoleLevel::Admin => 2,
        }
    }

    pub fn at_least(self, needed: RoleLevel) -> bool {
        self.rank() >= needed.rank()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_hash_matches() {
        let a = mint_token();
        let b = mint_token();
        assert_ne!(a.token, b.token);
        assert_eq!(a.sha256_hex, sha256_hex(&a.token));
        assert_eq!(a.token.len(), 64);
    }

    #[test]
    fn role_ranking() {
        assert!(RoleLevel::Admin.at_least(RoleLevel::Member));
        assert!(!RoleLevel::Guest.at_least(RoleLevel::Member));
        assert_eq!(RoleLevel::parse("admin"), RoleLevel::Admin);
        assert_eq!(RoleLevel::parse("nonsense"), RoleLevel::Member);
    }
}
