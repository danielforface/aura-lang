# Aura v1.0 Windows Installation Script
# This script will install Aura to the system and set up environment

param(
    [string]$InstallPath = "C:\Program Files\Aura",
    [switch]$AddToPath = $true,
    [switch]$CreateShortcuts = $true
)

Write-Host "╔════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  Aura v1.0 Installation Script      ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Check if running as admin
$isAdmin = ([System.Security.Principal.WindowsIdentity]::GetCurrent().Groups -contains [System.Security.Principal.SecurityIdentifier]"S-1-5-32-544")
if (-not $isAdmin) {
    Write-Host "⚠️  This script should be run as Administrator for full functionality" -ForegroundColor Yellow
}

Write-Host "Installation Settings:" -ForegroundColor Cyan
Write-Host "  Install Path: $InstallPath"
Write-Host "  Add to PATH:  $AddToPath"
Write-Host "  Create Shortcuts: $CreateShortcuts"
Write-Host ""

# Create installation directory
Write-Host "Creating installation directory..." -ForegroundColor Yellow
if (Test-Path $InstallPath) {
    Write-Host "  Directory exists, updating installation..."
} else {
    New-Item -ItemType Directory -Path $InstallPath | Out-Null
    Write-Host "  ✓ Created $InstallPath"
}

# Copy files
Write-Host "Copying files..." -ForegroundColor Yellow
$sourceDir = $PSScriptRoot
$subdirs = @("bin", "lib", "apps", "docs", "examples", "sdk", "config")

foreach ($dir in $subdirs) {
    $src = Join-Path $sourceDir $dir
    $dst = Join-Path $InstallPath $dir
    
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $dst -Recurse -Force
        Write-Host "  ✓ Copied $dir"
    }
}

# Copy documentation
Write-Host "Installing documentation..." -ForegroundColor Yellow
$docFiles = @("README.md", "ROADMAP.md", "BUILD_COMPLETE_SUMMARY.md")
foreach ($file in $docFiles) {
    $src = Join-Path $sourceDir $file
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $InstallPath
        Write-Host "  ✓ Copied $file"
    }
}

# Add to PATH
if ($AddToPath) {
    Write-Host "Adding to system PATH..." -ForegroundColor Yellow
    $binPath = Join-Path $InstallPath "bin"
    
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", [EnvironmentVariableTarget]::User)
    if ($currentPath -notlike "*$binPath*") {
        [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$binPath", [EnvironmentVariableTarget]::User)
        Write-Host "  ✓ Added $binPath to PATH"
    } else {
        Write-Host "  ℹ Already in PATH"
    }
}

# Create shortcuts
if ($CreateShortcuts) {
    Write-Host "Creating shortcuts..." -ForegroundColor Yellow
    
    # Desktop shortcut for Sentinel IDE
    $desktopPath = [Environment]::GetFolderPath("Desktop")
    $shellLink = New-Object -ComObject WScript.Shell
    
    $sentinelPath = Join-Path $InstallPath "apps\sentinel\index.html"
    if (Test-Path $sentinelPath) {
        $shortcut = $shellLink.CreateShortcut("$desktopPath\Sentinel IDE.lnk")
        $shortcut.TargetPath = $sentinelPath
        $shortcut.WorkingDirectory = Join-Path $InstallPath "apps\sentinel"
        $shortcut.Description = "Aura Sentinel IDE"
        $shortcut.Save()
        Write-Host "  ✓ Created Sentinel IDE shortcut"
    }
    
    # Start menu folder
    $startMenuPath = [Environment]::GetFolderPath("Programs")
    $auraFolder = Join-Path $startMenuPath "Aura"
    if (-not (Test-Path $auraFolder)) {
        New-Item -ItemType Directory -Path $auraFolder | Out-Null
    }
    
    # Documentation shortcut
    $readmeLink = $shellLink.CreateShortcut("$auraFolder\README.lnk")
    $readmeLink.TargetPath = Join-Path $InstallPath "README.md"
    $readmeLink.Description = "Aura Documentation"
    $readmeLink.Save()
    Write-Host "  ✓ Created Start menu shortcuts"
}

# Verify installation
Write-Host ""
Write-Host "Verifying installation..." -ForegroundColor Yellow
$binaries = @("aura.exe", "aura-lsp.exe", "aura-pkg.exe")
$binPath = Join-Path $InstallPath "bin"

$allGood = $true
foreach ($binary in $binaries) {
    $path = Join-Path $binPath $binary
    if (Test-Path $path) {
        Write-Host "  ✓ Found $binary"
    } else {
        Write-Host "  ✗ Missing $binary" -ForegroundColor Red
        $allGood = $false
    }
}

Write-Host ""
if ($allGood) {
    Write-Host "✅ Installation completed successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "  1. Open Command Prompt and type: aura --version"
    Write-Host "  2. Visit $InstallPath\README.md for documentation"
    Write-Host "  3. Launch Sentinel IDE from Start menu"
    Write-Host ""
    Write-Host "For more information, visit: https://aura-lang.dev" -ForegroundColor Cyan
} else {
    Write-Host "⚠️  Installation completed with issues" -ForegroundColor Yellow
    Write-Host "Please check the installation directory: $InstallPath"
}

Write-Host ""
Write-Host "Installation Path: $InstallPath" -ForegroundColor Green
