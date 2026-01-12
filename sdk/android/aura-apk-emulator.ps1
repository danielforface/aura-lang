#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Aura APK Emulator & Deployment System
    Complete setup, build, and run pipeline for Aura applications on Android

.DESCRIPTION
    This script sets up everything needed to build and run Aura APK files:
    1. Installs Android SDK/NDK and emulator tools
    2. Creates/starts Android Virtual Device (AVD)
    3. Builds APK from Aura source
    4. Deploys to emulator/device

.PARAMETER Mode
    Operation mode: 'setup', 'build', 'run', 'deploy', 'full'

.PARAMETER AuraSource
    Path to .aura source file to build (for 'build' and 'full')

.PARAMETER SdkRoot
    Android SDK installation root (default: $HOME\.aura\android-sdk)

.PARAMETER AvdName
    Android Virtual Device name (default: AuraEmulator)

.PARAMETER ApiLevel
    Android API level for emulator (default: 34)

.EXAMPLE
    # Full setup: install SDK + create emulator
    ./aura-apk-emulator.ps1 -Mode setup

    # Build APK from Aura source
    ./aura-apk-emulator.ps1 -Mode build -AuraSource ./my_app.aura

    # Start emulator and deploy APK
    ./aura-apk-emulator.ps1 -Mode run

    # Complete pipeline: setup + build + run
    ./aura-apk-emulator.ps1 -Mode full -AuraSource ./my_app.aura
#>

param(
    [ValidateSet("setup", "build", "run", "deploy", "full", "clean", "list-devices", "logcat")]
    [string]$Mode = "full",
    
    [string]$AuraSource,
    [string]$SdkRoot = "$HOME\.aura\android-sdk",
    [string]$AvdName = "AuraEmulator",
    [int]$ApiLevel = 34,
    [string]$Arch = "x86_64",
    [switch]$AcceptLicenses,
    [switch]$Headless
)

$ErrorActionPreference = "Stop"

# ============================================================================
# CONFIGURATION
# ============================================================================

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$REPO_ROOT = Resolve-Path (Join-Path $SCRIPT_DIR "..\..") | Select-Object -ExpandProperty Path
$SAMPLE_PROJECT = Join-Path $REPO_ROOT "samples\android\AuraSentinelSample"
$DIST_DIR = Join-Path $REPO_ROOT "dist\android"
$EMULATOR_LOG = Join-Path $DIST_DIR "emulator.log"
$AURA_TOOLKIT = Join-Path $SdkRoot "aura-toolkit"

$CMDLINE_TOOLS_URL = "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip"
$PLATFORM = "android-$ApiLevel"
$BUILD_TOOLS = "34.0.0"
$NDK_VERSION = "26.1.10909125"

# ============================================================================
# UTILITIES
# ============================================================================

function Write-Title([string]$Title) {
    Write-Host "`n=== $Title ===" -ForegroundColor Cyan
}

function Write-Success([string]$Message) {
    Write-Host "✓ $Message" -ForegroundColor Green
}

function Write-Error-Custom([string]$Message) {
    Write-Host "✗ $Message" -ForegroundColor Red
    exit 1
}

function Require-Command([string]$Name, [string]$Url = $null) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        $msg = "Missing required command: $Name"
        if ($Url) { $msg += "`nInstall from: $Url" }
        Write-Error-Custom $msg
    }
}

function Set-AndroidEnv {
    $env:ANDROID_SDK_ROOT = $SdkRoot
    $env:ANDROID_HOME = $SdkRoot
    $env:PATH = "$SdkRoot\platform-tools;$env:PATH"
}

# ============================================================================
# SETUP: Install Android SDK, NDK, Emulator, and create AVD
# ============================================================================

