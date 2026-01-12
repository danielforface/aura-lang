# Aura Android Toolchain & APK Emulator

Complete system for building and running Aura applications on Android via emulator or physical devices.

## What's Included

### Core Scripts
- **`aura-apk-emulator.ps1`** - Comprehensive PowerShell build/deploy/run pipeline
- **`aura_apk.py`** - Python wrapper for easy cross-platform usage
- **`setup-android.ps1`** - Android SDK/NDK installation (legacy, integrated into emulator script)
- **`build-apk.ps1`** - APK build with Gradle (legacy, integrated into emulator script)

### Functionality
✓ Install Android SDK, NDK, emulator, and build-tools
✓ Create/manage Android Virtual Devices (AVD)
✓ Build APK from Aura source files
✓ Deploy to Android emulator or physical device
✓ Monitor application logs via logcat
✓ Support for debug and release builds

## Requirements

### System
- **Windows 10+** (or WSL2 on Windows)
- **PowerShell 5.1+** (or PowerShell 7+)
- **Java 17+** (required for Android build tools)
  - Download: https://adoptium.net/ (Eclipse Temurin recommended)
  - Or use Android Studio's bundled Java

### Storage
- ~10 GB for Android SDK/NDK/emulator (first-time setup)
- ~500 MB per APK build
- Recommended: SSD for emulator performance

### Network
- Internet connection for downloading SDK/NDK packages
- ~2-3 GB downloads for initial setup

## Quick Start

### Option 1: Python (Recommended for simplicity)

```bash
# 1. Install dependencies (once)
python sdk/android/aura_apk.py setup

# 2. Build APK from Aura source
python sdk/android/aura_apk.py build --source my_app.aura

# 3. Deploy to emulator and run
python sdk/android/aura_apk.py run

# Or do everything in one command:
python sdk/android/aura_apk.py full --source my_app.aura
```

### Option 2: PowerShell (More control)

```powershell
# 1. Setup (install SDK/NDK/emulator)
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode setup

# 2. Build APK
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode build -AuraSource my_app.aura

# 3. Run (start emulator + deploy)
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode run

# Or full pipeline:
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode full -AuraSource my_app.aura -AcceptLicenses
```

## Python Script Usage

```bash
# Setup (install SDK, NDK, emulator, create AVD)
python aura_apk.py setup

# Build APK from Aura source file
python aura_apk.py build --source app.aura

# Start emulator and deploy APK
python aura_apk.py run

# Complete pipeline (setup + build + run)
python aura_apk.py full --source app.aura

# Manage devices
python aura_apk.py list-devices      # Show connected devices
python aura_apk.py logcat           # Show device logs (Ctrl+C to exit)
python aura_apk.py clean            # Stop emulator

# Options
python aura_apk.py build --source app.aura --sdk-root /custom/sdk --avd-name MyDevice
```

## PowerShell Script Modes

```powershell
# Setup mode: Install SDK/NDK/emulator, create AVD
-Mode setup

# Build mode: Compile Aura source to APK
-Mode build -AuraSource ./app.aura

# Run mode: Start emulator and deploy
-Mode run

# Deploy mode: Install APK on running device
-Mode deploy

# Full mode: Complete pipeline
-Mode full -AuraSource ./app.aura

# Diagnostics
-Mode list-devices    # Show devices
-Mode logcat          # Show logs
-Mode clean           # Stop emulator
```

## PowerShell Parameters

```powershell
-Mode             Operation (setup|build|run|deploy|full|clean|list-devices|logcat)
-AuraSource       Path to .aura source file (for build/full)
-SdkRoot          Android SDK root (default: $HOME\.aura\android-sdk)
-AvdName          Virtual device name (default: AuraEmulator)
-ApiLevel         Android API level (default: 34, min: 24)
-Arch             Emulator architecture (default: x86_64, or arm64-v8a)
-AcceptLicenses   Accept Android licenses automatically
-Headless         Run emulator without GUI
```

## Step-by-Step Walkthrough

### 1. First-Time Setup (5-10 minutes after download)

```bash
# Install Java (if not already installed)
# Download from: https://adoptium.net/

# Then run setup (will download ~2-3 GB)
python aura_apk.py setup

# This will:
# ✓ Download Android command-line tools
# ✓ Install SDK, NDK, build-tools, emulator
# ✓ Accept Android licenses
# ✓ Create Android Virtual Device (AVD)
```

**What gets installed:**
- `~/.aura/android-sdk/` - Android SDK (8-9 GB)
- `cmdline-tools/` - Android command-line tools
- `platform-tools/` - adb, fastboot
- `build-tools/34.0.0` - Gradle integration
- `platforms/android-34` - Android API 34 (latest)
- `ndk/26.1.10909125` - Android NDK (for native code)
- `emulator/` - Android emulator binary
- `system-images/` - Emulator OS images

### 2. Build Your App

Create an Aura app or use the sample:

```aura
// my_app.aura
Box(padding: 20, bg: "white") {
  Text("Hello from Aura!"),
  Button(label: "Click Me") on_click {
    io.println("Button pressed!")
  }
}
```

Build it:

