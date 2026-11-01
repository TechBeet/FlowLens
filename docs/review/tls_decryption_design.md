# TLS Decryption Pipeline Design Document

**Status**: Draft  
**Author**: Security Engineering Team  
**Last Updated**: 2026-11-01  
**Component**: `flowlens-tls` crate  

---

## Executive Summary

The current `decrypt_stream()` implementation in `flowlens-tls` is a **passthrough stub** that returns input data unchanged. This document outlines why TLS decryption is architecturally challenging without active MITM interception, presents two viable approaches, and provides a concrete implementation roadmap using rustls + rcgen.

### Verified Current State

| Location | Finding |
|----------|---------|
| `crates/flowlens-tls/src/lib.rs:158-160` | `decrypt_stream()` returns `Ok(data.to_vec())` — passthrough stub |
| `crates/flowlens-tls/src/lib.rs:138` | `sign_leaf` uses `&ca_key` for BOTH issuer and subject key (**BUG**) |
| `crates/flowlens-tls/src/concurrent.rs:76` | `decrypt_one()` strips 1 marker byte — stub |
| CA generation | rcgen works; rustls available as dependency |

### Critical Bug Identified

**Location**: `crates/flowlens-tls/src/lib.rs:138`

```rust
// BUG: Leaf cert embeds CA public key instead of generating unique leaf keypair
let cert = params.signed_by(&ca_key, &ca_cert, &ca_key)?;
let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
```

Every forged leaf cert contains the CA's public key, breaking the trust chain. **Fix**: generate a unique leaf keypair first, then sign with CA (see Section 4).

---

## 1. Why Proxy-less TLS Decryption Is Architecturally Hard

### The Cryptographic Reality

TLS was explicitly designed to prevent passive decryption. When you capture network packets:

1. **Application data arrives already encrypted** — The TCP payload is the TLS record layer, containing ciphertext produced by AES-GCM or ChaCha20-Poly1305 using session keys derived during the handshake.

2. **Session keys are ephemeral** — With perfect forward secrecy (PFS), each connection generates unique keys via ECDHE. Compromising the server's long-term private key later does not decrypt past sessions.

3. **The handshake itself is protected** — In TLS 1.3, everything after ClientHello is encrypted. You cannot observe the key exchange without terminating the connection.

### What You Need to Decrypt

| Secret Type | How Obtained | Limitations |
|-------------|--------------|-------------|
| Pre-master secret | From client/server memory during handshake | Requires instrumenting the application |
| Master secret | Derived from pre-master + randoms | Same as above |
| Session keys | Derived from master secret | Same as above |
| Server private key | Only if RSA key exchange | TLS ≤1.2 only, no PFS |
| MITM position | Intercept and terminate TLS | Requires redirection or proxy |

### The Two Fundamental Approaches

```
┌─────────────────────────────────────────────────────────────────┐
│                    TLS DECRYPTION STRATEGIES                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  APPROACH A: ACTIVE MITM PROXY                                  │
│  Client ←→ [Your Proxy] ←→ Server                               │
│            ↓ terminates TLS, sees plaintext                     │
│            ↓ re-encrypts to real server                         │
│  Pros: Works for any app, full visibility                       │
│  Cons: Requires WFP/DNS redirection, cert installation          │
│                                                                 │
│  APPROACH B: PASSIVE KEY EXTRACTION                             │
│  Client → [Packet Capture] → Server                             │
│           ↓ extract keys via SSLKEYLOGFILE / ETW                │
│           ↓ offline decryption                                  │
│  Pros: No network modification, stealthy                        │
│  Cons: Only works if app logs keys (config-dependent)           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## 2. Approach A: Full MITM Proxy Architecture

### 2.1 High-Level Design

```
┌──────────────┐     TLS 1.3      ┌─────────────────────┐     TLS 1.3     ┌──────────────┐
│   Browser    │ ◄──────────────► │  flowlens-tls     │ ◄──────────────► │   Server   │
│   (Chrome)   │   (MITM Cert)    │  MITM Proxy       │   (Real Cert)  │  (google)  │
└──────────────┘                  └─────────────────────┘                └──────────────┘
                                         ↓
                                   Plaintext HTTP
                                         ↓
                                  FlowLens Analysis
