param(
    [ValidateSet('preflight', 'select', 'restore')][string]$Operation = 'preflight',
    [int]$TargetProcess = 0,
    [string]$Layout = '00000409',
    [string]$Window = '0'
)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VelaTestDesktop {
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll")] public static extern bool SwitchDesktop(IntPtr desktop);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string name, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr window);
}
'@
if ($Operation -eq 'preflight') {
    $desktop = [VelaTestDesktop]::OpenInputDesktop(0, $false, 0x0100)
    if ($desktop -eq [IntPtr]::Zero) { throw 'Windows desktop is locked or unavailable; unlock the interactive session' }
    try {
        if (-not [VelaTestDesktop]::SwitchDesktop($desktop)) { throw 'Windows desktop is locked or unavailable' }
    } finally { [void][VelaTestDesktop]::CloseDesktop($desktop) }
    '{"interactive":true}'
    exit
}
$windowHandle = if ($Operation -eq 'select') { (Get-Process -Id $TargetProcess).MainWindowHandle } else { [IntPtr][long]$Window }
if ($Operation -eq 'select') {
    if ($windowHandle -eq [IntPtr]::Zero) { throw "test process $TargetProcess has no visible window" }
}
if ($Operation -eq 'restore' -and -not [VelaTestDesktop]::IsWindow($windowHandle)) { '{"closed":true}'; exit }
[uint32]$owner = 0
$thread = [VelaTestDesktop]::GetWindowThreadProcessId($windowHandle, [ref]$owner)
if ($owner -ne $TargetProcess -or $thread -eq 0) { throw 'keyboard layout operation requires the test-owned VS Code window' }
$previous = [VelaTestDesktop]::GetKeyboardLayout($thread)
$requested = if ($Operation -eq 'select') { [VelaTestDesktop]::LoadKeyboardLayout($Layout, 0) } else { [IntPtr][long]$Layout }
if ($requested -eq [IntPtr]::Zero) { throw 'requested keyboard layout is unavailable' }
[IntPtr]$result = [IntPtr]::Zero
if ([VelaTestDesktop]::SendMessageTimeout($windowHandle, 0x0050, [IntPtr]::Zero, $requested, 2, 2000, [ref]$result) -eq [IntPtr]::Zero) {
    throw 'test window did not acknowledge keyboard layout selection'
}
$observed = [VelaTestDesktop]::GetKeyboardLayout($thread)
if ($observed -ne $requested) { throw 'test window keyboard layout differs from requested layout' }
@{ window=$windowHandle.ToInt64().ToString(); previous=$previous.ToInt64().ToString(); observed=$observed.ToInt64().ToString() } | ConvertTo-Json -Compress
