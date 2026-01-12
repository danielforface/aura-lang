#!/usr/bin/env python3
"""build_release.py

Aura v1.0 Complete CI/CD Build Script

Builds compiler, IDE, Android APK, and generates complete release package.

This script is designed to be runnable on a fresh Windows machine by
bootstrapping missing Android build dependencies (portable JDK, Android SDK
command-line tools, Gradle wrapper) into a local .tools/ directory.
"""

import os
import sys
import subprocess
import shutil
import json
import urllib.request
import zipfile
import re
import argparse
from pathlib import Path
from datetime import datetime
from typing import Optional, List, Tuple

class AuraBuilder:
    def __init__(self):
        self.repo_root = Path(__file__).parent.resolve()
        self.build_log = []
        self.start_time = datetime.now()
        self.failed_steps = []
        self.tools_root = self.repo_root / ".tools"
        self.tools_root.mkdir(parents=True, exist_ok=True)

        # Toolchain state (populated by ensure_* helpers)
        self.java_home: Optional[Path] = None
        self.android_sdk_root: Optional[Path] = None
        
    def log(self, message: str, level: str = "INFO"):
        """Log a message with timestamp"""
        timestamp = datetime.now().strftime("%H:%M:%S")
        prefix = f"[{timestamp}] [{level}]"
        log_entry = f"{prefix} {message}"
        # Encode for Windows console compatibility
        try:
            print(log_entry)
        except UnicodeEncodeError:
            # Replace Unicode characters with ASCII equivalents
            safe_entry = log_entry.replace("✓", "[OK]").replace("✗", "[FAIL]")
            print(safe_entry)
        self.build_log.append(log_entry)
        
    def log_section(self, title: str):
        """Log a major section header"""
        separator = "=" * 70
        self.log("")
        self.log(separator)
        self.log(f"  {title}")
        self.log(separator)
        
    def _step_failed(self, step_name: str, error: str):
        self.failed_steps.append((step_name, error))

    def run_command(
        self,
        cmd: List[str] | str,
        cwd: Optional[Path] = None,
        description: str = "",
        env: Optional[dict] = None,
        timeout_sec: int = 300,
    ) -> Tuple[bool, str]:
        """Run a shell command and return success status and output.

        Notes:
        - Uses shell=True for Windows compatibility with .bat/.ps1.
        - Accepts optional env overrides (merged with current env).
        """
        if cwd is None:
            cwd = self.repo_root

        display_cmd = cmd if isinstance(cmd, str) else " ".join(cmd)
        self.log(f"Running: {display_cmd}")
        if description:
            self.log(f"  ({description})")

        merged_env = os.environ.copy()
        if env:
            merged_env.update({k: str(v) for k, v in env.items() if v is not None})

        try:
            result = subprocess.run(
                display_cmd,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout_sec,
                shell=True,
                env=merged_env,
            )
            
            if result.returncode != 0:
                error_msg = result.stderr if result.stderr else result.stdout
                self.log(f"FAILED: {error_msg}", level="ERROR")
                self.failed_steps.append((description or display_cmd, error_msg))
                return False, error_msg
            
            output = result.stdout
            if output:
                self.log(f"Output: {output[:200]}")
            return True, output
            
        except subprocess.TimeoutExpired:
            error_msg = f"Command timed out ({timeout_sec} seconds)"
            self.log(error_msg, level="ERROR")
            self.failed_steps.append((description or display_cmd, error_msg))
            return False, error_msg
        except Exception as e:
            error_msg = str(e)
            self.log(error_msg, level="ERROR")
            self.failed_steps.append((description or display_cmd, error_msg))
            return False, error_msg

    def _which(self, exe_name: str) -> Optional[str]:
        return shutil.which(exe_name)

    def _download_file(self, url: str, dest: Path) -> None:
        dest.parent.mkdir(parents=True, exist_ok=True)
        self.log(f"Downloading: {url}")
        self.log(f"  -> {dest}")
        req = urllib.request.Request(
            url,
            headers={
                # Some CDNs block requests without a UA.
                "User-Agent": "AuraBuild/1.0 (+https://example.invalid)",
                "Accept": "*/*",
            },
        )
        with urllib.request.urlopen(req) as r, open(dest, "wb") as f:
            shutil.copyfileobj(r, f)

    def _extract_zip(self, zip_path: Path, dest_dir: Path) -> None:
        dest_dir.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(zip_path, "r") as z:
            z.extractall(dest_dir)

    def ensure_jdk(self) -> bool:
        """Ensure a working JDK is available for Gradle/Android builds.

        If JAVA_HOME/java already exists, uses it. Otherwise downloads a portable
        Temurin JDK zip into .tools/ and uses it for all subsequent commands.
        """
        self.log_section("ANDROID DEP: Java (JDK)")

        java_home_env = os.environ.get("JAVA_HOME")
        if java_home_env:
            java_bin = Path(java_home_env) / "bin" / ("java.exe" if os.name == "nt" else "java")
            if java_bin.exists():
                self.java_home = Path(java_home_env)
                self.log(f"✓ Using JAVA_HOME: {self.java_home}", level="SUCCESS")
                return True

        if self._which("java"):
            self.log("✓ Using java from PATH", level="SUCCESS")
            return True

        # Download a portable JDK
        # Users can override with AURA_JDK_URL (zip).
        override = os.environ.get("AURA_JDK_URL")
        url_candidates: List[str] = [
            override,
            # Temurin/Adoptium API (may require a User-Agent header)
            "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse",
            # Microsoft Build of OpenJDK (versioned; may change over time)
            "https://aka.ms/download-jdk/microsoft-jdk-17.0.10-windows-x64.zip",
            "https://aka.ms/download-jdk/microsoft-jdk-17.0.9-windows-x64.zip",
        ]
        url_candidates = [u for u in url_candidates if u]

        cache_dir = self.tools_root / "cache"
        jdk_zip = cache_dir / "temurin-jdk17.zip"
        jdk_root = self.tools_root / "jdk-17"

        try:
            if not jdk_zip.exists():
                last_err: Optional[Exception] = None
                for url in url_candidates:
                    try:
                        self._download_file(url, jdk_zip)
                        last_err = None
                        break
                    except Exception as e:
                        last_err = e
                        self.log(f"Download failed: {e}", level="WARN")
                        if jdk_zip.exists():
                            try:
                                jdk_zip.unlink()
                            except Exception:
                                pass
                if last_err is not None:
                    # As a final fallback, try winget install (Windows).
                    if os.name == "nt" and self._which("winget"):
                        self.log("Trying winget to install JDK...", level="WARN")
                        ok, _ = self.run_command(
                            "winget install -e --id Microsoft.OpenJDK.17 --accept-source-agreements --accept-package-agreements",
                            cwd=self.repo_root,
                            description="Install Microsoft OpenJDK 17 via winget",
                            timeout_sec=1800,
                        )
                        if ok and self._which("java"):
                            self.log("✓ Installed JDK via winget", level="SUCCESS")
                            return True
                    raise last_err

            if not jdk_root.exists():
                tmp = self.tools_root / "_tmp_jdk_extract"
                if tmp.exists():
                    shutil.rmtree(tmp)
                self._extract_zip(jdk_zip, tmp)

                # Find the top-level extracted folder (jdk-17.*)
                candidates = [p for p in tmp.iterdir() if p.is_dir()]
                if len(candidates) != 1:
                    # pick first directory if layout is unexpected
                    candidates = sorted(candidates, key=lambda p: p.name)
                if not candidates:
                    raise RuntimeError("Unexpected JDK zip layout (no directory found)")

                extracted_home = candidates[0]
                shutil.move(str(extracted_home), str(jdk_root))
                shutil.rmtree(tmp, ignore_errors=True)

            java_bin = jdk_root / "bin" / "java.exe"
            if not java_bin.exists():
                raise RuntimeError(f"Downloaded JDK missing java.exe: {java_bin}")

            self.java_home = jdk_root
            self.log(f"✓ Bootstrapped JDK: {self.java_home}", level="SUCCESS")
            return True
        except Exception as e:
            self.log(f"Failed to bootstrap JDK: {e}", level="ERROR")
            return False

    def _android_env(self) -> dict:
        env: dict = {}
        if self.java_home:
            env["JAVA_HOME"] = str(self.java_home)
            # Ensure our JDK wins for child processes
            env["PATH"] = str(self.java_home / "bin") + os.pathsep + os.environ.get("PATH", "")

        if self.android_sdk_root:
            env["ANDROID_SDK_ROOT"] = str(self.android_sdk_root)
            env["ANDROID_HOME"] = str(self.android_sdk_root)

        return env

    def ensure_android_sdk(self) -> bool:
        """Ensure Android SDK + cmdline-tools are installed.

        Uses the repo's PowerShell setup script (sdk/android/setup-android.ps1)
        and installs into .tools/android-sdk unless overridden.
        """
        self.log_section("ANDROID DEP: Android SDK")

        override_root = os.environ.get("AURA_ANDROID_SDK_ROOT")
        sdk_root = Path(override_root).resolve() if override_root else (self.tools_root / "android-sdk")
        sdkmanager = sdk_root / "cmdline-tools" / "latest" / "bin" / "sdkmanager.bat"

        # Accept an existing SDK if user set it.
        existing = os.environ.get("ANDROID_SDK_ROOT") or os.environ.get("ANDROID_HOME")
        if existing:
            existing_root = Path(existing)
            existing_sdkmanager = existing_root / "cmdline-tools" / "latest" / "bin" / "sdkmanager.bat"
            if existing_sdkmanager.exists():
                self.android_sdk_root = existing_root
                self.log(f"✓ Using existing Android SDK: {self.android_sdk_root}", level="SUCCESS")
                return True

        # If already installed in our tools dir, use it.
        if sdkmanager.exists():
            self.android_sdk_root = sdk_root
            self.log(f"✓ Android SDK present: {self.android_sdk_root}", level="SUCCESS")
            return True

        # Bootstrap via the repo PowerShell script.
        setup_ps1 = self.repo_root / "sdk" / "android" / "setup-android.ps1"
        if not setup_ps1.exists():
            self.log(f"Missing setup script: {setup_ps1}", level="ERROR")
            return False

        if not self.ensure_jdk():
            return False

        self.log(f"Installing Android SDK into: {sdk_root}")
        cmd = (
            f'powershell -ExecutionPolicy Bypass -File "{setup_ps1}" '
            f'-InstallRoot "{sdk_root}" -AcceptLicenses'
        )
        success, _ = self.run_command(
            cmd,
            cwd=self.repo_root,
            description="Install Android SDK via setup-android.ps1",
            env=self._android_env(),
            timeout_sec=3600,
        )
        if not success:
            return False

        if not sdkmanager.exists():
            self.log(f"Android SDK install completed but sdkmanager not found: {sdkmanager}", level="ERROR")
            return False

        self.android_sdk_root = sdk_root
        self.log("✓ Android SDK installed", level="SUCCESS")
        return True

    def ensure_gradle_distribution(self) -> Optional[Path]:
        """Ensure a Gradle distribution exists under gradle/gradle-8.6/.

        Returns the path to gradle.bat.
        """
        gradle_home = self.repo_root / "gradle" / "gradle-8.6"
        gradle_bat = gradle_home / "bin" / "gradle.bat"
        if gradle_bat.exists():
            return gradle_bat

        url = os.environ.get(
            "AURA_GRADLE_ZIP_URL",
            "https://services.gradle.org/distributions/gradle-8.6-bin.zip",
        )
        cache_dir = self.tools_root / "cache"
        gradle_zip = cache_dir / "gradle-8.6-bin.zip"

        try:
            self.log_section("ANDROID DEP: Gradle (8.6)")
            if not gradle_zip.exists():
                self._download_file(url, gradle_zip)

            gradle_parent = self.repo_root / "gradle"
            gradle_parent.mkdir(parents=True, exist_ok=True)
            tmp = self.tools_root / "_tmp_gradle_extract"
            if tmp.exists():
                shutil.rmtree(tmp)
            self._extract_zip(gradle_zip, tmp)

            extracted = tmp / "gradle-8.6"
            if not extracted.exists():
                # Some zips include version folder; if not, pick the only folder.
                candidates = [p for p in tmp.iterdir() if p.is_dir()]
                if candidates:
                    extracted = candidates[0]

            if gradle_home.exists():
                shutil.rmtree(gradle_home)
            shutil.move(str(extracted), str(gradle_home))
            shutil.rmtree(tmp, ignore_errors=True)

            if gradle_bat.exists():
                self.log("✓ Gradle 8.6 ready", level="SUCCESS")
                return gradle_bat

            self.log(f"Gradle extraction succeeded but gradle.bat not found: {gradle_bat}", level="ERROR")
            return None
        except Exception as e:
            self.log(f"Failed to bootstrap Gradle: {e}", level="ERROR")
            return None

    def ensure_gradle_wrapper(self) -> bool:
        """Ensure Gradle wrapper exists for the Android sample."""
        self.log_section("ANDROID DEP: Gradle Wrapper")

        sample_project = self.repo_root / "samples" / "android" / "AuraSentinelSample"
        gradlew = sample_project / "gradlew.bat"
        wrapper_jar = sample_project / "gradle" / "wrapper" / "gradle-wrapper.jar"
        wrapper_props = sample_project / "gradle" / "wrapper" / "gradle-wrapper.properties"

        if gradlew.exists() and wrapper_jar.exists() and wrapper_props.exists():
            self.log("✓ Gradle wrapper already present", level="SUCCESS")
            return True

        if not self.ensure_jdk():
            return False

        gradle_bat = self.ensure_gradle_distribution()
        if not gradle_bat:
            return False

        cmd = f'"{gradle_bat}" wrapper'
        success, _ = self.run_command(
            cmd,
            cwd=sample_project,
            description="Generate Gradle wrapper (gradlew.bat)",
            env=self._android_env(),
            timeout_sec=600,
        )

        if not success:
            return False

        if not gradlew.exists():
            self.log("Wrapper task succeeded but gradlew.bat still missing", level="ERROR")
            return False

        self.log("✓ Gradle wrapper generated", level="SUCCESS")
        return True

    def reorg_repo_root(self, dry_run: bool = False) -> bool:
        """Reorganize repo-root docs/samples into subfolders.

        Goal: keep the repository root clean.

        - Moves root-level *.md into docs/root-md/ (except ROADMAP.md)
        - Moves root-level *.aura into examples/root-aura/

        This is intentionally opt-in (run via step key `reorg-root`).

        IMPORTANT: This will break any hardcoded links pointing to old paths.
        """
        self.log_section("REPO: Reorganize Root")

        # Default: keep README + ROADMAP in root.
        keep_md = {"README.md", "ROADMAP.md"}

        # Optional: explicit allowlist of root markdown files to keep.
        # Example: AURA_KEEP_ROOT_MD="README.md,ROADMAP.md,CHANGELOG.md"
        keep_env = os.environ.get("AURA_KEEP_ROOT_MD")
        if keep_env:
            keep_md = {x.strip() for x in keep_env.split(",") if x.strip()}

        docs_dest = self.repo_root / "docs" / "root-md"
        aura_dest = self.repo_root / "examples" / "root-aura"
        docs_dest.mkdir(parents=True, exist_ok=True)
        aura_dest.mkdir(parents=True, exist_ok=True)

        moved_md: List[tuple[str, str]] = []
        moved_aura: List[tuple[str, str]] = []

        for p in sorted(self.repo_root.glob("*.md"), key=lambda x: x.name.lower()):
            if p.name in keep_md:
                continue
            dest = docs_dest / p.name
            moved_md.append((p.name, str(dest.relative_to(self.repo_root)).replace("\\", "/")))
            if not dry_run:
                if dest.exists():
                    try:
                        dest.unlink()
                    except Exception:
                        pass
                shutil.move(str(p), str(dest))

        for p in sorted(self.repo_root.glob("*.aura"), key=lambda x: x.name.lower()):
            dest = aura_dest / p.name
            moved_aura.append((p.name, str(dest.relative_to(self.repo_root)).replace("\\", "/")))
            if not dry_run:
                if dest.exists():
                    try:
                        dest.unlink()
                    except Exception:
                        pass
                shutil.move(str(p), str(dest))

        # Write an index for what we moved.
        index = docs_dest / "INDEX.md"
        lines = [
            "# Root Markdown Index",
            "",
            f"Generated: {datetime.now().isoformat()}",
            "",
            "This folder contains markdown files that were previously located at the repository root.",
            "",
            "## Moved markdown",
            "",
        ]
        if moved_md:
            for src, dest in moved_md:
                lines.append(f"- `{src}` → `{dest}`")
        else:
            lines.append("- (none)")

        lines += ["", "## Moved Aura samples", ""]
        if moved_aura:
            for src, dest in moved_aura:
                lines.append(f"- `{src}` → `{dest}`")
        else:
            lines.append("- (none)")

        if not dry_run:
            index.write_text("\n".join(lines) + "\n", encoding="utf-8")
            self.log(f"✓ Wrote {index.relative_to(self.repo_root)}", level="SUCCESS")
        else:
            self.log("(dry-run) Not moving files", level="WARN")

        self.log(f"✓ Root MD moved: {len(moved_md)}", level="SUCCESS")
        self.log(f"✓ Root .aura moved: {len(moved_aura)}", level="SUCCESS")
        return True
    
    def build_aura_core(self) -> bool:
        """Build Aura compiler and tools with Cargo"""
        self.log_section("STEP 1: Building Aura Core (Cargo Release)")
        
        success, _ = self.run_command(
            ["cargo", "build", "--release"],
            cwd=self.repo_root,
            description="Compile Aura compiler, LSP, and package manager"
        )
        
        if success:
            self.log("✓ Aura core built successfully", level="SUCCESS")
        return success
    
    def build_sentinel_ide(self) -> bool:
        """Build Sentinel IDE with Vite"""
        self.log_section("STEP 2: Building Sentinel IDE (Vite)")
        
        sentinel_app = self.repo_root / "editors" / "sentinel-app"
        
        # Install dependencies
        self.log("Installing Node dependencies...")
        success, _ = self.run_command(
            ["npm", "install"],
            cwd=sentinel_app,
            description="Install npm packages"
        )
        
        if not success:
            return False
        
        # Build with Vite
        success, output = self.run_command(
            ["npm", "run", "build"],
            cwd=sentinel_app,
            description="Build Sentinel IDE with Vite"
        )
        
        if success:
            self.log("✓ Sentinel IDE built successfully", level="SUCCESS")
        return success

    def collect_markdown_docs(self) -> bool:
        """Collect markdown docs into a single directory inside dist-release.

        This does not move or rewrite originals. It produces:
        - dist-release/docs/all-md/<flattened files>
        - dist-release/docs/ALL_MARKDOWN_INDEX.md
        - dist-release/docs/UPDATES.md
        - dist-release/docs/summaries/<flattened summary files>
        - dist-release/docs/SUMMARY_INDEX.md
        - dist-release/docs/SUMMARIES_OVERVIEW.md
        """
        self.log_section("DOCS: Aggregating Markdown")

        dist_root = self.repo_root / "dist-release"
        dist_docs = dist_root / "docs"
        all_md_dir = dist_docs / "all-md"
        all_md_dir.mkdir(parents=True, exist_ok=True)

        # Exclude dependency/build/output dirs (very important on this repo)
        excluded_dir_names = {
            ".git",
            ".github",
            ".vscode",
            ".tools",
            "target",
            "target-alt",
            "target-lsp-test",
            "target-lsp-tests",
            "node_modules",
            "dist",
            "dist-release",
            "dist-complete",
            "build",
            "vendor",
            ".gradle",
        }

        md_sources: List[Path] = []
        for md in self.repo_root.rglob("*.md"):
            rel = md.relative_to(self.repo_root)
            # Skip anything under excluded dirs
            if any(part in excluded_dir_names for part in rel.parts):
                continue
            # Skip extremely deep Android SDK/JDK docs if user ran toolchain setup
            if rel.parts and rel.parts[0].startswith("."):
                continue
            md_sources.append(md)

        if not md_sources:
            self.log("No markdown files found to aggregate", level="WARN")
            return True

        # Copy into one folder with flattened names to avoid collisions.
        copied: List[tuple[str, str]] = []
        for src in sorted(md_sources, key=lambda p: str(p.relative_to(self.repo_root)).lower()):
            rel = src.relative_to(self.repo_root).as_posix()
            flat_name = rel.replace("/", "__")
            dest = all_md_dir / flat_name
            try:
                shutil.copy2(src, dest)
                copied.append((rel, f"docs/all-md/{flat_name}"))
            except Exception as e:
                self.log(f"Failed to copy {rel}: {e}", level="WARN")

        # Generate index
        index_path = dist_docs / "ALL_MARKDOWN_INDEX.md"
        lines = [
            "# All Markdown Files (Aggregated)",
            "",
            f"Generated: {datetime.now().isoformat()}",
            "",
            "This is a flattened copy of markdown files from the repo (excluding build/deps folders).",
            "",
            "## Files",
            "",
        ]
        for rel, dest_rel in copied:
            lines.append(f"- `{rel}` → `{dest_rel}`")

        index_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        self.log(f"✓ Wrote {index_path.relative_to(self.repo_root)}", level="SUCCESS")

        # Generate updates list (heuristic)
        update_keywords = re.compile(r"(UPDATE|STATUS|REPORT|SUMMARY|COMPLETE|COMPLETION|RELEASE|ANNOUNCEMENT)", re.IGNORECASE)
        update_files = [
            (rel, dest_rel)
            for (rel, dest_rel) in copied
            if update_keywords.search(Path(rel).name)
        ]

        def sort_key(item: tuple[str, str]):
            name = Path(item[0]).name
            # Prefer YYYY_MM_DD patterns
            m = re.search(r"(20\d{2})[_-](\d{2})[_-](\d{2})", name)
            if m:
                return (0, m.group(1), m.group(2), m.group(3), name.lower())
            # Then month-year patterns like JANUARY-2026
            m2 = re.search(r"(JANUARY|FEBRUARY|MARCH|APRIL|MAY|JUNE|JULY|AUGUST|SEPTEMBER|OCTOBER|NOVEMBER|DECEMBER)[-_ ](20\d{2})", name, re.IGNORECASE)
            if m2:
                return (1, m2.group(2), m2.group(1).lower(), name.lower())
            return (2, name.lower())

        updates_path = dist_docs / "UPDATES.md"
        u_lines = [
            "# Updates / Reports (Curated List)",
            "",
            "Heuristic list of status/update/release/report markdown files.",
            "",
            "## Entries",
            "",
        ]
        for rel, dest_rel in sorted(update_files, key=sort_key):
            u_lines.append(f"- `{rel}` → `{dest_rel}`")
        updates_path.write_text("\n".join(u_lines) + "\n", encoding="utf-8")
        self.log(f"✓ Wrote {updates_path.relative_to(self.repo_root)}", level="SUCCESS")

        # Collect summary/report/status markdowns into a dedicated folder + write overview
        summaries_dir = dist_docs / "summaries"
        summaries_dir.mkdir(parents=True, exist_ok=True)

        summary_name_re = re.compile(
            r"(SUMMARY|REPORT|STATUS|COMPLETE|COMPLETION|FINAL|RELEASE|DELIVERY|ANNOUNCEMENT|INDEX|GUIDE|CHECKLIST|DASHBOARD)",
            re.IGNORECASE,
        )

        # Prefer repo-root summaries and phase summaries; exclude generic docs like reference/book chapters.
        summary_candidates: List[tuple[str, str]] = []
        for rel, dest_rel in copied:
            name = Path(rel).name
            if name.lower() in {"readme.md", "license.md", "code_of_conduct.md", "contributing.md"}:
                continue
            if summary_name_re.search(name) or rel.startswith("PHASE_") or rel.startswith("WEEK_"):
                summary_candidates.append((rel, dest_rel))

        # Copy summaries into summaries/
        summary_map: List[tuple[str, str]] = []
        for rel, _ in summary_candidates:
            flat_name = rel.replace("/", "__")
            dest = summaries_dir / flat_name
            try:
                shutil.copy2(self.repo_root / rel, dest)
                summary_map.append((rel, f"docs/summaries/{flat_name}"))
            except Exception as e:
                self.log(f"Failed to copy summary {rel}: {e}", level="WARN")

        def extract_md_summary(src_path: Path) -> dict:
            content = src_path.read_text(encoding="utf-8", errors="replace")
            title = "(untitled)"
            for line in content.splitlines():
                if line.startswith("#"):
                    title = line.lstrip("#").strip()
                    break

            def find_field(pattern: str) -> Optional[str]:
                m = re.search(pattern, content, flags=re.IGNORECASE | re.MULTILINE)
                if m:
                    return m.group(1).strip()
                return None

            date = find_field(r"\*\*Date\*\*\s*:\s*(.+)$") or find_field(r"\*\*Created\*\*\s*:\s*(.+)$")
            status = find_field(r"\*\*Status\*\*\s*:\s*(.+)$")
            nxt = find_field(r"\*\*Next\*\*\s*:\s*(.+)$")

            # First paragraph (best-effort)
            para_lines: List[str] = []
            in_para = False
            for line in content.splitlines():
                stripped = line.strip()
                if stripped.startswith("#"):
                    continue
                if not stripped:
                    if in_para:
                        break
                    continue
                if stripped.startswith("---"):
                    continue
                # Skip obvious metadata lines
                if re.match(r"\*\*(Date|Status|Next|Created)\*\*\s*:", stripped, flags=re.IGNORECASE):
                    continue
                if stripped.startswith("-"):
                    # Stop before long bullet sections
                    if in_para:
                        break
                    continue
                in_para = True
                para_lines.append(stripped)
                if len(para_lines) >= 3:
                    break

            intro = " ".join(para_lines).strip() if para_lines else ""
            return {"title": title, "date": date, "status": status, "next": nxt, "intro": intro}

        summary_index = dist_docs / "SUMMARY_INDEX.md"
        idx_lines = [
            "# Summary Documents (Index)",
            "",
            f"Generated: {datetime.now().isoformat()}",
            "",
            "These are the repo's summary/report/status/checklist style documents, copied into `docs/summaries/`.",
            "",
            "## Files",
            "",
        ]
        for rel, dest_rel in sorted(summary_map, key=lambda x: x[0].lower()):
            idx_lines.append(f"- `{rel}` → `{dest_rel}`")
        summary_index.write_text("\n".join(idx_lines) + "\n", encoding="utf-8")
        self.log(f"✓ Wrote {summary_index.relative_to(self.repo_root)}", level="SUCCESS")

        overview = dist_docs / "SUMMARIES_OVERVIEW.md"
        ov_lines = [
            "# Summaries Overview", 
            "",
            f"Generated: {datetime.now().isoformat()}",
            "",
            "This file provides a quick, human-readable overview of the key summary documents.",
            "",
        ]
        for rel, dest_rel in sorted(summary_map, key=lambda x: x[0].lower()):
            meta = extract_md_summary(self.repo_root / rel)
            ov_lines.append(f"## {meta['title']}")
            ov_lines.append("")
            ov_lines.append(f"- Source: `{rel}`")
            ov_lines.append(f"- Copy: `{dest_rel}`")
            if meta.get("date"):
                ov_lines.append(f"- Date: {meta['date']}")
            if meta.get("status"):
                ov_lines.append(f"- Status: {meta['status']}")
            if meta.get("next"):
                ov_lines.append(f"- Next: {meta['next']}")
            if meta.get("intro"):
                ov_lines.append("")
                ov_lines.append(meta["intro"])
            ov_lines.append("")

        overview.write_text("\n".join(ov_lines) + "\n", encoding="utf-8")
        self.log(f"✓ Wrote {overview.relative_to(self.repo_root)}", level="SUCCESS")

        self.log(f"✓ Aggregated {len(copied)} markdown files", level="SUCCESS")
        return True

    def rearrange_roadmap_summary(self) -> bool:
        """Add a structured Mission/Features/Accomplishments summary to ROADMAP.md.

        This keeps the existing detailed checklist below; it only inserts a new
        top summary block after the Legend section to make the roadmap readable.
        """
        self.log_section("DOCS: Roadmap Restructure")
        roadmap = self.repo_root / "ROADMAP.md"
        if not roadmap.exists():
            self.log("ROADMAP.md not found; skipping", level="WARN")
            return True

        text = roadmap.read_text(encoding="utf-8", errors="replace")
        marker_start = "<!-- ROADMAP_SUMMARY:START -->"
        marker_end = "<!-- ROADMAP_SUMMARY:END -->"

        # Remove ALL existing summary blocks (including accidental duplicates), then re-insert exactly one.
        summary_pattern = re.escape(marker_start) + r".*?" + re.escape(marker_end) + r"\s*"
        text = re.sub(summary_pattern, "", text, flags=re.DOTALL)

        summary_lines = [
            marker_start,
            '## Aura v1.0 ("Reliability") — Certainty without Sacrifice',
            "",
            "### Mission (מטרה)",
            "",
            "Aura exists to make **formal verification the default UX**: proofs stream in real time (target: sub-200ms feedback), failures are explainable, and trust boundaries are explicit.",
            "",
            "Aura is the “Third Way”: ",
            "- C/Zig-level control and performance",
            "- Rust-level safety with simpler day-to-day ergonomics (Region + Linear Types)",
            "- Dafny/Lean-level certainty, without leaving production tooling behind",
            "",
            "### Feature Missions (Next) (משימות)",
            "",
            "- [ ] Z3 Gate proof streaming: keep verifier feedback under interactive latency (p95 < 200ms)",
            "- [ ] High-speed Merkle caching: statement/function-level incremental proofs with stable keys",
            "- [ ] Memory Model Option B: regions + linear ownership states (Owned / Borrowed / Consumed) codified end-to-end (typechecker + stdlib)",
            "- [ ] Trusted Core boundary: generate + surface a Trusted Core Report (\"דוח ליבה מהימנה\") on every build",
            "- [ ] Explain Engine (\"הסבר\"): unsat-core driven explanation + variable trace + concrete counterexample rendered in-editor",
            "- [ ] Race-free concurrency: static happens-before + protection mapping to prevent data races and deadlocks by construction",
            "- [ ] Differential backend testing: CI Trust Gate keeping Dev-VM and native backends behaviorally aligned",
            "",
            "### Past Accomplishments (הישגים)",
            "",
            "- [x] Explainable verification UX: typed counterexamples, variable traces, logic traces",
            "- [x] Package Manager: aura-pkg v1.0 complete and production-tested",
            "- [x] Deterministic release packaging and artifact manifests",
            "",
            "### Specs & Indices",
            "",
            "- v1.0 Reliability spec: [docs/v1.0-reliability.md](docs/v1.0-reliability.md)",
            "- Release summaries overview: [dist-release/docs/SUMMARIES_OVERVIEW.md](dist-release/docs/SUMMARIES_OVERVIEW.md)",
            "",
            marker_end,
            "",
        ]
        summary_block = "\n".join(summary_lines)

        # Insert after Legend section (after the first horizontal rule line).
        hr = re.search(r"(?m)^\s*---\s*$", text)
        if hr:
            insert_at = hr.end()
            text = text[:insert_at] + "\n\n" + summary_block + text[insert_at:].lstrip()
        else:
            text = summary_block + "\n" + text

        roadmap.write_text(text, encoding="utf-8")
        self.log("✓ Updated ROADMAP.md with summary structure", level="SUCCESS")
        return True
    
    def setup_android_toolchain(self) -> bool:
        """Bootstrap Android build prerequisites (JDK, Android SDK, Gradle wrapper)."""
        self.log_section("STEP 3: Bootstrapping Android Toolchain")

        if os.environ.get("AURA_SKIP_ANDROID") == "1":
            self.log("AURA_SKIP_ANDROID=1 set; skipping Android toolchain setup", level="WARN")
            return True

        if not self.ensure_jdk():
            return False

        if not self.ensure_android_sdk():
            return False

        if not self.ensure_gradle_wrapper():
            return False

        return True
    
    def build_android_apk(self) -> bool:
        """Build Android APK"""
        self.log_section("STEP 4: Building Android APK")

        if os.environ.get("AURA_SKIP_ANDROID") == "1":
            self.log("AURA_SKIP_ANDROID=1 set; skipping APK build", level="WARN")
            return True

        # Ensure toolchain first.
        if not self.setup_android_toolchain():
            return False

        build_ps1 = self.repo_root / "sdk" / "android" / "build-apk.ps1"
        if not build_ps1.exists():
            self.log(f"Missing build script: {build_ps1}", level="ERROR")
            return False

        sdk_root = self.android_sdk_root or (self.tools_root / "android-sdk")

        variants_env = os.environ.get("AURA_ANDROID_VARIANTS", "debug")
        variants = [v.strip().lower() for v in variants_env.split(",") if v.strip()]
        if not variants:
            variants = ["debug"]

        ok_all = True
        for variant in variants:
            if variant not in {"debug", "release"}:
                self.log(f"Unknown Android variant: {variant} (use debug,release)", level="ERROR")
                ok_all = False
                continue

            cmd = (
                f'powershell -ExecutionPolicy Bypass -File "{build_ps1}" '
                f'-SdkRoot "{sdk_root}" -BuildType "{variant}"'
            )
            success, _ = self.run_command(
                cmd,
                cwd=self.repo_root,
                description=f"Build APK ({variant}) via sdk/android/build-apk.ps1",
                env=self._android_env(),
                timeout_sec=3600,
            )
            if not success:
                ok_all = False

        return ok_all
    
    def copy_android_apk_to_dist(self) -> bool:
        """Copy APK to dist directory"""
        self.log_section("STEP 5: Packaging Android APK")
        
        sample_project = self.repo_root / "samples" / "android" / "AuraSentinelSample"
        apk_src = sample_project / "app" / "build" / "outputs" / "apk" / "debug" / "app-debug.apk"
        
        if not apk_src.exists():
            if os.environ.get("AURA_SKIP_ANDROID") == "1":
                self.log("APK not found (Android skipped)", level="WARN")
                return True
            self.log("APK not found; Android build did not produce output", level="ERROR")
            return False
        
        dist_android = self.repo_root / "dist" / "android"
        dist_android.mkdir(parents=True, exist_ok=True)
        
        apk_dest = dist_android / "AuraSentinelSample-debug.apk"
        try:
            shutil.copy2(apk_src, apk_dest)
            self.log(f"✓ APK copied to dist/android/", level="SUCCESS")
            return True
        except Exception as e:
            self.log(f"Failed to copy APK: {e}", level="WARN")
            return True
    
    def verify_binaries(self) -> bool:
        """Verify all expected binaries exist"""
        self.log_section("STEP 6: Verifying Build Artifacts")
        
        binaries = [
            "target/release/aura.exe",
            "target/release/aura-lsp.exe",
            "target/release/aura-pkg.exe",
        ]
        
        all_exist = True
        for binary in binaries:
            binary_path = self.repo_root / binary
            if binary_path.exists():
                size_mb = binary_path.stat().st_size / (1024 * 1024)
                self.log(f"✓ {binary} ({size_mb:.1f} MB)", level="SUCCESS")
            else:
                self.log(f"✗ {binary} NOT FOUND", level="ERROR")
                all_exist = False
        
        return all_exist
    
    def create_distribution(self) -> bool:
        """Create complete distribution package"""
        self.log_section("STEP 7: Creating Distribution Package")
        
        dist_root = self.repo_root / "dist-release"
        dist_root.mkdir(parents=True, exist_ok=True)
        
        # Create structure
        dirs = [
            dist_root / "bin",
            dist_root / "apps",
            dist_root / "sdk",
            dist_root / "docs",
            dist_root / "examples",
            dist_root / "android",
        ]
        
        for d in dirs:
            d.mkdir(parents=True, exist_ok=True)
        
        # Copy binaries
        binaries = [
            ("target/release/aura.exe", "bin/"),
            ("target/release/aura-lsp.exe", "bin/"),
            ("target/release/aura-pkg.exe", "bin/"),
        ]
        
        for src_rel, dest_dir in binaries:
            src = self.repo_root / src_rel
            if src.exists():
                dest = dist_root / dest_dir / src.name
                shutil.copy2(src, dest)
                self.log(f"Copied {src.name}")
        
        # Copy Sentinel IDE build
        sentinel_dist = self.repo_root / "editors" / "sentinel-app" / "dist"
        if sentinel_dist.exists():
            sentinel_app_dest = dist_root / "apps" / "sentinel"
            if sentinel_app_dest.exists():
                shutil.rmtree(sentinel_app_dest)
            shutil.copytree(sentinel_dist, sentinel_app_dest)
            self.log("Copied Sentinel IDE")
        
        # Copy SDK
        sdk_src = self.repo_root / "sdk"
        if sdk_src.exists():
            sdk_dest = dist_root / "sdk"
            if sdk_dest.exists():
                shutil.rmtree(sdk_dest)
            shutil.copytree(sdk_src, sdk_dest)
            self.log("Copied SDK")
        
        # Copy docs
        docs_files = list((self.repo_root / "docs").glob("*.md")) if (self.repo_root / "docs").exists() else []
        for doc in docs_files:
            shutil.copy2(doc, dist_root / "docs" / doc.name)
            self.log(f"Copied {doc.name}")
        
        # Copy APK if it exists
        apk = self.repo_root / "dist" / "android" / "AuraSentinelSample-debug.apk"
        if apk.exists():
            shutil.copy2(apk, dist_root / "android" / apk.name)
            self.log("Copied Android APK")
        
        self.log(f"✓ Distribution created in {dist_root}", level="SUCCESS")
        return True
    
    def generate_manifest(self) -> bool:
        """Generate manifest of all release files"""
        self.log_section("STEP 8: Generating Release Manifest")
        
        dist_root = self.repo_root / "dist-release"
        manifest = {
            "version": "1.0.0",
            "build_date": datetime.now().isoformat(),
            "components": {}
        }
        
        # Collect file information
        for root, dirs, files in os.walk(dist_root):
            for file in files:
                file_path = Path(root) / file
                rel_path = file_path.relative_to(dist_root)
                size = file_path.stat().st_size
                
                category = rel_path.parts[0] if rel_path.parts else "root"
                if category not in manifest["components"]:
                    manifest["components"][category] = []
                
                manifest["components"][category].append({
                    "file": str(rel_path).replace("\\", "/"),
                    "size_bytes": size,
                    "size_mb": round(size / (1024 * 1024), 2)
                })
        
        # Write manifest
        manifest_file = dist_root / "MANIFEST.json"
        with open(manifest_file, "w") as f:
            json.dump(manifest, f, indent=2)
        
        self.log(f"✓ Manifest generated: {manifest_file.name}", level="SUCCESS")
        return True
    
    def print_summary(self):
        """Print build summary"""
        self.log_section("BUILD SUMMARY")
        
        elapsed = datetime.now() - self.start_time
        hours, remainder = divmod(elapsed.seconds, 3600)
        minutes, seconds = divmod(remainder, 60)
        duration = f"{hours}h {minutes}m {seconds}s" if hours else f"{minutes}m {seconds}s"
        
        self.log(f"Build completed in: {duration}")
        
        if self.failed_steps:
            self.log(f"Failed steps: {len(self.failed_steps)}", level="WARN")
            for step, error in self.failed_steps:
                self.log(f"  - {step}: {error[:100]}", level="WARN")
        else:
            self.log("All steps completed successfully!", level="SUCCESS")
        
        dist_root = self.repo_root / "dist-release"
        if dist_root.exists():
            total_size = sum(f.stat().st_size for f in dist_root.rglob("*") if f.is_file())
            self.log(f"Release package size: {total_size / (1024*1024):.1f} MB")
            self.log(f"Location: {dist_root}")
    
    def save_build_log(self):
        """Save build log to file"""
        log_file = self.repo_root / "build-release.log"
        with open(log_file, "w", encoding="utf-8") as f:
            f.write("\n".join(self.build_log))
        self.log(f"Build log saved: {log_file}")
    
    def build_all(self) -> int:
        """Execute complete build pipeline"""
        self.log("Aura v1.0 Complete Release Build")
        self.log(f"Repository: {self.repo_root}")
        self.log(f"Started: {self.start_time.strftime('%Y-%m-%d %H:%M:%S')}")
        
        steps = [
            ("Aura Core", self.build_aura_core),
            ("Sentinel IDE", self.build_sentinel_ide),
            ("Android Setup", self.setup_android_toolchain),
            ("Android APK", self.build_android_apk),
            ("APK Packaging", self.copy_android_apk_to_dist),
            ("Binary Verification", self.verify_binaries),
            ("Distribution Creation", self.create_distribution),
            ("Manifest Generation", self.generate_manifest),
        ]
        
        failed_count = 0
        for step_name, step_func in steps:
            try:
                if not step_func():
                    self.log(f"✗ {step_name} failed", level="ERROR")
                    # Some failures happen without a subprocess (e.g., missing deps).
                    self.failed_steps.append((step_name, "Step returned False"))
                    failed_count += 1
            except Exception as e:
                self.log(f"✗ {step_name} raised exception: {e}", level="ERROR")
                self.failed_steps.append((step_name, str(e)))
                failed_count += 1
        
        self.print_summary()
        self.save_build_log()
        
        return 0 if failed_count == 0 else 1

    def run_selected_steps(self, selected: List[str]) -> int:
        """Run a selected subset of steps by name."""
        self.log("Aura v1.0 CI Build")
        self.log(f"Repository: {self.repo_root}")
        self.log(f"Started: {self.start_time.strftime('%Y-%m-%d %H:%M:%S')}")

        step_map = {
            "core": ("Aura Core", self.build_aura_core),
            "sentinel": ("Sentinel IDE", self.build_sentinel_ide),
            "android": ("Android Setup", self.setup_android_toolchain),
            "apk": ("Android APK", self.build_android_apk),
            "apk-package": ("APK Packaging", self.copy_android_apk_to_dist),
            "verify": ("Binary Verification", self.verify_binaries),
            "dist": ("Distribution Creation", self.create_distribution),
            "manifest": ("Manifest Generation", self.generate_manifest),
            "docs": ("Docs Aggregate", self.collect_markdown_docs),
            "roadmap": ("Roadmap Restructure", self.rearrange_roadmap_summary),
            "reorg-root": ("Reorg Root", self.reorg_repo_root),
        }

        ordered: List[tuple[str, callable]] = []
        for key in selected:
            if key not in step_map:
                self.log(f"Unknown step: {key}", level="ERROR")
                self._step_failed("args", f"Unknown step: {key}")
                continue
            ordered.append(step_map[key])

        failed_count = 0
        for step_name, step_func in ordered:
            try:
                if not step_func():
                    self.log(f"✗ {step_name} failed", level="ERROR")
                    self._step_failed(step_name, "Step returned False")
                    failed_count += 1
            except Exception as e:
                self.log(f"✗ {step_name} raised exception: {e}", level="ERROR")
                self._step_failed(step_name, str(e))
                failed_count += 1

        # Always ensure docs step can run after dist if requested
        self.print_summary()
        self.save_build_log()
        return 0 if failed_count == 0 else 1


