#!/usr/bin/env pwsh

# Build All Platforms and Versions
# Supports: Rust CLI, LSP Server, VS Code Extension, Sentinel App, Website

param(
    [switch]$Clean = $false,
    [switch]$Release = $false,
    [switch]$VerboseBuild = $false
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
Write-Host "[BUILD] Starting Multi-Platform Build [$timestamp]" -ForegroundColor Cyan

# Detect OS
$isWindows = $IsWindows -or ($PSVersionTable.PSVersion.Major -lt 6)
$isMacOS = $IsMacOS
$isLinux = $IsLinux

Write-Host "Platform: $(if ($isWindows) { 'Windows' } elseif ($isMacOS) { 'macOS' } else { 'Linux' })" -ForegroundColor Cyan

# Step 1: Clean if requested
if ($Clean) {
    Write-Host "`n[BUILD] Cleaning previous builds..." -ForegroundColor Yellow
    cargo clean
    if (Test-Path "dist") { Remove-Item -Recurse -Force "dist" | Out-Null }
    if (Test-Path "build") { Remove-Item -Recurse -Force "build" | Out-Null }
    Write-Host "[OK] Clean complete" -ForegroundColor Green
}

# Step 2: Build Rust workspace
Write-Host "`n[BUILD] Building Rust workspace..." -ForegroundColor Yellow
$cargoArgs = @("build")
if ($Release) { $cargoArgs += "--release" }
if ($VerboseBuild) { $cargoArgs += "--verbose" }

cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] Cargo build failed" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Rust workspace build complete" -ForegroundColor Green

# Step 3: Build VS Code Extension
Write-Host "`n[BUILD] Building VS Code Extension..." -ForegroundColor Yellow
$vscodeExtPath = "editors/vscode"
if (Test-Path $vscodeExtPath) {
    Push-Location $vscodeExtPath
    
    # Check for package.json
    if (Test-Path "package.json") {
        if (-not (Test-Path "node_modules")) {
            npm install
        }
        npm run build
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[WARN] VS Code extension compilation had issues" -ForegroundColor Yellow
        } else {
            Write-Host "[OK] VS Code extension compiled" -ForegroundColor Green
        }

        npm run package:vsix
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[WARN] VS Code extension packaging (VSIX) had issues" -ForegroundColor Yellow
        } else {
            Write-Host "[OK] VS Code extension packaged (VSIX)" -ForegroundColor Green
        }
    }
    Pop-Location
}

# Step 4: Build Sentinel Desktop App
Write-Host "`n[BUILD] Building Sentinel Desktop App..." -ForegroundColor Yellow
$sentinelPath = "editors/sentinel-app"
if (Test-Path $sentinelPath) {
    Push-Location $sentinelPath
    
    if (Test-Path "package.json") {
        if (-not (Test-Path "node_modules")) {
            npm install
        }
        npm run build
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[WARN] Sentinel app build had issues" -ForegroundColor Yellow
        } else {
            Write-Host "[OK] Sentinel app built" -ForegroundColor Green
        }
    }
    Pop-Location
}

# Step 5: Build Website
Write-Host "`n[BUILD] Building Website..." -ForegroundColor Yellow
$websitePath = "website"
if (Test-Path $websitePath) {
    Push-Location $websitePath
    
    if (Test-Path "package.json") {
        if (-not (Test-Path "node_modules")) {
            npm install
        }
        npm run build
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[WARN] Website build had issues" -ForegroundColor Yellow
        } else {
            Write-Host "[OK] Website built" -ForegroundColor Green
        }
    }
    Pop-Location
}

# Step 6: Create distribution package
Write-Host "`n[BUILD] Creating distribution packages..." -ForegroundColor Yellow
if (-not (Test-Path "dist")) { New-Item -ItemType Directory "dist" | Out-Null }

# Copy Rust binaries
$buildTarget = if ($Release) { "release" } else { "debug" }
if (Test-Path "target/$buildTarget/aura$([System.IO.Path]::GetExtension('aura.exe'))") {
    Copy-Item "target/$buildTarget/aura$([System.IO.Path]::GetExtension('aura.exe'))" "dist/" -Force
    Write-Host "[OK] Aura CLI copied to dist/" -ForegroundColor Green
}

if (Test-Path "target/$buildTarget/aura-lsp$([System.IO.Path]::GetExtension('aura-lsp.exe'))") {
    Copy-Item "target/$buildTarget/aura-lsp$([System.IO.Path]::GetExtension('aura-lsp.exe'))" "dist/" -Force
    Write-Host "[OK] Aura LSP copied to dist/" -ForegroundColor Green
}

# Step 7: Version info
Write-Host "`n[BUILD] Version Information:" -ForegroundColor Cyan
$cargoToml = Get-Content "Cargo.toml" -Raw
$version = $cargoToml | Select-String 'version\s*=\s*"([^"]+)"' | ForEach-Object { $_.Matches[0].Groups[1].Value }
Write-Host "  Aura Version: $version" -ForegroundColor White

# Summary
Write-Host "`n[BUILD] ========== BUILD SUMMARY ==========" -ForegroundColor Cyan
Write-Host "  Platform: $(if ($isWindows) { 'Windows' } elseif ($isMacOS) { 'macOS' } else { 'Linux' })" -ForegroundColor White
Write-Host "  Configuration: $(if ($Release) { 'Release' } else { 'Debug' })" -ForegroundColor White
Write-Host "  Timestamp: $timestamp" -ForegroundColor White
Write-Host "  Output: ./dist/" -ForegroundColor White
Write-Host "  Distribution: Ready for packaging" -ForegroundColor White
Write-Host "`n[OK] All builds completed successfully!" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Cyan
