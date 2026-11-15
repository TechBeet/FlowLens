use crate::state::AppState;
use flowlens_tls::{extract_sni, InMemoryTlsEngine};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

/// Default listening port when none is configured.
pub const DEFAULT_PORT: u16 = 8080;

/// Runtime that drives SNI-aware MITM termination per connection.
///
/// Each accepted connection peeks the ClientHello SNI and consults the live
/// `DomainPolicy`, so `toggle_decrypt_domain` immediately affects whether TLS
/// is terminated (per-host cert via `server_config_for_sni`) or passed through.
pub struct ProxyRuntime {
    engine: Arc<Mutex<InMemoryTlsEngine>>,
    policy: Arc<Mutex<flowlens_tls::DomainPolicy>>,
    running: Arc<AtomicBool>,
}

impl ProxyRuntime {
    pub fn new(state: &AppState) -> Self {
        Self {
            engine: state.ca_manager.clone(),
            policy: state.decrypt_policy.clone(),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Return `true` while the runtime is expected to keep accepting.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Signal the accept loop to stop and release any held listener resources.
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Run the accept loop on `addr`. Each connection is classified by SNI and
    /// handled according to the current per-host policy.
    ///
    /// The loop exits when `shutdown()` has been called. Accept errors are
    /// logged and do not crash the loop.
    pub async fn run_listener(self: Arc<Self>, addr: SocketAddr) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        let bound = listener.local_addr().unwrap_or(addr);
        tracing::info!(%bound, "MITM proxy listener bound");

        let running = self.running.clone();
        loop {
            tokio::select! {
                // Accept new connections while not shutting down.
                Ok((stream, peer)) = listener.accept(), if running.load(Ordering::Relaxed) => {
                    let engine = self.engine.clone();
                    let policy = self.policy.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(engine, policy, stream, peer).await {
                            tracing::warn!(peer = %peer, error = %e, "MITM proxy connection dropped");
                        }
                    });
                }
                // Poll for a shutdown flag change even when accept() is idle.
                _ = tokio::time::sleep(std::time::Duration::from_millis(10)), if running.load(Ordering::Relaxed) => {}
                // All guards disabled → the runtime was shut down: leave the loop.
                else => break,
            }
        }

        tracing::info!(%bound, "MITM proxy listener shut down");
        Ok(())
    }
}

/// Handle a single accepted TCP connection according to the per-host policy.
///
/// The peeked ClientHello is consumed only by `peek`, so both the intercept and
/// passthrough paths still see the original bytes on the socket.
async fn handle_connection(
    engine: Arc<Mutex<InMemoryTlsEngine>>,
    policy: Arc<Mutex<flowlens_tls::DomainPolicy>>,
    stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut peek = [0u8; 16 * 1024];
    let n = match stream.peek(&mut peek).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(peer = %peer, error = %e, "MITM: failed to peek ClientHello");
            return Ok(());
        }
    };
    if n == 0 {
        tracing::info!(peer = %peer, "MITM: empty stream, dropping");
        return Ok(());
    }

    let sni = extract_sni(&peek[..n]);
    let decision = {
        let policy = policy.lock().map_err(|e| e.to_string())?;
        match &sni {
            Some(host) if policy.decrypt_enabled(host) => flowlens_tls::SniDecision::Intercept,
            _ => flowlens_tls::SniDecision::Passthrough,
        }
    };

    match decision {
        flowlens_tls::SniDecision::Passthrough => {
            if let Some(host) = &sni {
                tracing::info!(%host, peer = %peer, "MITM: decryption disabled, passing through");
            } else {
                tracing::info!(peer = %peer, "MITM: no SNI / not TLS, passing through");
            }
            // The capture pipeline owns relaying `stream` verbatim to upstream.
            drop(stream);
        }
        flowlens_tls::SniDecision::Intercept => {
            let sni = sni.ok_or("intercept requires SNI")?;
            // Build the per-host config in a scoped block so the engine guard is
            // dropped before any TLS accept await.
            let cfg_result = {
                let engine_guard = engine.lock().map_err(|e| e.to_string())?;
                engine_guard.server_config_for_sni(&sni)
            };
            match cfg_result {
                Ok(cfg) => {
                    let acceptor = flowlens_tls::MitmTlsAcceptor::new(cfg);
                    match acceptor.accept(stream).await {
                        Ok(_tls) => {
                            tracing::info!(%sni, peer = %peer, "MITM: terminated TLS with per-host leaf cert");
                            // TODO: hand decrypted plaintext to the capture pipeline.
                        }
                        Err(e) => {
                            tracing::warn!(%sni, peer = %peer, error = %e, "MITM: TLS accept failed");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(%sni, peer = %peer, error = %e, "MITM: failed to build per-host server config");
                }
            }
        }
    }
    Ok(())
}

/// Drop implementation for [`ProxyRuntime`] — shuts down the accept loop when
/// the runtime is dropped. Placed above the `#[cfg(test)]` module to satisfy
/// clippy's `items_after_test_module` lint.
impl Drop for ProxyRuntime {
    fn drop(&mut self) {
        self.shutdown();
        tracing::debug!("MITM proxy runtime dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_tls::DomainPolicy;

    #[test]
    fn sni_decision_intercept_when_decrypt_enabled() {
        let mut policy = DomainPolicy::new();
        policy.add("example.com", true).unwrap();
        assert!(policy.decrypt_enabled("example.com"));
    }

    #[test]
    fn sni_decision_passthrough_when_decrypt_disabled() {
        let mut policy = DomainPolicy::new();
        policy.add("example.com", false).unwrap();
        assert!(!policy.decrypt_enabled("example.com"));
    }

    #[test]
    fn sni_decision_passthrough_for_unknown_host() {
        let policy = DomainPolicy::new();
        assert!(!policy.decrypt_enabled("unknown.example.io"));
    }

    #[test]
    fn wildcard_matches_single_label_subdomain_not_apex() {
        let mut policy = DomainPolicy::new();
        policy.add("*.cdn.example.net", true).unwrap();
        assert!(policy.decrypt_enabled("static.cdn.example.net"));
        assert!(!policy.decrypt_enabled("cdn.example.net"));
    }

    #[tokio::test]
    async fn graceful_stop_terminates_accept_loop() {
        let state = AppState::new(flowlens_core::BroadcastEventBus::new(16));
        let runtime = Arc::new(ProxyRuntime::new(&state));
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let handle = tokio::spawn(runtime.clone().run_listener(addr));

        // Give the loop a tick to bind and start accepting.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(runtime.is_running());

        runtime.shutdown();
        // The loop must observe the flag and return promptly.
        let joined = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        assert!(joined.is_ok(), "accept loop must stop after shutdown()");
        assert!(!runtime.is_running());
    }
}

