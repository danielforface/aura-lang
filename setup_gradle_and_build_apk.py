#!/usr/bin/env python3
"""
Gradle Setup Script - Downloads and configures Gradle wrapper for Android builds
"""

import os
import sys
import subprocess
import shutil
import zipfile
import urllib.request
from pathlib import Path
from datetime import datetime

class GradleSetup:
    def __init__(self):
        self.repo_root = Path(__file__).parent.resolve()
        self.gradle_version = "8.6"
        self.gradle_home = self.repo_root / "gradle" / f"gradle-{self.gradle_version}"
        self.sample_project = self.repo_root / "samples" / "android" / "AuraSentinelSample"
        self.gradle_zip = self.repo_root / f"gradle-{self.gradle_version}-bin.zip"
        
    def log(self, message: str, level: str = "INFO"):
        """Log a message"""
        timestamp = datetime.now().strftime("%H:%M:%S")
        prefix = f"[{timestamp}] [{level}]"
        print(f"{prefix} {message}")
    
    def download_gradle(self) -> bool:
        """Download Gradle binary"""
        if self.gradle_home.exists():
            self.log(f"✓ Gradle {self.gradle_version} already present", level="SUCCESS")
            return True
        
        gradle_url = f"https://services.gradle.org/distributions/gradle-{self.gradle_version}-bin.zip"
        self.log(f"Downloading Gradle {self.gradle_version} from official sources...")
        self.log(f"URL: {gradle_url}")
        
        try:
            # Download with progress
            def download_progress(block_num, block_size, total_size):
                downloaded = block_num * block_size
                percent = min(downloaded * 100 // total_size, 100)
                print(f"\r  Progress: {percent}%", end="", flush=True)
            
            urllib.request.urlretrieve(gradle_url, self.gradle_zip, download_progress)
            print()  # New line after progress
            
            self.log(f"✓ Downloaded {self.gradle_zip.name}", level="SUCCESS")
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
            
            # Clean up zip after extraction
            try:
                self.gradle_zip.unlink()
            except:
                pass
            
            return True
            
        except Exception as e:
            self.log(f"✗ Failed to extract Gradle: {e}", level="ERROR")
            return False
    
    def setup_gradle_wrapper(self) -> bool:
        """Generate Gradle wrapper files in sample project"""
        if not self.sample_project.exists():
            self.log(f"✗ Sample project not found: {self.sample_project}", level="ERROR")
            return False
        
        # Check if wrapper already exists
        gradlew = self.sample_project / "gradlew.bat"
        if gradlew.exists():
            self.log(f"✓ Gradle wrapper already exists", level="SUCCESS")
            return True
        
        self.log(f"Generating Gradle wrapper in sample project...")
        
        try:
            # Use gradle wrapper command
            gradle_bin = self.gradle_home / "bin" / "gradle"
            gradle_cmd = str(gradle_bin.with_suffix('.bat' if sys.platform == 'win32' else ''))
            
            if not Path(gradle_cmd).exists():
                self.log(f"✗ Gradle executable not found: {gradle_cmd}", level="ERROR")
                return False
            
            # Run gradle wrapper
            result = subprocess.run(
                [gradle_cmd, "wrapper", "--gradle-version", self.gradle_version],
                cwd=self.sample_project,
                capture_output=True,
                text=True,
                timeout=60
            )
            
            if result.returncode != 0:
                self.log(f"✗ Gradle wrapper generation failed: {result.stderr}", level="ERROR")
                return False
            
            # Verify wrapper was created
            if not gradlew.exists():
                self.log(f"✗ Gradle wrapper files not created", level="ERROR")
                return False
            
            self.log(f"✓ Gradle wrapper generated successfully", level="SUCCESS")
            self.log(f"  Location: {self.sample_project}", level="INFO")
            
            return True
            
        except Exception as e:
            self.log(f"✗ Error generating wrapper: {e}", level="ERROR")
            return False
    
    def verify_wrapper(self) -> bool:
        """Verify all wrapper files exist"""
        required_files = [
            self.sample_project / "gradlew.bat",
            self.sample_project / "gradlew",
            self.sample_project / "gradle" / "wrapper" / "gradle-wrapper.jar",
            self.sample_project / "gradle" / "wrapper" / "gradle-wrapper.properties",
        ]
        
        all_exist = True
        for file in required_files:
            if file.exists():
                self.log(f"✓ {file.relative_to(self.sample_project)}", level="SUCCESS")
            else:
                self.log(f"✗ {file.relative_to(self.sample_project)} - NOT FOUND", level="WARN")
                all_exist = False
        
        return all_exist
    
    def build_apk(self) -> bool:
        """Build APK using gradle wrapper"""
        self.log("Building APK...")
        
        gradlew = self.sample_project / "gradlew.bat"
        
        if not gradlew.exists():
            self.log(f"✗ gradlew.bat not found", level="ERROR")
            return False
        
        try:
            result = subprocess.run(
                [str(gradlew), "assembleDebug"],
                cwd=self.sample_project,
                capture_output=True,
                text=True,
                timeout=600  # 10 minutes
            )
            
            if result.returncode != 0:
                self.log(f"Build output:\n{result.stdout}", level="INFO")
                self.log(f"Build error:\n{result.stderr}", level="ERROR")
                return False
            
            self.log("✓ APK built successfully", level="SUCCESS")
            
            # Verify APK exists
            apk_path = self.sample_project / "app" / "build" / "outputs" / "apk" / "debug" / "app-debug.apk"
            if apk_path.exists():
                size_mb = apk_path.stat().st_size / (1024 * 1024)
                self.log(f"✓ APK created: {apk_path.name} ({size_mb:.1f} MB)", level="SUCCESS")
                return True
            else:
                self.log(f"✗ APK not found at expected location", level="WARN")
                return False
                
        except subprocess.TimeoutExpired:
            self.log(f"✗ APK build timed out (10 minutes)", level="ERROR")
            return False
        except Exception as e:
            self.log(f"✗ APK build failed: {e}", level="ERROR")
            return False
    
    def copy_apk_to_dist(self) -> bool:
        """Copy APK to distribution directory"""
        apk_src = self.sample_project / "app" / "build" / "outputs" / "apk" / "debug" / "app-debug.apk"
        
        if not apk_src.exists():
            self.log(f"APK not found, skipping distribution copy", level="WARN")
            return True
        
        dist_android = self.repo_root / "dist-release" / "android"
        dist_android.mkdir(parents=True, exist_ok=True)
        
        apk_dest = dist_android / "AuraSentinelSample-debug.apk"
        
        try:
            shutil.copy2(apk_src, apk_dest)
            size_mb = apk_dest.stat().st_size / (1024 * 1024)
            self.log(f"✓ APK copied to dist-release/android/ ({size_mb:.1f} MB)", level="SUCCESS")
            return True
        except Exception as e:
            self.log(f"✗ Failed to copy APK: {e}", level="ERROR")
            return False
    
    def setup_and_build(self) -> int:
        """Execute complete setup and build pipeline"""
        self.log("=" * 70)
        self.log("Aura Android Build - Setup & APK Generation")
        self.log("=" * 70)
        
        steps = [
            ("Download Gradle", self.download_gradle),
            ("Extract Gradle", self.extract_gradle),
            ("Generate Wrapper", self.setup_gradle_wrapper),
            ("Verify Wrapper", self.verify_wrapper),
            ("Build APK", self.build_apk),
            ("Copy to Distribution", self.copy_apk_to_dist),
        ]
        
        failed_steps = 0
        
        for step_name, step_func in steps:
            self.log("")
            self.log(f"[STEP] {step_name}...")
            self.log("-" * 70)
            
            try:
                if not step_func():
                    self.log(f"✗ {step_name} failed", level="ERROR")
                    failed_steps += 1
                    # Continue to next step for some non-critical failures
                    if step_name not in ["Build APK"]:
                        continue
                    else:
                        break  # Stop on APK build failure
            except Exception as e:
                self.log(f"✗ {step_name} raised exception: {e}", level="ERROR")
                failed_steps += 1
                if step_name == "Build APK":
                    break
        
        self.log("")
        self.log("=" * 70)
        self.log("BUILD SUMMARY")
        self.log("=" * 70)
        
        if failed_steps == 0:
            self.log("✓ All steps completed successfully!", level="SUCCESS")
            self.log(f"✓ Release package ready in: {self.repo_root / 'dist-release'}", level="SUCCESS")
            return 0
        else:
            self.log(f"⚠ {failed_steps} step(s) failed", level="WARN")
            return 1


def main():
    """Main entry point"""
    builder = GradleSetup()
    exit_code = builder.setup_and_build()
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
