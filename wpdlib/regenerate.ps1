# Regenerate Npcap import libraries (wpcap.lib, Packet.lib)
#
# The Rust `pcap` crate requires MSVC import libraries. They are normally
# shipped with the Npcap SDK, but when only the runtime is installed we can
# generate them from the installed DLLs using MSVC's dumpbin.exe + lib.exe.
#
# Run this from the repository root as an Administrator-capable developer
# machine that has Visual Studio tooling installed.

$ErrorActionPreference = 'Stop'

# --- Locate MSVC tools ---------------------------------------------------
$vsBase = 'C:\Program Files\Microsoft Visual Studio'
$msvcRoot = Get-ChildItem $vsBase -Directory -Recurse -Depth 3 -Filter 'MSVC' -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $msvcRoot) { throw "MSVC tools not found under $vsBase" }

$toolDir = Join-Path $msvcRoot.FullName 'bin\HostX64\x64'
$dumpbin = Join-Path $toolDir 'dumpbin.exe'
$libTool = Join-Path $toolDir 'lib.exe'
$outDir = Split-Path -Parent $MyInvocation.MyCommand.Path

$libs = @(
    @{ Dll = "$env:SystemRoot\System32\wpcap.dll"; Name = 'wpcap' },
    @{ Dll = "$env:SystemRoot\System32\Packet.dll"; Name = 'Packet' }
)

foreach ($lib in $libs) {
    $def  = Join-Path $outDir "$($lib.Name).def"
    $out  = Join-Path $outDir "$($lib.Name).lib"

    Write-Host "Exporting symbols from $($lib.Dll) ..."
    $exports = & $dumpbin /exports $lib.Dll 2>$null

    @("LIBRARY $($lib.Name).DLL", 'EXPORTS') | Set-Content -Path $def
    foreach ($line in $exports) {
        $parts = $line.Trim() -split '\s+'
        if ($parts.Count -ge 3 -and $parts[0] -match '^\d+$') {
            $name = $parts[-1]
            if ($name -notmatch '^\d+$' -and $name -notin @('base', 'functions', 'names')) {
                "    $name" | Add-Content -Path $def
            }
        }
    }

    Write-Host "Building $($lib.Name).lib ..."
    & $libTool /DEF:$def /OUT:$out /MACHINE:X64 2>&1 | Out-Host
    if (-not (Test-Path $out)) { throw "Failed to create $out" }
    Write-Host "  -> $out"
}

Write-Host 'Done.'
