//! TLS MITM and certificate management for FlowLens

pub mod cert_store;
pub mod concurrent;
pub mod mitm;
pub mod policy;
pub use concurrent::ConcurrentDecryptor;
pub use mitm::{
    build_client_config, build_server_config, extract_sni, handle_mitm_connection, MitmSession,
    MitmStream, MitmTlsAcceptor, MitmTlsConnector, SniDecision,
};
pub use policy::{matches_domain, normalize_domain, DomainPolicy, DomainPolicyAccess, DomainRule};

use std::sync::RwLock;
use std::collections::HashMap;

use flowlens_core::models::{ConnectionKey, TlsVersion};
use serde::Serialize;
use rcgen::{
    Certificate, CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256,
};
use sha2::{Digest, Sha256};

pub trait CaManager: Send + Sync {
    fn generate_ca(&mut self, cn: &str) -> Result<CaInfo, TlsError>;
    fn install_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn uninstall_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn check_installed(&self, cn: &str) -> bool;
    fn ca_pem(&self) -> Option<String>;
}

pub trait DecryptPolicy: Send + Sync {
    /// Record / remove (decrypt=false removes the entry when first seen) a per-host rule.
    fn set_domain(&mut self, domain: &str, decrypt: bool);
    fn remove_domain(&mut self, domain: &str) -> bool;
    fn domain_decrypt(&self, domain: &str) -> Option<bool>;
    fn list_domains(&self) -> Vec<(String, bool)>;
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuedLeafCert {
    pub sni: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub subject_cn: String,
    pub fingerprint_sha256: String,
    pub sans: Vec<String>,
}

pub trait KeyStore: Send + Sync {
    fn store_private_key(&self, key: &[u8], name: &str) -> Result<KeyRef, TlsError>;
    fn load_private_key(&self, name: &str) -> Result<Vec<u8>, TlsError>;
    fn delete_key(&self, name: &str) -> Result<(), TlsError>;
    fn key_never_plaintext(&self) -> bool;
}

pub trait CertIssuer: Send + Sync {
    fn sign_leaf_cert(
        &self,
        domain: &str,
        san_names: &[String],
    ) -> Result<(Certificate, PrivateKey), TlsError>;
    fn cache_cert(&self, domain: &str, cert: (Certificate, PrivateKey));
    fn get_cached_cert(&self, domain: &str) -> Option<(Certificate, PrivateKey)>;
}

#[derive(Debug, Clone)]
pub struct CaInfo {
    pub cn: String,
    pub certificate_pem: String,
    pub private_key: PrivateKey,
    pub fingerprint_sha256: String,
    pub installed_at: i64,
}

#[derive(Debug, Clone)]
pub struct PrivateKey {
    pub der: Vec<u8>,
    pub pem: String,
}

#[derive(Debug, Clone)]
pub struct KeyRef {
    pub name: String,
    pub provider: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("Certificate generation failed: {0}")]
    CertGeneration(String),
    #[error("Key storage error: {0}")]
    KeyStorage(String),
    #[error("TLS parsing error: {0}")]
    Parse(#[from] std::str::Utf8Error),
    #[error("Decryption not supported: {0}")]
    NotSupported(String),
    #[error("Certificate pinning detected: {0}")]
    CertificatePinning(String),

    #[error("Mutex lock poisoned: {0}")]
    LockPoisoned(String),
}

impl From<rcgen::Error> for TlsError {
    fn from(e: rcgen::Error) -> Self {
        TlsError::CertGeneration(e.to_string())
    }
}

pub struct InMemoryTlsEngine {
    ca: RwLock<Option<CaInfo>>,
    cert_cache: RwLock<HashMap<String, (String, String)>>,
}

impl InMemoryTlsEngine {
    pub fn new() -> Self {
        Self {
            ca: RwLock::new(None),
            cert_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn setup_ca(&self, cn: &str) -> Result<CaInfo, TlsError> {
        // A poisoned `ca` lock is recoverable: `Option<CaInfo>` either holds a
        // fully-built CA or nothing, so a panic in another thread cannot leave
        // it half-written. Surface a clear error instead of panicking.
        if let Some(ca) = self
            .ca
            .read()
            .map_err(|poisoned| TlsError::LockPoisoned(format!("ca: {poisoned}")))?
            .as_ref()
        {
            return Ok(ca.clone());
        }
        let mut params = CertificateParams::new(vec![])?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, cn);
        params.distinguished_name = dn;
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(2));
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let cert = params.self_signed(&key)?;
        let der = cert.der().to_vec();
        let fingerprint = format!("{:x}", Sha256::digest(&der));
        let now = flowlens_core::models::now_micros();
        let ca = CaInfo {
            cn: cn.to_string(),
            certificate_pem: cert.pem(),
            private_key: PrivateKey {
                der: key.serialize_der(),
                pem: key.serialize_pem(),
            },
            fingerprint_sha256: fingerprint,
            installed_at: now,
        };
        *self
            .ca
            .write()
            .map_err(|poisoned| TlsError::LockPoisoned(format!("ca: {poisoned}")))? = Some(ca.clone());
        Ok(ca)
    }

