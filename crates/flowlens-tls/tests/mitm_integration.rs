//! Integration tests for FlowLens MITM TLS engine.

use flowlens_tls::{CaManager, CertIssuer, InMemoryTlsEngine, MitmProxy};
use flowlens_core::models::{ConnectionKey, TlsVersion, TransportProtocol};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

const TEST_CA_CN: &str = "FlowLens Test CA";
const TEST_DOMAIN: &str = "example.test";

fn make_conn_key() -> ConnectionKey {
    ConnectionKey {
        src_addr: "127.0.0.1:12345".parse().unwrap(),
        dst_addr: "127.0.0.1:443".parse().unwrap(),
        protocol: TransportProtocol::Tcp,
        connection_id: 1,
        timestamp: 0,
        interface_guid: None,
    }
}

fn engine_with_ca() -> InMemoryTlsEngine {
    let mut e = InMemoryTlsEngine::new();
    e.generate_ca(TEST_CA_CN).unwrap();
    e
}

fn server_cfg_for(engine: &InMemoryTlsEngine, domain: &str) -> rustls::ServerConfig {
    let (cp, kp) = engine.issue_for_sni(domain).unwrap();
    flowlens_tls::build_server_config(&cp, &kp).unwrap()
}

fn client_cfg_trusting_ca(engine: &InMemoryTlsEngine) -> rustls::ClientConfig {
    let ca_pem = engine.ca_pem().unwrap();
    let mut store = rustls::RootCertStore::empty();
    for c in rustls_pemfile::certs(&mut ca_pem.as_bytes()).flatten() {
        store.add(c).unwrap();
    }
    rustls::ClientConfig::builder()
        .with_root_certificates(store)
        .with_no_client_auth()
}

#[test]
fn test_ca_generation() {
    let mut e = InMemoryTlsEngine::new();
    let ca = e.generate_ca(TEST_CA_CN).unwrap();
    assert_eq!(ca.cn, TEST_CA_CN);
    assert!(ca.certificate_pem.contains("-----BEGIN CERTIFICATE-----"));
    assert!(ca.private_key.pem.contains("-----BEGIN PRIVATE KEY-----"));
    assert_eq!(ca.fingerprint_sha256.len(), 64);
    let _parsed = rcgen::CertificateParams::from_ca_cert_pem(&ca.certificate_pem).unwrap();
    let ca2 = e.generate_ca(TEST_CA_CN).unwrap();
    assert_eq!(ca.fingerprint_sha256, ca2.fingerprint_sha256);
    assert!(e.check_installed(TEST_CA_CN));
}

#[test]
fn test_leaf_cert_signing() {
    let engine = engine_with_ca();
    let (cert, key) = engine.sign_leaf_cert(TEST_DOMAIN, &[TEST_DOMAIN.to_string()]).unwrap();
    assert!(cert.pem().contains("-----BEGIN CERTIFICATE-----"));
    assert!(key.pem.contains("-----BEGIN PRIVATE KEY-----"));
    let der = cert.der().to_vec();
    assert!(!der.is_empty());
    assert!(!key.der.is_empty());
    let pem = cert.pem();
    assert!(pem.contains("BEGIN CERTIFICATE"));
}

#[test]
fn test_sni_extraction() {
    let engine = engine_with_ca();
    let ch: &[u8] = &[
        0x16, 0x03, 0x03, 0x00, 0x44, 0x01, 0x00, 0x00, 0x40, 0x03, 0x03,
        0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81, 0x92, 0xa3, 0xb4, 0xc5, 0xd6, 0xe7, 0xf8,
        0x09, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81, 0x92, 0xa3, 0xb4, 0xc5, 0xd6, 0xe7,
        0xf8, 0x09, 0x00, 0x00, 0x02, 0x00, 0x2f, 0x01, 0x00,
        0x00, 0x15,
        0x00, 0x00,
        0x00, 0x11,
        0x00, 0x0f,
        0x00,
        0x00, 0x0c,
        b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b't', b'e', b's', b't',
    ];
    let sni = engine.extract_sni_from_client_hello(ch);
    assert_eq!(sni.as_deref(), Some(TEST_DOMAIN));
    assert!(engine.extract_sni_from_client_hello(&[]).is_none());
}

