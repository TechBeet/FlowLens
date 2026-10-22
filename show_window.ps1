Add-Type -MemberDefinition '[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd); [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);' -Name Win32 -Namespace User32

$procs = Get-Process flowlens -ErrorAction SilentlyContinue
if (-not $procs) {
    Write-Host "没有正在运行的 flowlens 进程"
    exit 1
}

foreach ($p in $procs) {
    $h = $p.MainWindowHandle
    Write-Host ("PID={0} Handle={1} Visible={2} Title='{3}'" -f $p.Id, $h, [User32.Win32]::IsWindowVisible($h), $p.MainWindowTitle)
    if ($h -ne 0) {
        [User32.Win32]::ShowWindow($h, 5) | Out-Null   # SW_SHOW
        Start-Sleep -Milliseconds 400
        [User32.Win32]::SetForegroundWindow($h) | Out-Null
        Write-Host "  -> 已显示并置顶"
    }
}
