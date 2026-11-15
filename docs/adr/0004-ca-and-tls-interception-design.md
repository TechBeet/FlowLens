# ADR 0004 — CA and TLS Interception Design

**Status:** Accepted  
**Deciders:** FlowLens maintainers  
**Date:** 2024-01-01  

## Context

FlowLens decrypts HTTPS traffic to inspect request/response content. It operates as a local **man-in-the-middle (MITM) proxy** on the host:

1. A local **root CA** is generated (`flowlens-tls` CA manager) and must be installed into the user's Windows certificate store so clients trust FlowLens-issued leaf certificates.
2. Captured TLS connections are classified by **SNI** (Server Name Indication) peeked from the TLS ClientHello (`extract_sni` in `mitm.rs`).
3. Per-host policy (`DomainPolicy` in `policy.rs`) decides whether to **intercept** (terminate TLS with a per-host leaf cert issued for the SNI, then forward to the real server) or **passthrough** (leave TLS untouched).
4. The user manages which domains are decrypted via the SNI Domain Selector UI (`SniDomainSelector.svelte`), backed by Tauri commands (`add_decrypt_domain`, `remove_decrypt_domain`, `toggle_decrypt_domain`, `issue_for_sni`, `list_decrypt_domains`).
5. CA install/uninstall/status use Windows CryptoAPI via `cert_store.rs` (`install_ca`, `uninstall_ca`, `check_ca_status`), and the CA PEM/QR can be exported (`get_ca_pem`, `export_ca_qr`).

The design decision covers CA generation/storage, SNI interception flow, per-host decrypt policy, and user trust model.

## Decision

- **Local root CA**: Generated once on first use (or on demand via `generate_ca`), stored in memory/derived per session; exposed to the user as PEM (`get_ca_pem`) and QR (`export_ca_qr`) for manual or guided installation.
- **Install into Windows trust store**: `install_ca`/`uninstall_ca` add/remove the root CA from the Windows certificate store (Local Machine / Current User) via `cert_store.rs` (Crypt32). `check_ca_status` reports whether it is installed; the CA status chip in `Header.svelte` reflects this.
- **SNI-based interception**: The capture engine peeks the ClientHello, extracts SNI (`extract_sni`), and `MitmSession.classify` returns `SniDecision::Intercept` or `Passthrough` based on the policy. Intercepted connections terminate TLS with a per-host leaf cert (`build_server_config`/`server_config_for_sni`), then forward to the origin.
- **Per-host decrypt policy**: `DomainPolicy` holds ordered `DomainRule`s (`domain`, `decrypt`, `wildcard`). Wildcard rules (`*.example.com`) match single-label subdomains. The policy is consulted per SNI; the UI lets the operator add/remove/toggle domains.
- **Trust model**: FlowLens is a **local, user-controlled** MITM. It does not proxy traffic to a remote server; decryption happens entirely on the user's machine for diagnostic purposes.

## Consequences

### Positive

- **Full HTTPS visibility**: Operators can inspect request/response headers and bodies for TLS traffic they choose to decrypt.
- **Granular control**: Per-domain and wildcard rules let users decrypt only sensitive targets and passthrough the rest (e.g. banking, pinned-cert apps).
- **User-controlled trust**: The root CA is generated and installed locally; the user decides when to install/uninstall it and can export the PEM/QR.
- **Windows-native trust integration**: `cert_store.rs` uses the real Windows certificate store, so browsers and OS tools that use the system store trust FlowLens leaf certs once the root CA is installed.

### Negative

- **Security responsibility**: Installing a local root CA gives FlowLens the ability to mint certificates for any domain; users must understand this and only install it for intentional diagnostics, then uninstall when done.
- **Certificate pinning breaks**: Apps/services using certificate pinning or custom trust stores will fail when their traffic is intercepted; these domains should be set to passthrough.
- **Windows-only trust store**: `cert_store.rs` targets Windows CryptoAPI; CA installation on macOS/Linux would need a separate implementation.
- **SNI visibility limits**: Encrypted Client Hello (ECH) or post-quantum TLS extensions can hide SNI, reducing interception coverage for some modern connections.

## Alternatives considered

| Alternative | Assessment |
|-------------|------------|
| **Passive (no decryption) capture only** | Rejected: cannot inspect HTTPS request/response content, which is a core FlowLens feature. |
| **Remote/proxied MITM (send traffic to a server)** | Rejected: adds latency, privacy risk, and infrastructure; FlowLens decrypts locally only. |
| **Install CA automatically without user consent** | Rejected: modifying the system trust store silently is unsafe; installation is explicit via UI, with status shown in the header. |
| **Decrypt-all / no per-host policy** | Rejected: breaks pinned-cert and sensitive domains; per-host policy with wildcard support is strictly more flexible. |