#[test]
fn test_cert_cache() {
    let engine = engine_with_ca();
    assert!(engine.get_cached_cert(TEST_DOMAIN).is_none());
    let (c1, k1) = engine.sign_leaf_cert(TEST_DOMAIN, &[TEST_DOMAIN.to_string()]).unwrap();
    engine.cache_cert(TEST_DOMAIN, (c1, k1));
    assert!(engine.get_cached_cert(TEST_DOMAIN).is_some());
    let (p1, _kp1) = engine.issue_for_sni(TEST_DOMAIN).unwrap();
    let (p2, _kp2) = engine.issue_for_sni(TEST_DOMAIN).unwrap();
    assert_eq!(p1, p2);

    let other = "other.test";
    let (co, ko) = engine.sign_leaf_cert(other, &[other.to_string()]).unwrap();
    engine.cache_cert(other, (co, ko));
    assert!(engine.get_cached_cert(other).is_some());
}

#[tokio::test]
async fn test_tls_termination() {
    let engine = engine_with_ca();
    let domain = TEST_DOMAIN;
    let srv_cfg = server_cfg_for(&engine, domain);
    let acceptor = TlsAcceptor::from(Arc::new(srv_cfg));
    let cli_cfg = client_cfg_trusting_ca(&engine);
    let connector = TlsConnector::from(Arc::new(cli_cfg));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let srv = tokio::spawn({
        let acceptor = acceptor.clone();
        async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut tls = acceptor.accept(stream).await.unwrap();
            let mut buf = [0u8; 1024];
            let n = tls.read(&mut buf).await.unwrap();
            let msg = String::from_utf8_lossy(&buf[..n]).to_string();
            let _ = tls.write_all(b"pong").await;
            msg
        }
    });
    let cli = tokio::spawn({
        let connector = connector.clone();
        async move {
            let stream = TcpStream::connect(addr).await.unwrap();
            let dns = rustls::pki_types::ServerName::try_from(domain).unwrap();
            let mut tls = connector.connect(dns, stream).await.unwrap();
            let _ = tls.write_all(b"ping").await;
            let mut buf = [0u8; 1024];
            let n = tls.read(&mut buf).await.unwrap();
            String::from_utf8_lossy(&buf[..n]).to_string()
        }
    });
    let (srv_msg, cli_msg) = tokio::join!(srv, cli);
    assert_eq!(srv_msg.unwrap(), "ping");
    assert_eq!(cli_msg.unwrap(), "pong");
}

#[tokio::test]
async fn test_plaintext_extraction() {
    let engine = engine_with_ca();
    let domain = TEST_DOMAIN;
    let srv_cfg = server_cfg_for(&engine, domain);
    let acceptor = TlsAcceptor::from(Arc::new(srv_cfg));
    let cli_cfg = client_cfg_trusting_ca(&engine);
    let connector = TlsConnector::from(Arc::new(cli_cfg));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let msg = b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n";
    let srv = tokio::spawn({
        let acceptor = acceptor.clone();
        async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut tls = acceptor.accept(stream).await.unwrap();
            let mut buf = [0u8; 4096];
            let n = tls.read(&mut buf).await.unwrap();
            let all = buf[..n].to_vec();
            let _ = tls.write_all(&all).await;
            String::from_utf8_lossy(&all).to_string()
        }
    });
    let cli = tokio::spawn({
        let connector = connector.clone();
        async move {
            let stream = TcpStream::connect(addr).await.unwrap();
            let dns = rustls::pki_types::ServerName::try_from(domain).unwrap();
            let mut tls = connector.connect(dns, stream).await.unwrap();
            let _ = tls.write_all(msg).await;
            let mut buf = [0u8; 4096];
            let n = tls.read(&mut buf).await.unwrap();
            String::from_utf8_lossy(&buf[..n]).to_string()
        }
    });
    let (srv_recv, cli_echo) = tokio::join!(srv, cli);
    assert_eq!(srv_recv.unwrap(), std::str::from_utf8(msg).unwrap());
    assert_eq!(cli_echo.unwrap(), std::str::from_utf8(msg).unwrap());
    let conn_key = make_conn_key();
    // Create a minimal MitmContext for API compatibility
    let leaf = engine.sign_leaf_cert(TEST_DOMAIN, &[TEST_DOMAIN.to_string()]).unwrap();
    let ctx = flowlens_tls::MitmContext {
        conn: conn_key,
        leaf_cert: leaf,
        server_name: TEST_DOMAIN.to_string(),
        tls_version: flowlens_core::models::TlsVersion::Tls12,
        alpn: None,
    };
    let decrypted = engine.decrypt_stream(&ctx, msg).unwrap();
    assert_eq!(decrypted, msg);
}
