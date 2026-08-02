# Phase 1 smoke test: start the app, drive it with synthetic hook events
# through the real shim, and screenshot the surface after each stage.
# This is the ancestor of the manual test script TODO.md asks for (induce
# every status colour across six sessions); today it induces a handful on
# a couple of tiles and proves the pipeline end to end.
#
# Assumes target\debug\deckhand.exe and deckhand-shim.exe are built.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$app = Join-Path $repo "target\debug\deckhand.exe"
$shim = Join-Path $repo "target\debug\deckhand-shim.exe"
$shots = Join-Path $repo "_scratch\smoke"

if (-not (Test-Path $app)) { throw "Not built: $app" }
New-Item -ItemType Directory -Force $shots | Out-Null

function Send-Hook([hashtable]$payload) {
    $json = $payload | ConvertTo-Json -Compress -Depth 6
    # Through the real shim, stdin to POST, exactly as Claude Code would.
    $json | & $shim
}

function Shot([string]$name, [System.Diagnostics.Process]$proc) {
    Add-Type -AssemblyName System.Drawing
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    public struct R { public int L, T, Rt, B; }
}
"@ -ErrorAction SilentlyContinue
    [Win]::SetProcessDPIAware() | Out-Null
    $proc.Refresh()
    $r = New-Object Win+R
    [Win]::GetWindowRect($proc.MainWindowHandle, [ref]$r) | Out-Null
    $w = $r.Rt - $r.L; $h = $r.B - $r.T
    if ($w -le 0) { return }
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $g.Dispose()
    $bmp.Save((Join-Path $shots "$name.png"))
    $bmp.Dispose()
}

$proc = Start-Process $app -WindowStyle Hidden -PassThru
foreach ($i in 1..40) {
    Start-Sleep -Milliseconds 250
    $proc.Refresh()
    if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { break }
}
Start-Sleep -Milliseconds 1500
Shot "0-start" $proc

# Tile 1: a session appears, starts a turn, runs a tool.
Send-Hook @{ hook_event_name = "SessionStart"; source = "startup"; session_id = "synth-1"; cwd = "C:\Users\owenp\dev\undertow"; permission_mode = "auto" }
Start-Sleep -Milliseconds 400
Shot "1-idle" $proc

Send-Hook @{ hook_event_name = "UserPromptSubmit"; session_id = "synth-1"; permission_mode = "auto" }
Send-Hook @{ hook_event_name = "PreToolUse"; session_id = "synth-1"; tool_name = "Bash"; tool_use_id = "t1" }
Start-Sleep -Milliseconds 400
Shot "2-thinking" $proc

# Tile 2: a session that asks a question (amber, kind question).
Send-Hook @{ hook_event_name = "SessionStart"; source = "startup"; session_id = "synth-2"; cwd = "C:\Users\owenp\dev\macrovox"; permission_mode = "manual" }
Send-Hook @{ hook_event_name = "UserPromptSubmit"; session_id = "synth-2" }
Send-Hook @{ hook_event_name = "PreToolUse"; session_id = "synth-2"; tool_name = "AskUserQuestion"; tool_use_id = "q1"; tool_input = @{ questions = @(@{ question = "Deploy now?"; options = @(@{ label = "Yes, deploy" }, @{ label = "Not yet" }) }) } }
Start-Sleep -Milliseconds 400

# Tile 1 finishes with a child still live: stays thinking, then greens.
Send-Hook @{ hook_event_name = "SubagentStart"; session_id = "synth-1" }
Send-Hook @{ hook_event_name = "PostToolUse"; session_id = "synth-1"; tool_name = "Bash"; tool_use_id = "t1" }
Send-Hook @{ hook_event_name = "Stop"; session_id = "synth-1" }
Start-Sleep -Milliseconds 400
Shot "3-amber-and-ledger" $proc

Send-Hook @{ hook_event_name = "SubagentStop"; session_id = "synth-1" }
# Tile 3: an error.
Send-Hook @{ hook_event_name = "SessionStart"; source = "startup"; session_id = "synth-3"; cwd = "C:\Users\owenp\dev\alpha-osk" }
Send-Hook @{ hook_event_name = "StopFailure"; session_id = "synth-3"; error = @{ type = "api_error" } }
Start-Sleep -Milliseconds 400
Shot "4-green-and-red" $proc

Stop-Process -Id $proc.Id -Force -Confirm:$false
"screenshots in $shots"
