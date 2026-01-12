# Aura v1.0 CI/CD Build Script

Automated Python script that builds and packages Aura v1.0 complete release from compiler to Android APK.

## Overview

The `build_release.py` script orchestrates a complete build pipeline:

1. **Aura Core** - Compile Rust compiler, LSP server, and package manager
2. **Sentinel IDE** - Build TypeScript/React IDE with Vite
3. **Android Setup** - Configure Gradle wrapper for APK builds
4. **Android APK** - Build debug APK of Sentinel sample app
5. **Artifacts Verification** - Confirm all binaries compiled successfully
6. **Distribution Creation** - Package all components into release directory
7. **Manifest Generation** - Create JSON manifest of all release files

## Requirements

### System Dependencies

```powershell
# Must be installed:
- Python 3.8+
- Rust 1.80+ (with cargo)
- Node.js 18+ (with npm)

# Optional (for Android APK builds):
- Gradle 8.0+ OR Android Studio
- Java Development Kit (JDK) 11+
- Android SDK with build-tools-35.0.0
```

### Quick Check

```powershell
python --version
cargo --version
npm --version
```

## Usage

### Basic Run

```powershell
cd c:\Users\danie\Documents\code\lang
python build_release.py
```

### Output

The script generates:

```
dist-release/
├── bin/                    # Compiled binaries
│   ├── aura.exe           (10.8 MB)
│   ├── aura-lsp.exe       (6.1 MB)
│   └── aura-pkg.exe       (2.4 MB)
├── apps/
│   └── sentinel/          # Web-based IDE
│       ├── assets/
│       └── index.html
├── sdk/                   # Complete SDK
│   ├── std/              # 15 stdlib modules
│   ├── docs/             # Documentation
│   ├── android/          # Android build tools
│   └── install.ps1       # Installation script
├── docs/                 # Release documentation (18 files)
├── examples/             # Example programs
├── android/              # Built APK (if available)
└── MANIFEST.json         # File inventory
```

### Build Log

Each run creates `build-release.log` with detailed build information:

```
[15:31:07] [INFO] Aura v1.0 Complete Release Build
[15:31:07] [INFO] Repository: C:\Users\danie\Documents\code\lang
...
[15:31:14] [SUCCESS] ✓ Aura core built successfully
[15:31:14] [SUCCESS] ✓ Sentinel IDE built successfully
[15:31:14] [INFO] Release package size: 19.9 MB
[15:31:14] [INFO] Location: C:\Users\danie\Documents\code\lang\dist-release
```

## Build Steps Explained

### Step 1: Aura Core (Cargo Release)

```bash
cargo build --release
```

Compiles three main executables:
- **aura.exe** - Main compiler with Z3 formal verification
- **aura-lsp.exe** - Language Server Protocol for IDE integration
- **aura-pkg.exe** - Package manager with dependency resolution

**Time**: ~44 seconds (fresh build) or <1 second (cached)

### Step 2: Sentinel IDE (Vite Build)

```bash
npm install
npm run build
```

Builds TypeScript/React web IDE:
- Installs 76 npm packages
- Transpiles TypeScript to optimized JavaScript
- Generates ~492 KB JavaScript bundle

**Time**: ~4-5 seconds

### Step 3: Android Gradle Wrapper

```bash
gradle wrapper
```

Sets up Android build system. Can be skipped if `gradlew.bat` already exists.

**Time**: ~30 seconds (if Gradle installed)

### Step 4: Android APK Build

```bash
gradlew assembleDebug
# or
gradle assembleDebug
```

Builds debug APK of Sentinel sample application for Android.

**Status**: Optional - build continues even if APK fails (Gradle not required)

### Step 5-8: Packaging & Verification

- Verifies all binaries exist with correct sizes
- Creates organized distribution directory
- Generates JSON manifest of all files
- Creates timestamped build log

## Manifest Format

The `MANIFEST.json` file in dist-release/ contains:

```json
{
  "version": "1.0.0",
  "build_date": "2026-01-11T15:31:14.123456",
  "components": {
    "bin": [
      {
        "file": "bin/aura.exe",
        "size_bytes": 11298340,
        "size_mb": 10.78
      },
      ...
    ],
    "apps": [...],
    "sdk": [...],
    "docs": [...],
    "android": [...]
  }
}
```

## Build Status Codes

### Success Indicators
- ✓ All required binaries present
- Build log created successfully
- Distribution package organized

### Warnings (Non-Fatal)
- Gradle not installed - APK builds skipped
- npm audit issues - Build continues
- Recent files section removed from IDE - Expected UI change