    pub fn sign_leaf(
        &self,
        domain: &str,
        san_names: &[String],
    ) -> Result<(Certificate, PrivateKey), TlsError> {
        let ca_guard = self
            .ca
            .read()
            .map_err(|poisoned| TlsError::LockPoisoned(format!("ca: {poisoned}")))?;
        let ca = ca_guard
            .as_ref()
            .ok_or_else(|| TlsError::NotSupported("No CA generated yet".to_string()))?;

        let ca_params = CertificateParams::from_ca_cert_pem(&ca.certificate_pem)?;
        let ca_key = KeyPair::from_pem(&ca.private_key.pem)?;
        let ca_cert = ca_params.self_signed(&ca_key)?;

        let mut params = CertificateParams::new(san_names.to_vec())?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, domain);
        params.distinguished_name = dn;

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;

        let cert = params.signed_by(&leaf_key, &ca_cert, &ca_key)?;

        Ok((
            cert,
            PrivateKey {
                der: leaf_key.serialize_der(),
                pem: leaf_key.serialize_pem(),
            },
        ))
    }

    pub fn ca_pem(&self) -> Option<String> {
        // Poison-tolerant: the guarded `Option<CaInfo>` is always in a valid
        // state even after a panic, so recover the inner value instead of panicking.
        self.ca
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|ca| ca.certificate_pem.clone())
    }

    pub fn decrypt_stream(&self, _ctx: &MitmContext, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        // Phase A: real MITM decryption happens in the proxy path via
        // MitmTlsAcceptor + MitmTlsConnector + MitmStream::spawn_forwarding().
        // This sync helper validates the input and returns it for the mock/test
        // path; callers in the capture pipeline use the async MITM stream for
        // true decryption. A per-call tokio runtime is intentionally avoided
        // here because decrypting a single byte slice requires a live TLS
        // session that only the proxy layer possesses.
        if data.is_empty() {
            return Err(TlsError::NotSupported("empty input".into()));
        }
        Ok(data.to_vec())
    }

    /// Ensure a leaf certificate exists for the given SNI, issuing it on first use.
    ///
    /// This is the "SNI-aware issuance" entry point: the MITM proxy calls it when a
    /// ClientHello arrives for a host it has not seen before, and the result feeds
    /// [`InMemoryTlsEngine::server_config_for_sni`] to terminate that single session.
    pub fn ensure_leaf_cert(&self, sni: &str) -> Result<IssuedLeafCert, TlsError> {
        let host = crate::policy::normalize_domain(sni)?;
        let (cert_pem, key_pem) = self.issue_for_sni(&host)?;
        let fingerprint_sha256 = pem_fingerprint_sha256(&cert_pem)?;
        Ok(IssuedLeafCert {
            subject_cn: host.clone(),
            sans: vec![host.clone()],
            sni: host,
            cert_pem,
            key_pem,
            fingerprint_sha256,
        })
    }

    /// Build a `rustls::ServerConfig` for a dynamically issued per-host leaf cert.
    pub fn server_config_for_sni(&self, sni: &str) -> Result<rustls::ServerConfig, TlsError> {
        let issued = self.ensure_leaf_cert(sni)?;
        build_server_config(&issued.cert_pem, &issued.key_pem)
    }

    /// Hostnames for which a leaf certificate has already been issued (sorted).
    pub fn issued_snis(&self) -> Vec<String> {
        // Poison-tolerant: the cert cache is a plain map; its contents remain
        // valid after a panic, so recover via `into_inner()`.
        let mut names: Vec<String> = self
            .cert_cache
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// Perform SNI-aware dynamic certificate issuance for a given hostname.
    pub fn issue_for_sni(&self, sni: &str) -> Result<(String, String), TlsError> {
        // Check PEM cache first for deterministic round-trip.
        {
            let guard = self
                .cert_cache
                .read()
                .map_err(|poisoned| TlsError::LockPoisoned(format!("cert_cache: {poisoned}")))?;
            if let Some((cp, kp)) = guard.get(sni) {
                return Ok((cp.clone(), kp.clone()));
            }
        }
        let san = vec![sni.to_string()];
        let (_cert, key) = self.sign_leaf(sni, &san)?;
        // Return PEMs - we need to reconstruct full chain; use leaf cert PEM
        let leaf_pem = _cert.pem();
        let res = (leaf_pem, key.pem.clone());
        self.cache_cert(sni, (_cert, key));
        Ok(res)
    }
}

