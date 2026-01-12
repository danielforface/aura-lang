#!/usr/bin/env python3
"""
Aura APK Builder & Emulator Manager

Comprehensive tool to build, deploy, and run Aura applications on Android.

Usage:
    python aura_apk.py setup                          # Install SDK/NDK/emulator
    python aura_apk.py build --source app.aura        # Build APK from Aura source
    python aura_apk.py run                            # Start emulator and deploy
    python aura_apk.py full --source app.aura         # Complete pipeline
    python aura_apk.py logcat                         # Show device logs
    python aura_apk.py list-devices                   # List connected devices
"""

import argparse
import os
import subprocess
import sys
import time
import shutil
from pathlib import Path
from typing import Optional, List


class AuraApkBuilder:
    def __init__(self, sdk_root: Optional[str] = None, avd_name: str = "AuraEmulator"):
        self.repo_root = Path(__file__).parent.parent.parent
        self.sdk_root = Path(sdk_root) if sdk_root else Path.home() / ".aura" / "android-sdk"
        self.avd_name = avd_name
        self.dist_dir = self.repo_root / "dist" / "android"
        self.sample_project = self.repo_root / "samples" / "android" / "AuraSentinelSample"
        
        self.dist_dir.mkdir(parents=True, exist_ok=True)
        
        # Executables
        self.adb = self.sdk_root / "platform-tools" / "adb.exe"
        self.emulator = self.sdk_root / "emulator" / "emulator.exe"
        self.sdkmanager = self.sdk_root / "cmdline-tools" / "latest" / "bin" / "sdkmanager.bat"
        self.avdmanager = self.sdk_root / "cmdline-tools" / "latest" / "bin" / "avdmanager.bat"
    
    def _run(self, cmd: List[str], description: str = "") -> bool:
        """Run command and report result."""
        if description:
            self._print_title(description)
        
        print(f"$ {' '.join(cmd)}\n")
        
        try:
            result = subprocess.run(cmd, capture_output=False, text=True)
            if result.returncode != 0:
                self._print_error(f"Command failed with exit code {result.returncode}")
                return False
            return True
        except FileNotFoundError as e:
            self._print_error(f"Command not found: {e}")
            return False
        except Exception as e:
            self._print_error(f"Error: {e}")
            return False
    
    def _run_silent(self, cmd: List[str]) -> bool:
        """Run command silently."""
        try:
            result = subprocess.run(cmd, capture_output=True, text=True)
            return result.returncode == 0
        except:
            return False
    
    def _print_title(self, title: str):
        print(f"\n{'='*60}")
        print(f"  {title}")
        print(f"{'='*60}\n")
    
    def _print_success(self, msg: str):
        print(f"✓ {msg}")
    
    def _print_error(self, msg: str):
        print(f"✗ {msg}")
    
    def _check_java(self) -> bool:
        """Verify Java is installed."""
        cmd = ["java", "-version"]
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            self._print_error("Java not found. Install Java 17+")
            self._print_error("Download from: https://adoptium.net/")
            return False
        
        output = result.stderr + result.stdout
        print(f"Java found:\n{output}")
        return True
    
    def setup(self) -> bool:
        """Complete Android SDK/NDK/Emulator setup."""
        self._print_title("Setting up Android SDK/NDK/Emulator")
        
        # Check Java
        if not self._check_java():
            return False
        
        # This delegates to PowerShell script
        ps_script = Path(__file__).parent / "aura-apk-emulator.ps1"
        
        cmd = [
            "powershell",
            "-ExecutionPolicy", "Bypass",
            "-File", str(ps_script),
            "-Mode", "setup",
            "-SdkRoot", str(self.sdk_root),
            "-AcceptLicenses"
        ]
        
        return self._run(cmd, "Running PowerShell setup script")
    
    def build(self, source: str) -> bool:
        """Build APK from Aura source."""
        source_path = Path(source)
        if not source_path.exists():
            self._print_error(f"Source file not found: {source}")
            return False
        
        ps_script = Path(__file__).parent / "aura-apk-emulator.ps1"
        
        cmd = [
            "powershell",
            "-ExecutionPolicy", "Bypass",
            "-File", str(ps_script),
            "-Mode", "build",
            "-AuraSource", str(source_path.absolute()),
            "-SdkRoot", str(self.sdk_root)
        ]
        
        return self._run(cmd, "Building APK from Aura source")
    
    def run_emulator(self) -> bool:
        """Start emulator and deploy APK."""
        # Set environment
        os.environ["ANDROID_SDK_ROOT"] = str(self.sdk_root)
        os.environ["ANDROID_HOME"] = str(self.sdk_root)
        
        ps_script = Path(__file__).parent / "aura-apk-emulator.ps1"
        
        cmd = [
            "powershell",
            "-ExecutionPolicy", "Bypass",
            "-File", str(ps_script),
            "-Mode", "run",
            "-SdkRoot", str(self.sdk_root),
            "-AvdName", self.avd_name
        ]
        
        return self._run(cmd, "Starting emulator and deploying APK")
    
    def list_devices(self) -> bool:
        """List connected devices."""
        if not self.adb.exists():
            self._print_error("adb not found. Run setup first.")
            return False
        
        os.environ["ANDROID_SDK_ROOT"] = str(self.sdk_root)
        os.environ["PATH"] = f"{self.sdk_root / 'platform-tools'};{os.environ.get('PATH', '')}"
        
        cmd = [str(self.adb), "devices", "-l"]
        return self._run(cmd, "Connected Devices")
    
    def logcat(self) -> bool:
        """Show device logs."""
        if not self.adb.exists():
            self._print_error("adb not found. Run setup first.")
            return False
        
        os.environ["ANDROID_SDK_ROOT"] = str(self.sdk_root)
        os.environ["PATH"] = f"{self.sdk_root / 'platform-tools'};{os.environ.get('PATH', '')}"
        
        cmd = [str(self.adb), "logcat", "*:V"]
        return self._run(cmd, "Device Logcat (Ctrl+C to exit)")
    
    def clean(self) -> bool:
        """Stop emulator and clean up."""
        if not self.adb.exists():
            return True
        
        os.environ["ANDROID_SDK_ROOT"] = str(self.sdk_root)
        os.environ["PATH"] = f"{self.sdk_root / 'platform-tools'};{os.environ.get('PATH', '')}"
        
        cmd = [str(self.adb), "emu", "kill"]
        return self._run(cmd, "Stopping Emulator")
    
    def full(self, source: str) -> bool:
        """Complete pipeline: setup + build + run."""
        # Check if SDK already set up
        if not (self.sdk_root / "platform-tools").exists():
            if not self.setup():
                return False
        
        if not self.build(source):
            return False
        
        if not self.run_emulator():
            return False
        
        self._print_success("Full pipeline completed!")
        return True


