param(
  [string]$InstallRoot = "$HOME\.aura\android-sdk",
  [string]$CmdlineToolsUrl = "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip",
  [string]$Platform = "android-34",
  [string]$BuildTools = "34.0.0",
  [string]$NdkVersion = "26.1.10909125",
  [switch]$AcceptLicenses
)

$ErrorActionPreference = "Stop"

function Require-Command([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $Name"
  }
}

Write-Host "== Aura Android toolchain setup =="
Require-Command "tar"
Require-Command "Expand-Archive"

# Java check (sdkmanager needs it)
if (-not $env:JAVA_HOME -and -not (Get-Command java -ErrorAction SilentlyContinue)) {
  throw "Java not found. Install Java 17+ and set JAVA_HOME (or ensure 'java' is on PATH)."
}

$root = Resolve-Path -LiteralPath (Split-Path -Parent $PSScriptRoot) | Select-Object -ExpandProperty Path
$repoRoot = Resolve-Path -LiteralPath (Join-Path $root "..") | Select-Object -ExpandProperty Path
$distDir = Join-Path $repoRoot "dist\android"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null

$zipPath = Join-Path $distDir "commandlinetools.zip"
Write-Host "Downloading command-line tools..."
Invoke-WebRequest -Uri $CmdlineToolsUrl -OutFile $zipPath

$tmp = Join-Path $distDir "cmdline-tools-tmp"
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

Write-Host "Extracting command-line tools..."
Expand-Archive -LiteralPath $zipPath -DestinationPath $tmp -Force

# The zip contains 'cmdline-tools'. Put under cmdline-tools/latest.
$cmdlineSrc = Join-Path $tmp "cmdline-tools"
if (-not (Test-Path $cmdlineSrc)) {
  throw "Unexpected command-line tools zip layout. Expected 'cmdline-tools/' folder."
}

$cmdlineDst = Join-Path $InstallRoot "cmdline-tools\latest"
New-Item -ItemType Directory -Force -Path $cmdlineDst | Out-Null
Copy-Item -Recurse -Force -Path (Join-Path $cmdlineSrc "*") -Destination $cmdlineDst

$sdkmanager = Join-Path $cmdlineDst "bin\sdkmanager.bat"
if (-not (Test-Path $sdkmanager)) {
  throw "sdkmanager not found at $sdkmanager"
}

Write-Host "Accepting licenses..."
if ($AcceptLicenses) {
  # sdkmanager reads from stdin; provide lots of 'y' answers.
  $ys = @()
  for ($i = 0; $i -lt 200; $i++) { $ys += "y" }
  $ys | & $sdkmanager --licenses | Out-Host
} else {
  Write-Host "(Interactive) If prompted, type 'y' to accept."
  & $sdkmanager --licenses | Out-Host
}

Write-Host "Installing Android packages..."
& $sdkmanager --sdk_root=$InstallRoot `
  "platform-tools" `
  "platforms;$Platform" `
  "build-tools;$BuildTools" `
  "ndk;$NdkVersion" `
  "cmake;3.22.1" | Out-Host

# Write env helper
$envFile = Join-Path $InstallRoot "aura-android-env.ps1"
$ndkHome = Join-Path $InstallRoot ("ndk\" + $NdkVersion)
@(
  "`$env:ANDROID_SDK_ROOT = '$InstallRoot'",
  "`$env:ANDROID_HOME = '$InstallRoot'",
  "`$env:ANDROID_NDK_HOME = '$ndkHome'",
  "`$env:ANDROID_NDK_ROOT = '$ndkHome'"
) | Set-Content -Encoding UTF8 -LiteralPath $envFile

Write-Host "Done."
Write-Host "- Android SDK: $InstallRoot"
Write-Host "- Env script: $envFile"
Write-Host "Next: run sdk/android/build-apk.ps1"
