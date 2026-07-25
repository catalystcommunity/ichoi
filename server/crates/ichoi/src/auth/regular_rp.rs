//! LinkKeys login through a DNS-pinned external RP service. Ichoi keeps its
//! sessions and admission policy locally while delegating protocol signing,
//! decryption, assertion verification, and user-info retrieval to the RP.

use hickory_resolver::Resolver;
use liblinkkeys::generated::{self, types as lk};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::local_rp::VerifiedIdentity;
use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingLogin {
    pub nonce: String,
    pub user_domain: String,
    pub callback_url: String,
    pub api_base: String,
}

pub trait Backend: Send + Sync {
    fn begin(
        &self,
        domain: &str,
        user_hint: Option<&str>,
        callback_url: &str,
    ) -> anyhow::Result<(String, String)>;

    fn complete(
        &self,
        pending_json: &str,
        encrypted_token: &str,
    ) -> anyhow::Result<VerifiedIdentity>;
}

pub type DynBackend = Arc<dyn Backend>;

pub struct RpcBackend {
    addr: String,
    fingerprints: Vec<String>,
    api_key: String,
    rp_domain: String,
}

impl RpcBackend {
    pub fn from_config(config: &Config) -> anyhow::Result<RpcBackend> {
        Ok(RpcBackend {
            addr: required(&config.linkkeys_rp_addr, "ICHOI_LINKKEYS_RP_ADDR")?.to_string(),
            fingerprints: config.linkkeys_rp_fingerprints.clone(),
            api_key: required(&config.linkkeys_rp_api_key, "ICHOI_LINKKEYS_RP_API_KEY")?
                .to_string(),
            rp_domain: required(&config.linkkeys_rp_domain, "ICHOI_LINKKEYS_RP_DOMAIN")?
                .to_string(),
        })
    }

    fn call(
        &self,
        op: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, linkkeys_rpc_client::ClientError> {
        linkkeys_rpc_client::send_request(
            &self.addr,
            self.fingerprints.clone(),
            None,
            "Rp",
            op,
            payload,
            Some(&self.api_key),
        )
    }
}

impl Backend for RpcBackend {
    fn begin(
        &self,
        domain: &str,
        user_hint: Option<&str>,
        callback_url: &str,
    ) -> anyhow::Result<(String, String)> {
        let api_base = resolve_api_base(domain);
        let nonce = crate::auth::mint_token().token;
        let response = self.call(
            "sign-request",
            generated::encode_rp_sign_request(&lk::RpSignRequest {
                callback_url: callback_url.to_string(),
                nonce: nonce.clone(),
                requested_claims: Some(login_claims()),
                flow_context: None,
            }),
        )?;
        let signed = generated::decode_rp_sign_response(&response)?;
        let redirect_url = format!(
            "{}/auth/authorize?callback_url={}&nonce={}&user_hint={}&relying_party={}&signed_request={}",
            api_base.trim_end_matches('/'),
            percent_encode(callback_url),
            percent_encode(&nonce),
            percent_encode(user_hint.unwrap_or("")),
            percent_encode(&self.rp_domain),
            percent_encode(&signed.signed_request),
        );
        let pending = PendingLogin {
            nonce,
            user_domain: domain.to_string(),
            callback_url: callback_url.to_string(),
            api_base,
        };
        Ok((redirect_url, serde_json::to_string(&pending)?))
    }

    fn complete(
        &self,
        pending_json: &str,
        encrypted_token: &str,
    ) -> anyhow::Result<VerifiedIdentity> {
        let pending: PendingLogin = serde_json::from_str(pending_json)?;
        let decrypted = self.call(
            "decrypt-token",
            generated::encode_rp_decrypt_request(&lk::RpDecryptRequest {
                encrypted_token: encrypted_token.to_string(),
            }),
        )?;
        let decrypted = generated::decode_rp_decrypt_response(&decrypted)?;

        let verified = self.call(
            "verify-assertion",
            generated::encode_rp_verify_request(&lk::RpVerifyRequest {
                signed_assertion: decrypted.signed_assertion.clone(),
                expected_domain: pending.user_domain.clone(),
            }),
        )?;
        let verified = generated::decode_rp_verify_response(&verified)?;
        anyhow::ensure!(verified.verified, "LinkKeys assertion was not verified");
        anyhow::ensure!(
            verified.assertion.nonce == pending.nonce,
            "LinkKeys assertion nonce mismatch"
        );
        anyhow::ensure!(
            verified
                .assertion
                .domain
                .eq_ignore_ascii_case(&pending.user_domain),
            "LinkKeys assertion domain mismatch"
        );

        let info = self.call(
            "userinfo-fetch",
            generated::encode_rp_user_info_request(&lk::RpUserInfoRequest {
                token: decrypted.signed_assertion,
                api_base: pending.api_base,
                domain: pending.user_domain.clone(),
            }),
        )?;
        let info = generated::decode_user_info(&info)?;
        anyhow::ensure!(
            info.user_id == verified.assertion.user_id
                && info.domain.eq_ignore_ascii_case(&verified.assertion.domain),
            "LinkKeys user info does not match the verified assertion"
        );
        let handle = info
            .claims
            .iter()
            .find(|claim| claim.claim_type == "handle")
            .map(|claim| String::from_utf8(claim.claim_value.clone()))
            .transpose()?
            .filter(|handle| !handle.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("verified login did not contain a handle claim"))?;

        Ok(VerifiedIdentity {
            user_id: info.user_id,
            domain: info.domain.to_ascii_lowercase(),
            handle: handle.to_ascii_lowercase(),
            display_name: (!info.display_name.trim().is_empty()).then_some(info.display_name),
        })
    }
}

fn required<'a>(value: &'a Option<String>, name: &str) -> anyhow::Result<&'a str> {
    value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("ICHOI_LINKKEYS_RP=true requires {name}"))
}

fn login_claims() -> lk::ClaimRequest {
    lk::ClaimRequest {
        required: vec![lk::RequestedClaim {
            claim_type: "handle".to_string(),
            datatype: "text".to_string(),
        }],
        optional: vec![lk::RequestedClaim {
            claim_type: "display_name".to_string(),
            datatype: "text".to_string(),
        }],
    }
}

fn resolve_api_base(domain: &str) -> String {
    let fallback = || format!("https://{domain}");
    let Ok(resolver) = Resolver::from_system_conf() else {
        return fallback();
    };
    let dns_name = format!("_linkkeys_apis.{domain}");
    let Ok(records) = resolver.txt_lookup(dns_name) else {
        return fallback();
    };
    for record in records.iter() {
        let text = record.to_string();
        if text.starts_with("v=lk1 ") {
            if let Some(endpoint) = text
                .split_whitespace()
                .find_map(|part| part.strip_prefix("https="))
            {
                return format!("https://{endpoint}");
            }
        }
    }
    fallback()
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_redirect_parameters() {
        assert_eq!(percent_encode("a+b@example.com"), "a%2Bb%40example.com");
    }

    #[test]
    fn login_claims_require_a_handle() {
        let claims = login_claims();
        assert_eq!(claims.required.len(), 1);
        assert_eq!(claims.required[0].claim_type, "handle");
    }
}
