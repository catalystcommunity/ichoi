//! DNS-less TLS for native CSIL connections.
//!
//! The core certificate fingerprint is its transport identity. Satellites pin one or more
//! fingerprints out of band, so neither public DNS nor a public CA is involved.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::config::Config;

pub const FINGERPRINT_PREFIX: &str = "sha256:";

pub struct CoreIdentity {
    pub server_config: Arc<rustls::ServerConfig>,
    pub fingerprint: String,
    pub cert_path: PathBuf,
}

pub fn core_identity(config: &Config) -> anyhow::Result<CoreIdentity> {
    let cert_path = config
        .tls_cert
        .clone()
        .unwrap_or_else(|| config.tls_dir().join("csil-cert.der"));
    let key_path = config
        .tls_key
        .clone()
        .unwrap_or_else(|| config.tls_dir().join("csil-key.der"));
    ensure_identity(&cert_path, &key_path)?;

    let cert_bytes = std::fs::read(&cert_path)
        .map_err(|e| anyhow::anyhow!("reading TLS certificate {}: {e}", cert_path.display()))?;
    let key_bytes = std::fs::read(&key_path)
        .map_err(|e| anyhow::anyhow!("reading TLS private key {}: {e}", key_path.display()))?;
    let cert = CertificateDer::from(cert_bytes.clone());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_bytes));
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;

    Ok(CoreIdentity {
        server_config: Arc::new(server_config),
        fingerprint: fingerprint(&cert_bytes)?,
        cert_path,
    })
}

fn ensure_identity(cert_path: &Path, key_path: &Path) -> anyhow::Result<()> {
    match (cert_path.exists(), key_path.exists()) {
        (true, true) => return Ok(()),
        (true, false) | (false, true) => anyhow::bail!(
            "incomplete TLS identity: {} and {} must both exist or both be absent",
            cert_path.display(),
            key_path.display()
        ),
        (false, false) => {}
    }
    if let Some(parent) = cert_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["ichoi-core.invalid".to_string()])?;
    write_private(key_path, &key_pair.serialize_der())?;
    write_public(cert_path, cert.der().as_ref())?;
    Ok(())
}

fn write_public(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(windows)]
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    use std::io::Write;
    options.open(path)?.write_all(bytes)?;
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "CURRENT_USER".into());
    let status = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &format!("{user}:F")])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to restrict TLS private-key ACL with icacls ({status})");
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    use std::io::Write;
    options.open(path)?.write_all(bytes)?;
    Ok(())
}

pub fn fingerprint(cert_der: &[u8]) -> anyhow::Result<String> {
    let (_, cert) = x509_parser::parse_x509_certificate(cert_der)
        .map_err(|_| anyhow::anyhow!("TLS certificate is not valid X.509 DER"))?;
    Ok(format!(
        "{FINGERPRINT_PREFIX}{}",
        hex::encode(Sha256::digest(cert.public_key().raw))
    ))
}

pub fn client_config(pins: &[String]) -> anyhow::Result<Arc<rustls::ClientConfig>> {
    let verifier = Arc::new(PinnedCertificateVerifier::new(pins)?);
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

#[derive(Clone)]
struct PinnedCertificateVerifier {
    pins: Vec<[u8; 32]>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl fmt::Debug for PinnedCertificateVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PinnedCertificateVerifier")
            .field("pin_count", &self.pins.len())
            .finish()
    }
}

impl PinnedCertificateVerifier {
    fn new(values: &[String]) -> anyhow::Result<Self> {
        if values.is_empty() {
            anyhow::bail!("satellite role requires at least one ICHOI_CORE_KEYS fingerprint");
        }
        let mut pins = Vec::with_capacity(values.len());
        for value in values {
            let raw = value.strip_prefix(FINGERPRINT_PREFIX).unwrap_or(value);
            let decoded = hex::decode(raw)
                .map_err(|_| anyhow::anyhow!("invalid core fingerprint {value:?}"))?;
            let pin: [u8; 32] = decoded.try_into().map_err(|_| {
                anyhow::anyhow!("core fingerprint must contain 32 bytes: {value:?}")
            })?;
            pins.push(pin);
        }
        Ok(Self {
            pins,
            algorithms: rustls::crypto::ring::default_provider().signature_verification_algorithms,
        })
    }
}

impl ServerCertVerifier for PinnedCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let (_, cert) = x509_parser::parse_x509_certificate(end_entity.as_ref())
            .map_err(|_| Error::InvalidCertificate(CertificateError::BadEncoding))?;
        let actual: [u8; 32] = Sha256::digest(cert.public_key().raw).into();
        if self.pins.iter().any(|pin| pin == &actual) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(Error::InvalidCertificate(CertificateError::UnknownIssuer))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

pub fn logical_server_name() -> ServerName<'static> {
    ServerName::try_from("ichoi-core.invalid")
        .expect("static Ichoi TLS server name is valid")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn test_server() -> (Arc<rustls::ServerConfig>, String) {
        let rcgen::CertifiedKey { cert, key_pair } =
            rcgen::generate_simple_self_signed(vec!["ichoi-core.invalid".into()]).unwrap();
        let fingerprint = fingerprint(cert.der()).unwrap();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], key)
            .unwrap();
        (Arc::new(config), fingerprint)
    }

    #[test]
    fn fingerprint_round_trip_and_validation() {
        let rcgen::CertifiedKey { cert, .. } =
            rcgen::generate_simple_self_signed(vec!["ichoi-core.invalid".into()]).unwrap();
        let fp = fingerprint(cert.der()).unwrap();
        assert!(PinnedCertificateVerifier::new(&[fp]).is_ok());
        assert!(PinnedCertificateVerifier::new(&[]).is_err());
        assert!(PinnedCertificateVerifier::new(&["sha256:abcd".into()]).is_err());
    }

    #[tokio::test]
    async fn pinned_handshake_encrypts_application_data() {
        let (server_config, pin) = test_server();
        let (client_io, server_io) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let mut tls = tokio_rustls::TlsAcceptor::from(server_config)
                .accept(server_io)
                .await
                .unwrap();
            tls.write_all(b"encrypted").await.unwrap();
            tls.shutdown().await.unwrap();
        });
        let mut client = tokio_rustls::TlsConnector::from(client_config(&[pin]).unwrap())
            .connect(logical_server_name(), client_io)
            .await
            .unwrap();
        let mut bytes = Vec::new();
        client.read_to_end(&mut bytes).await.unwrap();
        server.await.unwrap();
        assert_eq!(bytes, b"encrypted");
    }

    #[tokio::test]
    async fn wrong_pin_rejects_the_core_before_authentication() {
        let (server_config, _pin) = test_server();
        let (client_io, server_io) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            tokio_rustls::TlsAcceptor::from(server_config)
                .accept(server_io)
                .await
        });
        let wrong = format!("sha256:{}", "55".repeat(32));
        let result = tokio_rustls::TlsConnector::from(client_config(&[wrong]).unwrap())
            .connect(logical_server_name(), client_io)
            .await;
        assert!(result.is_err());
        assert!(server.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn plaintext_is_rejected_before_a_node_token_can_be_read() {
        let (server_config, _) = test_server();
        let (mut client_io, server_io) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            tokio_rustls::TlsAcceptor::from(server_config)
                .accept(server_io)
                .await
        });
        client_io
            .write_all(b"node_token=must-not-parse")
            .await
            .unwrap();
        client_io.shutdown().await.unwrap();
        assert!(server.await.unwrap().is_err());
    }
}