def main():
    parser = argparse.ArgumentParser(
        description="Aura APK Builder & Emulator Manager",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    
    parser.add_argument(
        "command",
        choices=["setup", "build", "run", "full", "list-devices", "logcat", "clean"],
        help="Operation to perform"
    )
    
    parser.add_argument(
        "--source", "-s",
        help="Path to .aura source file (for build/full)"
    )
    
    parser.add_argument(
        "--sdk-root",
        default=str(Path.home() / ".aura" / "android-sdk"),
        help="Android SDK root directory"
    )
    
    parser.add_argument(
        "--avd-name",
        default="AuraEmulator",
        help="Android Virtual Device name"
    )
    
    args = parser.parse_args()
    
    builder = AuraApkBuilder(sdk_root=args.sdk_root, avd_name=args.avd_name)
    
    try:
        if args.command == "setup":
            success = builder.setup()
        elif args.command == "build":
            if not args.source:
                parser.error("--source required for build")
            success = builder.build(args.source)
        elif args.command == "run":
            success = builder.run_emulator()
        elif args.command == "full":
            if not args.source:
                parser.error("--source required for full")
            success = builder.full(args.source)
        elif args.command == "list-devices":
            success = builder.list_devices()
        elif args.command == "logcat":
            success = builder.logcat()
        elif args.command == "clean":
            success = builder.clean()
        else:
            parser.print_help()
            return 1
        
        return 0 if success else 1
    
    except KeyboardInterrupt:
        print("\n\nInterrupted by user")
        return 130
    except Exception as e:
        print(f"\nFatal error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
