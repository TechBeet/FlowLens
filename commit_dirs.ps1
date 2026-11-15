# Commit directory-contained files individually
Set-Location "D:\github\FlowLens"

$files = @(
  "ui/src/lib/__fixtures__/events.ts",
  "ui/src/lib/composables/eventSubscriber.ts",
  "ui/src/lib/composables/index.ts",
  "ui/src/lib/composables/useFilter.test.ts",
  "ui/src/lib/composables/useFilter.ts",
  "ui/src/lib/composables/useStats.test.ts",
  "ui/src/lib/composables/useStats.ts",
  "ui/src/lib/generated/index.ts",
  "ui/src/lib/generated/MatchQuality.ts",
  "ui/src/lib/generated/ProcessArch.ts",
  "ui/src/lib/generated/ProcessInfo.ts",
  "ui/src/lib/generated/ProtocolKind.ts",
  "ui/src/lib/generated/SessionId.ts",
  "ui/src/lib/generated/TrafficFilter.ts",
  "ui/src/lib/generated/TrafficRow.ts",
  "ui/src/lib/generated/TrafficStats.ts",
  "ui/src/lib/generated/TransportProtocol.ts",
  "crates/flowlens-attribution/Cargo.lock",
  "crates/flowlens-attribution/clippy_err.txt",
  "crates/flowlens-attribution/clippy_out.txt",
  "crates/flowlens-core/Cargo.lock",
  "crates/flowlens-core/build_core.log",
  "crates/flowlens-core/tsrs_gen.log",
  "crates/flowlens-tls/Cargo.lock",
  "crates/flowlens-tls/tls_test.txt",
  "crates/flowlens-ui/Cargo.lock",
  "package-lock.json",
  "clippy_tauri.log",
  "clippy_tls.log",
  "commit_all_files.ps1",
  "core_clippy.txt",
  "tls_test.txt",
  "ui/build_err.txt",
  "ui/build_out.txt",
  "ui/src-tauri/build.log",
  "ui/src-tauri/build_err.txt",
  "ui/src-tauri/build_full.txt",
  "ui/src-tauri/build_out.txt",
  "ui/src-tauri/buildlog.txt",
  "ui/src-tauri/cargo_clippy_out.txt",
  "ui/src-tauri/run_cargo_clippy.bat",
  "ui/src-tauri/tsrs_gen.log",
  "ui/src-tauri/tsrs_gen_err.log",
  "ui_build.log"
)

foreach ($f in $files) {
  if (Test-Path $f) {
    git add -- $f
    $staged = git diff --cached --name-only
    if ($staged) {
      git commit -m "Add $f"
      Write-Output "COMMITTED: $f"
    } else {
      Write-Output "SKIP (no changes): $f"
    }
  } else {
    Write-Output "MISSING: $f"
  }
}
