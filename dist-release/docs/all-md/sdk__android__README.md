# Aura Android Toolchain (SDK/NDK)

This folder provides **bootstrap scripts** to install the Android toolchain and build the sample APK.

## What this does
- Downloads Android **command-line tools** into a local, self-contained SDK directory
- Installs required packages via `sdkmanager` (platform-tools, build-tools, platforms, NDK)
- Builds the sample APK in `samples/android/AuraSentinelSample`

## Requirements
- Windows: PowerShell 5.1+ (or PowerShell 7)
- Java 17+ (required by `sdkmanager` and recent AGP)

## Quick start (Windows)
1) Run toolchain setup:

   `powershell -ExecutionPolicy Bypass -File sdk/android/setup-android.ps1`

2) Build the sample APK:

   `powershell -ExecutionPolicy Bypass -File sdk/android/build-apk.ps1`

The resulting APK will be copied to `dist/android/`.

## Notes
- Android packages are large; first install can take a while.
- If you already have Android Studio installed, you can skip setup and only set:
  - `ANDROID_SDK_ROOT`
  - `ANDROID_NDK_HOME` (or `ANDROID_NDK_ROOT`)
