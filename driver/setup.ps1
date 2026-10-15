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

# Install the driver
if ($InstallDriver) {
    Write-Host "Installing FlowLens WFP driver..." -ForegroundColor Yellow

    # Copy driver files
    Copy-Item "$(Split-Path $PSCommandPath)" -Destination $DriverPath -Recurse -Force

    # Register driver in the registry
    $DriverRegPath = "HKLM:\SYSTEM\CurrentControlSet\Services\flowlens"
    if (-Not (Test-Path $DriverRegPath)) {
        New-Item -Path $DriverRegPath -Force
    }

    New-ItemProperty -Path $DriverRegPath -Name "DisplayName" -Value "FlowLens WFP Callout Driver" -PropertyType String -Force
    New-ItemProperty -Path $DriverRegPath -Name "ServiceType" -Value 1 -PropertyType DWord -Force
    New-ItemProperty -Path $DriverRegPath -Name "Start" -Value 3 -PropertyType DWord -Force
    New-ItemProperty -Path $DriverRegPath -Name "ErrorControl" -Value 1 -PropertyType DWord -Force

    Write-Host "FlowLens driver installed successfully!" -ForegroundColor Green
}

# Uninstall the driver
if ($UninstallDriver) {
    Write-Host "Uninstalling FlowLens WFP driver..." -ForegroundColor Yellow

    $DriverPath = Join-Path $InstallPath "driver"
    if (Test-Path $DriverPath) {
        Remove-Item -Path $DriverPath -Recurse -Force
    }

    $DriverRegPath = "HKLM:\SYSTEM\CurrentControlSet\Services\flowlens"
    if (Test-Path $DriverRegPath) {
        Remove-Item -Path $DriverRegPath -Force
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