```

### 2.2 Network Redirection Layer

You must intercept outgoing TCP connections to port 443 and redirect them to your local proxy.

| Method | Windows Implementation | Complexity | Stealth |
|--------|----------------------|------------|---------|
| WFP (Windows Filtering Platform) | `fwpm*` APIs, callout drivers | High | High |
| hosts file + DNS spoofing | Edit `C:\Windows\System32\drivers\etc\hosts` | Low | Medium |
| Proxy auto-config (PAC) | Set IE/Edge proxy settings | Low | Low |
| LSP (Layered Service Provider) | Winsock SPI (deprecated) | Very High | High |

**Recommended for MVP**: hosts file manipulation combined with local DNS cache clearing. This redirects specific domains to localhost where the proxy listens.

### 2.3 rustls Server Configuration (Sketch)

```rust
struct CertResolver {
    ca_key: Arc<KeyPair>,
    ca_cert: Arc<rcgen::CertifiedKey>,
    cache: DashMap<String, Arc<rustls::sign::CertifiedKey>>,
}

impl ResolvesServerCert for CertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello<'_>) -> Option<Arc<rustls::sign::CertifiedKey>> {
        let sni = client_hello.server_name()?;
        if let Some(cached) = self.cache.get(sni) { return Some(cached.clone()); }

        let leaf_key = KeyPair::generate().ok()?;
        let mut params = CertificateParams::new(vec![sni.to_string()]).ok()?;
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(rcgen::DnType::CommonName, format!("FlowLens MITM: {}", sni));

        let leaf_cert = params.signed_by(&leaf_key, &self.ca_cert.cert, &self.ca_key).ok()?;
        let certified_key = rustls::sign::CertifiedKey::new(
            vec![rustls::Certificate(leaf_cert.der().to_vec())],
            rustls::sign::any_supported_type(&leaf_key).ok()?,
        );

        let arc_key = Arc::new(certified_key);
        self.cache.insert(sni.to_string(), arc_key.clone());
        Some(arc_key)
    }
}
```

### 2.4 Dual-Stack Connection Handler (Sketch)

```rust
async fn handle_client_connection(client_stream: TcpStream, server_addr: SocketAddr, config: Arc<ServerConfig>) -> Result<()> {
    let tls_stream = tokio_rustls::TlsAcceptor::from(config).accept(client_stream).await?;
    let sni = tls_stream.get_ref().1.first().and_then(|hello| hello.server_name());
    let upstream = TcpStream::connect(server_addr).await?;

    let client_config = Arc::new(rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(get_system_roots())
        .with_no_client_auth());

    let dns_name = rustls::pki_types::ServerName::try_from(sni.unwrap_or("unknown"))?;
    let upstream_tls = tokio_rustls::TlsConnector::from(client_config).connect(dns_name, upstream).await?;

    // Log plaintext before forwarding
    let (client_read, client_write) = tokio::io::split(tls_stream);
    let (_, _) = tokio::io::split(upstream_tls);
    tokio::spawn(async move { /* copy and log */ });
    // bidirectional copy...
}
```

### 2.5 Certificate Installation (Sketch)

```rust
pub fn generate_and_install(&self, cn: &str) -> Result<CaInfo> {
    let params = CertificateParams::new(vec![cn.to_string()]);
    let key_pair = KeyPair::generate()?;
    let ca_cert = params.self_signed(&key_pair)?;
    let pem = ca_cert.serialize_pem()?;

    // certutil -addstore -f "ROOT" ca.pem
    std::process::Command::new("certutil")
        .args(["-addstore", "-f", "ROOT", &temp_pem_path])
        .output()?;

    Ok(CaInfo {
        certificate_pem: pem,
        private_key_pem: key_pair.serialize_pem(),
        fingerprint_sha256: compute_sha256(&ca_cert.der()),
        installed_at: Utc::now(),
    })
}
```

## 3. Approach B: SSLKEYLOGFILE / ETW Passive Extraction

### 3.1 SSLKEYLOGFILE Protocol

Applications that support NSS key logging write secrets to a file:

```
CLIENT_RANDOM <client_random_hex> <master_secret_hex>
```

**Format**: 48-byte client_random + 48-byte master_secret (for TLS 1.2)

### 3.2 Integration Strategy (Sketch)

```rust
pub struct KeylogCollector {
    keys: Arc<DashMap<[u8; 48], [u8; 48]>>,
}

