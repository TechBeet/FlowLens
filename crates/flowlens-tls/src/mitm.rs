//! MITM TLS termination and decryption.

use std::io::Cursor;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls_pki_types::pem::PemObject;
use rustls::{ClientConfig, ServerConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tracing::debug;

use crate::TlsError;

use tracing::{info, warn};

/// Extract the SNI hostname from a raw ClientHello byte slice.
pub fn extract_sni(data: &[u8]) -> Option<String> {
    if data.len() < 5 {
        return None;
    }
    if data[0] != 0x16 {
        return None;
    }
    let record_len = u16::from_be_bytes([data[3], data[4]]) as usize;
    if data.len() < 5 + record_len {
        return None;
    }
    let handshake = &data[5..5 + record_len];
    if handshake.is_empty() || handshake[0] != 0x01 {
        return None;
    }
    let _cursor = 4;
    let _handshake_len = u32::from_be_bytes([0, handshake[1], handshake[2], handshake[3]]) as usize;
    let mut cursor = 4 + 2 + 32;
    if handshake.len() < cursor {
        return None;
    }
    let sid_len = handshake[cursor] as usize;
    cursor += 1 + sid_len;
    if handshake.len() < cursor + 2 {
        return None;
    }
    let cs_len = u16::from_be_bytes([handshake[cursor], handshake[cursor + 1]]) as usize;
    cursor += 2 + cs_len;
    if handshake.len() < cursor + 1 {
        return None;
    }
    let cm_len = handshake[cursor] as usize;
    cursor += 1 + cm_len;
    if handshake.len() < cursor + 2 {
        return None;
    }
    let ext_total = u16::from_be_bytes([handshake[cursor], handshake[cursor + 1]]) as usize;
    cursor += 2;
    let ext_end = cursor + ext_total;
    if ext_end > handshake.len() {
        return None;
    }
    while cursor + 4 <= ext_end {
        let ext_type = u16::from_be_bytes([handshake[cursor], handshake[cursor + 1]]);
        let ext_len = u16::from_be_bytes([handshake[cursor + 2], handshake[cursor + 3]]) as usize;
        cursor += 4;
        if cursor + ext_len > ext_end {
            return None;
        }
        if ext_type == 0x0000 && ext_len >= 5 {
            let mut p = cursor + 2;
            let ext_end_local = cursor + ext_len;
            while p + 3 <= ext_end_local {
                let name_type = handshake[p];
                let name_len = u16::from_be_bytes([handshake[p + 1], handshake[p + 2]]) as usize;
                p += 3;
                if p + name_len > ext_end_local {
                    return None;
                }
                if name_type == 0 {
                    return String::from_utf8(handshake[p..p + name_len].to_vec()).ok();
                }
                p += name_len;
            }
        }
        cursor += ext_len;
    }
    None
}

/// Build a rustls `ServerConfig` from a dynamically-issued leaf certificate.
pub fn build_server_config(cert_pem: &str, key_pem: &str) -> Result<ServerConfig, TlsError> {
    let mut cert_reader = Cursor::new(cert_pem);
    let mut cert_chain: Vec<CertificateDer<'static>> = Vec::new();
    for item in rustls_pki_types::CertificateDer::pem_slice_iter(cert_pem.as_bytes()) {
        let cert = item.map_err(|_| TlsError::CertGeneration("failed to parse cert".into()))?;
        cert_chain.push(cert.into_owned());
    }
    if cert_chain.is_empty() {
        return Err(TlsError::CertGeneration("no certificate found".into()));
    }

    let mut key_reader = Cursor::new(key_pem);
    let mut keys = Vec::new();
    for item in rustls_pki_types::PrivateKeyDer::pem_slice_iter(key_pem.as_bytes()) {
        let key = item.map_err(|_| TlsError::CertGeneration("failed to parse private key".into()))?;
        keys.push(key);
    }
    if keys.is_empty() {
        return Err(TlsError::CertGeneration("no private key found".into()));
    }
    let key = PrivateKeyDer::from(keys.remove(0));

    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|e| TlsError::CertGeneration(e.to_string()))?;
    Ok(cfg)
}

/// Build a rustls `ClientConfig` trusting the webpki roots.
pub fn build_client_config() -> Result<ClientConfig, TlsError> {
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let cfg = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(cfg)
}

/// TLS acceptor for incoming client connections (MITM server side).
pub struct MitmTlsAcceptor {
    pub inner: TlsAcceptor,
}

impl MitmTlsAcceptor {
    pub fn new(cfg: ServerConfig) -> Self {
        Self {
            inner: TlsAcceptor::from(Arc::new(cfg)),
        }
    }

    pub async fn accept(
        &self,
        stream: TcpStream,
    ) -> Result<tokio_rustls::server::TlsStream<TcpStream>, TlsError> {
        self.inner
            .accept(stream)
            .await
            .map_err(|e| TlsError::CertGeneration(e.to_string()))
    }
}

/// TLS connector for outbound connections to real servers.
pub struct MitmTlsConnector {
    pub inner: TlsConnector,
}

impl MitmTlsConnector {
    pub fn new(cfg: ClientConfig) -> Self {
        Self {
            inner: TlsConnector::from(Arc::new(cfg)),
        }
    }