function Invoke-AndroidSetup {
    Write-Title "Setting up Android SDK/NDK/Emulator"
    
    # Check Java
    Require-Command "java" "https://adoptium.net/ (Java 17+)"
    Write-Success "Java found: $(java -version 2>&1 | Select-Object -First 1)"
    
    # Check prerequisites
    Require-Command "tar"
    Require-Command "Expand-Archive"
    
    # Create directories
    New-Item -ItemType Directory -Force -Path $SdkRoot | Out-Null
    New-Item -ItemType Directory -Force -Path $DIST_DIR | Out-Null
    Write-Success "Created SDK root: $SdkRoot"
    
    # Download and install command-line tools
    Write-Host "`nDownloading Android command-line tools..."
    $zipPath = Join-Path $DIST_DIR "cmdline-tools.zip"
    if (Test-Path $zipPath) { Remove-Item $zipPath }
    
    try {
        Invoke-WebRequest -Uri $CMDLINE_TOOLS_URL -OutFile $zipPath -UseBasicParsing
        Write-Success "Downloaded command-line tools"
    } catch {
        Write-Error-Custom "Failed to download: $_"
    }
    
    # Extract
    $tmp = Join-Path $DIST_DIR "cmdline-tools-tmp"
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    
    Write-Host "Extracting command-line tools..."
    Expand-Archive -LiteralPath $zipPath -DestinationPath $tmp -Force
    
    $cmdlineSrc = Join-Path $tmp "cmdline-tools"
    if (-not (Test-Path $cmdlineSrc)) {
        Write-Error-Custom "Unexpected archive layout"
    }
    
    $cmdlineDst = Join-Path $SdkRoot "cmdline-tools\latest"
    New-Item -ItemType Directory -Force -Path $cmdlineDst | Out-Null
    Copy-Item -Recurse -Force -Path (Join-Path $cmdlineSrc "*") -Destination $cmdlineDst
    Write-Success "Extracted to $cmdlineDst"
    
    # Set environment
    Set-AndroidEnv
    
    # Run sdkmanager
    $sdkmanager = Join-Path $cmdlineDst "bin\sdkmanager.bat"
    if (-not (Test-Path $sdkmanager)) {
        Write-Error-Custom "sdkmanager not found at $sdkmanager"
    }
    
    # Accept licenses
    Write-Host "`nAccepting Android licenses..."
    if ($AcceptLicenses) {
        $ys = @()
        for ($i = 0; $i -lt 200; $i++) { $ys += "y" }
        $ys | & $sdkmanager --licenses *>$null
        Write-Success "Licenses accepted"
    } else {
        & $sdkmanager --licenses | Out-Host
    }
    
    # Install packages
    Write-Host "`nInstalling Android packages..."
    $packages = @(
        "platform-tools",
        "platforms;$PLATFORM",
        "build-tools;$BUILD_TOOLS",
        "ndk;$NDK_VERSION",
        "cmake;3.22.1",
        "emulator",
        "system-images;$PLATFORM;google_apis;$Arch"
    )
    
    foreach ($pkg in $packages) {
        Write-Host "  Installing: $pkg"
        & $sdkmanager --sdk_root=$SdkRoot $pkg 2>&1 | Out-Host
    }
    Write-Success "Android SDK/NDK/Emulator installed"
    
    # Create environment file
    $envFile = Join-Path $SdkRoot "aura-android-env.ps1"
    @"
# Aura Android Environment Setup
`$env:ANDROID_SDK_ROOT = "$SdkRoot"
`$env:ANDROID_HOME = "$SdkRoot"
`$env:ANDROID_NDK_HOME = "$SdkRoot\ndk\$NDK_VERSION"
`$env:PATH = "$SdkRoot\platform-tools;`$env:PATH"
`$env:PATH = "$SdkRoot\emulator;`$env:PATH"
"@ | Out-File -Encoding UTF8 $envFile
    Write-Success "Created environment file: $envFile"
    
    # Create AVD
    Invoke-CreateAvd
}

# ============================================================================
# AVD: Create Android Virtual Device
# ============================================================================

