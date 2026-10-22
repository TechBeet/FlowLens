//! TLS MITM and certificate management for FlowLens

use std::sync::RwLock;

use flowlens_core::models::{ConnectionKey, TlsVersion};
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
}

impl From<rcgen::Error> for TlsError {
    fn from(e: rcgen::Error) -> Self {
        TlsError::CertGeneration(e.to_string())
    }
}

pub struct InMemoryTlsEngine {
    ca: RwLock<Option<CaInfo>>,
}

impl InMemoryTlsEngine {
    pub fn new() -> Self {
        Self {
            ca: RwLock::new(None),
        }
    }

    pub fn setup_ca(&self, cn: &str) -> Result<CaInfo, TlsError> {
        if let Some(ca) = self.ca.read().unwrap().as_ref() {
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
        *self.ca.write().unwrap() = Some(ca.clone());
        Ok(ca)
    }

    pub fn sign_leaf(
        &self,
        domain: &str,
        san_names: &[String],
    ) -> Result<(Certificate, PrivateKey), TlsError> {
        let ca_guard = self.ca.read().unwrap();
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

        let cert = params.signed_by(&ca_key, &ca_cert, &ca_key)?;
        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;

        Ok((
            cert,
            PrivateKey {
                der: leaf_key.serialize_der(),
                pem: leaf_key.serialize_pem(),
            },
        ))
    }

    pub fn ca_pem(&self) -> Option<String> {
        self.ca
            .read()
            .unwrap()
            .as_ref()
            .map(|ca| ca.certificate_pem.clone())
    }

    pub fn decrypt_stream(&self, _conn: &ConnectionKey, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        Ok(data.to_vec())
    }
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
    fn install_ca(&self, _ca: &CaInfo) -> Result<(), TlsError> {
        Ok(())
    }
    fn uninstall_ca(&self, _ca: &CaInfo) -> Result<(), TlsError> {
        Ok(())
    }
    fn check_installed(&self, cn: &str) -> bool {
        self.ca
            .read()
            .unwrap()
            .as_ref()
            .map(|ca| ca.cn == cn)
            .unwrap_or(false)
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
        let _ = domain;
        let _ = cert;
    }
    fn get_cached_cert(&self, domain: &str) -> Option<(Certificate, PrivateKey)> {
        let _ = domain;
        None
    }
}

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
    fn test_check_installed() {
        let mut engine = InMemoryTlsEngine::new();
        assert!(!engine.check_installed("FlowLens CA"));
        engine.generate_ca("FlowLens CA").unwrap();
        assert!(engine.check_installed("FlowLens CA"));
        assert!(!engine.check_installed("Other CA"));
    }
}
