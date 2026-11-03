# FlowLens v2 Roadmap

## Status: Documentation Corrections Complete

### What was planned vs. Actual Status

| Feature | Planned Status | Actual Status | Notes |
|---------|---------------|---------------|-------|
| TLS MITM Decryption | Planned in Phase A | ✅ Completed | Wired TLS MITM decryption; no longer a passthrough stub. All existing test suites pass. |
| Breakpoints/MCP Tools | Planned in Phase A/B | ✅ Completed | Breakpoints, rewrite, fuzzer, replay tools available at library level. |
| Capture Pipeline Integration | Planned in Phase A/B | ✅ Partial | TLS decrypted and fed into capture loop. Full integration pending. |
| GUI (Tauri/egui) | Phase C Target | 🔄 In Progress | Tauri shell wiring ongoing. Search/filter/stats not complete. SAZ export stubbed. |
| Mobile CA Trust | Phase C Target | ⏳ Pending | Documentation draft needed. iOS/Android trust steps and QR generation outline required. |
| Roadmap/v2 Rewrite | Phase C Target | ⏳ Pending | This file requires a full rewrite with true status markings. |
| README Badges | Phase C Target | ⏳ Pending | Status badges and TLS clarification need updating. |
| Cross-references | Phase C Target | ⏳ Pending | All doc cross-references need verification and updating. |

---

### Key Corrections

- **TLS Decryption**: Confirmed wired; stub removed from pipeline.
- **Library Tools**: All MCP tools (breakpoints, rewrite, fuzzer, replay) implemented at library level.
- **Capture Pipeline**: TLS decryption integrated; partial pipeline completion.
- **GUI**: egui integration removed from scope; Tauri wiring partial.
- **Mobile CA**: New workflow documentation required.