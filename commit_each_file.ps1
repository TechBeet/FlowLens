@powershell -NoProfile -ExecutionPolicy Bypass -Command "& {
Set-Location 'd:\github\FlowLens'
$changes = git status --porcelain
$committed = 0
foreach ($line in $changes) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $status = $line.Substring(0,2)
    $path = $line.Substring(3).Trim()
    if ([string]::IsNullOrWhiteSpace($path)) { continue }
    
    # Skip if path does not exist and not deleted
    $exists = Test-Path -LiteralPath $path -PathType Leaf
    $isDeleted = $status.Contains('D')
    
    if ($isDeleted -or $exists) {
        try {
            git add -- $path | Out-Null
            if ($isDeleted) {
                $msg = "delete: $path"
            } elseif ($status.Trim() -eq '??') {
                $msg = "add: $path"
            } else {
                $msg = "update: $path"
            }
            git commit -m $msg 2>&1 | Out-Null
            Write-Host "[OK] $msg"
            $committed++
        } catch {
            Write-Host "[SKIP] $path : $($_.Exception.Message)"
        }
    }
}
Write-Host "Total commits created: $committed"
git push origin main
}"
