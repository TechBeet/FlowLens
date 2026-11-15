<#
FlowLens 提交前检查脚本
按顺序执行：
  1. cargo fmt --all --check
  2. cargo clippy --workspace --all-targets -- -D warnings
  3. npm run build        (在 ui/ 下)
  4. npx vitest run       (在 ui/ 下)
任一步失败即以非零退出码终止。

用法：
  powershell -ExecutionPolicy Bypass -File scripts/pre-commit.ps1
#>

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

function Invoke-Step {
    param(
        [string]$Name,
        [string]$Command,
        [string]$WorkingDirectory = $repoRoot
    )
    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    Write-Host "    (in $WorkingDirectory) $Command"
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        Push-Location $WorkingDirectory
        & cmd /c "$Command 2>&1"
        $code = $LASTEXITCODE
        Pop-Location
    }
    catch {
        Pop-Location
        throw
    }
    finally {
        $ErrorActionPreference = $previous
    }
    if ($code -ne 0) {
        Write-Host "FAILED: $Name (exit code $code)" -ForegroundColor Red
        Pop-Location
        exit $code
    }
    Write-Host "OK: $Name" -ForegroundColor Green
}

# 1) Rust 格式检查
Invoke-Step -Name "cargo fmt --all --check" -Command "cargo fmt --all --check"

# 2) Rust clippy（警告即错误）
Invoke-Step -Name "cargo clippy --workspace --all-targets -- -D warnings" `
    -Command "cargo clippy --workspace --all-targets -- -D warnings"

# 3) 前端生产构建
Invoke-Step -Name "npm run build" -Command "npm run build" -WorkingDirectory (Join-Path $repoRoot "ui")

# 4) 前端单元测试
Invoke-Step -Name "npx vitest run" -Command "npx vitest run" -WorkingDirectory (Join-Path $repoRoot "ui")

Write-Host ""
Write-Host "All pre-commit checks passed." -ForegroundColor Green
Pop-Location
exit 0