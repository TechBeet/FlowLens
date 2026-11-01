# HTTP Debugger Pro vs FlowLens Feature Parity Matrix

> **Document Version**: 1.0  
> **Date**: 2026/01/20  
> **Purpose**: Gap analysis for "replacement-ready" status  
> **Scope**: Map HTTP Debugger Pro core features to FlowLens implementation status

## Executive Summary

FlowLens is a **proxy-less** HTTP/HTTPS debugging tool built in Rust, targeting the same use cases as HTTP Debugger Pro but with a fundamentally different architecture (WFP/Npcap vs traditional proxy). While FlowLens has strong foundations in capture, protocol parsing, and storage, several critical features remain incomplete or missing entirely.

### Replacement-Ready Verdict: **NOT READY** (core features exist at library level; CLI/UI wiring pending)

**Critical blockers**:
1. TLS MITM decryption is a passthrough stub (SEC-01)
2. Mobile debugging not tested/implemented

**Library-level implemented but not yet wired to CLI/UI pipeline**:
- Breakpoints (pause/release/modify) — `flowlens-core::InterceptEngine`
- Rewrite rules — `flowlens-core::RewriteEngine`
- Compare tool / diff — `flowlens-replay::diff_sessions`
- Replay — `flowlens-replay::ReplayClient`
- Fuzzer — `flowlens-replay::fuzz_param`

---

## Feature Status Matrix

### Legend
- ✅ **Fully Working**: Proven by tests/code, production-ready
- ⚠️ **Partial**: API exists, behavior incomplete or documented stub
- ❌ **Not Implemented**: Feature absent or only scaffolded

| # | HTTP Debugger Pro Feature | FlowLens Status | Evidence | Blocker? |
|---|---------------------------|-----------------|----------|----------|
| 1 | Proxy-based capture | ❌ Not Implemented | FlowLens targets proxy-less only; no proxy listener code | No |
| 2 | **Proxy-less capture** (WFP/Npcap) | ✅ Fully Working | `crates/flowlens-capture/src/pcap.rs` - Npcap engine with auto-fallback to mock | No |
| 3 | **HTTPS decryption** (root CA, dynamic cert) | ⚠️ **Partial** | CA generation works (`rcgen`), but `decrypt_stream` is passthrough stub (line 158-160 in `flowlens-tls/src/lib.rs`) | **YES** |
| 4 | Traffic filtering (process/host/url/time) | ✅ Fully Working | `crates/flowlens-core/src/filter.rs` - Full DSL parser with AND/OR/NOT, status ranges, glob patterns | No |
| 5 | **Breakpoints** (pause request/response) | ⚠️ **Partial** | Library present: `crates/flowlens-core/src/intercept.rs` (`InterceptEngine` with pause/release/modify/list_rules, unit tests passing). Wired into Tauri UI; not yet integrated into CLI pipeline. | No |
| 6 | **Rewrite rules** (modify headers/body) | ⚠️ **Partial** | Library present: `crates/flowlens-core/src/rewrite.rs` (`RewriteEngine` with add_rule/remove_rule/toggle_rule/matches_request/apply_rules, unit tests passing). Wired into Tauri UI; not yet integrated into CLI pipeline. | No |
| 7 | **Compare tool** (side-by-side diff) | ⚠️ **Partial** | Library present: `crates/flowlens-replay/src/diff.rs` (`diff_sessions` with unit tests passing). Logic exists but not wired into CLI/storage pipeline (TODO in `src/main.rs`). | No |
| 8 | **Replay** (resend requests) | ⚠️ **Partial** | Library present: `crates/flowlens-replay/src/client.rs` (`ReplayClient` with unit tests). CLI command exists in `crates/flowlens-cli` but storage integration is TODO. | No |
| 9 | **Fuzzer** (random data injection) | ⚠️ **Partial** | Library present: `crates/flowlens-replay/src/fuzzer.rs` (`fuzz_param` with unit tests). Not yet wired into CLI/UI pipeline. | No |
| 10 | **Mobile debugging** (trust CA on device) | ⚠️ Partial | CA lifecycle exists, but no mobile-specific test suite or documentation | **YES** |

