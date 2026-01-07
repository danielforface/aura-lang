#![forbid(unsafe_code)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, IntoDiagnostic};
use regex::Regex;
use thiserror::Error;

pub mod onnx;

#[derive(Debug, Error, Diagnostic)]
#[error("bridge error: {message}")]
#[diagnostic(code(aura::bridge))]
pub struct BridgeError {
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct BridgeConfig {
    pub headers: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub lib_dirs: Vec<PathBuf>,
    /// Additional libraries to link (e.g. opencv_world4100.lib)
    pub libs: Vec<String>,

    /// Best-effort refined type mapping for generated Aura shims.
    ///
    /// When enabled, the shim may use:
    /// - range constraints like `u32[0..255]` for small unsigned integer C types
    /// - `Option<u32>` for pointer-typed parameters/returns (nullable by default)
    pub refine_types: bool,
}

#[derive(Clone, Debug, Default)]
pub struct LinkInputs {
    pub lib_dirs: Vec<PathBuf>,
    pub libs: Vec<String>,
    /// Extra C/C++ sources to compile into the final executable.
    /// This is used for lightweight shims/wrappers (e.g. Raylib ABI adapters).
    pub c_sources: Vec<PathBuf>,
    /// DLLs to copy next to the final executable at run time.
    pub runtime_dlls: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct BridgeOutputs {
    pub aura_shim_path: PathBuf,
    pub link: LinkInputs,
    pub discovered: Vec<DiscoveredFn>,
}

#[derive(Clone, Debug)]
pub struct DiscoveredFn {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub ret: String,
}

/// Very small “universal bridge” v0:
/// - Parses simple C/C++ header function declarations via regex heuristics
/// - Emits an Aura shim file with `extern cell` decls + safe wrapper `cell`s
/// - Collects link inputs (lib dirs + `-l` style names)
///
/// This intentionally does NOT try to be a full C++ parser yet.
pub fn run_bridge(config: &BridgeConfig, out_dir: &Path) -> miette::Result<BridgeOutputs> {
    fs::create_dir_all(out_dir).into_diagnostic()?;

    let mut discovered = Vec::new();

    for header in &config.headers {
        let text = read_text_any(header)?;
        discovered.extend(parse_header_functions(&text));
    }

    let shim = generate_aura_shim(&discovered, config.refine_types);
    let shim_path = out_dir.join("bridge.aura");
    fs::write(&shim_path, shim).into_diagnostic()?;

    let mut link = LinkInputs::default();
    link.lib_dirs.extend(config.lib_dirs.iter().cloned());
    link.libs.extend(config.libs.iter().cloned());

    // Bootstrap discovery: look for import libs / DLLs next to the bridged headers.
    discover_artifacts_near_headers(&config.headers, &mut link)?;

    Ok(BridgeOutputs {
        aura_shim_path: shim_path,
        link,
        discovered,
    })
}

fn read_text_any(path: &Path) -> miette::Result<String> {
    let bytes = fs::read(path).into_diagnostic()?;

    // Fast path: UTF-8.
    if let Ok(s) = String::from_utf8(bytes.clone()) {
        return Ok(s);
    }

    // Common Windows case: UTF-16LE (often produced by PowerShell redirection/echo).
    // Heuristic: lots of NUL bytes or BOM.
    let is_utf16le = bytes.starts_with(&[0xFF, 0xFE]) || bytes.iter().take(64).any(|b| *b == 0);
    if is_utf16le {
        let mut u16s = Vec::with_capacity(bytes.len() / 2);
        let mut it = bytes.iter().copied();
        // If BOM present, skip it.
        if bytes.starts_with(&[0xFF, 0xFE]) {
            it.next();
            it.next();
        }
        while let (Some(lo), Some(hi)) = (it.next(), it.next()) {
            u16s.push(u16::from_le_bytes([lo, hi]));
        }
        return Ok(String::from_utf16_lossy(&u16s));
    }

    // Fallback: lossy UTF-8.
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn discover_artifacts_near_headers(headers: &[PathBuf], link: &mut LinkInputs) -> miette::Result<()> {
    let mut seen_dirs = std::collections::HashSet::<PathBuf>::new();

    for h in headers {
        let Some(dir) = h.parent() else { continue };
        let dir = dir.to_path_buf();

        // Always add the header directory as a potential -L.
        if !link.lib_dirs.iter().any(|d| d == &dir) {
            link.lib_dirs.push(dir.clone());
        }

        // Only include the shim source that matches the bridged header basename.
        // This prevents unrelated shims in a shared tools/ folder from being
        // compiled and linked into every project.
        if let Some(stem) = h.file_stem().and_then(|s| s.to_str()) {
            for ext in ["c", "cc", "cpp"] {
                let cand = dir.join(format!("{stem}.{ext}"));
                if cand.exists() && !link.c_sources.iter().any(|s| s == &cand) {
                    link.c_sources.push(cand);
                }
            }
        }

        if !seen_dirs.insert(dir.clone()) {
            continue;
        }

        // Bootstrap discovery: look for import libs / DLLs next to the bridged headers.
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let Some(ext) = p.extension().and_then(|x| x.to_str()) else { continue };
            match ext.to_ascii_lowercase().as_str() {
                "lib" => {
                    if let Some(name) = p.file_name().and_then(|x| x.to_str()) {
                        if !link.libs.iter().any(|l| l.eq_ignore_ascii_case(name)) {
                            link.libs.push(name.to_string());
                        }
                    }
                }
                "dll" => {
                    if !link.runtime_dlls.iter().any(|d| d == &p) {
                        link.runtime_dlls.push(p);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn parse_header_functions(header_text: &str) -> Vec<DiscoveredFn> {
    // Heuristic: match lines like
    //   int foo(int a, float* b, int len);
    //   void bar(const char* s);
    // Not robust; it’s a bootstrap.
    let re = Regex::new(
        r"(?m)^\s*(?P<ret>[a-zA-Z_][a-zA-Z0-9_\s\*&:<>]*)\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<args>[^;\)]*)\)\s*;\s*$",
    )
    .expect("regex compile");

    let mut out = Vec::new();
    for caps in re.captures_iter(header_text) {
        let ret = normalize_ws(caps.name("ret").unwrap().as_str());
        let name = caps.name("name").unwrap().as_str().to_string();
        let args = caps.name("args").unwrap().as_str();
        let params = parse_params(args);

        out.push(DiscoveredFn { name, params, ret });
    }
    out
}

fn parse_params(args: &str) -> Vec<(String, String)> {
    let args = args.trim();
    if args.is_empty() || args == "void" {
        return Vec::new();
    }

    args.split(',')
        .map(|p| normalize_ws(p))
        .filter(|p| !p.is_empty())
        .enumerate()
        .map(|(i, p)| {
            // Try to split by last space: "const char* name" => (type, name)
            if let Some((ty, name)) = p.rsplit_once(' ') {
                let name = name.trim();
                let ty = ty.trim();
                let name = if name.is_empty() {
                    format!("arg{i}")
                } else {
                    sanitize_ident(name)
                };
                (name, ty.to_string())
            } else {
                (format!("arg{i}"), p)
            }
        })
        .collect()
}

fn sanitize_ident(s: &str) -> String {
    // Drop pointer/reference tokens from the identifier slot if they were attached.
    s.trim_matches(&['*', '&'][..]).to_string()
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_qualifiers(ty: &str) -> String {
    // Keep this intentionally small; the bridge is heuristic.
    ty.replace("const ", "")
        .replace("volatile ", "")
        .replace("struct ", "")
        .trim()
        .to_string()
}

fn is_pointer_type(ty: &str) -> bool {
    ty.contains('*')
}

fn map_c_type_to_aura(ty: &str, refine_types: bool) -> String {
    // Minimal mapping with optional refinements.
    let t = strip_qualifiers(ty);
    let t = t.as_str();

    // String-like
    if matches!(t, "char*" | "char *" | "const char*" | "const char *") {
        return "String".to_string();
    }

    if refine_types && is_pointer_type(t) {
        // Best-effort nullability: pointers are nullable by default.
        // Represent as `Option<u32>` opaque handle.
        return "Option<u32>".to_string();
    }

    match t {
        "void" => "Unit".to_string(),

        // Common integer types
        "int" | "unsigned int" | "uint32_t" | "size_t" => "u32".to_string(),

        // Small unsigned types: emit range refinement when enabled.
        "uint8_t" | "unsigned char" => {
            if refine_types {
                "u32[0..255]".to_string()
            } else {
                "u32".to_string()
            }
        }
        "uint16_t" | "unsigned short" => {
            if refine_types {
                "u32[0..65535]".to_string()
            } else {
                "u32".to_string()
            }
        }

        // Fallback: treat unknowns as opaque handle.
        _ => "u32".to_string(),
    }
}

fn generate_aura_shim(funcs: &[DiscoveredFn], refine_types: bool) -> String {
    let mut out = String::new();
    out.push_str("# Auto-generated by aura-bridge (bootstrap)\n");
    out.push_str("# NOTE: C/C++ parsing is heuristic in this phase.\n\n");

    for f in funcs {
        let mut params_aura = String::new();
        for (idx, (name, c_ty)) in f.params.iter().enumerate() {
            if idx > 0 {
                params_aura.push_str(", ");
            }
            let aura_ty = map_c_type_to_aura(c_ty, refine_types);
            params_aura.push_str(&format!("{}: {}", name, aura_ty));
        }

        let ret_aura = map_c_type_to_aura(&f.ret, refine_types);

        // Emit a direct extern declaration matching the C symbol name.
        // We intentionally do NOT generate a same-named Aura wrapper `cell`,
        // because that would produce an LLVM `define` and collide with the C shim.
        out.push_str(&format!(
            "extern cell {}({}): {}\n\n",
            f.name, params_aura, ret_aura
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refined_mapping_emits_ranges_and_option_for_pointers() {
        assert_eq!(map_c_type_to_aura("uint8_t", false), "u32");
        assert_eq!(map_c_type_to_aura("uint8_t", true), "u32[0..255]");
        assert_eq!(map_c_type_to_aura("uint16_t", true), "u32[0..65535]");
        assert_eq!(map_c_type_to_aura("int*", false), "u32");
        assert_eq!(map_c_type_to_aura("int*", true), "Option<u32>");
        assert_eq!(map_c_type_to_aura("const char *", true), "String");
    }

    #[test]
    fn shim_generation_uses_refined_types_when_enabled() {
        let funcs = vec![DiscoveredFn {
            name: "foo".to_string(),
            params: vec![("p".to_string(), "int*".to_string()), ("n".to_string(), "uint8_t".to_string())],
            ret: "void".to_string(),
        }];

        let shim_plain = generate_aura_shim(&funcs, false);
        assert!(shim_plain.contains("extern cell foo(p: u32, n: u32): Unit"));

        let shim_refined = generate_aura_shim(&funcs, true);
        assert!(shim_refined.contains("extern cell foo(p: Option<u32>, n: u32[0..255]): Unit"));
    }
}

