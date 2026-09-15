# Build docs/WHITEPAPER.md into a typeset PDF.
#
#   powershell -NoProfile -File scripts/build-whitepaper.ps1
#   powershell -NoProfile -File scripts/build-whitepaper.ps1 -TexOnly
#
# The markdown is the source of truth; the PDF is a build product and lands
# in target\whitepaper\ (gitignored). Needs pandoc and tectonic:
#   winget install JohnMacFarlane.Pandoc
#   cargo install tectonic   (or: winget install TectonicTypesetting.Tectonic)
#
# What the conversion has to do that a pandoc one-liner does not:
# - Strip the front matter (title, status line, revision block) and feed the
#   title, author, date, and abstract to the LaTeX title block instead.
# - Shift headings up one level. The body starts at `##`, which pandoc would
#   otherwise set as subsections under no section at all.
# - Rewrite repo-relative links (DECISIONS.md#adr-035) to GitHub URLs, since
#   a PDF has no repository next to it.
# - Fail on "Missing character". LaTeX drops a glyph its font lacks and
#   reports success, so the PDF would silently lose characters.

param([switch]$TexOnly)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repo "docs\WHITEPAPER.md"
$outDir = Join-Path $repo "target\whitepaper"
$blob = "https://github.com/owenpkent/deckhand/blob/main"

function Find-Tool([string]$name, [string[]]$fallbacks) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($path in $fallbacks) { if (Test-Path $path) { return $path } }
    throw "$name not found. See the header of this script for install hints."
}

$pandoc = Find-Tool "pandoc" @("$env:LOCALAPPDATA\Pandoc\pandoc.exe")

$lines = [IO.File]::ReadAllLines($source, [Text.Encoding]::UTF8)

# Front matter: everything before the first horizontal rule.
$rule = [Array]::IndexOf($lines, "---")
if ($rule -lt 0) { throw "WHITEPAPER.md: no '---' rule closing the front matter" }
$title = $lines[0] -replace '^#\s+', ''
$front = $lines[0..($rule - 1)] -join "`n"
$date = if ($front -match '\*\*Document revision:\*\*\s*(.+)') { $Matches[1].Trim() } else { "" }
$author = if ($front -match '\*\*Author:\*\*\s*(.+)') { $Matches[1].Trim() } else { "" }

# Abstract: the `## Abstract` section, up to the next rule.
$body = $lines[($rule + 1)..($lines.Count - 1)]
$absStart = [Array]::IndexOf($body, "## Abstract")
if ($absStart -lt 0) { throw "WHITEPAPER.md: no '## Abstract' section" }
$absEnd = $absStart + 1
while ($absEnd -lt $body.Count -and $body[$absEnd] -ne "---") { $absEnd++ }
$abstract = $body[($absStart + 1)..($absEnd - 1)]
$body = $body[($absEnd + 1)..($body.Count - 1)]

# Repo-relative links to GitHub. Paths in the paper are relative to docs/.
$text = ($body -join "`n")
$text = [regex]::Replace($text, '\]\((?!https?:|#|mailto:)([^)\s]+)\)', {
    param($m)
    $target = $m.Groups[1].Value
    if ($target.StartsWith("../")) { "]($blob/$($target.Substring(3)))" }
    else { "]($blob/docs/$target)" }
})

New-Item -ItemType Directory -Force $outDir | Out-Null
$utf8 = New-Object Text.UTF8Encoding($false)
$bodyFile = Join-Path $outDir "body.md"
$metaFile = Join-Path $outDir "meta.yaml"
[IO.File]::WriteAllText($bodyFile, $text, $utf8)

function Quote-Yaml([string]$s) { "'" + ($s -replace "'", "''") + "'" }
$meta = @(
    "title: $(Quote-Yaml $title)",
    "author: $(Quote-Yaml $author)",
    "date: $(Quote-Yaml $date)",
    "abstract: |"
) + ($abstract | ForEach-Object { if ($_ -eq "") { "" } else { "  $_" } })
[IO.File]::WriteAllText($metaFile, ($meta -join "`n") + "`n", $utf8)

$common = @(
    $bodyFile,
    "--from=markdown-implicit_figures",
    "--metadata-file=$metaFile",
    "--shift-heading-level-by=-1",
    "--toc",
    "-V", "documentclass=article",
    "-V", "fontsize=11pt",
    "-V", "geometry:margin=1in",
    "-V", "colorlinks=true",
    "-V", "linkcolor=blue",
    "-V", "urlcolor=blue"
)

if ($TexOnly) {
    $out = Join-Path $outDir "deckhand-whitepaper.tex"
    $args2 = $common + @("--standalone", "-o", $out)
} else {
    $tectonic = Find-Tool "tectonic" @("$env:USERPROFILE\.cargo\bin\tectonic.exe")
    $out = Join-Path $outDir "deckhand-whitepaper.pdf"
    $args2 = $common + @("--pdf-engine=$tectonic", "-o", $out)
}

# Native stderr under "Stop" throws in Windows PowerShell 5.1; collect it
# as text instead, then judge by exit code and content.
$ErrorActionPreference = "Continue"
$log = & $pandoc @args2 2>&1 | ForEach-Object { "$_" }
$code = $LASTEXITCODE
$ErrorActionPreference = "Stop"

$log | ForEach-Object { Write-Host $_ }
if ($code -ne 0) { throw "pandoc failed with exit code $code" }
$missing = @($log | Where-Object { $_ -match 'Missing character' })
if ($missing.Count -gt 0) {
    throw "$($missing.Count) missing character(s): the PDF dropped glyphs. Replace them in the markdown."
}

Write-Host "Built: $out"
