use crate::state::AppState;
use flowlens_tls::{extract_sni, InMemoryTlsEngine};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

/// Runtime that drives SNI-aware MITM termination per connection.
///
/// Each accepted connection peeks the ClientHello SNI and consults the live
/// `DomainPolicy`, so `toggle_decrypt_domain` immediately affects whether TLS
/// is terminated (per-host cert via `server_config_for_sni`) or passed through.
pub struct ProxyRuntime {
    engine: Arc<Mutex<InMemoryTlsEngine>>,
    policy: Arc<Mutex<flowlens_tls::DomainPolicy>>,
}

impl ProxyRuntime {
    pub fn new(state: &AppState) -> Self {
        Self {
            engine: state.ca_manager.clone(),
            policy: state.decrypt_policy.clone(),
        }
    }

    /// Run a blocking accept loop on `addr`. Each connection is classified by
    /// SNI and handled according to the current per-host policy.
    pub async fn run_listener(self: Arc<Self>, addr: std::net::SocketAddr) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!(%addr, "MITM proxy listener bound");
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "MITM proxy accept failed");
                    continue;
                }
            };
            let engine = self.engine.clone();
            let policy = self.policy.clone();
            tokio::spawn(async move {
                // Peek first without holding any locks, so locks are never held
                // across an await point (keeps the future `Send`).
                let hello = match extract_sni_peek(&stream).await {
                    Some(h) => h,
                    None => {
                        tracing::info!(peer = %peer, "MITM: empty / non-TLS stream, passing through");
                        return;
                    }
                };
                let decision = {
                    let policy = policy.lock().unwrap();
                    match policy.decrypt_enabled(&hello) {
                        true => flowlens_tls::SniDecision::Intercept,
                        false => flowlens_tls::SniDecision::Passthrough,
                    }
                };
                match decision {
                    flowlens_tls::SniDecision::Intercept => {
                        // Build the per-host config in a scoped block so the
                        // engine guard is dropped before the TLS accept await.
                        let cfg_result = {
                            let engine_guard = engine.lock().unwrap();
                            engine_guard.server_config_for_sni(&hello)
                        };
                        match cfg_result {
                            Ok(cfg) => {
                                let acceptor = flowlens_tls::MitmTlsAcceptor::new(cfg);
                                if let Ok(tls) = acceptor.accept(stream).await {
                                    tracing::info!(%hello, peer = %peer, "MITM: terminated TLS with per-host leaf cert");
                                    let _ = tls;
                                }
                            }
                            Err(e) => tracing::warn!(%hello, error = %e, "MITM: failed to build per-host server config"),
                        }
                    }
                    flowlens_tls::SniDecision::Passthrough => {
                        tracing::info!(peer = %peer, %hello, "MITM: decryption disabled, passing through");
                        let _ = stream;
                    }
                }
            });
        }
    }
}

/// Peek the first bytes of a stream to try extracting SNI without consuming.
async fn extract_sni_peek(stream: &tokio::net::TcpStream) -> Option<String> {
    let mut buf = [0u8; 16 * 1024];
    let n = stream.peek(&mut buf).await.ok()?;
    if n == 0 {
        return None;
    }
    extract_sni(&buf[..n])
}

impl Drop for ProxyRuntime {
    fn drop(&mut self) {
        tracing::debug!("MITM proxy runtime dropped");
    }
}