### Critical Failures
- Cargo compilation errors → Build fails
- npm build errors → Build fails
- Distribution verification fails → Build fails

## Environment Variables

The script respects existing environment setup:

```powershell
# Optional: Customize Rust build
$env:CARGO_PROFILE_RELEASE_OPT_LEVEL = "3"
$env:CARGO_BUILD_JOBS = "4"

# Optional: Android SDK location
$env:ANDROID_SDK_ROOT = "C:\Android\Sdk"
$env:ANDROID_HOME = "C:\Android\Sdk"
```

## Customization

Edit `build_release.py` to customize:

### Modify Build Steps

```python
steps = [
    ("Aura Core", self.build_aura_core),
    ("Sentinel IDE", self.build_sentinel_ide),
    # Add custom steps here
]
```

### Change Output Directory

```python
dist_root = self.repo_root / "my-custom-release-dir"
```

### Add Build Configuration

```python
def build_aura_core(self) -> bool:
    return self.run_command(
        ["cargo", "build", "--release", "--features", "feature-name"],
        ...
    )
```

## Troubleshooting

### Cargo Not Found
```powershell
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### npm Not Found
```powershell
# Install Node.js
# Download from https://nodejs.org/ or use winget
winget install Node.js
```

### Gradle Not Found (Optional)
```powershell
# Install Gradle or use Android Studio
# Android Studio automatically sets up Gradle
# Or: choco install gradle
```

### Unicode Error in Log

The script handles Unicode characters (✓, ✗, etc.) automatically:
- Saves logs with UTF-8 encoding
- Compatible with PowerShell 5.1+

### Build Timeout

Increase timeout in `build_release.py`:

```python
timeout=600  # 10 minutes instead of 5
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Build Aura Release

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: python build_release.py
      - uses: actions/upload-artifact@v3
        with:
          name: aura-release
          path: dist-release/
```

### GitLab CI Example

```yaml
build_release:
  image: rust:latest
  stage: build
  script:
    - apt-get update && apt-get install -y python3 nodejs npm
    - python3 build_release.py
  artifacts:
    paths:
      - dist-release/
    expire_in: 30 days
```

## Performance Metrics

### Build Times (Sample)
| Component | Time | Cache | Notes |
|-----------|------|-------|-------|
| Aura Core | 44s | <1s | Rust release mode (-O3) |
| Sentinel IDE | 5s | 2s | Vite incremental build |
| Android APK | 60s | N/A | Requires Gradle |
| **Total** | **~110s** | **~7s** | Complete pipeline |

### Output Sizes
| Component | Size | Notes |
|-----------|------|-------|
| aura.exe | 10.8 MB | Compiler + Z3 + LLVM |
| aura-lsp.exe | 6.1 MB | Language server |
| aura-pkg.exe | 2.4 MB | Package manager |
| Sentinel IDE | 0.5 MB | Web bundle (gzip) |
| SDK | ~5 MB | 15 stdlib modules + docs |
| **Total Release** | **19.9 MB** | All components |

## Distribution Contents

### Binaries (bin/)
- **aura.exe** - Main Aura compiler
- **aura-lsp.exe** - IDE language server
- **aura-pkg.exe** - Package manager

### IDE (apps/sentinel/)
- **index.html** - Web app entry point
- **assets/index-*.js** - Bundled TypeScript code
- **assets/index-*.css** - Compiled styles

### SDK (sdk/)
- **std/** - 15 standard library modules
  - core, collections, crypto, net, json, etc.
- **docs/** - SDK documentation
- **android/** - Android build configuration
- **install.ps1** - Windows installer script

### Documentation (docs/)
- Release notes and changelogs
- Architecture and design documents
- Feature guides and examples
- 18 comprehensive markdown files (~50 KB total)

### Examples (examples/)
- Sample Aura programs
- Integration examples
- Best practices demonstrations

## Next Steps

1. **Install** - Run `sdk/install.ps1` to set up development environment
2. **Verify** - Test binaries: `bin\aura.exe --version`
3. **IDE** - Open `apps\sentinel\index.html` in browser
4. **Learn** - Read documentation in `docs/` and `sdk/docs/`
5. **Build** - Create Aura projects with `aura new` or `aura init`

## Support

For issues or contributions:
- Check `build-release.log` for detailed error messages
- Review build script output for warnings
- Verify system dependencies are installed
- Consult `docs/` for feature and API documentation

---

**Last Updated**: January 11, 2026
**Aura Version**: v1.0.0
**Status**: Production Ready (A+)
