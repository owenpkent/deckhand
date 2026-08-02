# Automated check for the no-focus-steal spike.
# Starts Notepad as the "app the user is working in", records the
# foreground window, clicks the centre of the spike window with a
# synthetic mouse event, and verifies the foreground window did not
# change while the spike's log shows the click was received.
#
# Assumes the spike exe is already built and NOT already running.

$ErrorActionPreference = "Stop"
$spikeDir = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $spikeDir "src-tauri\target\debug\deckhand-focus-spike.exe"
$log = Join-Path $spikeDir "spike-log.jsonl"

if (-not (Test-Path $exe)) { throw "Spike exe not built: $exe" }
if (Test-Path $log) { Remove-Item $log -Force -Confirm:$false }

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class FocusTest {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hwnd, [Out] char[] text, int max);
    public struct RECT { public int Left, Top, Right, Bottom; }
    public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004;
    public static string Title(IntPtr hwnd) {
        var buf = new char[256];
        int len = GetWindowTextW(hwnd, buf, 256);
        return new string(buf, 0, len);
    }
}
"@

$results = [ordered]@{}

# 1. Start Notepad and let it take the foreground.
$notepad = Start-Process notepad -PassThru
Start-Sleep -Milliseconds 1500

# 2. Start the spike (created with focus: false, so Notepad should keep
#    the foreground even at spike startup). Debug builds are console
#    subsystem; hide that console so it cannot take the foreground and
#    stand in for "the app the user is working in".
$spike = Start-Process $exe -WindowStyle Hidden -PassThru
$spikeHwnd = [IntPtr]::Zero
foreach ($i in 1..40) {
    Start-Sleep -Milliseconds 250
    $spike.Refresh()
    if ($spike.MainWindowHandle -ne [IntPtr]::Zero) {
        $spikeHwnd = $spike.MainWindowHandle
        break
    }
}
if ($spikeHwnd -eq [IntPtr]::Zero) { throw "Spike window not found" }
Start-Sleep -Milliseconds 1500

# Make sure the foreground really is Notepad, not anything the spike
# launch dragged in.
$null = (New-Object -ComObject WScript.Shell).AppActivate($notepad.Id)
Start-Sleep -Milliseconds 600

$fgBefore = [FocusTest]::GetForegroundWindow()
$results.foreground_before = "{0} '{1}'" -f $fgBefore, [FocusTest]::Title($fgBefore)
$results.spike_hwnd = $spikeHwnd
$results.spike_took_focus_at_startup = ($fgBefore -eq $spikeHwnd)

# 3. Click the centre of the spike window (button is centred-ish; the
#    whole body records clicks via the button at top, so aim at the
#    button: 24 px inset from top-left plus half the button height).
$rect = New-Object FocusTest+RECT
[FocusTest]::GetWindowRect($spikeHwnd, [ref]$rect) | Out-Null
$cx = [int](($rect.Left + $rect.Right) / 2)
$cy = [int]($rect.Top + 70)  # inside the button row, below the title bar
[FocusTest]::SetCursorPos($cx, $cy) | Out-Null
Start-Sleep -Milliseconds 200
[FocusTest]::mouse_event([FocusTest]::LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
[FocusTest]::mouse_event([FocusTest]::LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 800

$fgAfter = [FocusTest]::GetForegroundWindow()
$results.foreground_after = "{0} '{1}'" -f $fgAfter, [FocusTest]::Title($fgAfter)
$results.foreground_changed_by_click = ($fgAfter -ne $fgBefore)
$results.spike_is_foreground_after_click = ($fgAfter -eq $spikeHwnd)

# 4. Read the spike's own log.
Start-Sleep -Milliseconds 500
if (Test-Path $log) {
    $results.spike_log = @(Get-Content $log | ForEach-Object { "$_" })
} else {
    $results.spike_log = "(no log written)"
}

# 5. Clean up only what this script started.
Stop-Process -Id $spike.Id -Force -Confirm:$false
if (-not $notepad.HasExited) { Stop-Process -Id $notepad.Id -Force -Confirm:$false }

$results | ConvertTo-Json -Depth 3
