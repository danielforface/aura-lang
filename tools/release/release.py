#!/usr/bin/env python3
"""Build and package AuraSDK into a portable zip.

Outputs:
  dist/aura-sdk-v<version>-<platform>.zip
  dist/AuraSDK/...

This is intentionally lightweight and works without external Python deps.
"""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
import hashlib
import json
import zipfile
from pathlib import Path


def safe_replace_existing(path: Path) -> None:
    """Best-effort removal of an existing artifact.

    On Windows, Explorer/AV might briefly lock files; in that case, rename aside
    so the build can proceed deterministically.
    """

    if not path.exists():
        return

    try:
        path.unlink()
        return
    except PermissionError:
        pass

    # Rename aside.
    for i in range(1, 1000):
        alt = path.with_suffix(path.suffix + f".old{i}")
        try:
            path.rename(alt)
            return
        except Exception:
            continue

    raise SystemExit(f"could not remove or rename locked artifact: {path}")

REPO_ROOT = Path(__file__).resolve().parents[2]
DIST_DIR = REPO_ROOT / "dist"
SDK_STAGING = DIST_DIR / "AuraSDK"


def run(cmd: list[str], cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(cmd))
    subprocess.check_call(cmd, cwd=str(cwd or REPO_ROOT), env=env)


def maybe_sign_windows(path: Path, enabled: bool, env: dict[str, str]) -> None:
    if not enabled:
        return
    if os.name != "nt":
        print(f"(skip signing; not Windows) {path}")
        return
    if not path.exists():
        return

    signtool = env.get("AURA_SIGNTOOL", "signtool")
    timestamp_url = env.get("AURA_SIGN_TIMESTAMP_URL", "http://timestamp.digicert.com")

    thumb = env.get("AURA_SIGN_THUMBPRINT")
    pfx = env.get("AURA_SIGN_PFX")
    pfx_pw = env.get("AURA_SIGN_PFX_PASSWORD")

    if thumb:
        cmd = [
            signtool,
            "sign",
            "/sha1",
            thumb,
            "/fd",
            "sha256",
            "/tr",
            timestamp_url,
            "/td",
            "sha256",
            str(path),
        ]
    elif pfx:
        if pfx_pw is None:
            raise SystemExit("AURA_SIGN_PFX_PASSWORD is required when using AURA_SIGN_PFX")
        cmd = [
            signtool,
            "sign",
            "/f",
            pfx,
            "/p",
            pfx_pw,
            "/fd",
            "sha256",
            "/tr",
            timestamp_url,
            "/td",
            "sha256",
            str(path),
        ]
    else:
        raise SystemExit(
            "--sign requires either AURA_SIGN_THUMBPRINT (cert store) or AURA_SIGN_PFX + AURA_SIGN_PFX_PASSWORD"
        )

    run(cmd, env=env)


def read_version() -> str:
    # Workspace package version.
    cargo_toml = (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    for line in cargo_toml.splitlines():
        if line.strip().startswith("version") and "workspace.package" in cargo_toml:
            break
    # Fall back to aura crate version.
    aura_toml = (REPO_ROOT / "aura" / "Cargo.toml").read_text(encoding="utf-8")
    for line in aura_toml.splitlines():
        if line.strip().startswith("version"):
            return line.split("=", 1)[1].strip().strip('"')
    return "0.1.0"


def host_tag() -> str:
    sysname = platform.system().lower()
    arch = platform.machine().lower()
    if sysname.startswith("windows"):
        return f"windows-{arch}"
    if sysname.startswith("darwin"):
        return f"macos-{arch}"
    return f"linux-{arch}"


def copy_tree(src: Path, dst: Path) -> None:
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)


