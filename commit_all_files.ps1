# Commit each changed file individually (one commit per file)
Set-Location "D:\github\FlowLens"

$files = @(
  "crates/flowlens-tls/src/lib.rs",
  "crates/flowlens-tls/src/mitm.rs",
  "crates/flowlens-tls/tests/mitm_integration.rs",
  "ui/package.json",
  "ui/src-tauri/Cargo.lock",
  "ui/src-tauri/Cargo.toml",
  "ui/src-tauri/src/commands.rs",
  "ui/src-tauri/src/lib.rs",
  "ui/src-tauri/src/state.rs",
  "ui/src-tauri/tauri.conf.json",
  "ui/src/App.svelte",
  "ui/src/components/DetailPanel.svelte",
  "ui/src/components/FilterBar.svelte",
  "ui/src/components/Header.svelte",
  "ui/src/components/SettingsView.svelte",
  "ui/src/components/Sidebar.svelte",
  "ui/src/components/Toolbar.svelte",
  "ui/src/components/TrafficTable.svelte",
  "ui/src/lib/api.ts",
  "ui/src/lib/stores/eventStore.ts",
  "ui/src/lib/types.ts",
  "SPEC.md",
  "UI_MATRIX.md",
  "crates/flowlens-tls/src/policy.rs",
  "ui/src-tauri/src/proxy.rs",
  "ui/src/components/ReplayHistory.svelte",
  "ui/src/components/SniDomainSelector.svelte",
  "ui/src/global.d.ts",
  "ui/vitest.config.ts"
)

foreach ($f in $files) {
  if (Test-Path $f) {
    git add -- $f
    $staged = git diff --cached --name-only
    if ($staged) {
      git commit -m "Update $f"
      Write-Output "COMMITTED: $f"
    } else {
      Write-Output "SKIP (no changes): $f"
    }
  } else {
    Write-Output "MISSING: $f"
  }
}
