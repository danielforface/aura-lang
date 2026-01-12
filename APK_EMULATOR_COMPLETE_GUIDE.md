# Aura APK Emulator & Deployment Complete System

**Status**: ✅ Production-Ready
**Version**: 1.0
**Last Updated**: January 12, 2026

## Overview

Complete end-to-end system for building, deploying, and running Aura applications on Android devices and emulators.

### What You Get

✅ Automatic Android SDK/NDK installation  
✅ Android Virtual Device (emulator) setup  
✅ APK compilation from Aura source  
✅ One-click deployment to emulator or device  
✅ Real-time app logging and debugging  
✅ Python and PowerShell scripting options  

## System Architecture

```
User App (app.aura)
    ↓
Aura Compiler (aura-core)
    ↓
Gradle Build System
    ↓
Android APK Package
    ↓
ADB Installation
    ↓
Android Emulator / Device
    ↓
Aura Sentinel Sample Runtime
```

## Installation & Files

### Core Components

| File | Purpose | Language |
|------|---------|----------|
| `aura-deploy.py` | One-command deployment wizard | Python |
| `aura_apk.py` | APK builder and manager | Python |
| `aura-apk-emulator.ps1` | Full build/deploy/run pipeline | PowerShell |
| `setup-android.ps1` | SDK/NDK installer (legacy) | PowerShell |
| `build-apk.ps1` | APK builder (legacy) | PowerShell |

### Sample Project

- `samples/android/AuraSentinelSample/` - Complete Android project template
  - Gradle build configuration
  - Android manifest
  - MainActivity (Kotlin)
  - Asset bundling for `.aura` source files

## Quick Start (3 Commands)

### 1️⃣ First-Time Setup (5-15 minutes)

```bash
# One-time installation of Android SDK/NDK/emulator
python aura-deploy.py
```

This interactive wizard will:
- Verify Java installation
- Download Android SDK/NDK (~10 GB)
- Install build-tools and emulator
- Create an Android Virtual Device
- Optionally build and run a sample app

### 2️⃣ Build Your App

```bash
# Build APK from Aura source
python sdk/android/aura_apk.py build --source my_app.aura
```

Produces: `dist/android/AuraSample-debug.apk`

### 3️⃣ Run on Emulator

```bash
# Start emulator, deploy, and launch
python sdk/android/aura_apk.py run
```

Your app is now live on the Android emulator!

## Usage Examples

### Interactive Deployment (Easiest)

```bash
# Guided setup with all options
python aura-deploy.py
```

### Command-Line Deployment

```bash
# Setup only
python sdk/android/aura_apk.py setup

# Build APK
python sdk/android/aura_apk.py build --source app.aura

# Deploy and run
python sdk/android/aura_apk.py run

# Complete pipeline
python sdk/android/aura_apk.py full --source app.aura

# Manage running device
python sdk/android/aura_apk.py list-devices
python sdk/android/aura_apk.py logcat        # Show live logs
python sdk/android/aura_apk.py clean         # Stop emulator
```

### PowerShell Usage

```powershell
# Setup
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode setup -AcceptLicenses

# Build
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode build -AuraSource my_app.aura

# Run
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode run

# Full
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 -Mode full -AuraSource my_app.aura -AcceptLicenses
```

## Building Your First App

### Step 1: Create Aura App

Create `hello.aura`:

```aura
Box(padding: 20, bg: "white", border: "#ccc", radius: 8) {
  Text("🎉 Hello from Aura!"),
  Text("Running on Android"),
  
  Button(label: "Tap Me") on_click {
    io.println("Button tapped!")
  }
}
```

### Step 2: Build APK

```bash
python sdk/android/aura_apk.py build --source hello.aura
```

Output:
```
dist/android/AuraSample-debug.apk (5-8 MB)
```

### Step 3: Deploy & Run

```bash
python sdk/android/aura_apk.py run
```

Your app appears on the Android emulator! 🚀

### Step 4: Monitor Logs

In another terminal:
```bash
python sdk/android/aura_apk.py logcat
```

Shows real-time app output, errors, and debug messages.

## Architecture Details

### Component Breakdown

#### 1. **aura-deploy.py** (Entry Point)
- Interactive deployment wizard
- System prerequisite checking
- Orchestrates complete workflow
- User-friendly prompts and guidance

#### 2. **aura_apk.py** (Python CLI)
- Cross-platform build system
- Wraps PowerShell for Windows
- Device management (list, logcat, clean)
- Build configuration options

#### 3. **aura-apk-emulator.ps1** (Core Pipeline)
- Downloads Android command-line tools
- Installs SDK, NDK, build-tools via sdkmanager
- Creates Android Virtual Device via avdmanager
- Builds APK using Gradle
- Manages emulator lifecycle
- Deploys via adb

