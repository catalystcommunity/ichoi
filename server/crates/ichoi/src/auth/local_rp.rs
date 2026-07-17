//! DNS-less LinkKeys relying-party integration. The SDK verifies protocol
//! facts; Ichoi owns persistence, admission, accounts, and sessions.

use std::sync::Arc;

use chrono::{Duration, Utc};
use linkkeys_local_rp::{
    begin_local_login, complete_local_login, generate_local_rp_identity,
    local_rp_identity_from_bytes, local_rp_identity_to_bytes, BeginLocalLoginConfig,
    CompleteLocalLoginConfig, GenerateLocalRpIdentityConfig, LocalRpKeyMaterial,
};

use crate::config::Config;
use crate::db::{models, store, SqlitePool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSelector {
    pub domain: String,
    pub handle: Option<String>,
}

/// Parse `[handle@]domain`. Domains and handles are canonicalized to lower
/// case because LinkKeys usernames are case-insensitive at authentication.
pub fn parse_selector(input: &str) -> anyhow::Result<LoginSelector> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        anyhow::bail!("identity must be [handle@]domain");
    }
    let (handle, domain) = match input.rsplit_once('@') {
        Some((handle, domain)) if !handle.is_empty() => (Some(handle), domain),
        Some(_) => anyhow::bail!("identity handle and domain must not be empty"),
        None => (None, input),
    };
    validate_domain(domain)?;
    if let Some(handle) = handle {
        if handle.chars().any(|c| c.is_control() || c == '@') {
            anyhow::bail!("identity handle is invalid");
        }
    }
    Ok(LoginSelector {
        domain: domain.to_ascii_lowercase(),
        handle: handle.map(str::to_ascii_lowercase),
    })
}