---

## Detailed Gap Analysis

### 1. HTTPS Decryption (⚠️ Partial → Critical Blocker)

**Current State**:
- CA generation: ✅ Working (`rcgen` + `rustls`)
- Certificate installation: ✅ Scaffolded (`crates/flowlens-installer/src/lib.rs`)
- Actual decryption: ❌ **Passthrough stub**

**Evidence**:
```rust
// crates/flowlens-tls/src/lib.rs:158-160
pub fn decrypt_stream(&self, _conn: &ConnectionKey, data: &[u8]) -> Result<Vec<u8>, TlsError> {
    Ok(data.to_vec())  // <-- Passthrough, no decryption
}
```

**Code Review Finding** (SEC-01):
> `decrypt_stream` is a **passthrough stub**: `Ok(data.to_vec())`. No TLS 1.2/1.3 decryption occurs. Captured HTTPS traffic is exposed to consumers as opaque ciphertext while the tool advertises "MITM decryption" in README.

**Additional Bug** (SEC-02):
> `sign_leaf` signs the leaf certificate with **`&ca_key`** (the CA private key) for both issuer and subject key arguments. The leaf cert's embedded public key is therefore the CA's key, not the generated `leaf_key`.

**Acceptance Criteria for "Working"**:
- [ ] Real rustls-based decryption implemented
- [ ] Leaf cert signed with separate leaf key (not CA key)
- [ ] TLS 1.2 and TLS 1.3 sessions decrypted successfully
- [ ] Verified decryption of google.com, cloudflare.com
- [ ] Unit tests with known plaintext/ciphertext pairs
- [ ] Compile-time feature gate or runtime flag to distinguish ciphertext vs plaintext

**Estimated Effort**: 2-3 weeks (Phase 1 task 6, complexity ★★★★★)

---

### 2. Breakpoints (❌ Not Implemented → Critical Blocker)

**Current State**: No code exists. Search for `struct.*Breakpoint|breakpoint.*manager` returned zero results.

**Required Implementation**:
- Request breakpoint: Pause before sending, allow modification
- Response breakpoint: Pause before receiving, allow modification
- Conditional breakpoints (by host, path, status)
- UI controls to toggle/resume

**Acceptance Criteria**:
- [ ] `BreakpointManager` struct with add/remove/list operations
- [ ] Integration with capture pipeline to pause traffic
- [ ] UI controls in TUI/GUI to manage breakpoints
- [ ] Conditional breakpoint support (host=path*, status>=400)
- [ ] Resume single/multiple blocked requests

**Estimated Effort**: 1-2 weeks (High complexity due to async pipeline integration)

---

### 3. Rewrite Rules (❌ Not Implemented → Critical Blocker)

**Current State**: No code exists. Search for `fn.*rewrite|rule.*engine` returned zero results.

**Required Implementation**:
- Header manipulation (add/remove/replace)
- Body modification (regex replace, JSON patch)
- URL rewriting (host/path substitution)
- Rule persistence (save/load rule sets)
- Rule ordering/priority

**Acceptance Criteria**:
- [ ] `RewriteRule` struct with match conditions and actions
- [ ] `RewriteEngine` that applies rules to traffic
- [ ] Support for header/body/URL modifications
- [ ] Rule ordering (first-match-wins or priority-based)
- [ ] UI for rule management
- [ ] Import/export rule configurations

**Estimated Effort**: 2 weeks (Medium-high complexity)

---

### 4. Compare Tool (❌ Not Implemented → Critical Blocker)

**Current State**: HAR export exists (`crates/flowlens-export/src/har.rs`), but no diff engine.

**Required Implementation**:
- Side-by-side view of two requests/responses
- Line-by-line diff for text bodies
- Binary diff for images/files
- Header comparison
- Session-to-session comparison