```bash
python aura_apk.py build --source my_app.aura

# Output: dist/android/AuraSample-debug.apk
```

### 3. Run on Emulator

```bash
python aura_apk.py run

# This will:
# 1. Start Android emulator (takes 30-60 sec first time)
# 2. Install APK
# 3. Launch app
# 4. Show live logs (Ctrl+C to exit)
```

### 4. Interact with App

While app is running:

```bash
# View logs in another terminal
python aura_apk.py logcat

# List devices
python aura_apk.py list-devices

# Uninstall app
adb uninstall com.aura.sentinel.sample

# Stop emulator
python aura_apk.py clean
```

## Troubleshooting

### Java not found
```
Error: Java not found. Install Java 17+
```
**Solution**: Download and install Java from https://adoptium.net/

### emulator command not found
```
Error: emulator not found
```
**Solution**: Run `python aura_apk.py setup` to install emulator

### AVD not created
```
Error: Failed to create AVD
```
**Solution**: The setup script will automatically create it. If manual creation needed:
```bash
# Show existing AVDs
adb devices

# Create manually (if needed)
sdkmanager "system-images;android-34;google_apis;x86_64"
avdmanager create avd --name AuraEmulator --package "system-images;android-34;google_apis;x86_64" --device pixel_4
```

### Emulator won't start
**Solutions:**
1. Increase available RAM (emulator needs ~2-4 GB)
2. Check CPU virtualization is enabled (BIOS setting)
3. Try different architecture:
   ```bash
   python aura_apk.py setup --api-level 34 --arch arm64-v8a
   ```
4. Check logs:
   ```bash
   cat dist/android/emulator.log
   ```

### APK installation fails
```
Error: INSTALL_FAILED_...
```
**Solutions:**
1. Ensure emulator is running:
   ```bash
   adb devices
   ```
2. Uninstall old version:
   ```bash
   adb uninstall com.aura.sentinel.sample
   ```
3. Clear cache:
   ```bash
   python aura_apk.py clean
   python aura_apk.py run
   ```

### Slow emulator performance
**Solutions:**
1. Use x86_64 architecture (faster than ARM)
2. Enable hardware acceleration (if available)
3. Increase emulator RAM in AVD config
4. Run on host OS, not virtualized OS (WSL2, VirtualBox, etc.)

## Build Output

Successful build produces:

```
dist/android/
├── AuraSample-debug.apk      # Installable app (debug)
├── AuraSample-release.apk    # Release version (if built)
├── cmdline-tools.zip         # Downloaded tools
└── emulator.log              # Emulator logs
```

## Project Structure

```
samples/android/AuraSentinelSample/
├── app/
│   ├── build.gradle.kts       # Android build config
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── assets/
│       │   └── app.aura       # Your Aura source (copied here)
│       └── java/
│           └── MainActivity   # Android activity
├── settings.gradle.kts
└── gradlew/gradlew.bat
```

When you build with `aura_apk.py build --source my_app.aura`, the script:
1. Copies `my_app.aura` → `samples/android/AuraSentinelSample/app/src/main/assets/app.aura`
2. Runs Gradle to compile
3. Outputs APK to `dist/android/AuraSample-debug.apk`

## Using Existing Android Studio Installation

If you already have Android Studio installed:

```bash
# Set environment variables
$env:ANDROID_SDK_ROOT = "C:\Android\Sdk"           # Android Studio's SDK
$env:ANDROID_NDK_HOME = "C:\Android\Sdk\ndk\26.1"  # NDK inside Android Studio

# Then build directly
python aura_apk.py build --source app.aura --sdk-root "C:\Android\Sdk"
```

## Advanced Usage

### Building Release APK

Modify the script mode to use `assembleRelease`:

```powershell
# PowerShell: add to full command
powershell ... -Mode build ... # (modify internally to use assembleRelease)
```

### Custom AVD Configuration

Create `~/.android/avd/AuraEmulator.avd/config.ini`:

```ini
hw.ramSize=4096              # 4 GB RAM
hw.gpu=yes                   # Enable GPU
hw.keyboard=yes              # Physical keyboard
disk.dataPartition.size=6GB  # Data partition size
vm.heapSize=512              # Heap size
```

Then recreate:
```bash
python aura_apk.py setup --avd-name AuraEmulator
```

### Continuous Integration

Example GitHub Actions workflow:

```yaml
name: Android APK Build
on: [push, pull_request]

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-java@v3
        with:
          java-version: '17'
          distribution: 'temurin'
      - run: python sdk/android/aura_apk.py setup
      - run: python sdk/android/aura_apk.py build --source examples/grid_image_audio.aura
      - uses: actions/upload-artifact@v3
        with:
          name: aura-apk
          path: dist/android/*.apk
```

## See Also

- [Aura ROADMAP](../../ROADMAP.md#android--mobile) - Mobile platform status
- [Aura Sentinel](../..docs/lumina-sentinel.md) - IDE integration
- [Lumina UI Guide](../../docs/lumina-ui.md) - Building UIs with Grid, Image, Audio
- [Android Developers](https://developer.android.com/) - Official Android documentation

## License

These scripts and sample are part of the Aura language project. See LICENSE in the root directory.