impl KeylogCollector {
    pub fn spawn_watcher(path: PathBuf) -> Self {
        let keys = Arc::new(DashMap::new());
        tokio::spawn(async move {
            let mut file = tokio::fs::File::open(&path).await?;
            let mut buf = String::new();
            loop {
                buf.clear();
                file.read_line(&mut buf).await?;
                if buf.starts_with("CLIENT_RANDOM ") {
                    let parts: Vec<&str> = buf.trim().split_whitespace().collect();
                    if parts.len() == 3 {
                        let cr = hex::decode(parts[1]).ok()?;
                        let ms = hex::decode(parts[2]).ok()?;
                        if cr.len() == 48 && ms.len() == 48 {
                            let mut c = [0u8; 48]; c.copy_from_slice(&cr);
                            let mut m = [0u8; 48]; m.copy_from_slice(&ms);
                            keys.insert(c, m);
                        }
                    }
                }
            }
        });
        Self { keys }
    }
}
```

### 3.3 ETW-Schannel Alternative

ETW provider `Microsoft-Windows-Schannel-Ext` (Event ID 17) can expose keys for apps that don't support SSLKEYLOGFILE. Requires native Windows API bindings.

### 3.4 Offline Decryption

Use collected master secrets with `tshark` or a custom rustls-based decryptor.
## 4. Fixed `sign_leaf` Implementation

### Current Bug (`lib.rs:138`)

```rust
// WRONG
let cert = params.signed_by(&ca_key, &ca_cert, &ca_key)?;
let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
```

**Problem**: The leaf certificate's public key is the CA's public key, not a unique leaf keypair. This breaks the certificate hierarchy and makes forged leaves trivially detectable.

### Corrected Implementation

```rust
pub fn sign_leaf(
    &self,
    domain: &str,
    san_names: &[String],
) -> Result<(Certificate, PrivateKey), TlsError> {
    let ca_guard = self.ca.read().unwrap();
    let ca = ca_guard
        .as_ref()
        .ok_or_else(|| TlsError::NotSupported("No CA generated yet".to_string()))?;

    // Load CA cert and key
    let ca_params = CertificateParams::from_ca_cert_pem(&ca.certificate_pem)?;
    let ca_key = KeyPair::from_pem(&ca.private_key.pem)?;
    let ca_cert = ca_params.self_signed(&ca_key)?;

    // 1. Generate UNIQUE leaf keypair FIRST
    let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;

    // 2. Build leaf certificate with leaf's public key
    let mut params = CertificateParams::new(
        san_names
            .iter()
            .map(|s| rcgen::SanType::DnsName(s.clone()).into())
            .collect::<Vec<_>>()
    )?;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, domain);
    params.distinguished_name = dn;

    // Optional: set extended key usage for TLS server auth
    params.extended_key_usage = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

    // 3. Sign leaf with CA key, embedding leaf's PUBLIC key
    let leaf_cert = params.signed_by(&leaf_key, &ca_cert, &ca_key)?;

    Ok((
        Certificate {
            der: leaf_cert.der().to_vec(),
            pem: leaf_cert.serialize_pem()?,
        },
        PrivateKey {
            der: leaf_key.serialize_der(),
            pem: leaf_key.serialize_pem(),
        },
    ))
}
```

**Key changes**:
1. `leaf_key` is generated **before** signing (was after).
2. `params.signed_by(&leaf_key, ...)` — the first arg is the leaf's keypair (subject public key).
3. Added `extended_key_usage = ServerAuth` so browsers accept it for TLS.

---

## 5. DPAPI Private Key Storage on Windows

### Threat Model

- CA private key must be protected at rest.
- Accessible only to the current user.
- Should resist basic theft by malware running as the same user.

### Implementation

```rust
#[cfg(windows)]
pub struct WindowsKeyStore {
    container_name: String,
}

#[cfg(windows)]
impl WindowsKeyStore {
    pub fn protect_key(&self, key_pem: &str) -> Result<()> {
        use windows::Win32::Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, DATA_BLOB,
        };

        unsafe {
            let mut data_blob = DATA_BLOB {
                pbData: key_pem.as_ptr() as *mut _,
                cbData: key_pem.len() as u32,
            };

            let mut out_blob = DATA_BLOB::default();
            CryptProtectData(
                &mut data_blob,
                PCWSTR::null(),
                None,
                None,
                None,
                0,
                &mut out_blob,
            )?;

            let encrypted = std::slice::from_raw_parts(
                out_blob.pbData,
                out_blob.cbData as usize,
            ).to_vec();

            LocalFree(out_blob.pbData as *mut _);

            // Persist to user-specific location
            let store_path = dirs::data_local_dir()
                .join("flowlens")
                .join("ca_key.enc");
            std::fs::write(&store_path, encrypted)?;
        }