**Acceptance Criteria**:
- [ ] `DiffEngine` with text/binary diff algorithms
- [ ] Side-by-side UI view with syntax highlighting
- [ ] Highlight additions/deletions/changes
- [ ] Export diff reports
- [ ] Support comparing any two captured sessions

**Estimated Effort**: 1-2 weeks (Medium complexity; can leverage `similar` or `difflib` crates)

---

### 5. Replay Functionality (❌ Not Implemented → Critical Blocker)

**Current State**: Storage exists with full CRUD (`crates/flowlens-storage`), but no replayer.

**Required Implementation**:
- Resend captured request to original or modified destination
- Preserve headers/method/body
- Option to modify target host/port before replay
- Batch replay (resend multiple requests)
- Replay history/logging

**Acceptance Criteria**:
- [ ] `RequestReplayer` with single/batch operations
- [ ] Modify target before replay (host/port/path)
- [ ] Preserve original headers/body with optional overrides
- [ ] Display replay response alongside original
- [ ] Rate limiting for batch replay

**Estimated Effort**: 1 week (Low-medium complexity; uses existing HTTP client)

---

### 6. Fuzzer (❌ Not Implemented → High Priority)

**Current State**: No code exists.

**Required Implementation**:
- Random data injection in headers/body/URL params
- Pattern-based fuzzing (SQLi, XSS payloads)
- Boundary value testing
- Fuzzing templates
- Response analysis (detect errors/anomalies)

**Acceptance Criteria**:
- [ ] `FuzzerModule` with configurable mutation strategies
- [ ] Predefined payload libraries (SQLi, XSS, path traversal)
- [ ] Custom payload input
- [ ] Response pattern detection (500 errors, timing anomalies)
- [ ] Fuzzing report generation

**Estimated Effort**: 2-3 weeks (High complexity; security-sensitive)

---

### 7. Mobile Debugging (⚠️ Partial → High Priority)

**Current State**: CA lifecycle exists, but no mobile-specific implementation or tests.

**Evidence**:
- `docs/review/code_review.md:22` mentions the tool is a "mock/demo harness"
- No mobile test suite or equivalent found
- README does not document mobile setup

**Required Implementation**:
- Document CA installation on iOS/Android
- Provide QR code for easy CA download
- Test with Safari (iOS) and Chrome (Android)
- Handle Android 7+ user cert restrictions (require system cert)
- Handle iOS certificate trust settings

**Acceptance Criteria**:
- [ ] Step-by-step mobile setup guide
- [ ] QR code generator for CA download
- [ ] Integration tests with physical iOS/Android devices
- [ ] Verification of HTTPS decryption on mobile traffic
- [ ] Troubleshooting guide for common issues (Android 7+, iOS trust settings)

**Estimated Effort**: 1 week (Documentation + testing; CA infrastructure exists)

---

## Priority Matrix

| Feature | Priority | Blocker for "Replacement-Ready"? | Estimated Effort | Dependencies |
|---------|----------|----------------------------------|------------------|--------------|
| HTTPS Decryption | **Critical** | **YES** | 2-3 weeks | Phase 1 foundation |
| Breakpoints | **Critical** | **YES** | 1-2 weeks | Capture pipeline |
| Rewrite Rules | **Critical** | **YES** | 2 weeks | Traffic pipeline |
| Compare Tool | **Critical** | **YES** | 1-2 weeks | Storage/UI |
| Replay | **Critical** | **YES** | 1 week | Storage/HTTP client |
| Mobile Debugging | **High** | **YES** | 1 week | CA infrastructure |
| Fuzzer | **High** | No (nice-to-have) | 2-3 weeks | Security review |
| Proxy-based Capture | **Low** | No (out of scope) | N/A | Deliberately excluded |

---

## Implementation Roadmap to "Replacement-Ready"

### Phase A: Core Functionality (Weeks 1-6)
1. **Week 1-3**: Fix TLS decryption (SEC-01, SEC-02)
   - Implement real rustls MITM
   - Fix leaf cert signing bug
   - Add verification tests
   
2. **Week 4**: Implement Replay
   - Simple replayer using existing HTTP client
   - Basic UI controls
   
