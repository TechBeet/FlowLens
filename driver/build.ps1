# FlowLens Driver Build Script
# Requires WDK 10.0.22621+ and Visual Studio 2022

param(
    [string]$Configuration = "Release",
    [string]$Platform = "x64",
    [string]$WdkRoot = "C:\Program Files (x86)\Windows Kits\10"
)

$ErrorActionPreference = "Stop"

$wdkBin = Join-Path $WdkRoot "bin\10.0.22621.0\$Platform"
if (-not (Test-Path $wdkBin)) {
    # Try to find any WDK version
    $wdkBin = Get-ChildItem -Path (Join-Path $WdkRoot "bin") -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
    $wdkBin = Join-Path $wdkBin $Platform
}

if (-not (Test-Path $wdkBin)) {
    Write-Error "WDK not found at $WdkRoot. Install Windows Driver Kit."
    exit 1
}

Write-Host "Using WDK: $wdkBin" -ForegroundColor Cyan

$srcDir = Join-Path $PSScriptRoot "src"
$outDir = Join-Path $PSScriptRoot "build\$Platform\$Configuration"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$compiler = Join-Path $wdkBin "cl.exe"
if (-not (Test-Path $compiler)) {
    # Fallback to MSVC
    $vsPath = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC"
    if (Test-Path $vsPath) {
        $msvcVer = Get-ChildItem $vsPath | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name
        $compiler = Join-Path $vsPath "$msvcVer\bin\Hostx64\$Platform\cl.exe"
    }
}

if (-not (Test-Path $compiler)) {
    Write-Error "Compiler not found. Install Visual Studio 2022 with C++ workload."
    exit 1
}

$includePaths = @(
    (Join-Path $WdkRoot "Include\10.0.22621.0\km"),
    (Join-Path $WdkRoot "Include\10.0.22621.0\shared"),
    (Join-Path $WdkRoot "Include\10.0.22621.0\um"),
    $srcDir
) -join ";"

$libPaths = @(
    (Join-Path $WdkRoot "Lib\10.0.22621.0\km\$Platform"),
    (Join-Path $WdkRoot "Lib\10.0.22621.0\um\$Platform")
) -join ";"

$defines = "/D_WIN64 /D_AMD64_ /DNTDDI_VERSION=0x0A00000B /D_WIN32_WINNT=0x0A00"

$compileArgs = @(
    "/nologo", "/W4", "/WX-", "/O2", "/Oi", "/GL", "/Zi",
    "/D", "DBG=0",
    $defines,
    "/I", ($includePaths -replace ";", " /I "),
    "/c", (Join-Path $srcDir "callout.c"),
    "/Fo" + (Join-Path $outDir "callout.obj")
)

Write-Host "Compiling callout.c..." -ForegroundColor Yellow
& $compiler @compileArgs
if ($LASTEXITCODE -ne 0) { Write-Error "Compilation failed"; exit 1 }

Write-Host "Linking flowlens.sys..." -ForegroundColor Yellow
$linker = Join-Path $wdkBin "link.exe"
if (-not (Test-Path $linker)) {
    $linker = "link.exe"
}
$linkArgs = @(
    "/nologo", "/DRIVER", "/ENTRY:DriverEntry",
    "/SUBSYSTEM:NATIVE", "/NODEFAULTLIB",
    "/LIBPATH:$libPaths",
    "ntoskrnl.lib", "hal.lib", "wdmsec.lib", "ntstrsafe.lib",
    (Join-Path $outDir "callout.obj"),
    "/OUT:" + (Join-Path $outDir "flowlens.sys")
)
& $linker @linkArgs
if ($LASTEXITCODE -ne 0) { Write-Error "Linking failed"; exit 1 }

# Copy INF
Copy-Item (Join-Path $PSScriptRoot "flowlens.inf") $outDir -Force

Write-Host "Build complete: $outDir\flowlens.sys" -ForegroundColor Green
Write-Host "To install: run setup.ps1 -InstallDriver" -ForegroundColor Yellow