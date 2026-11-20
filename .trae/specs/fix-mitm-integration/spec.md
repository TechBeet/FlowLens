# Fix mitm_integration Hanging Tests Spec

## Why
The two integration tests `test_tls_termination` and `test_plaintext_extraction` in `crates/flowlens-tls/tests/mitm_integration.rs` hang indefinitely. This blocks the test suite and prevents verification of the TLS termination and plaintext extraction code paths. The root cause is that `tokio::join!` waits for both async tasks, but one or both tasks block on `tls.read()` or `tls.write_all()` due to missing TLS record flushing or handshake completion.

## What Changes
- **`crates/flowlens-tls/tests/mitm_integration.rs`**: Add `tokio::time::timeout` with 10s deadline around each `tokio::join!` in the two hanging tests. When timeout occurs, the test should `panic!` with a clear message rather than hang forever.
- **`crates/flowlens-tls/src/lib.rs`**: In `decrypt_stream`, add a note clarifying that real MITM decryption happens in the proxy path; the sync helper is only for test validation.
- **`crates/flowlens-tls/src/mitm.rs`**: Ensure `MitmTlsAcceptor::accept` and `MitmTlsConnector::connect` properly flush TLS records so that `read`/`write_all` return promptly.

## Impact
- Affected specs: `flowlens-tls`
- Affected code: `crates/flowlens-tls/tests/mitm_integration.rs`, `crates/flowlens-tls/src/lib.rs`, `crates/flowlens-tls/src/mitm.rs`

## ADDED Requirements
### Requirement: No hanging integration tests
The system SHALL not have integration tests that hang indefinitely. Each test must complete within a bounded time.

#### Scenario: test_tls_termination completes within timeout
- **WHEN** the test runs
- **THEN** it completes within 10 seconds and asserts `srv_msg == "ping"` and `cli_msg == "pong"`

#### Scenario: test_plaintext_extraction completes within timeout
- **WHEN** the test runs
- **THEN** it completes within 10 seconds and asserts decrypted data matches the original message

## MODIFIED Requirements
### Requirement: Integration test reliability
The `test_tls_termination` and `test_plaintext_extraction` tests SHALL use `tokio::time::timeout` to prevent indefinite blocking.

## REMOVED Requirements
无

## Verification
- `cargo test -p flowlens-tls --test mitm_integration` completes within 15 seconds
- All 6 tests in `mitm_integration.rs` pass
- No test output indicates "has been running for over 60 seconds"