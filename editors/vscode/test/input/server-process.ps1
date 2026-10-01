param(
    [ValidateSet('list', 'pause', 'stop')][string]$Operation = 'list',
    [string]$ServerPath,
    [int]$TargetProcess = 0,
    [string]$Created = ''
)
$ErrorActionPreference = 'Stop'
$taskBinary = [IO.Path]::GetFullPath($ServerPath)
$taskRecords = @(Get-CimInstance Win32_Process | ForEach-Object {
    [pscustomobject]@{ pid = [int]$_.ProcessId; ppid = [int]$_.ParentProcessId; executable = $_.ExecutablePath;
        created = if ($_.CreationDate) { $_.CreationDate.ToUniversalTime().ToString('o') } else { $null } }
})
if ($Operation -eq 'list') {
    ConvertTo-Json -InputObject $taskRecords -Compress
    exit
}
$taskRecord = @($taskRecords | Where-Object { $_.pid -eq $TargetProcess })
if ($taskRecord.Count -ne 1 -or $taskRecord[0].created -ne $Created -or
    -not [string]::Equals($taskRecord[0].executable, $taskBinary, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'the captured test server identity changed; refuse process mutation'
}
if ($Operation -eq 'stop') {
    Stop-Process -Id $TargetProcess -ErrorAction Stop
    '{"stopped":true}'
    exit
}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VelaTestServerThreads {
    [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr OpenThread(uint access, bool inherit, uint thread);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern uint SuspendThread(IntPtr thread);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$taskHandles = [Collections.Generic.List[IntPtr]]::new()
$taskSuspended = [Collections.Generic.List[IntPtr]]::new()
$taskSucceeded = $false
try {
    $taskThreads = @((Get-Process -Id $TargetProcess -ErrorAction Stop).Threads)
    if ($taskThreads.Count -eq 0) { throw 'test server has no threads' }
    foreach ($taskThread in $taskThreads) {
        $taskHandle = [VelaTestServerThreads]::OpenThread(2, $false, $taskThread.Id)
        if ($taskHandle -eq [IntPtr]::Zero) { throw 'cannot open a test-owned thread' }
        $taskHandles.Add($taskHandle)
        $taskPrevious = [VelaTestServerThreads]::SuspendThread($taskHandle)
        if ($taskPrevious -eq [uint32]::MaxValue) { throw 'cannot suspend a test-owned thread' }
        $taskSuspended.Add($taskHandle)
        if ($taskPrevious -ne 0) { throw 'test-owned thread was already suspended' }
    }
    $taskSucceeded = $true
    @{ suspended = $true; threads = $taskSuspended.Count } | ConvertTo-Json -Compress
} finally {
    # Roll back this helper's increments if a partial suspension failed. On
    # success the external driver kills the captured child and reaps the UI.
    if (-not $taskSucceeded) { foreach ($taskHandle in $taskSuspended) { [void][VelaTestServerThreads]::ResumeThread($taskHandle) } }
    foreach ($taskHandle in $taskHandles) { [void][VelaTestServerThreads]::CloseHandle($taskHandle) }
}
