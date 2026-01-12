#!/usr/bin/env python3
"""
Aura Android - Complete Setup & Build (Post-Java Installation)
Run this AFTER installing Java (Android Studio or JDK)
"""

import subprocess
import sys
import os
from pathlib import Path

def main():
    print("=" * 80)
    print("Aura Android - APK Build Pipeline")
    print("=" * 80)
    print()
    
    # Check Java is installed
    print("[*] Checking Java installation...")
    try:
        result = subprocess.run(
            ["java", "-version"],
            capture_output=True,
            text=True,
            timeout=10
        )
        if result.returncode == 0:
            print("[OK] Java found:")
            for line in result.stderr.split('\n')[:2]:
                if line.strip():
                    print(f"     {line}")
        else:
            print("[ERROR] Java not found. Please install:")
            print("  - Android Studio: https://developer.android.com/studio")
            print("  - Or JDK: https://www.oracle.com/java/")
            return 1
    except FileNotFoundError:
        print("[ERROR] Java command not found in PATH")
        print("Please install Java or set JAVA_HOME environment variable")
        return 1
    
    print()
    print("[*] Building complete release package...")
    print()
    
    # Run build_release.py
    script_path = Path(__file__).parent / "build_release.py"
    
    result = subprocess.run(
        [sys.executable, str(script_path)],
        cwd=Path(__file__).parent
    )
    
    if result.returncode == 0:
        print()
        print("=" * 80)
        print("[OK] Build completed successfully!")
        print("=" * 80)
        print()
        print("Release package location:")
        print(f"  {Path(__file__).parent / 'dist-release'}")
        print()
        print("Files generated:")
        dist = Path(__file__).parent / "dist-release"
        if dist.exists():
            print(f"  - Compiler binaries: dist-release/bin/")
            print(f"  - Sentinel IDE: dist-release/apps/sentinel/")
            print(f"  - SDK: dist-release/sdk/")
            print(f"  - Documentation: dist-release/docs/")
            if (dist / "android").exists():
                apk_files = list((dist / "android").glob("*.apk"))
                if apk_files:
                    for apk in apk_files:
                        size_mb = apk.stat().st_size / (1024 * 1024)
                        print(f"  - APK: dist-release/android/{apk.name} ({size_mb:.1f} MB)")
        print()
        return 0
    else:
        print()
        print("[ERROR] Build failed. See output above for details.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
