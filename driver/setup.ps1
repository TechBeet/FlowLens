# FlowLens Driver Setup Script

# This PowerShell script sets up the FlowLens WFP driver
# Requires administrative privileges

# Parameters
param(
    [string]$InstallPath = "C:\Program Files\FlowLens",
    [switch]$InstallDriver,
    [switch]$UninstallDriver,
    [switch]$InstallSampleConfig
)

# Check administrative privileges
if (-NOT ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires administrative privileges!"
    exit 1
}

# Create install directory
if (-Not (Test-Path $InstallPath)) {
    New-Item -Path $InstallPath -ItemType Directory -Force
}

# Create driver directory if it exists
$DriverPath = Join-Path $InstallPath "driver"
if (Test-Path $DriverPath) {
    Write-Host "FlowLens driver is already installed at $InstallPath" -ForegroundColor Green
    return
}

# Install the driver using pnputil (Windows 10+) or sc
if ($InstallDriver) {
    Write-Host "Installing FlowLens WFP driver..." -ForegroundColor Yellow

    $sysPath = Join-Path $DriverPath "flowlens.sys"
    $infPath = Join-Path $DriverPath "flowlens.inf"

    if (Test-Path $infPath) {
        # Preferred: pnputil
        $pnputil = Join-Path $env:SystemRoot "System32\pnputil.exe"
        if (Test-Path $pnputil) {
            & $pnputil /add-driver $infPath /install
            if ($LASTEXITCODE -eq 0) {
                Write-Host "Driver installed via pnputil." -ForegroundColor Green
            } else {
                Write-Warning "pnputil failed; falling back to sc create."
            }
        }
    }

    # Fallback: sc create
    $existing = sc.exe query flowlens 2>$null
    if ($LASTEXITCODE -ne 0) {
        sc.exe create flowlens type= kernel binPath= "$sysPath" start= demand
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Service created." -ForegroundColor Green
        }
    }
    sc.exe start flowlens
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Driver started." -ForegroundColor Green
    } else {
        Write-Warning "Driver start failed (may require test signing: bcdedit /set testsigning on)."
    }
}

# Uninstall the driver
if ($UninstallDriver) {
    Write-Host "Uninstalling FlowLens WFP driver..." -ForegroundColor Yellow

    # Stop and delete service
    sc.exe stop flowlens 2>$null
    sc.exe delete flowlens 2>$null
    $pnputil = Join-Path $env:SystemRoot "System32\pnputil.exe"
    if (Test-Path $pnputil) {
        & $pnputil /delete-driver flowlens.inf /uninstall 2>$null
    }

    Write-Host "FlowLens driver uninstalled successfully!" -ForegroundColor Green
}

# Install sample configuration
if ($InstallSampleConfig) {
    Write-Host "Installing sample configuration..." -ForegroundColor Yellow

    $ConfigPath = Join-Path $InstallPath "config\sample.json"
    $ConfigDir = Split-Path $ConfigPath -Parent
    if (-Not (Test-Path $ConfigDir)) {
        New-Item -Path $ConfigDir -ItemType Directory -Force
    }

    $Config = @{
        driver = @{
            enabled = $true
            log_level = "INFO"
            ring_buffer_size = 16777216
        }
        capture = @{
            default_protocol = "http"
            max_packet_size = 65535
            filters = @()
            advanced_filters = @()
        }
        tls = @{
            enabled = $true
            ca_path = ""
            cache_size = 1024
            decrypt_timeout = 30000
        }
        attribution = @{
            track_processes = $true
            resolution_timeout = 30000
            map_unknown_to = "unknown"
        }
        mcp = @{
            enabled = $false
            host = "localhost"
            port = 9229
        }
    }

    $Config | ConvertTo-Json -Depth 10 | Set-Content -Path $ConfigPath -Encoding UTF8

    Write-Host "Sample configuration installed at $ConfigPath" -ForegroundColor Green
}

Write-Host "FlowLens driver setup completed successfully!" -ForegroundColor Green
Write-Host "Use --install-driver to install the WFP driver" -ForegroundColor Yellow
Write-Host "Use --uninstall-driver to uninstall the WFP driver" -ForegroundColor Yellow
Write-Host "Use --install-sample-config to install configuration" -ForegroundColor Yellow