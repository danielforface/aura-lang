#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const AURA_HOME_ENV: &str = "AURA_HOME";

pub fn detect_aura_home() -> Option<PathBuf> {
    if let Ok(v) = env::var(AURA_HOME_ENV) {
        let p = PathBuf::from(v);
        if p.exists() {
            return Some(p);
        }
    }

    let exe = env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // Installed layout: AuraSDK/bin/aura(.exe)
    if exe_dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("bin")) {
        let home = exe_dir.parent()?.to_path_buf();
        if home.exists() {
            return Some(home);
        }
    }

    // Dev layout: repo/target/{debug,release}/aura(.exe)
    if exe_dir
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("debug") || n.eq_ignore_ascii_case("release"))
    {
        if let Some(target_dir) = exe_dir.parent() {
            if target_dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("target")) {
                if let Some(repo_root) = target_dir.parent() {
                    return Some(repo_root.to_path_buf());
                }
            }
        }
    }

    // Fallback: just use the executable directory.
    Some(exe_dir.to_path_buf())
}

pub fn candidate_std_dirs(aura_home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.push(aura_home.join("std"));
    out.push(aura_home.join("sdk").join("std"));
    out
}

pub fn find_std_dir(aura_home: &Path) -> Option<PathBuf> {
    for d in candidate_std_dirs(aura_home) {
        if d.is_dir() {
            return Some(d);
        }
    }
    None
}

fn parse_aura_std_import(line: &str) -> Option<String> {
    // Accept:
    // - import aura::lumina
    // - import std::io
    // Keep it conservative: line-based match without requiring the full lexer.
    let before_comment = line.split('#').next().unwrap_or("");
    let t = before_comment.trim();
    if !t.starts_with("import ") {
        return None;
    }
    let rest = t.trim_start_matches("import ").trim();
    let rest = rest
        .strip_prefix("aura::")
        .or_else(|| rest.strip_prefix("std::"))?;
    let name = rest.trim();
    if name.is_empty() {
        return None;
    }
    if !name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some(name.to_string())
}

fn load_std_module(std_dir: &Path, name: &str) -> io::Result<Option<String>> {
    let p = std_dir.join(format!("{name}.aura"));
    if !p.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(p)?))
}

/// Returns a *single* augmented source string that preserves the original user's offsets.
///
/// Strategy: keep the original text intact, and append any discovered stdlib modules at the end.
/// This keeps LSP diagnostics and spans stable for the user's file.
pub fn augment_source_with_std(source: &str, aura_home: &Path) -> io::Result<String> {
    let Some(std_dir) = find_std_dir(aura_home) else {
        return Ok(source.to_string());
    };

    let mut requested: BTreeSet<String> = BTreeSet::new();
    for line in source.lines() {
        if let Some(name) = parse_aura_std_import(line) {
            requested.insert(name);
        }
    }

    if requested.is_empty() {
        return Ok(source.to_string());
    }

    let mut out = String::from(source);
    out.push_str("\n\n# --- AuraSDK stdlib (auto-injected) ---\n");

    let mut injected: BTreeSet<String> = BTreeSet::new();

    // Shallow (non-recursive) injection is enough for now; std modules can be self-contained.
    for name in requested {
        if injected.contains(&name) {
            continue;
        }
        if let Some(text) = load_std_module(&std_dir, &name)? {
            injected.insert(name.clone());
            out.push_str("\n\n# --- std:aura::");
            out.push_str(&name);
            out.push_str(" ---\n");
            out.push_str(&text);
            out.push('\n');
        }
    }

    Ok(out)
}

pub fn augment_source_with_default_std(source: &str) -> io::Result<String> {
    let Some(home) = detect_aura_home() else {
        return Ok(source.to_string());
    };
    augment_source_with_std(source, &home)
}
