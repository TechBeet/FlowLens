# Commit ALL remaining working-tree changes, one file per commit.
Set-Location "D:\github\FlowLens"

$files = @(
  ".gitignore",
  ".github/workflows/phase1-ci.yml",
  "scripts/check_docs_links.py",
  ".vscode/launch.json",
  ".vscode/tasks.json",
  "crates/flowlens-core/src/errors.rs",
  "crates/flowlens-core/src/event_bus.rs",
  "crates/flowlens-core/src/session.rs",
  "crates/flowlens-storage/src/lib.rs",
  "crates/flowlens-storage/src/lru_cache.rs",
  "crates/flowlens-tls/src/lib.rs",
  "crates/flowlens-tls/src/policy.rs",
  "crates/flowlens-ui/src/tui.rs",
  "ui/package.json",
  "ui/package-lock.json",
  "ui/playwright.config.ts",
  "ui/e2e/fixtures.ts",
  "ui/e2e/smoke.spec.ts",
  "ui/src-tauri/src/commands.rs",
  "ui/src-tauri/src/lib.rs",
  "ui/src-tauri/src/proxy.rs",
  "ui/src-tauri/src/state.rs",
  "ui/src/App.svelte",
  "ui/src/components/FilterBar.svelte",
  "ui/src/components/FilterBar.test.ts"
)

foreach ($f in $files) {
  if (-not (Test-Path $f)) { Write-Output "MISSING: $f"; continue }
  git add -- $f
  $tracked = git ls-files --error-unmatch $f 2>$null
  $msg = if ($LASTEXITCODE -eq 0) { "Fix $f" } else { "Add $f" }
  $res = git commit -m "$msg" 2>&1
  if ($res -match 'nothing to commit') { Write-Output "SKIP: $f" } else { Write-Output "COMMITTED: $f" }
}