fn validate_domain(domain: &str) -> anyhow::Result<()> {
    if domain.is_empty()
        || domain.len() > 253
        || domain.contains('/')
        || domain.contains(':')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    {
        anyhow::bail!("identity domain is invalid");
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct VerifiedIdentity {
    pub user_id: String,
    pub domain: String,
    pub handle: String,
    pub display_name: Option<String>,
}

pub trait Backend: Send + Sync {
    fn fingerprint(&self) -> &str;
    fn begin(&self, domain: &str, callback_url: &str) -> anyhow::Result<(String, String)>;
    fn complete(
        &self,
        pending_json: &str,
        encrypted_token: &str,
        arrived_url: &str,
    ) -> anyhow::Result<VerifiedIdentity>;
}

pub type DynBackend = Arc<dyn Backend>;

pub struct SdkBackend {
    identity: LocalRpKeyMaterial,
}

impl SdkBackend {
    pub fn load(pool: &SqlitePool) -> anyhow::Result<SdkBackend> {
        let mut conn = pool.get()?;
        let stored = store::active_local_rp_identity(&mut conn)?
            .ok_or_else(|| anyhow::anyhow!("local RP identity is not initialized"))?;
        Ok(SdkBackend {
            identity: local_rp_identity_from_bytes(&stored.identity_bundle)?,
        })
    }
}

impl Backend for SdkBackend {
    fn fingerprint(&self) -> &str {
        &self.identity.fingerprint
    }

    fn begin(&self, domain: &str, callback_url: &str) -> anyhow::Result<(String, String)> {
        let (redirect, pending) = begin_local_login(BeginLocalLoginConfig::new(
            &self.identity,
            callback_url,
            domain,
            Utc::now(),
        ))?;
        Ok((redirect.redirect_url, serde_json::to_string(&pending)?))
    }

    fn complete(
        &self,
        pending_json: &str,
        encrypted_token: &str,
        arrived_url: &str,
    ) -> anyhow::Result<VerifiedIdentity> {
        let pending = serde_json::from_str(pending_json)?;
        let verified = complete_local_login(CompleteLocalLoginConfig::new(
            &self.identity,
            &pending,
            encrypted_token,
            arrived_url,
            Utc::now(),
        ))?;
        let claim_text = |kind: &str| -> anyhow::Result<Option<String>> {
            verified
                .claims
                .iter()
                .find(|claim| claim.claim_type == kind)
                .map(|claim| {
                    String::from_utf8(claim.claim_value.clone())
                        .map_err(|_| anyhow::anyhow!("verified {kind} claim is not UTF-8"))
                })
                .transpose()
        };
        let handle = claim_text("handle")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("verified login did not contain a handle claim"))?;
        Ok(VerifiedIdentity {
            user_id: verified.user_id,
            domain: verified.user_domain.to_ascii_lowercase(),
            handle: handle.to_ascii_lowercase(),
            display_name: claim_text("display_name")?.filter(|value| !value.trim().is_empty()),
        })
    }
}

/// Seed configured admission rules and create the SDK identity once. This is
/// called after migrations and before any network listener starts.
pub fn initialize_database(pool: &SqlitePool, config: &Config) -> anyhow::Result<()> {
    if !config.linkkeys_local_rp {
        return Ok(());
    }
    let name = config
        .linkkeys_local_rp_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("local RP mode requires ICHOI_LINKKEYS_LOCAL_RP_NAME"))?;
    if config.linkkeys_trusted_identities.is_empty() {
        anyhow::bail!("local RP mode requires ICHOI_LINKKEYS_TRUSTED_IDENTITIES");
    }

    let mut conn = pool.get()?;
    for configured in &config.linkkeys_trusted_identities {
        let selector = parse_selector(configured)?;
        store::add_linkkeys_trust(
            &mut conn,
            &selector.domain,
            selector.handle.as_deref(),
            "config",
        )?;
        if selector.handle.is_none() {
            store::add_trusted_domain(&mut conn, &selector.domain)?;
        }
    }

    if let Some(existing) = store::active_local_rp_identity(&mut conn)? {
        let decoded = local_rp_identity_from_bytes(&existing.identity_bundle)
            .map_err(|e| anyhow::anyhow!("stored local RP identity is invalid: {e}"))?;
        if decoded.fingerprint != existing.fingerprint {
            anyhow::bail!("stored local RP identity fingerprint does not match its key bundle");
        }
        if is_expired(&existing.expires_at) {
            anyhow::bail!("stored local RP identity has expired; explicit rotation is required");
        }
        if existing.name != name {
            log::warn!(
                "configured local RP name {:?} differs from stored identity name {:?}; keeping the stable stored identity",
                name,
                existing.name
            );
        }
        log::info!("LinkKeys local RP fingerprint: {}", existing.fingerprint);
        return Ok(());
    }

    let now = Utc::now();
    let identity = generate_local_rp_identity(GenerateLocalRpIdentityConfig::new(name, now))?;
    let row = models::LinkkeysLocalRpIdentity {
        fingerprint: identity.fingerprint.clone(),
        name: name.to_string(),
        identity_bundle: local_rp_identity_to_bytes(&identity),
        active: 1,
        created_at: now.to_rfc3339(),
        expires_at: (now + Duration::days(3650)).to_rfc3339(),
    };
    store::insert_local_rp_identity(&mut conn, &row)?;
    log::info!(
        "generated LinkKeys local RP fingerprint: {}",
        row.fingerprint
    );
    Ok(())
}

pub fn is_expired(expires_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(expires_at)
        .map(|expires| expires.with_timezone(&Utc) <= Utc::now())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_domain_and_optional_handle() {
        assert_eq!(
            parse_selector("Alice@Family.Example").unwrap(),
            LoginSelector {
                domain: "family.example".into(),
                handle: Some("alice".into())
            }
        );
        assert_eq!(
            parse_selector("Family.Example").unwrap(),
            LoginSelector {
                domain: "family.example".into(),
                handle: None
            }
        );
    }

    #[test]
    fn rejects_addresses_and_malformed_domains() {
        for bad in [
            "",
            "@example.com",
            "alice@",
            "http://example.com",
            "a..b",
            "-a.com",
        ] {
            assert!(parse_selector(bad).is_err(), "accepted {bad:?}");
        }
    }
}
