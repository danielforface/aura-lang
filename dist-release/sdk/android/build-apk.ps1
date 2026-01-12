param(
  [string]$SdkRoot = "$HOME\.aura\android-sdk"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..") | Select-Object -ExpandProperty Path
$sample = Join-Path $repoRoot "samples\android\AuraSentinelSample"
if (-not (Test-Path $sample)) {
  throw "Missing sample project at $sample"
}

$envFile = Join-Path $SdkRoot "aura-android-env.ps1"
if (Test-Path $envFile) {
  . $envFile
} else {
  if (-not $env:ANDROID_SDK_ROOT) {
    $env:ANDROID_SDK_ROOT = $SdkRoot
    $env:ANDROID_HOME = $SdkRoot
  }
}

if (-not $env:ANDROID_SDK_ROOT) {
  throw "ANDROID_SDK_ROOT is not set. Run sdk/android/setup-android.ps1 first or set ANDROID_SDK_ROOT."
}

# Prefer gradlew if user created it; otherwise fall back to gradle.
$gradlew = Join-Path $sample "gradlew.bat"
Push-Location $sample
try {
  if (Test-Path $gradlew) {
    & $gradlew assembleDebug
  } elseif (Get-Command gradle -ErrorAction SilentlyContinue) {
    gradle assembleDebug
  } else {
    throw "Neither gradlew.bat nor 'gradle' found. Open the project in Android Studio OR install Gradle and re-run."
  }
} finally {
  Pop-Location
}

$apk = Join-Path $sample "app\build\outputs\apk\debug\app-debug.apk"
if (-not (Test-Path $apk)) {
  throw "APK not found at expected path: $apk"
}

$outDir = Join-Path $repoRoot "dist\android"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
Copy-Item -Force -Path $apk -Destination (Join-Path $outDir "AuraSentinelSample-debug.apk")
Write-Host "Wrote dist/android/AuraSentinelSample-debug.apk"
