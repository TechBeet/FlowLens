//! End-to-end HTTPS decryption test through the async MITM proxy path.
use flowlens_tls::{
    build_server_config, InMemoryTlsEngine, MitmTlsAcceptor, MitmTlsConnector,
    MitmStream, CaManager,
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use rustls::pki_types::ServerName;

const TEST_DOMAIN: &str = "flowlens-mitm.test";
const PLAINTEXT: &str = "flowlens-tls-e2e-plaintext";

fn engine_with_ca() -> InMemoryTlsEngine {
    let mut engine = InMemoryTlsEngine::new();
    engine.generate_ca("FlowLens E2E CA").unwrap();
    engine
}

fn client_config_trusting_ca(engine: &InMemoryTlsEngine) -> rustls::ClientConfig {
    let pem = engine.ca_pem().unwrap();
    let mut store = rustls::RootCertStore::empty();
    for c in rustls_pemfile::certs(&mut pem.as_bytes()) {
        let c = c.unwrap();
        store.add(c).unwrap();
    }
    rustls::ClientConfig::builder()
        .with_root_certificates(store)
        .with_no_client_auth()
}

#[tokio::test]
async fn e2e_https_decrypt_through_mitm() {
    let engine = engine_with_ca();
    let domain = TEST_DOMAIN;

    // ---- Real TLS server (leaf cert signed by engine CA) ----
    let server_cfg = {
        let (cert_pem, key_pem) = engine.issue_for_sni(domain).unwrap();
        build_server_config(&cert_pem, &key_pem).unwrap()
    };
    let server_acceptor = TlsAcceptor::from(Arc::new(server_cfg));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut tls = server_acceptor.accept(stream).await.unwrap();
        let mut buf = [0u8; 1024];
        let n = tls.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], PLAINTEXT.as_bytes());
        tls.write_all(PLAINTEXT.as_bytes()).await.unwrap();
    });

    // ---- MITM proxy: accepts client TLS, connects to real server ----
    let proxy_leaf = engine.issue_for_sni(domain).unwrap();
    let proxy_server_cfg = build_server_config(&proxy_leaf.0, &proxy_leaf.1).unwrap();
    let acceptor = MitmTlsAcceptor::new(proxy_server_cfg);

    let client_cfg = client_config_trusting_ca(&engine);
    let connector = MitmTlsConnector::new(client_cfg);

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    let proxy_handle = tokio::spawn(async move {
        let (client_stream, _) = proxy_listener.accept().await.unwrap();
        let client_tls = acceptor.accept(client_stream).await.unwrap();
        let server_stream = TcpStream::connect(server_addr).await.unwrap();
        let server_tls = connector.connect(domain, server_stream).await.unwrap();
        let mitm = MitmStream::new(client_tls, server_tls);
        mitm.spawn_forwarding();
    });

    // ---- End client trusts the FlowLens CA and talks through the proxy ----
    let dns = ServerName::try_from(TEST_DOMAIN).unwrap();
    let client_connector =
        tokio_rustls::TlsConnector::from(Arc::new(client_config_trusting_ca(&engine)));
    let stream = TcpStream::connect(proxy_addr).await.unwrap();
    let mut tls = client_connector.connect(dns, stream).await.unwrap();

    // Send plaintext through the MITM; the proxy decrypts + re-encrypts it.
    tls.write_all(PLAINTEXT.as_bytes()).await.unwrap();
    let mut buf = [0u8; 1024];
    let n = tls.read(&mut buf).await.unwrap();
    assert_eq!(
        &buf[..n],
        PLAINTEXT.as_bytes(),
        "MITM must deliver plaintext"
    );

    tls.shutdown().await.ok();

    server_handle.await.unwrap();
    proxy_handle.await.unwrap();
}