function Invoke-CreateAvd {
    Write-Title "Creating Android Virtual Device"
    
    Set-AndroidEnv
    
    # Check if AVD already exists
    $avdManager = Join-Path $SdkRoot "cmdline-tools\latest\bin\avdmanager.bat"
    if (-not (Test-Path $avdManager)) {
        Write-Error-Custom "avdmanager not found"
    }
    
    # List existing AVDs
    $avds = & $avdManager list avd 2>&1
    if ($avds -match $AvdName) {
        Write-Success "AVD '$AvdName' already exists"
        return
    }
    
    # Create new AVD
    Write-Host "Creating AVD '$AvdName' (API $ApiLevel, $Arch)..."
    
    # Use echo to pipe 'no' to abi and other prompts
    echo "no" | & $avdManager create avd `
        --name $AvdName `
        --package "system-images;$PLATFORM;google_apis;$Arch" `
        --device "pixel_4" `
        --force 2>&1 | Out-Host
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "AVD '$AvdName' created successfully"
    } else {
        Write-Error-Custom "Failed to create AVD"
    }
}

# ============================================================================
# BUILD: Compile Aura source to APK
# ============================================================================

function Invoke-BuildApk {
    Write-Title "Building APK"
    
    if (-not $AuraSource) {
        Write-Error-Custom "AuraSource parameter required for build mode"
    }
    
    $sourceFile = Resolve-Path $AuraSource -ErrorAction SilentlyContinue
    if (-not (Test-Path $sourceFile)) {
        Write-Error-Custom "Aura source file not found: $AuraSource"
    }
    
    Write-Host "Source: $sourceFile"
    
    # For now, use the sample project
    # In future: auto-generate Android project from .aura source
    Write-Host "`nUsing sample project template: $SAMPLE_PROJECT"
    
    # Copy source to sample assets
    $assetsDir = Join-Path $SAMPLE_PROJECT "app\src\main\assets"
    New-Item -ItemType Directory -Force -Path $assetsDir | Out-Null
    Copy-Item -Force $sourceFile (Join-Path $assetsDir "app.aura")
    Write-Success "Copied source to assets"
    
    Set-AndroidEnv
    
    # Build with Gradle
    $gradlew = Join-Path $SAMPLE_PROJECT "gradlew.bat"
    
    Push-Location $SAMPLE_PROJECT
    try {
        Write-Host "Running Gradle build..."
        if (Test-Path $gradlew) {
            & $gradlew assembleDebug 2>&1 | Out-Host
        } else {
            Require-Command "gradle"
            gradle assembleDebug 2>&1 | Out-Host
        }
    } finally {
        Pop-Location
    }
    
    # Verify APK
    $apk = Join-Path $SAMPLE_PROJECT "app\build\outputs\apk\debug\app-debug.apk"
    if (-not (Test-Path $apk)) {
        Write-Error-Custom "APK build failed: not found at $apk"
    }
    
    # Copy to dist
    $outApk = Join-Path $DIST_DIR "AuraSample-debug.apk"
    Copy-Item -Force $apk $outApk
    Write-Success "APK built: $outApk"
}

# ============================================================================
# RUN: Start emulator and deploy APK
# ============================================================================

