<#
.SYNOPSIS
    The one place Deckhand's house style rules are enforced.

.DESCRIPTION
    Deckhand is specification-only, so CI guards the specification. This
    script is what CI runs and what the local docs-gate command runs, so a
    rule lives here once instead of in a workflow, a CLAUDE.md snippet, and
    a CONTRIBUTING.md paragraph.

    Gates, in order:
      1. No em (U+2014) or en (U+2013) dashes in authored markdown.
      2. Every docs/*.md carries a status line in its first ten lines.
      3. docs/DECISIONS.md ADR numbering is contiguous, ascending, anchored.
      4. The Constellation contract: README.md has a Status heading and
         TODO.md list items are checkboxes.
      5. Wrap width. Warning only, never a failure.

    PowerShell because pwsh is preinstalled on ubuntu-latest and the
    owner's shell examples are PowerShell. Written to run on both Windows
    PowerShell 5.1 and PowerShell 7.

.PARAMETER Staged
    Scope the dash and wrap scans to files staged for commit. The repo-wide
    gates (status lines, ADRs, Constellation) always run: they are cheap
    and they are invariants, not per-file rules.

.PARAMETER All
    Scope the dash and wrap scans to every tracked markdown file. This is
    the default when neither switch is given, and what CI uses.

.EXAMPLE
    pwsh -NoProfile -File scripts/check-docs.ps1 -All

.EXAMPLE
    pwsh -NoProfile -File scripts/check-docs.ps1 -Staged
#>
[CmdletBinding()]
param(
    [switch]$Staged,
    [switch]$All
)

$ErrorActionPreference = 'Stop'

# Built from code points on purpose: the literal characters are banned in
# this repo, including inside the check that bans them.
$emDash = [char]0x2014
$enDash = [char]0x2013
$dashPattern = '[' + $emDash + $enDash + ']'

# Inherited third-party texts. This list is the CI exemption list; if it
# changes here it changes everywhere, which is the point of the script.
$exempt = @('CODE_OF_CONDUCT.md', 'CONSTELLATION_INTEGRATION_GUIDE.md')

$failures = New-Object System.Collections.Generic.List[string]
$warnings = New-Object System.Collections.Generic.List[string]

function Add-Failure([string]$Message) {
    $failures.Add($Message) | Out-Null
    Write-Host "[FAIL] $Message"
}

function Add-Warning([string]$Message) {
    $warnings.Add($Message) | Out-Null
    Write-Host "[warn] $Message"
}

function Read-Lines([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    $text = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
    if ($null -eq $text) { return @() }
    return ($text -replace "`r`n", "`n") -split "`n"
}

# Locate the repository root so the script works from any directory.
$root = $null
try {
    $root = (& git rev-parse --show-toplevel 2>$null | Select-Object -First 1)
} catch {
    $root = $null
}
if ([string]::IsNullOrWhiteSpace($root)) {
    $root = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $root

# ---------------------------------------------------------------------
# File selection
# ---------------------------------------------------------------------

$scope = 'all'
if ($Staged -and -not $All) { $scope = 'staged' }

$markdown = @()
if ($scope -eq 'staged') {
    $markdown = @(& git diff --cached --name-only --diff-filter=ACM |
        Where-Object { $_ -like '*.md' })
} else {
    $markdown = @(& git ls-files '*.md')
}
$markdown = @($markdown |
    Where-Object { $_ -and (Test-Path -LiteralPath $_) } |
    Where-Object { $exempt -notcontains (Split-Path -Leaf $_) })

Write-Host "check-docs: scope=$scope, $($markdown.Count) markdown file(s)"

# ---------------------------------------------------------------------
# Gate 1: no em or en dashes in authored markdown
# ---------------------------------------------------------------------

$dashHits = 0
foreach ($file in $markdown) {
    $lines = Read-Lines $file
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $dashPattern) {
            $dashHits++
            $shown = $lines[$i].Trim()
            if ($shown.Length -gt 100) { $shown = $shown.Substring(0, 100) }
            Add-Failure "$file`:$($i + 1): em or en dash. Use a comma, colon, parentheses, or a period. | $shown"
        }
    }
}
if ($dashHits -eq 0) { Write-Host "[ok]   no em or en dashes" }

# ---------------------------------------------------------------------
# Gate 2: every docs/*.md declares a status in its first ten lines
# ---------------------------------------------------------------------

$statusPattern = '^Status: \*\*(proposed|accepted|verified against .+)\*\*'
$statusMissing = 0
foreach ($file in @(& git ls-files 'docs/*.md')) {
    if (-not (Test-Path -LiteralPath $file)) { continue }
    $head = @(Read-Lines $file | Select-Object -First 10)
    $found = $false
    foreach ($line in $head) {
        if ($line -match $statusPattern) { $found = $true; break }
    }
    if (-not $found) {
        $statusMissing++
        Add-Failure "$file`: no status line in the first ten lines. Expected Status: **proposed**, **accepted**, or **verified against ...**"
    }
}
if ($statusMissing -eq 0) { Write-Host "[ok]   every docs/*.md declares a status" }

# ---------------------------------------------------------------------
# Gate 3: ADR numbering is contiguous, ascending, and anchored
# ---------------------------------------------------------------------

$decisions = 'docs/DECISIONS.md'
if (Test-Path -LiteralPath $decisions) {
    $lines = Read-Lines $decisions
    $adrFail = 0
    $expected = 1
    $seenAny = $false
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $m = [regex]::Match($lines[$i], '^## ADR-(\d{3}): ')
        if (-not $m.Success) {
            if ($lines[$i] -match '^## ADR-') {
                $adrFail++
                Add-Failure "$decisions`:$($i + 1): ADR heading does not match '## ADR-NNN: ' with three digits."
            }
            continue
        }
        $seenAny = $true
        $number = [int]$m.Groups[1].Value
        if ($number -ne $expected) {
            $adrFail++
            $want = '{0:D3}' -f $expected
            Add-Failure "$decisions`:$($i + 1): ADR numbering jumped. Expected ADR-$want, found ADR-$($m.Groups[1].Value). Numbering starts at 001, ascends, no gaps, no reuse."
            $expected = $number
        }
        $anchor = '<a id="adr-' + $m.Groups[1].Value + '"></a>'
        $prev = ''
        if ($i -gt 0) { $prev = $lines[$i - 1].Trim() }
        if ($prev -ne $anchor) {
            $adrFail++
            Add-Failure "$decisions`:$($i + 1): ADR-$($m.Groups[1].Value) is missing its anchor. The line directly above the heading must be exactly $anchor"
        }
        $expected = $number + 1
    }
    if (-not $seenAny) {
        Add-Failure "$decisions`: no ADR headings found. Expected '## ADR-001: ' onwards."
    } elseif ($adrFail -eq 0) {
        $highest = '{0:D3}' -f ($expected - 1)
        Write-Host "[ok]   ADRs 001 to $highest contiguous and anchored"
    }
} else {
    Add-Failure "docs/DECISIONS.md is missing."
}

# ---------------------------------------------------------------------
# Gate 4: the Constellation contract
# ---------------------------------------------------------------------
# Constellation scrapes this repo. A stray bullet or a renamed heading
# breaks the owner's dashboard silently and nothing here would say so.

$constellationFail = 0

$readme = Read-Lines 'README.md'
if (-not ($readme | Where-Object { $_ -match '^## Status\s*$' })) {
    $constellationFail++
    Add-Failure "README.md`: no '## Status' heading. Constellation reads the current phase from it."
}

$todo = Read-Lines 'TODO.md'
$inSection = $false
for ($i = 0; $i -lt $todo.Count; $i++) {
    $line = $todo[$i]
    if ($line -match '^### ') { $inSection = $true; continue }
    if ($line -match '^#{1,2} ') { $inSection = $false; continue }
    if (-not $inSection) { continue }
    if ($line -match '^\s*[-*+] ' -and $line -notmatch '^- \[[ x]\] ') {
        $constellationFail++
        Add-Failure "TODO.md`:$($i + 1): list item under a '###' heading is not a Constellation checkbox. Use '- [ ] ' or '- [x] ' at column 1. | $($line.Trim())"
    }
}
if ($constellationFail -eq 0) {
    Write-Host "[ok]   Constellation contract intact (README Status, TODO checkboxes)"
}

# ---------------------------------------------------------------------
# Gate 5: wrap width. Warning only, never a failure.
# ---------------------------------------------------------------------
# Roughly 45 lines legitimately exceed 80 columns today, nearly all of them
# table rows and URLs. This counts what is left so drift is visible without
# ever blocking a commit or a merge.

$wide = New-Object System.Collections.Generic.List[string]
foreach ($file in $markdown) {
    $lines = Read-Lines $file
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if ($line.Length -le 80) { continue }
        if ($line -match '^\s*\|') { continue }
        if ($line -match 'https?://') { continue }
        $wide.Add("$file`:$($i + 1) ($($line.Length) cols)") | Out-Null
    }
}
if ($wide.Count -gt 0) {
    Add-Warning "$($wide.Count) prose line(s) exceed 80 columns. Not a failure."
    foreach ($entry in ($wide | Select-Object -First 10)) {
        Write-Host "       $entry"
    }
    if ($wide.Count -gt 10) {
        Write-Host "       ... and $($wide.Count - 10) more"
    }
} else {
    Write-Host "[ok]   no over-wide prose lines"
}

# ---------------------------------------------------------------------

Write-Host ""
if ($failures.Count -gt 0) {
    Write-Host "check-docs: FAIL, $($failures.Count) problem(s), $($warnings.Count) warning(s)"
    exit 1
}
Write-Host "check-docs: PASS, $($warnings.Count) warning(s)"
exit 0
