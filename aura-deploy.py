#!/usr/bin/env python3
"""
Aura APK Complete Deployment System

One-command setup and deployment for Aura apps on Android.
Handles:
1. Java verification and installation guidance
2. Android SDK/NDK setup
3. Emulator AVD creation
4. APK building from Aura source
5. Deployment to emulator/device
6. Log monitoring

Usage:
    python aura-deploy.py                           # Interactive setup
    python aura-deploy.py --source app.aura        # Build & run
    python aura-deploy.py --help                   # Show options
"""

import argparse
import os
import sys
import platform
import subprocess
import time
from pathlib import Path
from typing import Optional, Tuple


class AuraDeploymentSystem:
    """Complete Aura app deployment system."""
    
    def __init__(self):
        self.repo_root = Path(__file__).parent
        self.sdk_root = Path.home() / ".aura" / "android-sdk"
        self.python_script = Path(__file__).parent / "sdk" / "android" / "aura_apk.py"
        self.ps_script = Path(__file__).parent / "sdk" / "android" / "aura-apk-emulator.ps1"
        self.os_name = platform.system()
    
    def print_banner(self):
        """Display startup banner."""
        print(r"""
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║        AURA APK EMULATOR & DEPLOYMENT SYSTEM v1.0           ║
║                                                              ║
║   Build, deploy, and run Aura apps on Android              ║
║   Complete pipeline: Setup → Build → Deploy → Run          ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝
        """)
    
    def check_prerequisites(self) -> Tuple[bool, str]:
        """Check if system has required tools."""
        checks = []
        
        # Check Windows/PowerShell
        if self.os_name != "Windows":
            return False, f"Currently requires Windows (detected: {self.os_name})"
        
        # Check PowerShell
        try:
            result = subprocess.run(
                ["powershell", "-Version"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0:
                checks.append("✓ PowerShell available")
            else:
                checks.append("✗ PowerShell not working properly")
                return False, "PowerShell 5.1+ required"
        except:
            return False, "PowerShell not found"
        
        # Check Python
        checks.append(f"✓ Python {sys.version.split()[0]}")
        
        # Check Git (optional but useful)
        try:
            subprocess.run(["git", "--version"], capture_output=True, timeout=2)
            checks.append("✓ Git available")
        except:
            checks.append("ℹ Git not found (optional)")
        
        # Check Java
        try:
            result = subprocess.run(
                ["java", "-version"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0:
                version = result.stderr.split('\n')[0]
                checks.append(f"✓ Java found ({version})")
            else:
                return False, "Java 17+ not working"
        except:
            checks.append("✗ Java not found (REQUIRED)")
            return False, (
                "Java 17+ is required.\n"
                "Download from: https://adoptium.net/\n"
                "Then retry this script."
            )
        
        # Print checks
        print("\nSystem Prerequisites:")
        for check in checks:
            print(f"  {check}")
        
        return True, ""
    
    def interactive_setup(self) -> bool:
        """Interactive setup wizard."""
        print("\n" + "="*60)
        print("INTERACTIVE SETUP WIZARD")
        print("="*60)
        
        print("""
This wizard will:
1. Install Android SDK/NDK/Emulator (~10GB, 5-15 min)
2. Create Android Virtual Device (AVD)
3. Build your first APK
4. Deploy to emulator
        """)
        
        response = input("Continue? (yes/no): ").strip().lower()
        if response not in ["yes", "y"]:
            print("Setup cancelled.")
            return False
        
        # Get source file
        source = input("\nPath to .aura source file (or press Enter for sample): ").strip()
        if not source:
            source = str(Path(__file__).parent / "examples" / "grid_image_audio.aura")
            print(f"Using sample: {source}")
        
        source_path = Path(source)
        if not source_path.exists():
            print(f"Error: File not found: {source}")
            return False
        
        # Run full deployment
        return self.deploy_full(str(source_path))
    
    def deploy_full(self, source: str) -> bool:
        """Complete deployment pipeline."""
        source_path = Path(source)
        
        if not source_path.exists():
            print(f"Error: Source file not found: {source}")
            return False
        
        print(f"\nDeploying: {source_path.name}")
        
        # Step 1: Setup (if needed)
        if not (self.sdk_root / "platform-tools").exists():
            print("\nStep 1/4: Setting up Android SDK/NDK/Emulator...")
            print("(This will download ~10GB and take 5-15 minutes)")
            
            if not self._run_ps_mode("setup"):
                return False
            
            print("✓ Setup complete")
        else:
            print("\n✓ Android SDK already installed")
        
        # Step 2: Build
        print("\nStep 2/4: Building APK...")
        if not self._run_python_build(str(source_path)):
            return False
        
        print("✓ Build complete")
        
        # Step 3: Run
        print("\nStep 3/4: Starting emulator and deploying...")
        if not self._run_ps_mode("run"):
            return False
        
        print("✓ Deployment complete")
        
        # Step 4: Done
        print("\n" + "="*60)
        print("SUCCESS!")
        print("="*60)
        print("""
Your Aura app is now running on the Android emulator.

Next steps:
- Check app output: python aura_apk.py logcat
- Rebuild app: python aura_apk.py build --source app.aura
- Stop emulator: python aura_apk.py clean
- Build release APK: (modify build script to use assembleRelease)

For more info: cat sdk/android/README.md
        """)
        
        return True
    
    def _run_python_build(self, source: str) -> bool:
        """Run Python build script."""
        cmd = [
            sys.executable,
            str(self.python_script),
            "build",
            "--source", source,
            "--sdk-root", str(self.sdk_root)
        ]
        
        try:
            result = subprocess.run(cmd, check=False)
            return result.returncode == 0
        except Exception as e:
            print(f"Error: {e}")
            return False
    
    def _run_ps_mode(self, mode: str) -> bool:
        """Run PowerShell deployment script."""
        cmd = [
            "powershell",
            "-ExecutionPolicy", "Bypass",
            "-File", str(self.ps_script),
            "-Mode", mode,
            "-SdkRoot", str(self.sdk_root),
            "-AcceptLicenses"
        ]
        
        try:
            result = subprocess.run(cmd, check=False)
            return result.returncode == 0
        except Exception as e:
            print(f"Error: {e}")
            return False
    
    def run(self, source: Optional[str] = None, interactive: bool = True):
        """Main entry point."""
        self.print_banner()
        
        # Check prerequisites
        ok, msg = self.check_prerequisites()
        if not ok:
            print(f"\n✗ Error: {msg}")
            return 1
        
        # Interactive or direct
        if source:
            if not self.deploy_full(source):
                print("\n✗ Deployment failed")
                return 1
        elif interactive:
            if not self.interactive_setup():
                return 1
        else:
            print("No source provided. Use --source or run interactively.")
            return 1
        
        return 0


def main():
    parser = argparse.ArgumentParser(
        description="Aura APK Deployment System",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Interactive setup wizard
  python aura-deploy.py

  # Deploy specific app
  python aura-deploy.py --source my_app.aura

  # Just show status
  python aura-deploy.py --status
        """
    )
    
    parser.add_argument(
        "--source", "-s",
        help="Path to .aura source file to deploy"
    )
    
    parser.add_argument(
        "--non-interactive",
        action="store_true",
        help="Skip interactive prompts (requires --source)"
    )
    
    parser.add_argument(
        "--status",
        action="store_true",
        help="Check system status and prerequisites"
    )
    
    args = parser.parse_args()
    
    system = AuraDeploymentSystem()
    
    if args.status:
        system.print_banner()
        ok, msg = system.check_prerequisites()
        if not ok:
            print(f"\n✗ Error: {msg}")
            return 1
        print("\n✓ All prerequisites met!")
        return 0
    
    return system.run(
        source=args.source,
        interactive=not args.non_interactive
    )


if __name__ == "__main__":
    sys.exit(main())
