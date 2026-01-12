# 📱 Aura APK Emulator & Deployment System

**Build. Deploy. Run. Aura apps on Android in 3 commands.**

## Status: ✅ Production Ready

Complete, integrated system for building Aura applications into APKs and deploying them to Android emulators or physical devices.

## What Is This?

Aura is a programming language for verified systems. The APK Emulator system lets you:

- ✅ Build Android APKs from Aura source code
- ✅ Deploy to Android emulator (no physical device needed)
- ✅ Debug with live logcat monitoring
- ✅ Manage virtual devices and emulator lifecycle
- ✅ One-command setup of complete Android toolchain

## Three-Minute Quick Start

### Prerequisites
- Windows 10+ (native or WSL2)
- Java 17+ ([download](https://adoptium.net/))
- ~10 GB disk space (first-time setup)
- ~5 minutes for initial setup

### Step 1: Verify System

```bash
python verify-aura-apk.py
```

### Step 2: One-Command Setup

```bash
python aura-deploy.py
```

This interactive wizard will:
- Check Java installation ✓
- Download Android SDK/NDK (~10 GB)
- Install emulator and build-tools
- Create Android Virtual Device
- Optionally build and run a sample app

### Step 3: Build Your App

Create `hello.aura`:
```aura
Box(padding: 20, bg: "white") {
  Text("Hello from Aura!"),
  Button(label: "Tap") on_click { io.println("Tapped!") }
}
```

Build:
```bash
python sdk/android/aura_apk.py build --source hello.aura
```

### Step 4: Run

```bash
python sdk/android/aura_apk.py run
```

Your app is now running on the Android emulator! 🚀

## Files & Scripts

### Main Entry Points

| Script | Purpose | Audience |
|--------|---------|----------|
| **aura-deploy.py** | Interactive wizard (easiest) | First-time users |
| **verify-aura-apk.py** | System verification | Setup validation |
| **sdk/android/aura_apk.py** | Command-line builder | Developers |
| **sdk/android/aura-apk-emulator.ps1** | Low-level pipeline | PowerShell users |

### Features

#### 1. Interactive Deployment (`aura-deploy.py`)
```bash
python aura-deploy.py
```
- Guided setup wizard
- Prerequisite checking
- Automated Android SDK/NDK installation
- Virtual device creation
- One-command deployment

#### 2. Command-Line Builder (`aura_apk.py`)
```bash
# Full pipeline
python sdk/android/aura_apk.py full --source app.aura

# Individual steps
python sdk/android/aura_apk.py setup                    # Install SDK/NDK
python sdk/android/aura_apk.py build --source app.aura # Build APK
python sdk/android/aura_apk.py run                      # Deploy & run
python sdk/android/aura_apk.py logcat                   # Show logs
python sdk/android/aura_apk.py list-devices            # Show devices
python sdk/android/aura_apk.py clean                    # Stop emulator
```

#### 3. PowerShell Pipeline (`aura-apk-emulator.ps1`)
```powershell
# For advanced users and automation
powershell -ExecutionPolicy Bypass -File sdk/android/aura-apk-emulator.ps1 `
  -Mode full `
  -AuraSource my_app.aura `
  -AcceptLicenses
```

#### 4. Verification (`verify-aura-apk.py`)
```bash
python verify-aura-apk.py
```
- Check Java installation
- Verify script files
- Validate sample project
- Provide next steps

## Complete Usage Examples

### Example 1: Complete Pipeline (New User)

```bash
# 1. Verify system
python verify-aura-apk.py

# 2. First-time setup (interactive, 5-15 min)
python aura-deploy.py

# Follow prompts, builds sample app automatically
```

### Example 2: Build Custom App

```bash
# 1. Create your Aura app
echo "Box(padding: 20) { Text(\"My App\") }" > myapp.aura

# 2. Build APK
python sdk/android/aura_apk.py build --source myapp.aura

# 3. Deploy to emulator
python sdk/android/aura_apk.py run

# 4. Monitor logs
python sdk/android/aura_apk.py logcat
```

### Example 3: Non-Interactive Deployment

```bash
# Automated setup (answers prompts automatically)
python aura-deploy.py --source app.aura --non-interactive
```

### Example 4: Using Existing Android SDK

```bash
# If you have Android Studio or custom SDK
python sdk/android/aura_apk.py build \
  --source app.aura \
  --sdk-root "C:\Android\Sdk"
```

## Architecture

### Data Flow

```
app.aura (Aura source)
    ↓ (copy to assets)
samples/android/AuraSentinelSample/
    ↓ (Gradle build)
Android Build Tools (javac, aapt, dx, apkbuilder)
    ↓ (compile)
app-debug.apk (5-8 MB)
    ↓ (adb install -r)
Android Emulator / Device
    ↓ (runtime)
Aura Sentinel App
    ↓ (load app.aura from assets)
Aura Interpreter (AVM)
    ↓ (execute)
Lumina UI Runtime (Raylib/OpenGL)
    ↓ (render)
User Interface on Screen
```

### Component Stack

```
┌─────────────────────────────────────┐
│  Your Aura App (.aura source)       │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  aura-deploy.py / aura_apk.py       │  Python orchestrator
│  (Orchestrates workflow)            │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  aura-apk-emulator.ps1              │  PowerShell pipeline
│  (SDK, build, deploy, run)          │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  Gradle Build System                │  Android build tools
│  (Compile to APK)                   │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  ADB (Android Debug Bridge)         │  Device management
│  (Install, run, debug)              │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  Android Emulator (QEMU)            │  Runtime environment
│  OR Physical Device                 │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  Aura Sentinel Sample App           │  Android app
│  (MainActivity + Assets)            │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  Aura Interpreter (AVM)             │  Language runtime
│  Load + execute app.aura            │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  Lumina UI Runtime                  │  Graphics system
│  (Raylib backend, OpenGL ES 3.0)    │
└─────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────┐
│  User Interface                     │  On-screen app
│  (Grid, Image, Button, Text, etc.)  │
└─────────────────────────────────────┘
```

## Installation Details

### First-Time Setup

When you run setup, it installs:

```
~/.aura/android-sdk/          (8-9 GB)
├── cmdline-tools/            Command-line tools
├── platform-tools/           adb, fastboot
├── build-tools/34.0.0/       Gradle integration
├── platforms/android-34/     Android API 34
├── ndk/26.1.10909125/        Native development kit
├── emulator/                 Android emulator
└── system-images/            OS images for emulator

~/.android/avd/              (3-4 GB)
└── AuraEmulator.avd/        Virtual device
```

**Total: ~10-12 GB**  
**Time: 5-15 minutes** (depending on network)

## Troubleshooting

### "Java not found"
```bash
# Install Java 17+
# Download: https://adoptium.net/
# Verify: java -version
```

### "Emulator won't start"
1. Check RAM available (need 4+ GB)
2. Enable CPU virtualization in BIOS
3. Check logs: `cat dist/android/emulator.log`

### "APK won't install"
```bash
# Ensure emulator is running
python sdk/android/aura_apk.py list-devices

# Uninstall old version
adb uninstall com.aura.sentinel.sample

# Rebuild APK
python sdk/android/aura_apk.py build --source app.aura
```

See full troubleshooting guide: [APK_EMULATOR_COMPLETE_GUIDE.md](APK_EMULATOR_COMPLETE_GUIDE.md)

## Project Structure

```
aura/
├── aura-deploy.py                    ← Start here!
├── verify-aura-apk.py                ← Verify system
├── APK_EMULATOR_COMPLETE_GUIDE.md    ← Full documentation
│
├── sdk/android/
│   ├── aura_apk.py                   ← CLI tool
│   ├── aura-apk-emulator.ps1         ← PowerShell pipeline
│   ├── setup-android.ps1             ← SDK installer (legacy)
│   ├── build-apk.ps1                 ← APK builder (legacy)
│   └── README.md                      ← Detailed guide
│
├── samples/android/AuraSentinelSample/
│   ├── app/
│   │   ├── build.gradle.kts
│   │   └── src/main/
│   │       ├── AndroidManifest.xml
│   │       ├── assets/
│   │       │   └── app.aura          ← Your source goes here
│   │       └── java/
│   │           └── MainActivity.kt
│   └── settings.gradle.kts
│
└── dist/android/
    └── AuraSample-debug.apk          ← Output APK
```

## Documentation

### Quick References
- **Quick Start**: This file (README)
- **Detailed Guide**: [APK_EMULATOR_COMPLETE_GUIDE.md](APK_EMULATOR_COMPLETE_GUIDE.md)
- **Android Setup**: [sdk/android/README.md](sdk/android/README.md)

### Learning Resources
- **Aura Language**: [aura-lang.org](https://aura-lang.org)
- **Lumina UI**: [docs/lumina-ui.md](docs/lumina-ui.md)
- **Android Docs**: [developer.android.com](https://developer.android.com)
- **Aura GitHub**: [github.com/aura-lang/aura](https://github.com/aura-lang/aura)

## Features Overview

### ✅ Fully Integrated
- PowerShell deployment pipeline
- Python CLI wrapper
- Interactive wizard
- System verification

### ✅ Complete Lifecycle
- SDK/NDK auto-installation
- Virtual device creation
- APK compilation
- Emulator management
- Device deployment
- Log monitoring

### ✅ Production Ready
- Error handling
- Progress reporting
- Troubleshooting guidance
- Documentation

### 🚀 Future Enhancements
- Physical device detection
- CI/CD integration
- Release build signing
- Performance profiling
- Code hot-reload

## Support

### Getting Help

1. **Check system status:**
   ```bash
   python verify-aura-apk.py
   ```

2. **Read detailed guide:**
   ```bash
   cat APK_EMULATOR_COMPLETE_GUIDE.md
   ```

3. **Check Android SDK docs:**
   ```bash
   cat sdk/android/README.md
   ```

4. **Report issues:**
   - GitHub: [aura-lang/aura/issues](https://github.com/aura-lang/aura/issues)
   - Include: Java version, OS, error message

### Community
- **Aura Discord**: [Join](https://discord.gg/aura-lang)
- **GitHub Discussions**: [aura-lang/aura/discussions](https://github.com/aura-lang/aura/discussions)

## Version History

**v1.0 (January 12, 2026)**
- ✅ Complete SDK/NDK/emulator setup
- ✅ APK building from Aura source
- ✅ Emulator and device deployment
- ✅ Interactive and CLI workflows
- ✅ Live logging and debugging
- ✅ Comprehensive documentation

## License

Part of the Aura programming language project. See LICENSE file.

## Next Steps

🚀 **Ready to build?**

```bash
# 1. Verify system
python verify-aura-apk.py

# 2. Interactive setup
python aura-deploy.py

# 3. Start building Aura apps!
```

**Questions?** Check [APK_EMULATOR_COMPLETE_GUIDE.md](APK_EMULATOR_COMPLETE_GUIDE.md) for detailed documentation and troubleshooting.

---

**Happy coding with Aura! 🎉**