function Invoke-RunEmulator {
    Write-Title "Starting Android Emulator"
    
    Set-AndroidEnv
    
    # Check if emulator is running
    $adb = Join-Path $SdkRoot "platform-tools\adb.exe"
    if (-not (Test-Path $adb)) {
        Write-Error-Custom "adb not found"
    }
    
    # Try to connect to existing emulator
    $devices = & $adb devices 2>&1
    $running = $devices | Select-String "emulator-"
    
    if (-not $running) {
        Write-Host "Starting emulator '$AvdName'..."
        
        $emulator = Join-Path $SdkRoot "emulator\emulator.exe"
        if (-not (Test-Path $emulator)) {
            Write-Error-Custom "emulator not found at $emulator"
        }
        
        # Start emulator in background
        $args = "-avd", $AvdName, "-no-snapshot-load"
        if ($Headless) { $args += "-no-window" }
        
        Start-Process -FilePath $emulator -ArgumentList $args -NoNewWindow -RedirectStandardOutput $EMULATOR_LOG -RedirectStandardError $EMULATOR_LOG
        Write-Success "Emulator starting (PID: $($_.Handle))..."
        
        # Wait for boot
        Write-Host "Waiting for emulator to boot (this takes 30-60 seconds)..."
        $timeout = 120
        $elapsed = 0
        
        while ($elapsed -lt $timeout) {
            Start-Sleep -Seconds 3
            $elapsed += 3
            
            try {
                $bootState = & $adb shell getprop sys.boot_completed 2>&1
                if ($bootState -eq "1") {
                    Write-Success "Emulator booted!"
                    break
                }
            } catch {
                # Emulator still starting
            }
            
            Write-Host "  [$elapsed/$timeout] Waiting for boot..."
        }
        
        if ($elapsed -ge $timeout) {
            Write-Error-Custom "Emulator failed to boot within timeout"
        }
    } else {
        Write-Success "Emulator already running"
    }
    
    # Show device list
    Write-Host "`nConnected devices:"
    & $adb devices 2>&1 | Select-Object -Skip 1
}

function Invoke-DeployApk {
    Write-Title "Deploying APK to Emulator"
    
    Set-AndroidEnv
    
    $adb = Join-Path $SdkRoot "platform-tools\adb.exe"
    if (-not (Test-Path $adb)) {
        Write-Error-Custom "adb not found"
    }
    
    $apk = Join-Path $DIST_DIR "AuraSample-debug.apk"
    if (-not (Test-Path $apk)) {
        Write-Error-Custom "APK not found: $apk"
    }
    
    Write-Host "Installing APK: $apk"
    & $adb install -r $apk 2>&1 | Out-Host
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "APK installed successfully"
    } else {
        Write-Error-Custom "APK installation failed"
    }
    
    # Launch app
    Write-Host "`nLaunching application..."
    & $adb shell am start -n "com.aura.sentinel.sample/.MainActivity" 2>&1 | Out-Host
    
    Write-Success "App launched!"
}

# ============================================================================
# CLEANUP
# ============================================================================

function Invoke-Clean {
    Write-Title "Cleaning up"
    
    Set-AndroidEnv
    
    $adb = Join-Path $SdkRoot "platform-tools\adb.exe"
    
    Write-Host "Killing emulator..."
    & $adb emu kill 2>&1 | Out-Host
    
    Write-Success "Cleanup complete"
}

# ============================================================================
# DIAGNOSTICS
# ============================================================================

function Invoke-ListDevices {
    Write-Title "Available Devices"
    
    Set-AndroidEnv
    
    $adb = Join-Path $SdkRoot "platform-tools\adb.exe"
    & $adb devices -l 2>&1 | Out-Host
}

function Invoke-Logcat {
    Write-Title "Device Logcat"
    
    Set-AndroidEnv
    
    $adb = Join-Path $SdkRoot "platform-tools\adb.exe"
    & $adb logcat *:V 2>&1 | Out-Host
}

# ============================================================================
# MAIN
# ============================================================================

try {
    switch ($Mode) {
        "setup" {
            Invoke-AndroidSetup
            Invoke-CreateAvd
        }
        
        "build" {
            Invoke-BuildApk
        }
        
        "run" {
            Invoke-RunEmulator
            Invoke-DeployApk
            Invoke-Logcat
        }
        
        "deploy" {
            Invoke-DeployApk
        }
        
        "full" {
            if (-not (Test-Path "$SdkRoot\platform-tools")) {
                Invoke-AndroidSetup
                Invoke-CreateAvd
            }
            
            Invoke-BuildApk
            Invoke-RunEmulator
            Invoke-DeployApk
            Invoke-Logcat
        }
        
        "clean" {
            Invoke-Clean
        }
        
        "list-devices" {
            Invoke-ListDevices
        }
        
        "logcat" {
            Invoke-Logcat
        }
    }
    
    Write-Success "Done!"
    
} catch {
    Write-Error-Custom "Error: $_"
}