#### 4. **Sample Project** (Gradle + Kotlin)
- Stock Android application template
- Integrates with Aura compiler
- Bundles `.aura` source in assets
- Loads and executes via Aura runtime

### Data Flow

```
my_app.aura
    ↓
Copy to: samples/android/.../assets/app.aura
    ↓
Gradle ./gradlew assembleDebug
    ↓
Android Build Tools (javac, aapt, dx, apkbuilder)
    ↓
app-debug.apk (5-8 MB)
    ↓
adb install -r app-debug.apk
    ↓
Android Emulator / Device
    ↓
Aura Sentinel Sample App
    ↓
Loads app.aura from assets
    ↓
Parses and executes Aura code
    ↓
Renders UI with Lumina (Raylib/OpenGL)
```

## Installation Details

### First-Time Setup: What Gets Installed

When you run `python aura-deploy.py` and confirm setup:

#### Directory Structure (10 GB total)

```
~/.aura/android-sdk/                    # Root (8-9 GB)
├── cmdline-tools/
│   └── latest/
│       ├── bin/
│       │   ├── sdkmanager.bat
│       │   ├── avdmanager.bat
│       │   └── [other tools]
│       └── lib/
├── platform-tools/                     # 150 MB
│   ├── adb.exe
│   ├── fastboot.exe
│   └── [platform-specific binaries]
├── build-tools/
│   └── 34.0.0/                         # 200 MB
│       ├── aapt.exe
│       ├── dx.bat
│       └── [gradle integration]
├── platforms/
│   └── android-34/                     # 1 GB
│       ├── android.jar
│       └── system resources
├── ndk/
│   └── 26.1.10909125/                  # 2 GB
│       ├── toolchains/
│       ├── prebuilt/
│       └── libraries
├── emulator/                           # 400 MB
│   ├── emulator.exe
│   ├── qemu/                           # QEMU binaries
│   └── lib/
├── system-images/
│   └── android-34/google_apis/x86_64/  # 3 GB
│       ├── system.img
│       ├── ramdisk.img
│       └── [OS images for emulator]
└── aura-android-env.ps1                # Environment helper

~/.android/avd/                         # Virtual Devices
└── AuraEmulator.avd/
    ├── config.ini
    ├── hardware-qemu.ini
    ├── system.img
    └── cache.img
```

#### Installation Time

| Component | Size | Download Time | Install Time |
|-----------|------|---------------|--------------|
| cmdline-tools | 800 MB | 2-3 min | 1 min |
| platform-tools | 150 MB | 1 min | 1 min |
| build-tools | 200 MB | 1-2 min | 1 min |
| platforms (API 34) | 1 GB | 3-5 min | 2 min |
| NDK | 2 GB | 5-10 min | 2 min |
| Emulator | 400 MB | 2-3 min | 1 min |
| System Images | 3 GB | 5-10 min | 1 min |
| **TOTAL** | **~10 GB** | **20-35 min** | **10-15 min** |

**First-time total: 5-15 minutes** (depending on network speed)

## How It Works: Under the Hood

### 1. SDK Installation Process

```powershell
# Download Android command-line tools
Invoke-WebRequest -Uri $CmdlineToolsUrl -OutFile cmdline-tools.zip

# Extract
Expand-Archive cmdline-tools.zip -DestinationPath $SdkRoot

# Use sdkmanager to install packages
sdkmanager --sdk_root=$SdkRoot "platform-tools"
sdkmanager --sdk_root=$SdkRoot "platforms;android-34"
sdkmanager --sdk_root=$SdkRoot "build-tools;34.0.0"
sdkmanager --sdk_root=$SdkRoot "ndk;26.1.10909125"
sdkmanager --sdk_root=$SdkRoot "emulator"
sdkmanager --sdk_root=$SdkRoot "system-images;android-34;google_apis;x86_64"
```

### 2. AVD Creation

```bash
# Create Android Virtual Device using avdmanager
avdmanager create avd \
  --name AuraEmulator \
  --package "system-images;android-34;google_apis;x86_64" \
  --device "pixel_4" \
  --force
```

Creates: `~/.android/avd/AuraEmulator.avd/`

### 3. APK Build (Gradle)

```bash
cd samples/android/AuraSentinelSample/

# Build with Gradle Wrapper
./gradlew assembleDebug

# OR use system gradle
gradle assembleDebug
```

Steps:
1. Compile Kotlin → bytecode
2. Process resources (AndroidManifest.xml, layouts, assets)
3. Package DEX (Dalvik Executable)
4. Create APK (ZIP archive)
5. Sign (debug certificate)

Output: `app/build/outputs/apk/debug/app-debug.apk`

### 4. Deployment (adb)

```bash
# Install APK on emulator
adb install -r dist/android/AuraSample-debug.apk

# Launch app
adb shell am start -n "com.aura.sentinel.sample/.MainActivity"

# Monitor logs
adb logcat *:V
```

