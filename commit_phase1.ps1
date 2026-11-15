# Commit each changed file individually (one commit per file)
Set-Location "D:\github\FlowLens"

$files = @(
  "crates/flowlens-attribution/src/lib.rs",
  "crates/flowlens-core/src/intercept.rs",
  "crates/flowlens-replay/src/client.rs",
  "crates/flowlens-storage/src/lru_cache.rs",
  "crates/flowlens-storage/src/recovery.rs",
  "crates/flowlens-tls/src/lib.rs",
  "ui/src-tauri/Cargo.toml",
  "ui/src-tauri/src/commands.rs",
  "ui/src-tauri/src/lib.rs",
  "ui/src/components/FilterBar.svelte",
  "ui/src/components/SettingsView.svelte",
  "ui/src/components/StatsPanel.svelte",
  "ui/src/components/Toolbar.svelte",
  "ui/src/lib/api.ts",
  ".github/workflows/phase1-ci.yml",
  "Cargo.lock",
  "Cargo.toml",
  "crates/flowlens-capture/Cargo.lock",
  "crates/flowlens-replay/Cargo.lock",
  "ui/final_build.txt",
  "ui/final_tests.txt",
  "ui/src-tauri/clippy.txt",
  "ui/src-tauri/test.txt",
  "ui/src/lib/stores/eventStore.svelte.ts",
  "ui/src/lib/stores/eventStore.svelte.test.ts",
  "ui/vitest.config.mts"
)

# Deletions
$deletions = @(
  "ui/src/lib/stores/eventStore.ts",
  "ui/vitest.config.ts"
)

foreach ($f in $deletions) {
  if (Test-Path $f) {
    git add -- $f
  } else {
    git rm -- $f 2>$null
  }
  git commit -m "Remove $f"
  Write-Output "COMMITTED (removed): $f"
}

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
