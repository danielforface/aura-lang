use std::path::{Path, PathBuf};

fn repo_root_from_manifest(manifest_dir: &Path) -> PathBuf {
  // editors/sentinel-app/src-tauri -> editors/sentinel-app -> editors -> repo root
  manifest_dir
    .parent()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    .map(Path::to_path_buf)
    .unwrap_or_else(|| manifest_dir.to_path_buf())
}

fn stage_sidecar() {
  let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
  let repo_root = repo_root_from_manifest(&manifest_dir);

  let target_triple = std::env::var("TAURI_ENV_TARGET_TRIPLE")
    .or_else(|_| std::env::var("TARGET"))
    .unwrap_or_else(|_| "unknown-target".to_string());
  let exe_ext = if target_triple.contains("windows") { ".exe" } else { "" };

  let bin_dir = manifest_dir.join("bin");
  let _ = std::fs::create_dir_all(&bin_dir);

  let dest = bin_dir.join(format!("aura-lsp-{target_triple}{exe_ext}"));
  if !dest.is_file() {
    let src_release = repo_root.join("target").join("release").join(format!("aura-lsp{exe_ext}"));
    let src_debug = repo_root.join("target").join("debug").join(format!("aura-lsp{exe_ext}"));

    let src = if src_release.is_file() {
      src_release
    } else if src_debug.is_file() {
      src_debug
    } else {
      // Build aura-lsp once, then copy it into the bin dir.
      let status = std::process::Command::new("cargo")
        .current_dir(&repo_root)
        .args(["build", "-p", "aura-lsp", "--release", "--locked"])
        .status();
      match status {
        Ok(s) if s.success() => {}
        Ok(s) => panic!("failed to build aura-lsp sidecar (exit {s})"),
        Err(e) => panic!("failed to invoke cargo to build aura-lsp sidecar: {e}"),
      }
      let built = repo_root.join("target").join("release").join(format!("aura-lsp{exe_ext}"));
      if !built.is_file() {
        panic!("aura-lsp build completed but binary not found at {built:?}");
      }
      built
    };

    std::fs::copy(&src, &dest).expect("copy aura-lsp into src-tauri/bin");
  }

  // Bundle Z3 DLLs on Windows so aura-lsp runs without PATH hacks.
  if target_triple.contains("windows") {
    let z3_bin = repo_root
      .join("tools")
      .join("z3")
      .join("dist")
      .join("z3-4.15.4-x64-win")
      .join("bin");
    if z3_bin.is_dir() {
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
      ] {
        let src = z3_bin.join(name);
        let dst = bin_dir.join(name);
        if src.is_file() && !dst.is_file() {
          let _ = std::fs::copy(src, dst);
        }
      }
    }
  }
}

fn main() {
  stage_sidecar();
  tauri_build::build()
}