        Ok(())
    }

    pub fn unprotect_key(&self) -> Result<String> {
        let store_path = dirs::data_local_dir().join("flowlens").join("ca_key.enc");
        let encrypted = std::fs::read(&store_path)?;

        unsafe {
            let mut in_blob = DATA_BLOB {
                pbData: encrypted.as_ptr() as *mut _,
                cbData: encrypted.len() as u32,
            };
            let mut out_blob = DATA_BLOB::default();
            CryptUnprotectData(&mut in_blob, None, None, None, None, 0, &mut out_blob)?;

            let decrypted = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
            LocalFree(out_blob.pbData as *mut _);
            String::from_utf8(decrypted).map_err(|e| e.into())
        }
    }
}
```

### Notes

- `CryptProtectData` ties encryption to the current user profile by default.
- Consider adding an entropy blob (`pbOptionalEntropy`) to bind ciphertext to the app.
- Alternative: Windows Credential Locker (`PasswordVault` WinRT API).

---

## 6. Certificate Pinning Detection Strategy

### Detection Methods

| Technique | Detection Signal | Bypass Difficulty |
|-----------|-----------------|-------------------|
| Public Key Pinning | Client rejects MITM cert despite CA trusted | Hard |
| HPKP (deprecated) | HTTP header `Public-Key-Pins` | Medium |
| Custom pin validation | App-specific logic checking cert fingerprint | Very Hard |
| Transparency log verification | CT log monitoring in client | Medium |

### Implementation (Sketch)

```rust
pub struct PinningDetector {
    known_pins: DashMap<String, Vec<String>>, // domain → SHA256 hashes
}

impl PinningDetector {
    /// Detect pinning by observing connection failures
    pub fn observe_connection_failure(&self, domain: &str, error: &rustls::Error) -> bool {
        matches!(error,
            rustls::Error::InvalidCertificate(rustls::ErrorKind::UnknownIssuer))
        // CA trusted but connection fails → likely pinning
    }

    /// Active probing: connect and record SPKI hash
    pub async fn probe_pins(&self, domain: &str) -> Result<Vec<String>> {
        let config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinProbeVerifier::new()))
            .with_no_client_auth();
        let stream = TcpStream::connect(format!("{}:443", domain)).await?;
        let _tls = tokio_rustls::TlsConnector::from(Arc::new(config))
            .connect(ServerName::try_from(domain)?, stream).await?;
        Ok(self.known_pins.get(domain).map(|v| v.clone()).unwrap_or_default())
    }
}

struct PinProbeVerifier { observed_hash: Mutex<Option<String>> }

impl rustls::client::danger::ServerCertVerifier for PinProbeVerifier {
    fn verify_server_cert(&self, end_entity: &Certificate, _i: &[Certificate],
        _sn: &ServerName, _ocsp: &[u8], _now: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        *self.observed_hash.lock().unwrap() = Some(compute_spki_sha256(end_entity));
        Ok(ServerCertVerified::assertion())
    }
    // verify_tls12_signature / verify_tls13_signature: accept all
}
```

### Bypass Strategies (for testing only)

1. **Disable pinning in client config** — if you control the client, remove pin checks.
2. **Patch binary** — use Frida/x64dbg to NOP out pin validation calls.
3. **Network-level MITM with cert override** — some apps accept `-k`/insecure flags.
4. **Extract pins from app bundle** — static analysis of APK/IPA/resources.

---

## 7. Testing Strategy

### 7.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_leaf_generates_unique_keypair() {
        let engine = InMemoryTlsEngine::new();
        engine.generate_ca("TestCA").unwrap();
        let (cert1, key1) = engine.sign_leaf("example.com", &[]).unwrap();
        let (cert2, key2) = engine.sign_leaf("example.org", &[]).unwrap();
        assert_ne!(key1.der, key2.der);
        assert_ne!(cert1.der, cert2.der);
    }
}
```

### 7.2 Integration Test with OpenSSL

```bash
openssl req -x509 -newkey rsa:2048 -nodes -keyout srv.key -out srv.crt \
    -subj "/CN=test.local"
openssl s_server -cert srv.crt -key srv.key -port 8443 &
cargo run --bin flowlens-proxy -- --listen 127.0.0.1:8444 \
    --upstream 127.0.0.1:8443 --ca-pem ca.pem &
curl --cacert ca.pem https://127.0.0.1:8444
```

### 7.3 Rust Integration Test

```rust
#[tokio::test]
async fn test_mitm_handshake() {
    // Spawn test server, start proxy, connect client, verify plaintext logged
}
```

---

## 8. Ordered Implementation Steps

### Phase 1: Fix Core Bugs (Week 1)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 1.1 | `crates/flowlens-tls/src/lib.rs` | Fix `sign_leaf()` to generate leaf key before signing | **CRITICAL** |
| 1.2 | `crates/flowlens-tls/src/lib.rs` | Add `ExtendedKeyUsage` for serverAuth in leaf certs | High |
| 1.3 | `crates/flowlens-tls/src/lib.rs` | Add unit tests for certificate chain validity | High |