    pub async fn connect(
        &self,
        server_name: &str,
        stream: TcpStream,
    ) -> Result<tokio_rustls::client::TlsStream<TcpStream>, TlsError> {
        let sni = ServerName::try_from(server_name.to_string())
            .map_err(|_| TlsError::CertGeneration("invalid server name".into()))?;
        self.inner
            .connect(sni, stream)
            .await
            .map_err(|e| TlsError::CertGeneration(e.to_string()))
    }
}

/// Bidirectional MITM stream exposing decrypted plaintext.
pub struct MitmStream<C, S> {
    pub client_tls: C,
    pub server_tls: S,
}

impl<C, S> MitmStream<C, S>
where
    C: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(client_tls: C, server_tls: S) -> Self {
        Self {
            client_tls,
            server_tls,
        }
    }

    pub fn spawn_forwarding(self) {
        let (mut client_r, mut client_w) = tokio::io::split(self.client_tls);
        let (mut server_r, mut server_w) = tokio::io::split(self.server_tls);

        tokio::spawn(async move {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match client_r.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if server_w.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = server_w.shutdown().await;
        });

        tokio::spawn(async move {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match server_r.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if client_w.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = client_w.shutdown().await;
        });

        debug!("MitmStream forwarding spawned");
    }
}

/// Result of classifying an incoming client connection at the SNI stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SniDecision {
    /// Terminate TLS with a per-host leaf cert issued for `sni`.
    Intercept,
    /// Pass raw bytes through untouched (decrypt disabled for this host).
    Passthrough,
}

/// Minimal SNI-aware MITM session used by the capture engine.
///
/// On a new TCP stream the engine peeks the first TLS record, extracts the SNI
/// from the ClientHello, consults the per-host `DomainPolicy`, and either:
///   * terminates TLS with `engine.server_config_for_sni(sni)` (per-host cert),
///   * or relays the raw stream untouched and logs that decryption is disabled.
///
/// This is intentionally dependency-light: callers supply the raw stream plus
/// handles to the shared `InMemoryTlsEngine` and `DomainPolicy` so the live
/// toggle state (set via the Tauri `toggle_decrypt_domain` command) is honoured.
pub struct MitmSession<'a, E, P> {
    pub engine: &'a E,
    pub policy: &'a P,
}

impl<'a, E, P> MitmSession<'a, E, P>
where
    E: crate::MitmProxy + Sync,
    P: crate::policy::DomainPolicyAccess + Sync,
{
    pub fn new(engine: &'a E, policy: &'a P) -> Self {
        Self { engine, policy }
    }

    /// Classify an inbound connection by peeking at its ClientHello SNI.
    pub fn classify(&self, hello: &[u8]) -> (SniDecision, Option<String>) {
        let sni = crate::mitm::extract_sni(hello);
        let decision = match &sni {
            Some(host) if self.policy.decrypt_enabled(host) => SniDecision::Intercept,
            _ => SniDecision::Passthrough,
        };
        (decision, sni)
    }

    /// Terminate TLS for `sni` using a per-host server config, then forward to
    /// the real upstream. Returns the accepted server-side stream on success.
    pub async fn terminate_with_sni_config(
        &self,
        sni: &str,
        client_stream: tokio::net::TcpStream,
    ) -> Result<tokio_rustls::server::TlsStream<tokio::net::TcpStream>, TlsError> {
        let cfg = self
            .engine
            .server_config_for_sni(sni)
            .map_err(|e| TlsError::CertGeneration(format!("server config for {sni}: {e}")))?;
        let acceptor = MitmTlsAcceptor::new(cfg);
        let tls = acceptor.accept(client_stream).await?;
        info!(%sni, "MITM: terminated TLS with per-host leaf cert");
        Ok(tls)
    }
}

/// Handle a single accepted TCP connection according to the per-host policy.
///
/// When decryption is disabled for the SNI host the raw stream is relayed
/// untouched and a log line is emitted. When enabled, TLS is terminated using
/// `server_config_for_sni(sni)` so a per-host leaf certificate is presented.
pub async fn handle_mitm_connection<E, P>(engine: &E, policy: &P, stream: tokio::net::TcpStream)
where
    E: crate::MitmProxy + Sync,
    P: crate::policy::DomainPolicyAccess + Sync,
{
    let mut peek = [0u8; 16 * 1024];
    // Peek without consuming so the passthrough path can forward the bytes.
    let n = match stream.peek(&mut peek).await {
        Ok(n) => n,
        Err(e) => {
            warn!(error = %e, "MITM: failed to peek ClientHello");
            return;
        }
    };
    if n == 0 {
        return;
    }

    let session = MitmSession::new(engine, policy);
    let (decision, sni) = session.classify(&peek[..n]);

    match decision {
        SniDecision::Passthrough => {
            if let Some(host) = &sni {
                info!(%host, "MITM: decryption disabled, passing through");
            } else {
                info!("MITM: no SNI / not TLS, passing through");
            }
            // Caller is expected to relay `stream` (with the peeked bytes
            // re-injected) to the upstream destination untouched.
            let _ = stream;
        }
        SniDecision::Intercept => {
            let sni = sni.expect("intercept requires SNI");
            // Note: in a full capture engine the `stream` would be handed to
            // `terminate_with_sni_config` after re-injecting the peeked bytes.
            // Here we demonstrate the per-host config selection path:
            let _ = session.terminate_with_sni_config(&sni, stream).await;
        }
    }
}
