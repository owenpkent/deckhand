# Build the Phase 1 app: compile the TypeScript surface, then the Rust
# workspace. The surface must build first because tauri embeds
# app/ui (including dist/) into the binary at compile time.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

Push-Location (Join-Path $repo "app\ui")
try {
    if (-not (Test-Path "node_modules")) {
        npm install --no-audit --no-fund
        if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
    }
    npx tsc
    if ($LASTEXITCODE -ne 0) { throw "tsc failed" }
} finally {
    Pop-Location
}

Push-Location $repo
try {
    cargo build --workspace
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
} finally {
    Pop-Location
}

Write-Host "Built: target\debug\deckhand.exe and target\debug\deckhand-shim.exe"