def _interactive_select() -> List[str]:
    print("\nAura CI Menu")
    print("1) Build everything (core + sentinel + android + dist + manifest + docs)")
    print("2) Build core (Rust) only")
    print("3) Build Sentinel IDE only")
    print("4) Build Android APK only")
    print("5) Package dist + manifest only")
    print("6) Docs aggregate only")
    print("7) Roadmap restructure only")
    print("8) Reorg repo root (move root md/aura)")
    print("9) Custom (type step keys)")
    choice = input("Select: ").strip()

    if choice == "1":
        return ["core", "sentinel", "android", "apk", "apk-package", "verify", "dist", "manifest", "docs"]
    if choice == "2":
        return ["core", "verify"]
    if choice == "3":
        return ["sentinel"]
    if choice == "4":
        return ["android", "apk", "apk-package"]
    if choice == "5":
        return ["verify", "dist", "manifest"]
    if choice == "6":
        return ["docs"]
    if choice == "7":
        return ["roadmap"]
    if choice == "8":
        return ["reorg-root", "docs", "roadmap"]
    if choice == "9":
        print("Available step keys:")
        print("  core, sentinel, android, apk, apk-package, verify, dist, manifest, docs, roadmap")
        print("  reorg-root")
        raw = input("Enter comma-separated keys: ").strip()
        return [s.strip() for s in raw.split(",") if s.strip()]

    # Default
    return ["core", "sentinel", "verify", "dist", "manifest", "docs"]


def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description="Aura CI main script")
    parser.add_argument(
        "--steps",
        help=(
            "Comma-separated step keys (e.g. core,sentinel,android,apk,dist,manifest,docs,roadmap). "
            "If omitted, an interactive menu is shown."
        ),
        default=None,
    )
    parser.add_argument(
        "--skip-android",
        action="store_true",
        help="Skip Android steps (equivalent to AURA_SKIP_ANDROID=1).",
    )
    args = parser.parse_args()

    if args.skip_android:
        os.environ["AURA_SKIP_ANDROID"] = "1"

    builder = AuraBuilder()
    if args.steps:
        selected = [s.strip() for s in args.steps.split(",") if s.strip()]
        exit_code = builder.run_selected_steps(selected)
    else:
        selected = _interactive_select()
        exit_code = builder.run_selected_steps(selected)

    sys.exit(exit_code)


if __name__ == "__main__":
    main()