### Phase 2: MITM Proxy Core (Week 2-3)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 2.1 | `crates/flowlens-tls/src/mitm.rs` (NEW) | Create `MitmProxy` struct with `ServerConfig` | High |
| 2.2 | `crates/flowlens-tls/src/mitm.rs` | Implement `CertResolver` with SNI-based caching | High |
| 2.3 | `crates/flowlens-tls/src/mitm.rs` | Implement dual-stack handler (client ↔ proxy ↔ server) | High |
| 2.4 | `crates/flowlens-tls/src/mitm.rs` | Add plaintext logging hooks at decryption points | Medium |
| 2.5 | `crates/flowlens-tls/Cargo.toml` | Add `dashmap`, `tokio-rustls` dependencies | High |

### Phase 3: Network Redirection (Week 4)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 3.1 | `crates/flowlens-tls/src/redirection.rs` (NEW) | Implement hosts file manipulation | Medium |
| 3.2 | `crates/flowlens-tls/src/redirection.rs` | Add WFP callout driver stub (Windows-only) | Low |
| 3.3 | `crates/flowlens-cli/src/main.rs` | Add CLI flags for redirect configuration | Medium |

### Phase 4: Key Extraction (Week 5)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 4.1 | `crates/flowlens-tls/src/keylog.rs` (NEW) | Implement SSLKEYLOGFILE watcher | Medium |
| 4.2 | `crates/flowlens-tls/src/keylog.rs` | Add master secret lookup API | Medium |
| 4.3 | `crates/flowlens-tls/src/offline.rs` (NEW) | Implement offline record decryption | Low |

### Phase 5: Windows Integration (Week 6)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 5.1 | `crates/flowlens-tls/src/windows.rs` (NEW) | Add DPAPI key protection wrapper | Medium |
| 5.2 | `crates/flowlens-tls/src/windows.rs` | Implement CA installation via certutil | Medium |
| 5.3 | `Cargo.toml` | Add conditional `windows` crate dependency | Medium |

### Phase 6: Pinning Detection (Week 7)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 6.1 | `crates/flowlens-tls/src/pinning.rs` (NEW) | Implement `PinningDetector` | Low |
| 6.2 | `crates/flowlens-tls/src/pinning.rs` | Add active probing mode | Low |
| 6.3 | `crates/flowlens-tls/src/mitm.rs` | Integrate pinning warnings into proxy | Low |

### Phase 7: Testing & Documentation (Week 8)

| Step | File | Action | Priority |
|------|------|--------|----------|
| 7.1 | `tests/integration/` | Add OpenSSL s_server integration tests | High |
| 7.2 | `docs/review/tls_decryption_design.md` | This document (finalize) | Medium |
| 7.3 | `crates/flowlens-tls/README.md` | Usage examples and architecture diagram | Medium |

---

## Appendix A: New Dependencies

Add to `crates/flowlens-tls/Cargo.toml`:

```toml
rustls = "0.23"                    # TLS implementation
tokio-rustls = "0.26"              # Tokio integration
dashmap = "6.0"                    # Concurrent SNI cache
x509-parser = "0.16"               # Certificate parsing
hex = "0.4"                        # Hex encoding for keylog
dirs = "5.0"                       # Platform paths

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Security_Cryptography",
]}
```

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| SNI | Server Name Indication — TLS extension specifying requested hostname |
| ALPN | Application-Layer Protocol Negotiation |
| PFS | Perfect Forward Secrecy — session keys safe against long-term key leak |
| DPAPI | Data Protection API — Windows per-user encryption service |
| ETW | Event Tracing for Windows |
| WFP | Windows Filtering Platform — network stack hook points |
| CT | Certificate Transparency — public logs for issued certificates |

---

## Appendix C: References

1. [RFC 8446 - TLS 1.3](https://datatracker.ietf.org/doc/html/rfc8446)
2. [rustls Documentation](https://docs.rs/rustls/)
3. [rcgen Crate](https://docs.rs/rcgen/)
4. [NSS Key Log Format](https://developer.mozilla.org/en-US/docs/Mozilla/Projects/NSS/Key_Log_Format)
5. [Windows WFP APIs](https://learn.microsoft.com/en-us/windows/win32/fwp/filtering-platform-portal)
6. [DPAPI Overview](https://learn.microsoft.com/en-us/windows/win32/seccrypto/data-protection-api)

---

*Document ends.*
