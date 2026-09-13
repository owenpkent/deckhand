# Register (or remove) the Deckhand shim as a Claude Code hook in the
# user-level settings.json, so sessions in every repo report state to
# the daemon, not only this one. See TODO.md and
# docs/CLAUDE_CODE_ADAPTER.md#hook-installation.
#
# Usage:
#   powershell -NoProfile -File scripts\install-hooks.ps1
#   powershell -NoProfile -File scripts\install-hooks.ps1 -Uninstall
#   powershell -NoProfile -File scripts\install-hooks.ps1 -WhatIf

[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$SettingsPath = (Join-Path $env:USERPROFILE ".claude\settings.json"),

    [string]$ShimPath,

    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

# $PSScriptRoot is not reliably populated while a [CmdletBinding()] script's
# param() defaults are evaluated, so the ShimPath default is computed here
# in the body instead of inline in the param block.
if ([string]::IsNullOrEmpty($ShimPath)) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $rawShimPath = Join-Path $repoRoot "target\debug\deckhand-shim.exe"
    $ShimPath = ([System.IO.Path]::GetFullPath($rawShimPath)) -replace '\\', '/'
}

# The twelve events Deckhand installs (docs/CLAUDE_CODE_ADAPTER.md). The
# three tool events carry a wildcard matcher; every other event fires
# unconditionally. This mirrors the dogfood wiring hand-written in this
# repo's gitignored .claude\settings.local.json.
$events = @(
    "SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse",
    "PostToolUseFailure", "SubagentStart", "SubagentStop", "Notification",
    "PermissionDenied", "Stop", "StopFailure", "SessionEnd"
)
$matcherEvents = @("PreToolUse", "PostToolUse", "PostToolUseFailure")

if (-not $Uninstall -and -not (Test-Path -LiteralPath $ShimPath)) {
    throw "Shim not found at '$ShimPath'. Build it first (scripts\build-app.ps1) or pass -ShimPath."
}

function Test-JsonProperty {
    param($InputObject, [string]$Name)
    if ($null -eq $InputObject) { return $false }
    return [bool]($InputObject.PSObject.Properties.Name -contains $Name)
}

# $obj.$dynamicName = $value only works when the property already exists;
# PowerShell will not auto-vivify a new NoteProperty through a variable
# name (only a literal $obj.Name = $value does that), so every dynamic
# set goes through Add-Member -Force, which handles both cases.
function Set-JsonProperty {
    param($InputObject, [string]$Name, $Value)
    Add-Member -InputObject $InputObject -NotePropertyName $Name -NotePropertyValue $Value -Force | Out-Null
}

function Test-ShimGroup {
    param($Group, [string]$Shim)
    if ($null -eq $Group) { return $false }
    $hooks = @($Group.hooks)
    if ($hooks.Count -ne 1) { return $false }
    $cmd = $hooks[0].command
    if ([string]::IsNullOrEmpty($cmd)) { return $false }
    return $cmd.ToLowerInvariant() -eq $Shim.ToLowerInvariant()
}

# Load the existing file, or start from an empty object. A missing file
# is a normal state, not an error.
if (Test-Path -LiteralPath $SettingsPath) {
    $raw = Get-Content -LiteralPath $SettingsPath -Raw -Encoding UTF8
    if ([string]::IsNullOrWhiteSpace($raw)) { $raw = "{}" }
} else {
    $raw = "{}"
}
$root = $raw | ConvertFrom-Json

if (-not (Test-JsonProperty $root "hooks") -or ($null -eq $root.hooks)) {
    Set-JsonProperty -InputObject $root -Name "hooks" -Value ([PSCustomObject]@{})
}
$hooksObj = $root.hooks

$added = 0
$alreadyPresent = 0
$removed = 0

foreach ($eventName in $events) {
    $groups = @()
    if ((Test-JsonProperty $hooksObj $eventName) -and ($null -ne $hooksObj.$eventName)) {
        $groups = @($hooksObj.$eventName)
    }

    if ($Uninstall) {
        $keep = @()
        $removedHere = 0
        foreach ($group in $groups) {
            if (Test-ShimGroup -Group $group -Shim $ShimPath) {
                $removedHere++
            } else {
                $keep += $group
            }
        }
        if ($removedHere -gt 0) {
            $removed += $removedHere
            if ($keep.Count -eq 0) {
                $hooksObj.PSObject.Properties.Remove($eventName)
            } else {
                Set-JsonProperty -InputObject $hooksObj -Name $eventName -Value $keep
            }
        }
        continue
    }

    $hasShim = $false
    foreach ($group in $groups) {
        if (Test-ShimGroup -Group $group -Shim $ShimPath) { $hasShim = $true; break }
    }
    if ($hasShim) {
        $alreadyPresent++
        continue
    }

    $hookEntry = [PSCustomObject][ordered]@{
        type    = "command"
        command = $ShimPath
        async   = $true
        timeout = 5
    }
    if ($matcherEvents -contains $eventName) {
        $groupEntry = [PSCustomObject][ordered]@{
            matcher = "*"
            hooks   = @($hookEntry)
        }
    } else {
        $groupEntry = [PSCustomObject][ordered]@{
            hooks = @($hookEntry)
        }
    }

    Set-JsonProperty -InputObject $hooksObj -Name $eventName -Value ($groups + $groupEntry)
    $added++
}

if ((Test-JsonProperty $root "hooks") -and ($root.hooks.PSObject.Properties.Count -eq 0)) {
    $root.PSObject.Properties.Remove("hooks")
}

$summary = "Added: $added, already present: $alreadyPresent, removed: $removed"

if ($added -eq 0 -and $removed -eq 0) {
    Write-Host $summary
} else {
    $actionLabel = if ($Uninstall) { "Remove Deckhand shim hooks" } else { "Register Deckhand shim hooks" }
    if ($PSCmdlet.ShouldProcess($SettingsPath, $actionLabel)) {
        $json = $root | ConvertTo-Json -Depth 32

        if (Test-Path -LiteralPath $SettingsPath) {
            Copy-Item -LiteralPath $SettingsPath -Destination "$SettingsPath.bak" -Force
        }
        $parentDir = Split-Path -Parent $SettingsPath
        if ($parentDir -and -not (Test-Path -LiteralPath $parentDir)) {
            New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
        }
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($SettingsPath, $json, $utf8NoBom)
    }
    Write-Host $summary
}

$repoRootForCheck = Split-Path -Parent $PSScriptRoot
$localSettingsPath = Join-Path $repoRootForCheck ".claude\settings.local.json"
if (Test-Path -LiteralPath $localSettingsPath) {
    Write-Warning "This repo's .claude\settings.local.json wires the same hook events; with both installed, every event from this repo fires twice. Remove that file once the user-level registration is in place."
}