3. **Week 5**: Implement Breakpoints
   - Pause/resume mechanism in capture pipeline
   - UI controls
   
4. **Week 6**: Implement Rewrite Rules
   - Rule engine with header/body modifications
   - Persistence layer

### Phase B: Advanced Features (Weeks 7-10)
5. **Week 7-8**: Implement Compare Tool
   - Diff engine (text/binary)
   - Side-by-side UI view
   
6. **Week 9**: Mobile Debugging
   - Documentation + QR code generator
   - Device testing
   
7. **Week 10**: Integration testing + polish
   - End-to-end tests for all features
   - Performance validation

### Phase C: Optional Enhancements (Weeks 11+)
8. **Week 11-13**: Fuzzer module
9. **Week 14+**: Performance optimization (Phase 5 roadmap)

---

## Acceptance Criteria for "Replacement-Ready"

To declare FlowLens "replacement-ready" for HTTP Debugger Pro, the following must be verified:

### Functional Requirements
- [ ] **TLS Decryption**: Successfully decrypts HTTPS traffic from Chrome, Edge, Firefox, and mobile devices
- [ ] **Breakpoints**: Can pause request/response and modify before continuing
- [ ] **Rewrite Rules**: Can modify headers/body based on match conditions
- [ ] **Compare Tool**: Side-by-side diff of two captured sessions with visual highlighting
- [ ] **Replay**: Can resend captured requests to original or modified destinations
- [ ] **Mobile Debugging**: Documented workflow with successful HTTPS decryption on iOS/Android

### Non-Functional Requirements
- [ ] **Performance**: CPU < 10% under moderate load (100 req/sec)
- [ ] **Stability**: 24-hour continuous capture without memory leak
- [ ] **Compatibility**: HAR export importable into Chrome DevTools
- [ ] **Documentation**: Complete user guide covering all features

### Test Coverage
- [ ] Unit tests: >80% coverage on core modules
- [ ] Integration tests: All critical paths covered
- [ ] E2E tests: Full workflow validation (capture → decrypt → filter → export)

---

## Known Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| TLS decryption complexity | High (blocks all HTTPS inspection) | Prioritize Week 1-3; allocate senior engineer |
| Breakpoint pipeline integration | Medium (requires async coordination) | Design pause mechanism early; unit test with mock traffic |
| Mobile OS restrictions (Android 7+, iOS) | Medium (may limit mobile debugging) | Document limitations; provide workarounds (root/jailbreak options) |
| WFP driver stability (Phase 4) | High (kernel-mode crashes) | Maintain Npcap fallback; extensive VM testing |

---

## Conclusion

FlowLens has a **strong foundation** with working Npcap capture, protocol parsing (HTTP/1.1, HTTP/2, WebSocket, SSE, gRPC), filtering, and storage. However, it is **not replacement-ready** for HTTP Debugger Pro due to:

1. **Critical missing features**: TLS decryption (stub), breakpoints, rewrite rules, compare tool, replay
2. **Untested mobile debugging**: CA infrastructure exists but no mobile workflow
3. **Missing fuzzer**: Nice-to-have but valuable for security testing

**Recommendation**: Prioritize fixing TLS decryption immediately (SEC-01, SEC-02). Breakpoints, rewrite rules, replay, compare tool, and fuzzer already exist at library level — prioritize wiring them into the CLI/UI pipeline rather than re-implementing. Mobile debugging documentation should accompany CA fixes. Total estimated effort: **3-5 weeks** (reduced from 6-10 weeks; library implementations already exist). Breakpoints, rewrite rules, diff, replay, and fuzzer need CLI pipeline integration only.

Once these gaps are closed, FlowLens will not only match HTTP Debugger Pro's core features but exceed it with:
- Native Rust performance
- MCP server for AI agent integration
- Multi-frontend support (TUI, GUI, Tauri)
- Modern build system and cross-platform support

---

*Last updated: 2026/01/20*  
*Last verified against source: 2026/11/01*  
*Next review: After CLI pipeline integration complete*