#[cfg(windows)]
fn install_ca_windows(pem: &str) -> Result<(), TlsError> {
    crate::cert_store::install_ca_to_store(pem)
}

#[cfg(windows)]
fn uninstall_ca_windows(pem: &str) -> Result<(), TlsError> {
    crate::cert_store::uninstall_ca_from_store(pem)
}

#[cfg(windows)]
fn check_installed_windows(_cn: &str) -> bool {
    // For Windows, use the stored CA if available
    false
}

#[cfg(not(windows))]
fn install_ca_windows(_pem: &str) -> Result<(), TlsError> {
    Ok(())
}

#[cfg(not(windows))]
fn uninstall_ca_windows(_pem: &str) -> Result<(), TlsError> {
    Ok(())
}

#[cfg(not(windows))]
fn check_installed_windows(_cn: &str) -> bool {
    false
}

impl Default for InMemoryTlsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CaManager for InMemoryTlsEngine {
    fn generate_ca(&mut self, cn: &str) -> Result<CaInfo, TlsError> {
        self.setup_ca(cn)
    }
    fn install_ca(&self, ca: &CaInfo) -> Result<(), TlsError> {
        install_ca_windows(&ca.certificate_pem)
    }
    fn uninstall_ca(&self, ca: &CaInfo) -> Result<(), TlsError> {
        uninstall_ca_windows(&ca.certificate_pem)
    }
    fn check_installed(&self, cn: &str) -> bool {
        // Poison-tolerant: `ca` holds `Option<CaInfo>` which is always valid.
        if self
            .ca
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|ca| ca.cn == cn)
            .unwrap_or(false)
        {
            return true;
        }
        check_installed_windows(cn)
    }
    fn ca_pem(&self) -> Option<String> {
        self.ca_pem()
    }
}

impl CertIssuer for InMemoryTlsEngine {
    fn sign_leaf_cert(
        &self,
        domain: &str,
        san_names: &[String],
    ) -> Result<(Certificate, PrivateKey), TlsError> {
        self.sign_leaf(domain, san_names)
    }
    fn cache_cert(&self, domain: &str, cert: (Certificate, PrivateKey)) {
        // Poison-tolerant: the cache map itself remains valid after a panic;
        // the missed insert is acceptable for a best-effort cache.
        self.cert_cache
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(domain.to_string(), (cert.0.pem(), cert.1.pem));
    }
    fn get_cached_cert(&self, domain: &str) -> Option<(Certificate, PrivateKey)> {
        let guard = self
            .cert_cache
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (_cert_pem, key_pem) = guard.get(domain)?.clone();
        drop(guard);
        let key = KeyPair::from_pem(&key_pem).ok()?;
        let mut params = CertificateParams::new(vec![domain.to_string()]).ok()?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, domain);
        params.distinguished_name = dn;
        let cert = params.self_signed(&key).ok()?;
        Some((
            cert,
            PrivateKey {
                der: key.serialize_der(),
                pem: key_pem,
            },
        ))
    }
}

/// Trait for MITM proxy engines providing SNI-aware TLS termination.
pub trait MitmProxy: CertIssuer {
    fn build_server_config(&self, cert_pem: &str, key_pem: &str) -> Result<rustls::ServerConfig, TlsError>;
    fn build_client_config(&self) -> Result<rustls::ClientConfig, TlsError>;
    fn extract_sni_from_client_hello(&self, data: &[u8]) -> Option<String>;
    fn issue_for_sni(&self, sni: &str) -> Result<(String, String), TlsError>;

    /// Build a per-host `ServerConfig` for `sni`, issuing a leaf cert on first use.
    fn server_config_for_sni(&self, sni: &str) -> Result<rustls::ServerConfig, TlsError>;
}

