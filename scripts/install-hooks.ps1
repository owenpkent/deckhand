# Register (or remove) the Deckhand shim as a Claude Code hook in the
# user-level settings.json, so sessions in every repo report state to
# the daemon, not only this one. See TODO.md and
# docs/CLAUDE_CODE_ADAPTER.md#hook-installation.
#
# Usage:
#   powershell -NoProfile -File scripts\install-hooks.ps1
#   powershell -NoProfile -File scripts\install-hooks.ps1 -Uninstall
#   powershell -NoProfile -File scripts\install-hooks.ps1 -WhatIf
#
# The first write to an existing settings file saves a copy as
# "<SettingsPath>.bak" before touching it. Later runs leave that backup
# alone, so it keeps holding the pre-Deckhand original rather than being
# overwritten with the most recent prior state.

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
#
# ConvertTo-CanonicalShimPath is the one place a path becomes the identity
# used for existence checks and for matching an installed group: a full
# path with forward slashes, regardless of whether it came from the
# default or from an explicit -ShimPath spelled with backslashes. Without
# this, an explicit -ShimPath that names the same file the default would
# have found reads as a second, different shim.
function ConvertTo-CanonicalShimPath {
    param([string]$Path)
    return ([System.IO.Path]::GetFullPath($Path)) -replace '\\', '/'
}

if ([string]::IsNullOrEmpty($ShimPath)) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $rawShimPath = Join-Path $repoRoot "target\debug\deckhand-shim.exe"
    $ShimPath = ConvertTo-CanonicalShimPath $rawShimPath
} else {
    $ShimPath = ConvertTo-CanonicalShimPath $ShimPath
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

# Hook commands on Windows run through Git Bash when present, PowerShell
# only as a fallback (docs/CLAUDE_CODE_ADAPTER.md, "Windows notes"), so the
# stored command is rendered as one literal bash word: single-quoted, with
# embedded apostrophes escaped the POSIX way ('\'' closes the quote, adds
# an escaped quote, reopens it). Without this a shim path containing a
# space splits into multiple argv entries and the hook fails to launch.
function ConvertTo-ShellCommandToken {
    param([string]$Path)
    return "'" + $Path.Replace("'", "'\''") + "'"
}

# Inverse of ConvertTo-ShellCommandToken, tolerant of the raw, unquoted
# command this script wrote before quoting existed. Given either form,
# returns the bare path so callers can compare on the path itself rather
# than on quoting style.
function ConvertFrom-ShellCommandToken {
    param([string]$Command)
    if ($Command.Length -ge 2 -and $Command.StartsWith("'") -and $Command.EndsWith("'")) {
        return $Command.Substring(1, $Command.Length - 2).Replace("'\''", "'")
    }
    return $Command
}

function Test-ShimGroup {
    param($Group, [string]$Shim)
    if ($null -eq $Group) { return $false }
    $hooks = @($Group.hooks)
    if ($hooks.Count -ne 1) { return $false }
    $cmd = $hooks[0].command
    if ([string]::IsNullOrEmpty($cmd)) { return $false }
    # Recognises both the legacy raw path this script used to write and
    # today's single-quoted form, and normalizes slash direction and case
    # so a backslash -ShimPath matches the forward-slash form of the same
    # file instead of reading as a second, unrelated shim.
    $rawCmd = ConvertFrom-ShellCommandToken $cmd
    $normalizedCmd = ($rawCmd -replace '\\', '/').ToLowerInvariant()
    $normalizedShim = ($Shim -replace '\\', '/').ToLowerInvariant()
    return $normalizedCmd -eq $normalizedShim
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
$upgraded = 0
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

    $matchedGroup = $null
    foreach ($group in $groups) {
        if (Test-ShimGroup -Group $group -Shim $ShimPath) { $matchedGroup = $group; break }
    }
    $desiredCommand = ConvertTo-ShellCommandToken $ShimPath
    if ($null -ne $matchedGroup) {
        # Same group, same shim: upgrade a legacy raw command or a
        # leftover timeout in place rather than leaving it, and rather
        # than adding a second group for the same shim.
        $hookObj = @($matchedGroup.hooks)[0]
        $hasTimeout = Test-JsonProperty $hookObj "timeout"
        if ($hookObj.command -cne $desiredCommand -or $hasTimeout) {
            Set-JsonProperty -InputObject $hookObj -Name "command" -Value $desiredCommand
            Set-JsonProperty -InputObject $hookObj -Name "async" -Value $true
            if ($hasTimeout) { $hookObj.PSObject.Properties.Remove("timeout") }
            $upgraded++
        } else {
            $alreadyPresent++
        }
        continue
    }

    # Status hooks are async and never carry a timeout: the reference
    # block in docs/CLAUDE_CODE_ADAPTER.md#hook-installation sets none on
    # any of them, and rule 7 there is explicit that SessionEnd hooks in
    # particular must not get one (they share a shared ~1.5s exit budget,
    # and a per-hook timeout only raises that ceiling). This script never
    # installs the gating PreToolUse entry, which is the only entry
    # allowed a timeout, so no event handled here should carry one.
    $hookEntry = [PSCustomObject][ordered]@{
        type    = "command"
        command = $desiredCommand
        async   = $true
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

$summary = "Added: $added, upgraded: $upgraded, already present: $alreadyPresent, removed: $removed"

if ($added -eq 0 -and $upgraded -eq 0 -and $removed -eq 0) {
    Write-Host $summary
} else {
    $actionLabel = if ($Uninstall) { "Remove Deckhand shim hooks" } else { "Register Deckhand shim hooks" }
    if ($PSCmdlet.ShouldProcess($SettingsPath, $actionLabel)) {
        $json = $root | ConvertTo-Json -Depth 32

        # Back up only the pre-Deckhand original: if a backup already
        # exists, a later run must not overwrite it with a state that is
        # itself already post-Deckhand.
        if ((Test-Path -LiteralPath $SettingsPath) -and -not (Test-Path -LiteralPath "$SettingsPath.bak")) {
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