def zip_write_deterministic(z: zipfile.ZipFile, src: Path, arcname: str) -> None:
    """Write a file to a zip with deterministic metadata.

    zipfile.ZipFile.write() captures filesystem mtimes and can vary across runs.
    For reproducible artifacts, we fix timestamps and permissions.
    """

    data = src.read_bytes()
    info = zipfile.ZipInfo(filename=arcname)
    # Earliest representable DOS timestamp in zip.
    info.date_time = (1980, 1, 1, 0, 0, 0)

    # Preserve basic executable bit on Unix; otherwise default to 0644.
    is_exe = False
    try:
        is_exe = bool(src.stat().st_mode & 0o111)
    except Exception:
        is_exe = False
    perm = 0o755 if is_exe else 0o644
    info.external_attr = (perm & 0xFFFF) << 16

    z.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def repack_zip_deterministic(src_zip: Path, dst_zip: Path) -> None:
    """Repack a zip-like archive deterministically.

    Some tooling (e.g. VSIX packaging) can produce zips with variable timestamps
    or entry ordering. This repack fixes ordering + timestamps.
    """

    tmp = dst_zip.with_suffix(dst_zip.suffix + ".tmp")
    safe_replace_existing(tmp)
    with zipfile.ZipFile(src_zip, "r") as zin:
        infos = list(zin.infolist())
        infos.sort(key=lambda i: i.filename)
        with zipfile.ZipFile(tmp, "w", compression=zipfile.ZIP_DEFLATED) as zout:
            for i in infos:
                if i.is_dir():
                    continue
                data = zin.read(i.filename)
                out_i = zipfile.ZipInfo(filename=i.filename)
                out_i.date_time = (1980, 1, 1, 0, 0, 0)
                out_i.external_attr = i.external_attr
                zout.writestr(out_i, data, compress_type=zipfile.ZIP_DEFLATED)

    safe_replace_existing(dst_zip)
    tmp.rename(dst_zip)


def write_attestation(path: Path, meta: dict) -> None:
    out = path.with_suffix(path.suffix + ".attestation.json")
    payload = {
        "schema": "aura.attestation.v1",
        "artifact": str(path.as_posix()),
        "sha256": sha256_file(path) if path.exists() else None,
        "meta": meta,
    }
    out.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def newest_file(paths: list[Path]) -> Path | None:
    if not paths:
        return None
    return max(paths, key=lambda p: p.stat().st_mtime)


def stage_sentinel_sidecar(sentinel_app_dir: Path, lsp_bin: Path, env: dict[str, str]) -> None:
    """Copy aura-lsp (and required DLLs) into src-tauri/bin for bundling.

    Tauri v2 bundles sidecars declared in tauri.conf.json (bundle.externalBin).
    We stage the binary into src-tauri/bin so builds are self-contained.
    """

    src_tauri = sentinel_app_dir / "src-tauri"
    bin_dir = src_tauri / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)

    exe = ".exe" if os.name == "nt" else ""
    # Tauri expects sidecars to be suffixed by the target triple at build time.
    target_triple = None
    try:
        out = subprocess.check_output(["rustc", "-vV"], cwd=str(REPO_ROOT)).decode("utf-8", "replace")
        for line in out.splitlines():
            if line.startswith("host:"):
                target_triple = line.split(":", 1)[1].strip()
                break
    except Exception:
        target_triple = None

    if target_triple:
        shutil.copy2(lsp_bin, bin_dir / f"aura-lsp-{target_triple}{exe}")

    # Also write an unsuffixed copy for convenience (some local tooling expects it).
    shutil.copy2(lsp_bin, bin_dir / f"aura-lsp{exe}")

    # Bundle Z3 DLLs on Windows so the sidecar runs on clean machines.
    if os.name == "nt":
        z3_bin = REPO_ROOT / "tools" / "z3" / "dist" / "z3-4.15.4-x64-win" / "bin"
        if z3_bin.is_dir():
            for name in [
                "libz3.dll",
                "vcruntime140.dll",
                "vcruntime140_1.dll",
                "msvcp140.dll",
                "msvcp140_1.dll",
                "msvcp140_2.dll",
                "vcomp140.dll",
                "msvcp140_atomic_wait.dll",
                "msvcp140_codecvt_ids.dll",
                "vcruntime140_threads.dll",
            ]:
                p = z3_bin / name
                if p.exists():
                    shutil.copy2(p, bin_dir / name)