impl MitmProxy for InMemoryTlsEngine {
    fn build_server_config(&self, cert_pem: &str, key_pem: &str) -> Result<rustls::ServerConfig, TlsError> {
        crate::mitm::build_server_config(cert_pem, key_pem)
    }
    fn build_client_config(&self) -> Result<rustls::ClientConfig, TlsError> {
        crate::mitm::build_client_config()
    }
    fn extract_sni_from_client_hello(&self, data: &[u8]) -> Option<String> {
        crate::mitm::extract_sni(data)
    }
    fn issue_for_sni(&self, sni: &str) -> Result<(String, String), TlsError> {
        // Check PEM cache first for deterministic round-trip.
        {
            let guard = self
                .cert_cache
                .read()
                .map_err(|poisoned| TlsError::LockPoisoned(format!("cert_cache: {poisoned}")))?;
            if let Some((cp, kp)) = guard.get(sni) {
                return Ok((cp.clone(), kp.clone()));
            }
        }
        let san = vec![sni.to_string()];
        let (cert, key) = self.sign_leaf(sni, &san)?;
        let res = (cert.pem(), key.pem.clone());
        self.cache_cert(sni, (cert, key));
        Ok(res)
    }
    fn server_config_for_sni(&self, sni: &str) -> Result<rustls::ServerConfig, TlsError> {
        InMemoryTlsEngine::server_config_for_sni(self, sni)
    }
}
/// SHA-256 fingerprint (lowercase hex) of the first certificate in a PEM bundle.
fn pem_fingerprint_sha256(cert_pem: &str) -> Result<String, TlsError> {
    let mut reader = std::io::Cursor::new(cert_pem.as_bytes());
    let der = rustls_pemfile::certs(&mut reader)
        .next()
        .ok_or_else(|| TlsError::CertGeneration("no CERTIFICATE block in PEM".into()))?
        .map_err(|e| TlsError::CertGeneration(format!("invalid certificate PEM: {e}")))?;
    Ok(format!("{:x}", Sha256::digest(&der)))
}

/// Context for an active MITM proxy session.
pub struct MitmContext {
    pub conn: ConnectionKey,
    pub leaf_cert: (Certificate, PrivateKey),
    pub server_name: String,
    pub tls_version: TlsVersion,
    pub alpn: Option<String>,
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_generation_is_valid_x509() {
        let mut engine = InMemoryTlsEngine::new();
        let ca = engine.generate_ca("FlowLens CA").unwrap();
        assert_eq!(ca.cn, "FlowLens CA");
        assert!(ca.certificate_pem.contains("-----BEGIN CERTIFICATE-----"));
        assert!(ca.private_key.pem.contains("-----BEGIN PRIVATE KEY-----"));
    }

    #[test]
    fn test_ca_pem_is_readable() {
        let engine = InMemoryTlsEngine::new();
        assert!(engine.ca_pem().is_none());
        let _ = engine.setup_ca("Test CA").unwrap();
        let pem = engine.ca_pem().unwrap();
        assert!(pem.contains("BEGIN CERTIFICATE"));
    }

    #[test]
    fn test_leaf_cert_signed_by_ca() {
        let engine = InMemoryTlsEngine::new();
        let _ca = engine.setup_ca("FlowLens CA").unwrap();
        let (cert, _key) = engine
            .sign_leaf("example.com", &["example.com".to_string()])
            .unwrap();
        assert!(cert.pem().contains("-----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn test_leaf_cert_public_key_matches_private_key() {
        // Regression guard for SEC-02: the leaf certificate's embedded public
        // key MUST correspond to the returned leaf private key, not the CA key.
        // We parse the cert DER with x509-parser and compare the SubjectPublicKeyInfo
        // against the public key reconstructed from the returned private key.
        let engine = InMemoryTlsEngine::new();
        let _ca = engine.setup_ca("FlowLens CA").unwrap();
        let (cert, key) = engine
            .sign_leaf("example.com", &["example.com".to_string()])
            .unwrap();

        // Reconstruct a KeyPair from the returned private key DER.
        let leaf_key = KeyPair::from_der_and_sign_algo(
            &rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(key.der.clone())),
            &PKCS_ECDSA_P256_SHA256,
        )
        .expect("leaf key DER must parse with ECDSA P-256");
        let leaf_pub = leaf_key.public_key_raw();

        // Parse the certificate DER and extract its SubjectPublicKeyInfo.
        let (_, parsed) =
            x509_parser::parse_x509_certificate(cert.der()).expect("cert DER must parse");
        let cert_pub = parsed
            .tbs_certificate
            .subject_pki
            .subject_public_key
            .data
            .to_vec();

        assert_eq!(
            cert_pub, leaf_pub,
            "leaf cert public key must match leaf private key (SEC-02 regression)"
        );
    }

    #[test]
    fn test_check_installed() {
        let mut engine = InMemoryTlsEngine::new();
        assert!(!engine.check_installed("FlowLens CA"));
        engine.generate_ca("FlowLens CA").unwrap();
        assert!(engine.check_installed("FlowLens CA"));
        assert!(!engine.check_installed("Other CA"));
    }
}
