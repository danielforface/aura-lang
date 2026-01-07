#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use miette::{Diagnostic, IntoDiagnostic};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("manifest error: {message}")]
#[diagnostic(code(aura::manifest))]
pub struct ManifestError {
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct ResolvedManifest {
    pub manifest_path: Option<PathBuf>,
    pub project_root: PathBuf,

    /// Workspace member directories (relative to `project_root`) when `[workspace]` is present.
    pub workspace_members: Vec<PathBuf>,

    pub bridge_headers: Vec<PathBuf>,

    pub lib_dirs: Vec<PathBuf>,
    pub libs: Vec<String>,

    pub nexus_plugins: Vec<aura_nexus::PluginManifest>,

    /// Language edition (e.g. "2026").
    pub edition: Option<String>,

    /// Enabled unstable features.
    pub features: Vec<String>,
}

impl ResolvedManifest {
    pub fn empty(project_root: PathBuf) -> Self {
        Self {
            manifest_path: None,
            project_root,
            workspace_members: Vec::new(),
            bridge_headers: Vec::new(),
            lib_dirs: Vec::new(),
            libs: Vec::new(),
            nexus_plugins: Vec::new(),
            edition: None,
            features: Vec::new(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    project: Option<Project>,

    #[serde(default)]
    workspace: Option<Workspace>,

    // Reserved for future use (e.g., fetching deps / lockfiles)
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,

    #[serde(default)]
    bridge: Option<Bridge>,

    // Support both new and legacy naming.
    #[serde(default)]
    linker: Option<Linker>,

    #[serde(default)]
    linking: Option<Linking>,

    // Aura Nexus plugin list.
    #[serde(default)]
    plugins: Vec<aura_nexus::PluginManifest>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Workspace {
    #[serde(default)]
    members: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Project {
    #[serde(default)]
    name: Option<String>,

    #[serde(default)]
    edition: Option<String>,

    #[serde(default)]
    features: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Bridge {
    #[serde(default)]
    headers: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Linker {
    // Requested in prompt: `[linker.paths]` (we accept `paths = [...]` under `[linker]`).
    #[serde(default)]
    paths: Vec<String>,

    // Optional explicit libs list.
    #[serde(default)]
    libs: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Linking {
    // Example in prompt: `[linking] lib_dirs = [...]; libs = [...]`
    #[serde(default)]
    lib_dirs: Vec<String>,

    #[serde(default)]
    libs: Vec<String>,
}

pub fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut cur = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = cur.join("aura.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        let parent = cur.parent().map(|p| p.to_path_buf());
        match parent {
            Some(p) => cur = p,
            None => return None,
        }
    }
}

pub fn load_resolved_manifest(start: &Path) -> Result<ResolvedManifest, ManifestError> {
    let project_root = if start.is_file() {
        start.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    } else {
        start.to_path_buf()
    };

    let Some(manifest_path) = find_manifest(&project_root) else {
        return Ok(ResolvedManifest::empty(project_root));
    };

    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| ManifestError {
            message: "manifest has no parent directory".to_string(),
        })?
        .to_path_buf();

    let raw = fs::read_to_string(&manifest_path).into_diagnostic().map_err(|e| ManifestError {
        message: format!("failed to read {}: {e}", manifest_path.display()),
    })?;

    let parsed: Manifest = toml::from_str(&raw).map_err(|e| ManifestError {
        message: format!("failed to parse {}: {e}", manifest_path.display()),
    })?;

    let mut out = ResolvedManifest {
        manifest_path: Some(manifest_path),
        project_root: manifest_dir.clone(),
        workspace_members: Vec::new(),
        bridge_headers: Vec::new(),
        lib_dirs: Vec::new(),
        libs: Vec::new(),
        nexus_plugins: Vec::new(),
        edition: None,
        features: Vec::new(),
    };

    if let Some(project) = parsed.project {
        out.edition = project.edition;
        out.features = project.features;
        // De-dupe (case-insensitive) while preserving order.
        out.features = dedup_strings(out.features);
    }

    if let Some(ws) = parsed.workspace {
        for m in ws.members {
            out.workspace_members.push(resolve_path(&manifest_dir, &m));
        }
        out.workspace_members = dedup_paths(out.workspace_members);
    }

    if let Some(bridge) = parsed.bridge {
        for h in bridge.headers {
            out.bridge_headers.push(resolve_path(&manifest_dir, &h));
        }
    }

    // Unify link dirs + libs from both shapes.
    if let Some(linker) = parsed.linker {
        for p in linker.paths {
            out.lib_dirs.push(resolve_path(&manifest_dir, &p));
        }
        out.libs.extend(linker.libs);
    }

    if let Some(linking) = parsed.linking {
        for p in linking.lib_dirs {
            out.lib_dirs.push(resolve_path(&manifest_dir, &p));
        }
        out.libs.extend(linking.libs);
    }

    // Nexus plugins (top-level `plugins = [...]`).
    out.nexus_plugins = parsed.plugins;

    // De-dupe while preserving order.
    out.bridge_headers = dedup_paths(out.bridge_headers);
    out.lib_dirs = dedup_paths(out.lib_dirs);
    out.libs = dedup_strings(out.libs);

    Ok(out)
}

fn resolve_path(base: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        base.join(pb)
    }
}

fn dedup_paths(mut v: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(v.len());
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for p in v.drain(..) {
        let key = normalize_for_dedup(&p);
        if seen.insert(key) {
            out.push(p);
        }
    }
    out
}

fn dedup_strings(v: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(v.len());
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for s in v {
        let k = s.to_ascii_lowercase();
        if seen.insert(k) {
            out.push(s);
        }
    }
    out
}

fn normalize_for_dedup(p: &Path) -> PathBuf {
    // Best-effort normalization for Windows; avoid failing on non-existent paths.
    match p.canonicalize() {
        Ok(c) => c,
        Err(_) => p.to_path_buf(),
    }
}
