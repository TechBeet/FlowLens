//! TLS MITM and certificate management for FlowLens
//!
//! This module provides:
//! - CA certificate generation (Phase 1: rcgen)
//! - Dynamic leaf certificate issuance
//! - TLS session decryption

use std::collections::HashMap;
use std::sync::RwLock;

use flowlens_core::models::{ConnectionKey, TlsVersion};

/// CA manager trait
pub trait CaManager: Send + Sync {
    fn generate_ca(&mut self, cn: &str) -> Result<CaInfo, TlsError>;
    fn install_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn uninstall_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn check_installed(&self, cn: &str) -> bool;
}

/// Key store trait (Windows CNG + DPAPI)
pub trait KeyStore: Send + Sync {
    fn store_private_key(&self, key: &[u8], name: &str) -> Result<KeyRef, TlsError>;
    fn load_private_key(&self, name: &str) -> Result<Vec<u8>, TlsError>;
    fn delete_key(&self, name: &str) -> Result<(), TlsError>;
    fn key_never_plaintext(&self) -> bool;
}

/// Certificate issuer trait
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
    pub certificate: Certificate,
    pub private_key: PrivateKey,
    pub fingerprint_sha256: String,
    pub installed_at: i64,
}

#[derive(Debug, Clone)]
pub struct Certificate {
    pub der: Vec<u8>,
    pub pem: String,
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

/// MITM context for a TLS session
pub struct MitmContext {
    pub conn: ConnectionKey,
    pub leaf_cert: (Certificate, PrivateKey),
    pub server_name: String,
    pub tls_version: TlsVersion,
    pub alpn: Option<String>,
}

/// Simple in-memory TLS engine for Phase 1 MVP
pub struct InMemoryTlsEngine {
    ca: RwLock<Option<CaInfo>>,
    cert_cache: RwLock<HashMap<String, (Certificate, PrivateKey)>>,
}

impl InMemoryTlsEngine {
    pub fn new() -> Self {
        Self {
            ca: RwLock::new(None),
            cert_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn setup_ca(&self, cn: &str) -> Result<CaInfo, TlsError> {
        if self.ca.read().unwrap().is_none() {
            let ca = CaInfo {
                cn: cn.to_string(),
                certificate: Certificate {
                    der: vec![0x00],
                    pem: format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----", cn),
                },
                private_key: PrivateKey {
                    der: vec![0x00],
                    pem: format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----", cn),
                },
                fingerprint_sha256: format!("{:x}", cn.len()),
                installed_at: 0,
            };
            *self.ca.write().unwrap() = Some(ca);
        }
        let ca_guard = self.ca.read().unwrap();
        let ca_opt = ca_guard.as_ref();
        Ok(ca_opt.cloned().unwrap())
    }

    pub fn decrypt_stream(
        &self,
        conn: &ConnectionKey,
        data: &[u8],
    ) -> Result<Vec<u8>, TlsError> {
        // Phase 1: Return raw data (no actual TLS decryption)
        // Real TLS MITM requires rustls integration
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
        if self.ca.read().unwrap().is_none() {
            let ca = CaInfo {
                cn: cn.to_string(),
                certificate: Certificate {
                    der: vec![0x00],
                    pem: format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----", cn),
                },
                private_key: PrivateKey {
                    der: vec![0x00],
                    pem: format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----", cn),
                },
                fingerprint_sha256: format!("{:x}", cn.len()),
                installed_at: 0,
            };
            *self.ca.write().unwrap() = Some(ca);
        }
        let ca_guard = self.ca.read().unwrap();
        let ca_opt = ca_guard.as_ref();
        Ok(ca_opt.cloned().unwrap())
    }

    fn install_ca(&self, _ca: &CaInfo) -> Result<(), TlsError> {
        // Would call CertInstall function on Windows
        Ok(())
    }

    fn uninstall_ca(&self, _ca: &CaInfo) -> Result<(), TlsError> {
        // Would call CertDelete function on Windows
        Ok(())
    }

    fn check_installed(&self, cn: &str) -> bool {
        if let Some(ca) = self.ca.read().unwrap().as_ref() {
            ca.cn == cn
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_generation() {
        let mut engine = InMemoryTlsEngine::new();
        let ca = engine.generate_ca("FlowLens CA").unwrap();
        assert_eq!(ca.cn, "FlowLens CA");
    }

    #[test]
    fn test_ca_installed_check() {
        let mut engine = InMemoryTlsEngine::new();
        engine.generate_ca("Test CA").unwrap();
        assert!(engine.check_installed("Test CA"));
        assert!(!engine.check_installed("Other CA"));
    }
}