### 5. Runtime Execution

```
Android Emulator boots
    ↓
MainActivity.onCreate() called
    ↓
Loads app.aura from APK assets
    ↓
Aura Sentinel runtime parses Aura code
    ↓
Calls Aura interpreter (AVM)
    ↓
Renders UI with Lumina (Raylib backend)
    ↓
Listens for touch events
    ↓
Executes on_click callbacks
```

## Troubleshooting

### ❌ "Java not found"

```
Error: Java not found. Install Java 17+
```

**Solution:**
1. Download Java 17+: https://adoptium.net/
2. Install (choose JDK, not JRE)
3. Verify: `java -version`
4. Set JAVA_HOME (if installer didn't):
   ```powershell
   $env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-17.0.5"
   ```
5. Retry: `python aura-deploy.py`

### ❌ "Emulator won't start"

**Symptoms:**
```
Error: Emulator failed to boot within timeout
```

**Solutions:**
1. Check available RAM: Need ~4 GB free
2. Enable CPU virtualization in BIOS:
   - Restart → BIOS/UEFI → Enable "VT-x" (Intel) or "AMD-V" (AMD)
3. Check emulator logs:
   ```bash
   cat dist/android/emulator.log
   ```
4. Try ARM instead of x86_64:
   ```powershell
   powershell ... -Arch arm64-v8a
   ```

### ❌ "APK installation fails"

```
Error: INSTALL_FAILED_INVALID_APK
```

**Solutions:**
1. Ensure emulator is running:
   ```bash
   python sdk/android/aura_apk.py list-devices
   ```
2. Clear old installation:
   ```bash
   adb uninstall com.aura.sentinel.sample
   ```
3. Rebuild APK:
   ```bash
   python sdk/android/aura_apk.py build --source app.aura
   ```

### ❌ "Gradle not found"

```
Error: Neither gradlew.bat nor 'gradle' found
```

**Solutions:**
1. Gradle Wrapper should be in sample project
2. If missing, install Gradle:
   ```powershell
   choco install gradle  # or download from gradle.org
   ```
3. Or use Android Studio (includes Gradle)

### ❌ "ANDROID_SDK_ROOT not set"

**Solution:**
```powershell
$env:ANDROID_SDK_ROOT = "$HOME\.aura\android-sdk"
python sdk/android/aura_apk.py build --source app.aura
```

## Advanced Topics

### Custom Emulator Configuration

Edit `~/.android/avd/AuraEmulator.avd/config.ini`:

```ini
# Increase RAM
hw.ramSize=4096

# Enable hardware acceleration
hw.gpu=yes

# Use hardware keyboard
hw.keyboard=yes

# Increase data partition
disk.dataPartition.size=6GB

# Increase heap size
vm.heapSize=512
```

Rebuild: `python aura-deploy.py`

### Building Release APK

Modify PowerShell script mode to use `assembleRelease`:

```powershell
& $gradlew assembleRelease  # Instead of assembleDebug
```

Signs with default debug key. For production, create keystore:

```bash
keytool -genkey -v -keystore release.keystore -alias AuraKey -keyalg RSA -keysize 2048 -validity 10000
```

### Using Android Studio SDK

If you have Android Studio installed:

```bash
# Point to Android Studio's SDK
python sdk/android/aura_apk.py build --source app.aura --sdk-root "C:\Android\Sdk"
```

### Continuous Integration (GitHub Actions)

```yaml
name: Build Android APK

on: [push, pull_request]

jobs:
  android:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - uses: actions/setup-java@v3
        with:
          java-version: '17'
          distribution: 'temurin'
      
      - name: Setup Android SDK
        run: python aura-deploy.py --non-interactive --source examples/grid_image_audio.aura
      
      - name: Build APK
        run: python sdk/android/aura_apk.py build --source examples/grid_image_audio.aura
      
      - uses: actions/upload-artifact@v3
        with:
          name: android-apk
          path: dist/android/*.apk
```

## File Reference

### aura-deploy.py

**Entry point for new users.** Interactive setup wizard with:
- Prerequisite checking
- Step-by-step guidance
- Error handling and suggestions
- Complete deployment orchestration

**Usage:**
```bash
python aura-deploy.py              # Interactive
python aura-deploy.py --source app.aura --non-interactive  # Automated
python aura-deploy.py --status     # Check system
```

### aura_apk.py

**Command-line tool for builders.** Supports:
- setup: Install Android SDK/NDK
- build: Compile Aura → APK
- run: Deploy and execute
- full: Complete pipeline
- list-devices: Show connected devices
- logcat: Monitor logs
- clean: Stop emulator

**Usage:**
```bash
python aura_apk.py [command] [options]
```

### aura-apk-emulator.ps1

**Low-level PowerShell pipeline.** Modes:
- setup: SDK/NDK/AVD installation
- build: Gradle compilation
- run: Emulator + deployment
- deploy: APK installation only
- full: Complete workflow

**Usage:**
```powershell
powershell -ExecutionPolicy Bypass -File aura-apk-emulator.ps1 -Mode [mode] [options]
```

## Performance Tuning

### Emulator Speed

1. **Use x86_64 architecture** (faster than ARM):
   ```bash
   python aura-deploy.py  # Default is x86_64
   ```

2. **Enable hardware acceleration:**
   - Update `config.ini`: `hw.gpu=yes`

3. **Allocate more RAM:**
   - Update `config.ini`: `hw.ramSize=4096`

4. **Use host OS instead of virtualization:**
   - Don't run inside VirtualBox, VMware, or WSL2
   - Run on native Windows for best performance

5. **Reduce graphics quality** (if needed):
   - Update `config.ini`: `hw.initialOrientation=0`

### APK Build Speed

1. **Use Gradle daemon:**
   ```bash
   gradle build --daemon
   ```

2. **Enable parallel compilation:**
   ```bash
   gradle build --parallel
   ```

3. **Cache builds:**
   - Gradle caches by default
   - Use: `python aura_apk.py build --source app.aura` (reuses cache)

## Deployment Models

### Model 1: Emulator (Development)

```
Your Machine
  ↓
Android Emulator (QEMU)
  ↓
Your Aura App

Pros: Fast, no device needed, full debugger access
Cons: Slower than hardware, requires virtualization
```

### Model 2: Physical Device (Testing)

```
Your Machine
  ↓ USB/WiFi (adb)
Physical Android Phone/Tablet
  ↓
Your Aura App

Pros: Real hardware, true performance, user experience
Cons: Requires device, USB drivers, developer mode
```

### Model 3: Cloud Device Farm (CI/CD)

```
GitHub Actions
  ↓
Cloud Device Farm (BrowserStack, AWS Device Farm)
  ↓
Physical Devices (100+)
  ↓
APK Testing

Pros: Scale, real devices, automated testing
Cons: Cost, setup complexity
```

## Monitoring & Debugging

### Logcat (App Logs)

```bash
# All messages, maximum verbosity
python aura_apk.py logcat

# Filter by tag
adb logcat *:V | grep "aura"

# Save to file
adb logcat > app.log

# Clear logs
adb logcat -c

# Kill and restart
adb logcat -c && adb shell am start -n com.aura.sentinel.sample/.MainActivity
```

### Device State

```bash
# Check device connectivity
python aura_apk.py list-devices

# Get device properties
adb shell getprop

# Check boot status
adb shell getprop sys.boot_completed

# Get API level
adb shell getprop ro.build.version.sdk
```

### Performance Profiling

```bash
# CPU usage
adb shell dumpsys cpuinfo

# Memory usage
adb shell dumpsys meminfo com.aura.sentinel.sample

# Battery stats
adb shell dumpsys batteryusage
```

## FAQ

**Q: Do I need Android Studio?**  
A: No. This system uses command-line tools. Android Studio optional if you want GUI IDE.

**Q: Can I use a physical phone instead of emulator?**  
A: Yes. Enable USB debugging, connect, then `python aura_apk.py run`.

**Q: How do I uninstall everything?**  
A: Delete `~/.aura/android-sdk/` and `~/.android/avd/`. Python/PS scripts unchanged.

**Q: Can I target older Android versions?**  
A: Yes. Modify `-ApiLevel` parameter (min 24). Download that platform via sdkmanager.

**Q: Is it free?**  
A: Yes. Android SDK, NDK, emulator all open-source and free.

**Q: What about iOS?**  
A: Not yet. Aura prioritizes Android. iOS support planned for future release.

**Q: Can I use different emulator (like Genymotion)?**  
A: Yes. Build APK normally, then deploy to any Android emulator via adb.

## Support & Resources

- **Documentation**: [aura-lang.org](https://aura-lang.org)
- **GitHub Issues**: Report bugs at [aura-lang/aura](https://github.com/aura-lang/aura)
- **Aura ROADMAP**: See [ROADMAP.md](../../ROADMAP.md#android--mobile)
- **Android Docs**: [developer.android.com](https://developer.android.com)

## Version History

### v1.0 (January 12, 2026)
- ✅ Complete SDK/NDK installation
- ✅ AVD creation and management
- ✅ APK building from Aura source
- ✅ Emulator deployment
- ✅ Device logging and monitoring
- ✅ Python CLI and PowerShell pipeline
- ✅ Interactive deployment wizard

## License

Part of the Aura programming language project. See LICENSE in repository root.

---

**Ready to deploy your Aura app?** 

```bash
python aura-deploy.py
```

Start building! 🚀
