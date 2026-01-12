#!/usr/bin/env python3
"""
Aura Android APK Build Script
Downloads Gradle, generates wrapper files, and builds APK
No Java or Gradle pre-installation required
"""

import os
import sys
import subprocess
import shutil
import zipfile
import urllib.request
from pathlib import Path
from datetime import datetime

class AndroidBuilder:
    def __init__(self):
        self.repo_root = Path(__file__).parent.resolve()
        self.gradle_version = "8.6"
        self.gradle_home = self.repo_root / "gradle" / f"gradle-{self.gradle_version}"
        self.sample_project = self.repo_root / "samples" / "android" / "AuraSentinelSample"
        self.gradle_zip = self.repo_root / f"gradle-wrapper-{self.gradle_version}.zip"
        
    def log(self, message: str, level: str = "INFO"):
        """Log a message"""
        timestamp = datetime.now().strftime("%H:%M:%S")
        colors = {
            "SUCCESS": "\033[92m",
            "INFO": "\033[94m",
            "WARN": "\033[93m",
            "ERROR": "\033[91m",
            "RESET": "\033[0m"
        }
        color = colors.get(level, colors["INFO"])
        reset = colors["RESET"]
        prefix = f"[{timestamp}] [{level}]"
        print(f"{color}{prefix}{reset} {message}")
    
    def download_gradle(self) -> bool:
        """Download Gradle binary distribution"""
        if self.gradle_home.exists():
            self.log(f"✓ Gradle {self.gradle_version} already present", level="SUCCESS")
            return True
        
        gradle_url = f"https://services.gradle.org/distributions/gradle-{self.gradle_version}-bin.zip"
        self.log(f"Downloading Gradle {self.gradle_version}...")
        self.log(f"URL: {gradle_url}")
        
        try:
            # Download with progress callback
            def download_progress(block_num, block_size, total_size):
                downloaded = block_num * block_size
                if total_size > 0:
                    percent = min(downloaded * 100 // total_size, 100)
                    mb_downloaded = downloaded / (1024 * 1024)
                    mb_total = total_size / (1024 * 1024)
                    print(f"\r  Progress: {percent}% ({mb_downloaded:.1f}/{mb_total:.1f} MB)", end="", flush=True)
            
            urllib.request.urlretrieve(gradle_url, self.gradle_zip, download_progress)
            print()  # New line after progress
            
            size_mb = self.gradle_zip.stat().st_size / (1024 * 1024)
            self.log(f"✓ Downloaded gradle-{self.gradle_version}-bin.zip ({size_mb:.1f} MB)", level="SUCCESS")
            return True
            
        except Exception as e:
            self.log(f"✗ Failed to download Gradle: {e}", level="ERROR")
            return False
    
    def extract_gradle(self) -> bool:
        """Extract Gradle archive"""
        if self.gradle_home.exists():
            self.log(f"✓ Gradle already extracted", level="SUCCESS")
            return True
        
        if not self.gradle_zip.exists():
            self.log(f"✗ Gradle zip not found: {self.gradle_zip}", level="ERROR")
            return False
        
        self.log(f"Extracting Gradle to {self.gradle_home.parent}...")
        
        try:
            extract_path = self.gradle_home.parent
            extract_path.mkdir(parents=True, exist_ok=True)
            
            with zipfile.ZipFile(self.gradle_zip, 'r') as zip_ref:
                zip_ref.extractall(extract_path)
            
            self.log(f"✓ Gradle extracted successfully", level="SUCCESS")
            
            # Verify extraction
            gradle_bin = self.gradle_home / "bin" / "gradle.bat"
            if not gradle_bin.exists():
                self.log(f"✗ Gradle binary not found after extraction", level="ERROR")
                return False
            
            # Clean up zip after successful extraction
            try:
                self.gradle_zip.unlink()
                self.log(f"Cleaned up download archive", level="INFO")
            except:
                pass
            
            return True
            
        except Exception as e:
            self.log(f"✗ Failed to extract Gradle: {e}", level="ERROR")
            return False
    
    def verify_gradle(self) -> bool:
        """Verify Gradle installation"""
        gradle_bin = self.gradle_home / "bin" / "gradle.bat"
        
        if not gradle_bin.exists():
            self.log(f"✗ Gradle binary not found: {gradle_bin}", level="ERROR")
            return False
        
        self.log(f"✓ Gradle binary verified: {gradle_bin}", level="SUCCESS")
        return True
    
    def build_apk(self) -> bool:
        """Build APK using Gradle"""
        self.log("Building Android APK...")
        
        gradle_bin = self.gradle_home / "bin" / "gradle.bat"
        
        if not gradle_bin.exists():
            self.log(f"✗ Gradle binary not found", level="ERROR")
            return False
        
        try:
            # Set GRADLE_HOME environment variable
            env = os.environ.copy()
            env['GRADLE_HOME'] = str(self.gradle_home)
            
            self.log(f"Running: {gradle_bin} assembleDebug", level="INFO")
            
            result = subprocess.run(
                [str(gradle_bin), "assembleDebug"],
                cwd=self.sample_project,
                capture_output=True,
                text=True,
                timeout=900,  # 15 minutes
                env=env
            )
            
            # Print full output for debugging
            if result.stdout:
                print(result.stdout)
            
            if result.returncode != 0:
                self.log(f"✗ APK build failed (exit code {result.returncode})", level="ERROR")
                if result.stderr:
                    print(result.stderr)
                return False
            
            self.log("✓ APK build completed", level="SUCCESS")
            
            # Verify APK exists
            apk_path = self.sample_project / "app" / "build" / "outputs" / "apk" / "debug" / "app-debug.apk"
            if apk_path.exists():
                size_mb = apk_path.stat().st_size / (1024 * 1024)
                self.log(f"✓ APK created: app-debug.apk ({size_mb:.1f} MB)", level="SUCCESS")
                return True
            else:
                self.log(f"✗ APK not found at: {apk_path}", level="WARN")
                # Check for release APK
                release_apk = self.sample_project / "app" / "build" / "outputs" / "apk" / "release" / "app-release.apk"
                if release_apk.exists():
                    size_mb = release_apk.stat().st_size / (1024 * 1024)
                    self.log(f"✓ Release APK found instead: app-release.apk ({size_mb:.1f} MB)", level="SUCCESS")
                    return True
                return False
                
        except subprocess.TimeoutExpired:
            self.log(f"✗ APK build timed out (15 minutes)", level="ERROR")
            return False
        except Exception as e:
            self.log(f"✗ APK build failed: {e}", level="ERROR")
            return False
    
    def copy_apk_to_dist(self) -> bool:
        """Copy APK to distribution directory"""
        apk_src = self.sample_project / "app" / "build" / "outputs" / "apk" / "debug" / "app-debug.apk"
        
        # Try release APK if debug doesn't exist
        if not apk_src.exists():
            apk_src = self.sample_project / "app" / "build" / "outputs" / "apk" / "release" / "app-release.apk"
        
        if not apk_src.exists():
            self.log(f"APK not found, skipping distribution copy", level="WARN")
            return True
        
        dist_android = self.repo_root / "dist-release" / "android"
        dist_android.mkdir(parents=True, exist_ok=True)
        
        apk_dest = dist_android / apk_src.name
        
        try:
            shutil.copy2(apk_src, apk_dest)
            size_mb = apk_dest.stat().st_size / (1024 * 1024)
            self.log(f"✓ APK copied to dist-release/android/ ({size_mb:.1f} MB)", level="SUCCESS")
            return True
        except Exception as e:
            self.log(f"✗ Failed to copy APK: {e}", level="ERROR")
            return False
    
    def build(self) -> int:
        """Execute complete build pipeline"""
        self.log("=" * 80)
        self.log("Aura Android APK Build - Gradle Setup & Compilation")
        self.log("=" * 80)
        self.log("")
        
        steps = [
            ("Download Gradle", self.download_gradle),
            ("Extract Gradle", self.extract_gradle),
            ("Verify Gradle", self.verify_gradle),
            ("Build APK", self.build_apk),
            ("Copy to Distribution", self.copy_apk_to_dist),
        ]
        
        failed = False
        
        for step_name, step_func in steps:
            self.log("")
            self.log(f"→ {step_name}")
            self.log("-" * 80)
            
            try:
                if not step_func():
                    self.log(f"✗ {step_name} failed", level="ERROR")
                    failed = True
                    if step_name == "Build APK":
                        break  # Stop on critical failure
            except Exception as e:
                self.log(f"✗ {step_name} raised exception: {e}", level="ERROR")
                failed = True
                if step_name == "Build APK":
                    break
        
        self.log("")
        self.log("=" * 80)
        self.log("BUILD SUMMARY")
        self.log("=" * 80)
        
        if not failed:
            self.log("✓ All steps completed successfully!", level="SUCCESS")
            dist_apk = self.repo_root / "dist-release" / "android" / "app-debug.apk"
            if dist_apk.exists():
                self.log(f"✓ Release package complete: dist-release/", level="SUCCESS")
            return 0
        else:
            self.log("⚠ One or more steps failed", level="WARN")
            self.log("See output above for details", level="WARN")
            return 1


def main():
    """Main entry point"""
    builder = AndroidBuilder()
    exit_code = builder.build()
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
