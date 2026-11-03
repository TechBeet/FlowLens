# Mobile CA Trust Workflow - FlowLens Phase C

This document describes the workflow for users to trust the FlowLens CA certificate on iOS and Android devices. This is required for HTTPS MITM proxying to work on mobile devices.

## iOS Steps

1. **Download the CA certificate**
   - Option A: Scan the QR code below with the iPhone's Camera app
   - Option B: Email the CA certificate PEM to yourself and open it on the iPhone

2. **Open Settings**
   - Go to `Settings > General`

3. **Navigate to Certificate Trust Settings**
   - Tap `About` at the bottom of the General settings screen
   - Tap `Certificate Trust Settings` at the bottom

4. **Enable full trust for the CA**
   - Under "Enable full trust for", toggle on the FlowLens CA certificate
   - The toggle will turn green when enabled

## Android Steps

1. **Download the CA certificate**
   - Scan the QR code below or download the PEM file

2. **Open Settings**
   - Go to `Settings > Security` (or `Settings > Security & privacy`)

3. **Navigate to VPN & app certificates**
   - Tap `Encryption & credentials`
   - Tap `VPN & app certificates` (or similar, depending on Android version)

4. **Select the CA certificate**
   - Tap the FlowLens CA certificate from the list

5. **Enable certificates for VPN and apps**
   - Toggle on both "VPN" and "Apps" options (or "Use for certificate auth" / similar)
   - The certificate is now trusted for all app traffic

## QR Code Generation

For easier mobile download, a QR code can be generated from the CA certificate PEM:

```bash
# Generate QR code from CA cert PEM
# Requires: qrencode installed (brew install qrencode on macOS, apt-get install qrencode on Linux)

qrencode -t UTF8 < flowlens-ca.pem
# Or for PNG output:
qrencode -o flowlens-ca-qr.png flowlens-ca.pem
```

The QR code encodes the raw PEM content. Users can scan it with their device's camera or a QR code scanner app, which will import the certificate directly.

## Reference: CaManager API and CLI Commands

The following FlowLens internal interfaces support CA certificate management:

### CaManager API (`crates/flowlens-tls/src/lib.rs`)

```rust
pub trait CaManager: Send + Sync {
    fn generate_ca(&mut self, cn: &str) -> Result<CaInfo, TlsError>;
    fn install_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn uninstall_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn check_installed(&self, cn: &str) -> bool;
    fn ca_pem(&self) -> Option<String>;
}
```

Key types:
- `CaInfo` contains: `cn`, `certificate_pem`, `private_key`, `fingerprint_sha256`, `installed_at`
- `ca_pem()` returns the PEM-encoded certificate for sharing via QR code or email

### CLI Commands (Phase B - `crates/flowlens-cli/src/lib.rs`)

| Command | Description |
|---|---|
| `flowlens install-ca [--cn FlowLens CA]` | Install the FlowLens CA certificate into the Windows trusted root store |
| `flowlens uninstall-ca [--cn FlowLens CA]` | Uninstall the FlowLens CA certificate from the Windows trusted root store |
| `flowlens ca-status [--cn FlowLens CA]` | Check whether the FlowLens CA is installed |

### Workflow Summary

1. Generate CA via FlowLens TLS engine: `engine.generate_ca("FlowLens CA")`
2. Export the CA PEM: `engine.ca_pem().unwrap()`
3. Share PEM via QR code or email for mobile trust
4. User follows iOS or Android steps above to trust the certificate
5. Capture HTTPS traffic on mobile with FlowLens proxy

## Known Gaps

- [ ] **Android 12+ scoped CA trust**: Some Android versions restrict CA trust to specific apps rather than system-wide. Document which FlowLens mode applies.
- [ ] **iOS enterprise provisioning**: Enterprise-deployed FlowLens may use MDM to push the CA trust configuration automatically.
- [ ] **Android enterprise work profiles**: Additional steps may be needed for work profile certificate trust.
- [ ] ** macOS trust flow**: Not covered in this document (different Keychain workflow).
- [ ] Windows CLI `install-ca`/`uninstall-ca` commands exist (Phase B) but mobile trust workflow is separate.