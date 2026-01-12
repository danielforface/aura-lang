#!/usr/bin/env python3
"""
Aura APK System - Verification & Quick Start

Checks system readiness and provides next steps.
"""

import subprocess
import sys
from pathlib import Path


def check_java():
    """Verify Java installation."""
    try:
        result = subprocess.run(
            ["java", "-version"],
            capture_output=True,
            text=True,
            timeout=5
        )
        if result.returncode == 0:
            version_line = (result.stderr + result.stdout).split('\n')[0]
            return True, version_line
        return False, "Java found but not working"
    except FileNotFoundError:
        return False, "Java not found"


def check_powershell():
    """Verify PowerShell availability."""
    try:
        result = subprocess.run(
            ["powershell", "-NoProfile", "-Command", "Write-Host OK"],
            capture_output=True,
            text=True,
            timeout=5
        )
        return result.returncode == 0, "PowerShell 5.1+" if result.returncode == 0 else "PowerShell error"
    except:
        return False, "PowerShell not found"


def main():
    print("""
╔═══════════════════════════════════════════════════════════════╗
║                                                               ║
║         AURA APK EMULATOR & DEPLOYMENT SYSTEM v1.0           ║
║                   Verification & Quick Start                 ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
    """)
    
    print("\n[1/3] Checking system prerequisites...")
    
    # Check Java
    java_ok, java_msg = check_java()
    print(f"  {'✓' if java_ok else '✗'} Java: {java_msg}")
    
    # Check PowerShell
    ps_ok, ps_msg = check_powershell()
    print(f"  {'✓' if ps_ok else '✗'} PowerShell: {ps_msg}")
    
    # Check script files
    print("\n[2/3] Checking script files...")
    
    scripts = [
        ("aura-deploy.py", Path(__file__).parent / "aura-deploy.py"),
        ("aura_apk.py", Path(__file__).parent / "sdk" / "android" / "aura_apk.py"),
        ("aura-apk-emulator.ps1", Path(__file__).parent / "sdk" / "android" / "aura-apk-emulator.ps1"),
    ]
    
    all_scripts_ok = True
    for name, path in scripts:
        exists = path.exists()
        print(f"  {'✓' if exists else '✗'} {name}")
        if not exists:
            all_scripts_ok = False
    
    # Check sample project
    print("\n[3/3] Checking sample project...")
    
    sample_path = Path(__file__).parent / "samples" / "android" / "AuraSentinelSample"
    sample_ok = sample_path.exists()
    print(f"  {'✓' if sample_ok else '✗'} Sample Android project")
    
    # Print status
    print("\n" + "="*60)
    
    if java_ok and ps_ok and all_scripts_ok and sample_ok:
        print("✓ SYSTEM READY")
        print("="*60)
        print("""
Your system is ready to build and deploy Aura apps!

QUICK START:
  1. Deploy with interactive wizard:
     python aura-deploy.py

  2. Or build specific app:
     python sdk/android/aura_apk.py build --source app.aura

  3. Then deploy:
     python sdk/android/aura_apk.py run

DOCUMENTATION:
  - Full guide: cat APK_EMULATOR_COMPLETE_GUIDE.md
  - Android setup: cat sdk/android/README.md
  - Examples: cat examples/

FOR HELP:
  python aura-deploy.py --help
  python sdk/android/aura_apk.py --help
        """)
        return 0
    else:
        print("✗ SYSTEM NOT READY")
        print("="*60)
        
        if not java_ok:
            print("""
MISSING: Java 17+
ACTION: Install Java from https://adoptium.net/
  1. Download Eclipse Temurin (JDK, not JRE)
  2. Install and verify: java -version
  3. Then retry this script
        """)
        
        if not ps_ok:
            print("""
MISSING: PowerShell 5.1+
ACTION: Update PowerShell
  - Windows 11: Already included
  - Windows 10: Download from https://github.com/PowerShell/PowerShell/releases
        """)
        
        if not all_scripts_ok:
            print("""
MISSING: Script files
ACTION: Verify installation
  - Ensure sdk/android/ directory exists
  - Check git clone was complete
        """)
        
        return 1


if __name__ == "__main__":
    sys.exit(main())