def sentinel_candidate_release_dirs(sentinel_app_dir: Path, env: dict[str, str]) -> list[Path]:
    """Possible cargo release output dirs for the Tauri app.

    In CI/dev on Windows, we sometimes use an alternate Cargo target directory
    to avoid locked binaries. Tauri will then place artifacts under:
      editors/sentinel-app/src-tauri/<target-dir>/release/
    """

    src_tauri = sentinel_app_dir / "src-tauri"
    candidates: list[Path] = [
        src_tauri / "target" / "release",
        src_tauri / "target-lsp-test" / "release",
    ]

    cargo_target_dir = env.get("CARGO_TARGET_DIR")
    if cargo_target_dir:
        p = Path(cargo_target_dir)
        if p.is_absolute():
            candidates.insert(0, p / "release")
        else:
            # Most commonly this is relative to src-tauri.
            candidates.insert(0, src_tauri / p / "release")

    # Deduplicate while preserving order.
    seen: set[Path] = set()
    out: list[Path] = []
    for d in candidates:
        if d in seen:
            continue
        seen.add(d)
        out.append(d)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--features", default="z3,lumina-raylib", help="Cargo features for aura build")
    ap.add_argument("--no-build", action="store_true", help="Skip cargo builds")
    ap.add_argument(
        "--android-arm64",
        action="store_true",
        help="Also build AuraSDK for aarch64-linux-android (requires ANDROID_NDK_HOME).",
    )
    ap.add_argument(
        "--vsix",
        action="store_true",
        help="Build Aura Sentinel VS Code extension as a .vsix into dist/",
    )
    ap.add_argument(
        "--sentinel-app",
        action="store_true",
        help="Build Aura Sentinel standalone desktop app (Tauri) into dist/",
    )
    ap.add_argument(
        "--sign",
        action="store_true",
        help="(Windows) Code-sign produced artifacts using signtool (requires AURA_SIGN_* env vars)",
    )
    ap.add_argument(
        "--attest",
        action="store_true",
        help="Write JSON attestations (sha256 + build metadata) next to produced artifacts.",
    )
    ap.add_argument("--website", action="store_true", help="Copy zip into website/public/downloads/")
    args = ap.parse_args()

    version = read_version()
    tag = host_tag()

    DIST_DIR.mkdir(parents=True, exist_ok=True)
    if SDK_STAGING.exists():
        shutil.rmtree(SDK_STAGING)

    (SDK_STAGING / "bin").mkdir(parents=True)

    env = os.environ.copy()

    npm = "npm.cmd" if os.name == "nt" else "npm"

    if not args.no_build:
        run(["cargo", "build", "-p", "aura", "--release", "--locked", "--features", args.features])
        run(["cargo", "build", "-p", "aura-lsp", "--release", "--locked"])

        if args.android_arm64:
            ndk_home = env.get("ANDROID_NDK_HOME") or env.get("ANDROID_NDK_ROOT")
            if not ndk_home:
                raise SystemExit("--android-arm64 requires ANDROID_NDK_HOME (or ANDROID_NDK_ROOT)")

            # Ensure the target is installed.
            run(["rustup", "target", "add", "aarch64-linux-android"], env=env)

            # Build the SDK binary for Android.
            run(
                [
                    "cargo",
                    "build",
                    "-p",
                    "aura",
                    "--release",
                    "--locked",
                    "--features",
                    args.features,
                    "--target",
                    "aarch64-linux-android",
                ],
                env=env,
            )

    exe = ".exe" if os.name == "nt" else ""
    aura_bin = REPO_ROOT / "target" / "release" / f"aura{exe}"
    lsp_bin = REPO_ROOT / "target" / "release" / f"aura-lsp{exe}"

    if not aura_bin.exists():
        raise SystemExit(f"missing build output: {aura_bin}")
    if not lsp_bin.exists():
        raise SystemExit(f"missing build output: {lsp_bin}")

    shutil.copy2(aura_bin, SDK_STAGING / "bin" / aura_bin.name)
    shutil.copy2(lsp_bin, SDK_STAGING / "bin" / lsp_bin.name)

    # Stdlib + docs + styles.
    copy_tree(REPO_ROOT / "sdk" / "std", SDK_STAGING / "std")
    copy_tree(REPO_ROOT / "sdk" / "docs", SDK_STAGING / "docs")
    copy_tree(REPO_ROOT / "sdk" / "styles", SDK_STAGING / "styles")
    if (REPO_ROOT / "sdk" / "android").is_dir():
        copy_tree(REPO_ROOT / "sdk" / "android", SDK_STAGING / "android")
    if (REPO_ROOT / "samples" / "android").is_dir():
        copy_tree(REPO_ROOT / "samples" / "android", SDK_STAGING / "samples" / "android")
    shutil.copy2(REPO_ROOT / "sdk" / "install.ps1", SDK_STAGING / "install.ps1")
    shutil.copy2(REPO_ROOT / "sdk" / "install.sh", SDK_STAGING / "install.sh")

    # Bundle Z3 DLLs on Windows for a "no missing DLL" experience.
    if os.name == "nt":
        z3_bin = REPO_ROOT / "tools" / "z3" / "dist" / "z3-4.15.4-x64-win" / "bin"
        if z3_bin.is_dir():
            for name in [
                "libz3.dll",
                "vcruntime140.dll",
                "vcruntime140_1.dll",
                "msvcp140.dll",
                "msvcp140_1.dll",
                "msvcp140_2.dll",
                "vcomp140.dll",
                "msvcp140_atomic_wait.dll",
                "msvcp140_codecvt_ids.dll",
                "vcruntime140_threads.dll",
            ]:
                p = z3_bin / name
                if p.exists():
                    shutil.copy2(p, SDK_STAGING / "bin" / name)

    out_zip = DIST_DIR / f"aura-sdk-v{version}-{tag}.zip"
    safe_replace_existing(out_zip)

    with zipfile.ZipFile(out_zip, "w", compression=zipfile.ZIP_DEFLATED) as z:
        files = [p for p in SDK_STAGING.rglob("*") if p.is_file()]
        files.sort(key=lambda p: p.relative_to(DIST_DIR).as_posix())
        for p in files:
            rel = p.relative_to(DIST_DIR).as_posix()
            zip_write_deterministic(z, p, rel)

    print(f"wrote {out_zip}")

    if args.attest:
        write_attestation(
            out_zip,
            {
                "kind": "sdk-zip",
                "version": version,
                "platform": tag,
            },
        )

    # Optional Android ARM64 SDK zip.
    if args.android_arm64:
        android_tag = "android-aarch64"
        android_staging = DIST_DIR / f"AuraSDK-{android_tag}"
        if android_staging.exists():
            shutil.rmtree(android_staging)
        (android_staging / "bin").mkdir(parents=True)

        android_aura = REPO_ROOT / "target" / "aarch64-linux-android" / "release" / "aura"
        if not android_aura.exists():
            raise SystemExit(f"missing Android build output: {android_aura}")

        shutil.copy2(android_aura, android_staging / "bin" / "aura")

        copy_tree(REPO_ROOT / "sdk" / "std", android_staging / "std")
        copy_tree(REPO_ROOT / "sdk" / "docs", android_staging / "docs")
        copy_tree(REPO_ROOT / "sdk" / "styles", android_staging / "styles")
        if (REPO_ROOT / "sdk" / "android").is_dir():
            copy_tree(REPO_ROOT / "sdk" / "android", android_staging / "android")
        if (REPO_ROOT / "samples" / "android").is_dir():
            copy_tree(REPO_ROOT / "samples" / "android", android_staging / "samples" / "android")
        shutil.copy2(REPO_ROOT / "sdk" / "install.ps1", android_staging / "install.ps1")
        shutil.copy2(REPO_ROOT / "sdk" / "install.sh", android_staging / "install.sh")

        android_zip = DIST_DIR / f"aura-sdk-v{version}-{android_tag}.zip"
        safe_replace_existing(android_zip)
        with zipfile.ZipFile(android_zip, "w", compression=zipfile.ZIP_DEFLATED) as z:
            files = [p for p in android_staging.rglob("*") if p.is_file()]
            files.sort(key=lambda p: p.relative_to(DIST_DIR).as_posix())
            for p in files:
                rel = p.relative_to(DIST_DIR).as_posix()
                zip_write_deterministic(z, p, rel)
        print(f"wrote {android_zip}")

        if args.attest:
            write_attestation(
                android_zip,
                {
                    "kind": "sdk-zip",
                    "version": version,
                    "platform": android_tag,
                },
            )

    vsix_out = DIST_DIR / f"aura-sentinel-v{version}.vsix"
    app_out = DIST_DIR / f"aura-sentinel-app-v{version}-{tag}{exe}"
    app_latest = DIST_DIR / f"aura-sentinel-app{exe}"
    msi_out = DIST_DIR / f"aura-sentinel-v{version}-{tag}.msi"
    msi_latest = DIST_DIR / "aura-sentinel.msi"
    nsis_out = DIST_DIR / f"aura-sentinel-setup-v{version}-{tag}{exe}"
    nsis_latest = DIST_DIR / f"aura-sentinel-setup{exe}"
    if args.vsix or args.website:
        # Package the VS Code extension into a .vsix.
        vscode_dir = REPO_ROOT / "editors" / "vscode"
        # Keep it deterministic via the lockfile.
        run([npm, "ci"], cwd=vscode_dir, env=env)
        run([npm, "run", "package:vsix"], cwd=vscode_dir, env=env)

        # The package script writes dist/aura-sentinel.vsix.
        produced = DIST_DIR / "aura-sentinel.vsix"
        if not produced.exists():
            raise SystemExit(f"missing VSIX output: {produced}")

        safe_replace_existing(vsix_out)
        shutil.copy2(produced, vsix_out)
        # VSIX is a zip; repack deterministically (timestamps + entry ordering).
        repack_zip_deterministic(vsix_out, vsix_out)
        print(f"wrote {vsix_out}")

        if args.attest:
            write_attestation(
                vsix_out,
                {
                    "kind": "vsix",
                    "version": version,
                },
            )

    if args.sentinel_app:
        sentinel_app_dir = REPO_ROOT / "editors" / "sentinel-app"
        stage_sentinel_sidecar(sentinel_app_dir, lsp_bin, env)
        run([npm, "ci"], cwd=sentinel_app_dir, env=env)
        run([npm, "run", "tauri:build"], cwd=sentinel_app_dir, env=env)

        produced_exe: Path | None = None
        for release_dir in sentinel_candidate_release_dirs(sentinel_app_dir, env):
            candidate = release_dir / f"aura-sentinel-app{exe}"
            if candidate.exists():
                produced_exe = candidate
                break
        if produced_exe is None:
            raise SystemExit(
                "missing Sentinel app output (checked: "
                + ", ".join(
                    str(d / f"aura-sentinel-app{exe}")
                    for d in sentinel_candidate_release_dirs(sentinel_app_dir, env)
                )
                + ")"
            )

        safe_replace_existing(app_out)
        shutil.copy2(produced_exe, app_out)
        print(f"wrote {app_out}")

        if args.attest:
            write_attestation(
                app_out,
                {
                    "kind": "sentinel-app",
                    "version": version,
                    "platform": tag,
                },
            )

        maybe_sign_windows(app_out, args.sign, env)

        safe_replace_existing(app_latest)
        shutil.copy2(produced_exe, app_latest)
        print(f"wrote {app_latest}")

        if args.attest:
            write_attestation(
                app_latest,
                {
                    "kind": "sentinel-app",
                    "version": version,
                    "platform": tag,
                    "channel": "latest",
                },
            )

        maybe_sign_windows(app_latest, args.sign, env)

        # If bundling is enabled, Tauri will also produce installers under:
        # src-tauri/<target-dir>/release/bundle/{msi,nsis}/
        bundle_dirs = [d / "bundle" for d in sentinel_candidate_release_dirs(sentinel_app_dir, env)]

        msi_candidates: list[Path] = []
        for bd in bundle_dirs:
            if bd.exists():
                msi_candidates.extend(list((bd / "msi").glob("*.msi")))

        msi_src = newest_file(msi_candidates)
        if msi_src is not None and msi_src.exists():
            safe_replace_existing(msi_out)
            shutil.copy2(msi_src, msi_out)
            print(f"wrote {msi_out}")

            if args.attest:
                write_attestation(
                    msi_out,
                    {
                        "kind": "msi",
                        "version": version,
                        "platform": tag,
                    },
                )

            maybe_sign_windows(msi_out, args.sign, env)

            safe_replace_existing(msi_latest)
            shutil.copy2(msi_src, msi_latest)
            print(f"wrote {msi_latest}")

            if args.attest:
                write_attestation(
                    msi_latest,
                    {
                        "kind": "msi",
                        "version": version,
                        "platform": tag,
                        "channel": "latest",
                    },
                )

            maybe_sign_windows(msi_latest, args.sign, env)

        nsis_candidates: list[Path] = []
        for bd in bundle_dirs:
            if bd.exists():
                nsis_candidates.extend(list((bd / "nsis").glob(f"*{exe}")))

        nsis_src = newest_file(nsis_candidates)
        if nsis_src is not None and nsis_src.exists():
            safe_replace_existing(nsis_out)
            shutil.copy2(nsis_src, nsis_out)
            print(f"wrote {nsis_out}")

            if args.attest:
                write_attestation(
                    nsis_out,
                    {
                        "kind": "nsis",
                        "version": version,
                        "platform": tag,
                    },
                )

            maybe_sign_windows(nsis_out, args.sign, env)

            safe_replace_existing(nsis_latest)
            shutil.copy2(nsis_src, nsis_latest)
            print(f"wrote {nsis_latest}")

            if args.attest:
                write_attestation(
                    nsis_latest,
                    {
                        "kind": "nsis",
                        "version": version,
                        "platform": tag,
                        "channel": "latest",
                    },
                )

            maybe_sign_windows(nsis_latest, args.sign, env)

    if args.website:
        downloads = REPO_ROOT / "website" / "public" / "downloads"
        downloads.mkdir(parents=True, exist_ok=True)

        # Versioned artifacts.
        shutil.copy2(out_zip, downloads / out_zip.name)
        print(f"copied to {downloads / out_zip.name}")

        if vsix_out.exists():
            shutil.copy2(vsix_out, downloads / vsix_out.name)
            print(f"copied to {downloads / vsix_out.name}")

        if app_out.exists():
            shutil.copy2(app_out, downloads / app_out.name)
            print(f"copied to {downloads / app_out.name}")

        if msi_out.exists():
            shutil.copy2(msi_out, downloads / msi_out.name)
            print(f"copied to {downloads / msi_out.name}")

        if nsis_out.exists():
            shutil.copy2(nsis_out, downloads / nsis_out.name)
            print(f"copied to {downloads / nsis_out.name}")

        # Stable 'latest' names expected by the website downloads page.
        shutil.copy2(out_zip, downloads / "aura-sdk.zip")
        print(f"copied to {downloads / 'aura-sdk.zip'}")

        if vsix_out.exists():
            shutil.copy2(vsix_out, downloads / "aura-sentinel.vsix")
            print(f"copied to {downloads / 'aura-sentinel.vsix'}")

        if app_latest.exists():
            shutil.copy2(app_latest, downloads / app_latest.name)
            print(f"copied to {downloads / app_latest.name}")

        if msi_latest.exists():
            shutil.copy2(msi_latest, downloads / msi_latest.name)
            print(f"copied to {downloads / msi_latest.name}")

        if nsis_latest.exists():
            shutil.copy2(nsis_latest, downloads / nsis_latest.name)
            print(f"copied to {downloads / nsis_latest.name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
