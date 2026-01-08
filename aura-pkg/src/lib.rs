#![forbid(unsafe_code)]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer as _, Verifier as _};
use miette::{IntoDiagnostic, Report};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Public module exports for metadata, signing, resolver, and CLI
pub mod metadata;
pub mod signing;
pub mod resolver;
pub mod cli;

pub use metadata::PackageMetadata;
pub use signing::{PackageSigningKey, PackageVerifyingKey, PackageSignature};
pub use resolver::{DependencyResolver, PackageRegistry, ResolvedDependencies};
pub use cli::{Cli, Commands, InitArgs, AddArgs, RemoveArgs, ListArgs, PublishArgs, VerifyArgs};

pub type PkgError = Report;

fn pkg_msg(message: impl Into<String>) -> PkgError {
    Report::msg(message.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKind {
    WindowsX64Msvc,
    Other,
}

pub fn detect_host() -> HostKind {
    // Enough for Stage 18: we only auto-resolve Windows x64 MSVC artifacts for now.
    if cfg!(all(windows, target_arch = "x86_64")) {
        HostKind::WindowsX64Msvc
    } else {
        HostKind::Other
    }
}

#[derive(Clone, Debug)]
pub struct ProjectLayout {
    pub root: PathBuf,
    pub deps_dir: PathBuf,
    pub include_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub lock_path: PathBuf,
    pub manifest_path: PathBuf,
}

pub fn project_layout(project_root: &Path) -> ProjectLayout {
    let root = project_root.to_path_buf();
    ProjectLayout {
        deps_dir: root.join("deps"),
        include_dir: root.join("include"),
        cache_dir: root.join(".aura").join("pkg-cache"),
        lock_path: root.join("aura.lock"),
        manifest_path: root.join("aura.toml"),
        root,
    }
}

#[derive(Clone, Debug)]
pub struct AddOptions {
    pub package: String,
    pub version: Option<String>,
    pub url: Option<String>,
    pub smoke_test: bool,
    pub force: bool,

    /// Optional registry root. Supports:
    /// - local directory path (preferred, enables offline workflows)
    /// - http(s) URL
    pub registry: Option<String>,

    /// If true, fail if the registry entry does not include a signature.
    pub require_signature: bool,

    /// If provided, verify signatures for signed releases.
    /// File format: hex-encoded 32-byte ed25519 public key.
    pub trusted_public_key: Option<PathBuf>,

    /// If true, fail when selecting a deprecated package version.
    pub deny_deprecated: bool,
}

#[derive(Clone, Debug)]
pub struct InstallResult {
    pub package: String,
    pub version: String,
    pub source_url: String,
    pub sha256: String,
    pub checksum_status: ChecksumStatus,
    pub installed_libs: Vec<PathBuf>,
    pub installed_dlls: Vec<PathBuf>,
    pub installed_headers: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChecksumStatus {
    /// Downloaded artifact matched the already-pinned hash in aura.lock.
    Verified,
    /// First time seeing this package; wrote a new lock entry.
    Recorded,
    /// User forced an update; lock was updated.
    Updated,
}

impl std::fmt::Display for ChecksumStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChecksumStatus::Verified => write!(f, "Verified checksum"),
            ChecksumStatus::Recorded => write!(f, "Recorded checksum"),
            ChecksumStatus::Updated => write!(f, "Updated checksum"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct AuraLock {
    #[serde(default)]
    packages: std::collections::BTreeMap<String, LockedPackage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LockedPackage {
    version: String,
    url: String,
    sha256: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    registry: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature_key_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub package: String,
    #[serde(default)]
    pub versions: Vec<RegistryVersion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryVersion {
    pub version: String,
    pub url: String,
    pub sha256: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_key_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deprecation {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

pub fn add_package(project_root: &Path, opts: &AddOptions) -> Result<InstallResult, PkgError> {
    let layout = project_layout(project_root);
    fs::create_dir_all(&layout.deps_dir).into_diagnostic()?;
    fs::create_dir_all(&layout.include_dir).into_diagnostic()?;
    fs::create_dir_all(&layout.cache_dir).into_diagnostic()?;

    let host = detect_host();
    if host != HostKind::WindowsX64Msvc {
        return Err(pkg_msg(
            "Stage 18: only Windows x64 artifact retrieval is implemented",
        ));
    }

    // If a registry is provided, use the registry workflow.
    if opts.registry.is_some() {
        return install_from_registry(&layout, opts);
    }

    // Back-compat: legacy, hardcoded native packages with discovery.
    let pkg = opts.package.to_ascii_lowercase();
    match pkg.as_str() {
        "raylib" => install_raylib(&layout, opts),
        "onnxruntime" | "onnx" | "ort" => install_onnxruntime(&layout, opts),
        other => Err(pkg_msg(format!(
            "unknown package '{other}'. Provide --registry for registry packages. Try: aura pkg add raylib | onnxruntime"
        ))),
    }
}

fn install_from_registry(layout: &ProjectLayout, opts: &AddOptions) -> Result<InstallResult, PkgError> {
    let registry = opts
        .registry
        .as_ref()
        .ok_or_else(|| pkg_msg("missing registry"))?;

    let index = load_registry_index(registry, &opts.package)?;
    let req = parse_version_req(opts.version.as_deref())?;
    let selected = select_version(&index, req.as_ref())?;

    if let Some(dep) = &selected.deprecated {
        let mut msg = format!("deprecated package version {} {}: {}", opts.package, selected.version, dep.message);
        if let Some(r) = &dep.replaced_by {
            msg.push_str(&format!(" (replaced by {r})"));
        }
        if opts.deny_deprecated {
            return Err(pkg_msg(msg));
        }
        eprintln!("warning: {msg}");
    }

    if opts.require_signature && selected.signature.is_none() {
        return Err(pkg_msg(format!(
            "registry entry for {}@{} is not signed (use without --require-signature or publish with signing)",
            opts.package, selected.version
        )));
    }

    // Resolve URL relative to registry root.
    let resolved_url = resolve_registry_url(registry, &opts.package, &selected.url);

    let cache_pkg_dir = layout
        .cache_dir
        .join(sanitize_component(&opts.package))
        .join(sanitize_component(&selected.version));
    fs::create_dir_all(&cache_pkg_dir).into_diagnostic()?;
    let zip_path = cache_pkg_dir.join("artifact.zip");

    let zip_bytes = if zip_path.exists() && !opts.force {
        fs::read(&zip_path).into_diagnostic()?
    } else {
        let bytes = download_maybe_file_url(&resolved_url)?;
        fs::write(&zip_path, &bytes).into_diagnostic()?;
        bytes
    };

    let sha256 = sha256_hex(&zip_bytes);
    if sha256 != selected.sha256 {
        return Err(pkg_msg(format!(
            "artifact hash mismatch for {}@{}. registry sha256={}, downloaded={}",
            opts.package, selected.version, selected.sha256, sha256
        )));
    }

    // Optional signature verification.
    if let (Some(sig_b64), Some(pubkey_path)) = (&selected.signature, opts.trusted_public_key.as_ref()) {
        verify_signature_over_sha256(pubkey_path, &sha256, sig_b64).map_err(|e| {
            pkg_msg(format!("signature verification failed for {}@{}: {e}", opts.package, selected.version))
        })?;
    }

    // TOFU lock: verify or record.
    let mut lock = read_lock(&layout.lock_path)?;
    let existing = lock.packages.get(&opts.package).cloned();
    if let Some(existing) = &existing {
        if !opts.force && existing.sha256 != sha256 {
            return Err(pkg_msg(format!(
                "{} artifact hash mismatch. locked={}, downloaded={}. Use --force to update lock.",
                opts.package, existing.sha256, sha256
            )));
        }
    }

    let checksum_status = if opts.force {
        ChecksumStatus::Updated
    } else if existing
        .as_ref()
        .is_some_and(|e| e.sha256 == sha256)
    {
        ChecksumStatus::Verified
    } else {
        ChecksumStatus::Recorded
    };

    lock.packages.insert(
        opts.package.clone(),
        LockedPackage {
            version: selected.version.clone(),
            url: resolved_url.clone(),
            sha256: sha256.clone(),
            registry: Some(registry.clone()),
            signature: selected.signature.clone(),
            signature_key_id: selected.signature_key_id.clone(),
        },
    );
    write_lock(&layout.lock_path, &lock)?;

    let (libs, dlls, headers) = extract_zip_layout_zip(&zip_bytes, layout)?;

    Ok(InstallResult {
        package: opts.package.clone(),
        version: selected.version.clone(),
        source_url: resolved_url,
        sha256,
        checksum_status,
        installed_libs: libs,
        installed_dlls: dlls,
        installed_headers: headers,
    })
}

fn parse_version_req(s: Option<&str>) -> Result<Option<VersionReq>, PkgError> {
    let Some(s) = s.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    // If the user gives a plain version like "1.2.3", treat it as exact.
    if Version::parse(s).is_ok() {
        let exact = format!("={s}");
        return VersionReq::parse(&exact)
            .map(Some)
            .map_err(|e| pkg_msg(format!("invalid version '{s}': {e}")));
    }

    VersionReq::parse(s)
        .map(Some)
        .map_err(|e| pkg_msg(format!("invalid version requirement '{s}': {e}")))
}

fn select_version<'a>(index: &'a RegistryIndex, req: Option<&VersionReq>) -> Result<&'a RegistryVersion, PkgError> {
    let mut candidates: Vec<(&RegistryVersion, Version)> = Vec::new();
    for v in &index.versions {
        let ver = Version::parse(v.version.trim()).map_err(|e| {
            pkg_msg(format!("registry contains non-semver version '{}' for {}: {e}", v.version, index.package))
        })?;
        if let Some(req) = req {
            if !req.matches(&ver) {
                continue;
            }
        }
        candidates.push((v, ver));
    }

    candidates.sort_by(|a, b| a.1.cmp(&b.1));
    candidates
        .last()
        .map(|(v, _)| *v)
        .ok_or_else(|| {
            let req_s = req.map(|r| r.to_string()).unwrap_or_else(|| "(any)".to_string());
            pkg_msg(format!("no matching versions for {} {req_s}", index.package))
        })
}

fn load_registry_index(registry_root: &str, package: &str) -> Result<RegistryIndex, PkgError> {
    let index_url = registry_index_location(registry_root, package);
    let bytes = download_maybe_file_url(&index_url)?;
    serde_json::from_slice::<RegistryIndex>(&bytes)
        .map_err(|e| pkg_msg(format!("failed to parse registry index for {package}: {e}")))
}

fn registry_index_location(registry_root: &str, package: &str) -> String {
    let pkg_path = package.replace('\\', "/");
    if registry_root.starts_with("http://") || registry_root.starts_with("https://") {
        format!("{}/{}{}", registry_root.trim_end_matches('/'), pkg_path, "/index.json")
    } else {
        // Local directory path.
        let mut p = PathBuf::from(registry_root);
        for seg in pkg_path.split('/') {
            if seg.is_empty() {
                continue;
            }
            p.push(seg);
        }
        p.push("index.json");
        format!("file://{}", p.to_string_lossy())
    }
}

fn resolve_registry_url(registry_root: &str, package: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("file://") {
        return url.to_string();
    }

    let pkg_path = package.replace('\\', "/");

    if registry_root.starts_with("http://") || registry_root.starts_with("https://") {
        format!(
            "{}/{}/{}",
            registry_root.trim_end_matches('/'),
            pkg_path.trim_start_matches('/').trim_end_matches('/'),
            url.trim_start_matches('/')
        )
    } else {
        // Local directory path.
        let mut p = PathBuf::from(registry_root);
        for seg in pkg_path.split('/') {
            if seg.is_empty() {
                continue;
            }
            p.push(seg);
        }
        for seg in url.replace('\\', "/").split('/') {
            if seg.is_empty() {
                continue;
            }
            p.push(seg);
        }
        format!("file://{}", p.to_string_lossy())
    }
}

fn download_maybe_file_url(url: &str) -> Result<Vec<u8>, PkgError> {
    if let Some(path) = url.strip_prefix("file://") {
        return fs::read(path).into_diagnostic();
    }
    download_url(url)
}

fn verify_signature_over_sha256(public_key_path: &Path, sha256_hex_str: &str, sig_b64: &str) -> Result<(), String> {
    let pk_hex = fs::read_to_string(public_key_path).map_err(|e| e.to_string())?;
    let pk_hex = pk_hex.trim();
    let pk_bytes = hex::decode(pk_hex).map_err(|e| format!("invalid public key hex: {e}"))?;
    if pk_bytes.len() != 32 {
        return Err("public key must be 32 bytes (hex-encoded)".to_string());
    }
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes.try_into().unwrap())
        .map_err(|e| format!("invalid public key: {e}"))?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64.trim())
        .map_err(|e| format!("invalid signature base64: {e}"))?;
    let signature = Signature::from_slice(&sig_bytes).map_err(|e| format!("invalid signature bytes: {e}"))?;

    let msg = hex::decode(sha256_hex_str).map_err(|e| format!("invalid sha256 hex: {e}"))?;
    verifying_key
        .verify(&msg, &signature)
        .map_err(|e| format!("signature mismatch: {e}"))
}

/// Extracts a registry-published zip (expects `deps/**` and `include/**`).
fn extract_zip_layout_zip(zip_bytes: &[u8], layout: &ProjectLayout) -> Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>), PkgError> {
    use zip::ZipArchive;
    let reader = std::io::Cursor::new(zip_bytes);
    let mut zip = ZipArchive::new(reader).into_diagnostic()?;

    let mut libs = Vec::new();
    let mut dlls = Vec::new();
    let mut headers = Vec::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).into_diagnostic()?;
        let name = file.name().replace('\\', "/");
        if name.ends_with('/') {
            continue;
        }

        if let Some(rel) = name.strip_prefix("deps/") {
            let out_path = layout.deps_dir.join(rel);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).into_diagnostic()?;
            }
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).into_diagnostic()?;
            fs::write(&out_path, &buf).into_diagnostic()?;
            let is_lib = out_path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lib"));
            let is_dll = out_path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dll"));
            if is_lib {
                libs.push(out_path.clone());
            }
            if is_dll {
                dlls.push(out_path);
            }
            continue;
        }

        if let Some(rel) = name.strip_prefix("include/") {
            let out_path = layout.include_dir.join(rel);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).into_diagnostic()?;
            }
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).into_diagnostic()?;
            fs::write(&out_path, &buf).into_diagnostic()?;
            headers.push(out_path);
            continue;
        }
    }

    Ok((libs, dlls, headers))
}

pub struct PublishOptions {
    pub package: String,
    pub version: String,
    pub registry_dir: PathBuf,
    pub from_dir: PathBuf,
    /// Optional signing key file (hex-encoded 32-byte ed25519 secret key).
    pub signing_key: Option<PathBuf>,
    pub signature_key_id: Option<String>,
}

pub fn publish_package(opts: &PublishOptions) -> Result<(String, String), PkgError> {
    let zip_bytes = build_registry_zip(&opts.from_dir)?;
    let sha256 = sha256_hex(&zip_bytes);

    let (sig_b64, key_id) = if let Some(sk_path) = &opts.signing_key {
        let sig_b64 = sign_sha256_hex(sk_path, &sha256)?;
        (Some(sig_b64), opts.signature_key_id.clone())
    } else {
        (None, None)
    };

    let mut pkg_dir = opts.registry_dir.clone();
    for seg in opts.package.replace('\\', "/").split('/') {
        if seg.is_empty() {
            continue;
        }
        pkg_dir.push(seg);
    }
    fs::create_dir_all(&pkg_dir).into_diagnostic()?;

    let artifact_rel = format!("{}.zip", opts.version);
    let artifact_path = pkg_dir.join(&artifact_rel);
    fs::write(&artifact_path, &zip_bytes).into_diagnostic()?;

    let index_path = pkg_dir.join("index.json");
    let mut index = if index_path.exists() {
        let b = fs::read(&index_path).into_diagnostic()?;
        serde_json::from_slice::<RegistryIndex>(&b)
            .map_err(|e| pkg_msg(format!("failed to parse existing index.json: {e}")))?
    } else {
        RegistryIndex {
            package: opts.package.clone(),
            versions: Vec::new(),
        }
    };

    // Upsert version.
    index.versions.retain(|v| v.version != opts.version);
    index.versions.push(RegistryVersion {
        version: opts.version.clone(),
        url: artifact_rel.clone(),
        sha256: sha256.clone(),
        signature: sig_b64.clone(),
        signature_key_id: key_id.clone(),
        deprecated: None,
    });

    // Ensure semver sorting in index.
    index.versions.sort_by(|a, b| {
        let av = Version::parse(a.version.trim());
        let bv = Version::parse(b.version.trim());
        match (av, bv) {
            (Ok(av), Ok(bv)) => av.cmp(&bv),
            _ => a.version.cmp(&b.version),
        }
    });

    let out = serde_json::to_vec_pretty(&index).into_diagnostic()?;
    fs::write(&index_path, out).into_diagnostic()?;

    Ok((sha256, sig_b64.unwrap_or_default()))
}

pub struct DeprecateOptions {
    pub package: String,
    pub version: String,
    pub registry_dir: PathBuf,
    pub message: String,
    pub replaced_by: Option<String>,
}

pub fn deprecate_version(opts: &DeprecateOptions) -> Result<(), PkgError> {
    let mut pkg_dir = opts.registry_dir.clone();
    for seg in opts.package.replace('\\', "/").split('/') {
        if seg.is_empty() {
            continue;
        }
        pkg_dir.push(seg);
    }
    let index_path = pkg_dir.join("index.json");
    if !index_path.exists() {
        return Err(pkg_msg("package not found in registry"));
    }

    let b = fs::read(&index_path).into_diagnostic()?;
    let mut index = serde_json::from_slice::<RegistryIndex>(&b)
        .map_err(|e| pkg_msg(format!("failed to parse index.json: {e}")))?;

    let mut found = false;
    for v in &mut index.versions {
        if v.version == opts.version {
            v.deprecated = Some(Deprecation {
                message: opts.message.clone(),
                replaced_by: opts.replaced_by.clone(),
                since: None,
            });
            found = true;
        }
    }
    if !found {
        return Err(pkg_msg("version not found in registry"));
    }

    let out = serde_json::to_vec_pretty(&index).into_diagnostic()?;
    fs::write(&index_path, out).into_diagnostic()?;
    Ok(())
}

fn build_registry_zip(from_dir: &Path) -> Result<Vec<u8>, PkgError> {
    use zip::write::SimpleFileOptions;

    let deps = from_dir.join("deps");
    let include = from_dir.join("include");
    if !deps.exists() && !include.exists() {
        return Err(pkg_msg("publish source must contain deps/ and/or include/"));
    }

    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if deps.exists() {
        zip_dir_recursive(&mut zip, from_dir, &deps, "deps", opts)?;
    }
    if include.exists() {
        zip_dir_recursive(&mut zip, from_dir, &include, "include", opts)?;
    }

    let cursor = zip.finish().into_diagnostic()?;
    Ok(cursor.into_inner())
}

fn zip_dir_recursive<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    root: &Path,
    dir: &Path,
    prefix: &str,
    opts: zip::write::SimpleFileOptions,
) -> Result<(), PkgError> {
    for entry in fs::read_dir(dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel = rel.to_string_lossy().replace('\\', "/");
        let name = if rel.starts_with(prefix) { rel } else { format!("{prefix}/{}", rel.trim_start_matches("./")) };

        if path.is_dir() {
            zip.add_directory(format!("{}/", name.trim_end_matches('/')), opts)
                .into_diagnostic()?;
            zip_dir_recursive(zip, root, &path, prefix, opts)?;
        } else {
            zip.start_file(name, opts).into_diagnostic()?;
            let bytes = fs::read(&path).into_diagnostic()?;
            zip.write_all(&bytes).into_diagnostic()?;
        }
    }
    Ok(())
}

fn sign_sha256_hex(signing_key_path: &Path, sha256_hex_str: &str) -> Result<String, PkgError> {
    let sk_hex = fs::read_to_string(signing_key_path).into_diagnostic()?;
    let sk_hex = sk_hex.trim();
    let sk_bytes = hex::decode(sk_hex)
        .map_err(|e| pkg_msg(format!("invalid signing key hex: {e}")))?;
    if sk_bytes.len() != 32 {
        return Err(pkg_msg("signing key must be 32 bytes (hex-encoded)"));
    }
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&sk_bytes.try_into().unwrap());
    let msg = hex::decode(sha256_hex_str)
        .map_err(|e| pkg_msg(format!("invalid sha256 hex: {e}")))?;
    let sig = signing_key.sign(&msg);
    Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_hex(path: &Path, bytes: &[u8]) {
        fs::write(path, hex::encode(bytes)).unwrap();
    }

    #[test]
    fn registry_publish_and_install_resolves_semver_and_writes_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = tmp.path().join("registry");
        let pkg_src = tmp.path().join("pkg_src");
        let proj = tmp.path().join("proj");
        fs::create_dir_all(&reg).unwrap();
        fs::create_dir_all(pkg_src.join("deps")).unwrap();
        fs::create_dir_all(pkg_src.join("include")).unwrap();
        fs::create_dir_all(&proj).unwrap();

        // Dummy artifacts.
        fs::write(pkg_src.join("deps").join("foo.lib"), b"lib").unwrap();
        fs::write(pkg_src.join("include").join("foo.h"), b"// header").unwrap();

        publish_package(&PublishOptions {
            package: "acme/foo".to_string(),
            version: "1.0.0".to_string(),
            registry_dir: reg.clone(),
            from_dir: pkg_src.clone(),
            signing_key: None,
            signature_key_id: None,
        })
        .unwrap();

        // Publish a newer version.
        fs::write(pkg_src.join("deps").join("foo.lib"), b"lib2").unwrap();
        publish_package(&PublishOptions {
            package: "acme/foo".to_string(),
            version: "1.2.0".to_string(),
            registry_dir: reg.clone(),
            from_dir: pkg_src.clone(),
            signing_key: None,
            signature_key_id: None,
        })
        .unwrap();

        let res = add_package(
            &proj,
            &AddOptions {
                package: "acme/foo".to_string(),
                version: Some("^1.0".to_string()),
                url: None,
                smoke_test: false,
                force: false,
                registry: Some(reg.to_string_lossy().to_string()),
                require_signature: false,
                trusted_public_key: None,
                deny_deprecated: false,
            },
        )
        .unwrap();

        assert_eq!(res.version, "1.2.0");
        assert!(proj.join("aura.lock").exists());
        assert!(proj.join("deps").join("foo.lib").exists());
        assert!(proj.join("include").join("foo.h").exists());
    }

    #[test]
    fn registry_deprecation_can_be_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = tmp.path().join("registry");
        let pkg_src = tmp.path().join("pkg_src");
        let proj = tmp.path().join("proj");
        fs::create_dir_all(&reg).unwrap();
        fs::create_dir_all(pkg_src.join("deps")).unwrap();
        fs::create_dir_all(&proj).unwrap();
        fs::write(pkg_src.join("deps").join("foo.lib"), b"lib").unwrap();

        publish_package(&PublishOptions {
            package: "acme/foo".to_string(),
            version: "1.0.0".to_string(),
            registry_dir: reg.clone(),
            from_dir: pkg_src.clone(),
            signing_key: None,
            signature_key_id: None,
        })
        .unwrap();

        deprecate_version(&DeprecateOptions {
            package: "acme/foo".to_string(),
            version: "1.0.0".to_string(),
            registry_dir: reg.clone(),
            message: "use acme/foo2".to_string(),
            replaced_by: Some("acme/foo2".to_string()),
        })
        .unwrap();

        let err = add_package(
            &proj,
            &AddOptions {
                package: "acme/foo".to_string(),
                version: Some("=1.0.0".to_string()),
                url: None,
                smoke_test: false,
                force: false,
                registry: Some(reg.to_string_lossy().to_string()),
                require_signature: false,
                trusted_public_key: None,
                deny_deprecated: true,
            },
        )
        .expect_err("expected deny_deprecated to fail");

        let msg = format!("{err:?}");
        assert!(msg.contains("deprecated"));
    }

    #[test]
    fn registry_signature_is_verified_when_key_provided() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = tmp.path().join("registry");
        let pkg_src = tmp.path().join("pkg_src");
        let proj = tmp.path().join("proj");
        fs::create_dir_all(&reg).unwrap();
        fs::create_dir_all(pkg_src.join("deps")).unwrap();
        fs::create_dir_all(&proj).unwrap();
        fs::write(pkg_src.join("deps").join("foo.lib"), b"lib").unwrap();

        // Deterministic signing key for test.
        let sk_bytes = [7u8; 32];
        let sk_path = tmp.path().join("sk.hex");
        write_hex(&sk_path, &sk_bytes);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let vk_bytes = signing_key.verifying_key().to_bytes();
        let vk_path = tmp.path().join("vk.hex");
        write_hex(&vk_path, &vk_bytes);

        publish_package(&PublishOptions {
            package: "acme/foo".to_string(),
            version: "1.0.0".to_string(),
            registry_dir: reg.clone(),
            from_dir: pkg_src.clone(),
            signing_key: Some(sk_path),
            signature_key_id: Some("test".to_string()),
        })
        .unwrap();

        let res = add_package(
            &proj,
            &AddOptions {
                package: "acme/foo".to_string(),
                version: Some("=1.0.0".to_string()),
                url: None,
                smoke_test: false,
                force: false,
                registry: Some(reg.to_string_lossy().to_string()),
                require_signature: true,
                trusted_public_key: Some(vk_path),
                deny_deprecated: false,
            },
        )
        .unwrap();

        assert_eq!(res.version, "1.0.0");
    }
}

fn install_onnxruntime(layout: &ProjectLayout, opts: &AddOptions) -> Result<InstallResult, PkgError> {
    let (version, url) = resolve_onnxruntime_source(opts)?;

    let cache_pkg_dir = layout
        .cache_dir
        .join("onnxruntime")
        .join(sanitize_component(&version));
    fs::create_dir_all(&cache_pkg_dir).into_diagnostic()?;

    let zip_path = cache_pkg_dir.join("onnxruntime.zip");

    let zip_bytes = if zip_path.exists() && !opts.force {
        fs::read(&zip_path).into_diagnostic()?
    } else {
        let bytes = download_url(&url)?;
        fs::write(&zip_path, &bytes).into_diagnostic()?;
        bytes
    };

    let sha256 = sha256_hex(&zip_bytes);

    let mut lock = read_lock(&layout.lock_path)?;
    let existing = lock.packages.get("onnxruntime").cloned();
    if let Some(existing) = &existing {
        if !opts.force && existing.sha256 != sha256 {
            return Err(pkg_msg(format!(
                "onnxruntime artifact hash mismatch. locked={}, downloaded={}. Use --force to update lock.",
                existing.sha256, sha256
            )));
        }
    }

    let checksum_status = if opts.force {
        ChecksumStatus::Updated
    } else if existing
        .as_ref()
        .is_some_and(|e| e.sha256 == sha256)
    {
        ChecksumStatus::Verified
    } else {
        ChecksumStatus::Recorded
    };

    lock.packages.insert(
        "onnxruntime".to_string(),
        LockedPackage {
            version: version.clone(),
            url: url.clone(),
            sha256: sha256.clone(),
            registry: None,
            signature: None,
            signature_key_id: None,
        },
    );
    write_lock(&layout.lock_path, &lock)?;

    let (libs, dlls, headers) = extract_zip_selective(&zip_bytes, layout)?;

    Ok(InstallResult {
        package: "onnxruntime".to_string(),
        version,
        source_url: url,
        sha256,
        checksum_status,
        installed_libs: libs,
        installed_dlls: dlls,
        installed_headers: headers,
    })
}

fn resolve_onnxruntime_source(opts: &AddOptions) -> Result<(String, String), PkgError> {
    if let Some(url) = &opts.url {
        let version = opts
            .version
            .clone()
            .unwrap_or_else(|| "custom".to_string());
        return Ok((version, url.clone()));
    }

    let api = "https://api.github.com/repos/microsoft/onnxruntime/releases/latest";
    let rel = github_latest_release(api)?;

    let mut candidates: Vec<_> = rel
        .assets
        .into_iter()
        .filter(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        .collect();

    candidates.sort_by_key(|a| score_onnxruntime_asset(&a.name));
    candidates.reverse();

    let best = candidates
        .into_iter()
        .next()
        .ok_or_else(|| pkg_msg("could not find a .zip asset in onnxruntime latest release"))?;

    let url = best.browser_download_url;
    if !url.starts_with("https://github.com/") {
        return Err(pkg_msg(format!(
            "refusing to download from unexpected host: {url}"
        )));
    }

    let version = rel
        .tag_name
        .trim()
        .trim_start_matches('v')
        .to_string();

    Ok((version, url))
}

fn score_onnxruntime_asset(name: &str) -> i32 {
    let n = name.to_ascii_lowercase();
    let mut score = 0;

    // Prefer official Windows x64 CPU zip.
    if n.contains("win") {
        score += 10;
    }
    if n.contains("win-x64") || n.contains("windows-x64") || n.contains("x64") {
        score += 40;
    }
    if n.contains("arm64") {
        score -= 50;
    }
    if n.contains("nuget") {
        score -= 200;
    }
    if n.contains("training") {
        score -= 100;
    }
    if n.contains("gpu") {
        score -= 20;
    }
    if n.contains("zip") {
        score += 1;
    }

    score
}

fn install_raylib(layout: &ProjectLayout, opts: &AddOptions) -> Result<InstallResult, PkgError> {
    let (version, url) = resolve_raylib_source(opts)?;

    let cache_pkg_dir = layout
        .cache_dir
        .join("raylib")
        .join(sanitize_component(&version));
    fs::create_dir_all(&cache_pkg_dir).into_diagnostic()?;

    let zip_path = cache_pkg_dir.join("raylib.zip");

    let zip_bytes = if zip_path.exists() && !opts.force {
        fs::read(&zip_path).into_diagnostic()?
    } else {
        let bytes = download_url(&url)?;
        fs::write(&zip_path, &bytes).into_diagnostic()?;
        bytes
    };

    let sha256 = sha256_hex(&zip_bytes);

    // TOFU lock: if already locked, verify. Otherwise write lock.
    let mut lock = read_lock(&layout.lock_path)?;
    let existing = lock.packages.get("raylib").cloned();
    if let Some(existing) = &existing {
        if !opts.force && existing.sha256 != sha256 {
            return Err(pkg_msg(format!(
                "raylib artifact hash mismatch. locked={}, downloaded={}. Use --force to update lock.",
                existing.sha256, sha256
            )));
        }
    }

    let checksum_status = if opts.force {
        ChecksumStatus::Updated
    } else if existing
        .as_ref()
        .is_some_and(|e| e.sha256 == sha256)
    {
        ChecksumStatus::Verified
    } else {
        ChecksumStatus::Recorded
    };

    lock.packages.insert(
        "raylib".to_string(),
        LockedPackage {
            version: version.clone(),
            url: url.clone(),
            sha256: sha256.clone(),
            registry: None,
            signature: None,
            signature_key_id: None,
        },
    );
    write_lock(&layout.lock_path, &lock)?;

    // Extract
    let (libs, dlls, headers) = extract_zip_selective(&zip_bytes, layout)?;

    Ok(InstallResult {
        package: "raylib".to_string(),
        version,
        source_url: url,
        sha256,
        checksum_status,
        installed_libs: libs,
        installed_dlls: dlls,
        installed_headers: headers,
    })
}

fn resolve_raylib_source(opts: &AddOptions) -> Result<(String, String), PkgError> {
    if let Some(url) = &opts.url {
        let version = opts
            .version
            .clone()
            .unwrap_or_else(|| "custom".to_string());
        return Ok((version, url.clone()));
    }

    // Zero-config path: query GitHub Releases API.
    // We only accept downloads from api.github.com / github.com for safety.
    let api = "https://api.github.com/repos/raysan5/raylib/releases/latest";
    let rel = github_latest_release(api)?;

    // Heuristic: prefer a Windows x64 MSVC zip.
    let mut candidates: Vec<_> = rel
        .assets
        .into_iter()
        .filter(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        .collect();

    candidates.sort_by_key(|a| score_raylib_asset(&a.name));
    candidates.reverse();

    let best = candidates
        .into_iter()
        .next()
        .ok_or_else(|| pkg_msg("could not find a .zip asset in raylib latest release"))?;

    let url = best.browser_download_url;
    if !url.starts_with("https://github.com/") {
        return Err(pkg_msg(format!(
            "refusing to download from unexpected host: {url}"
        )));
    }

    let version = rel
        .tag_name
        .trim()
        .trim_start_matches('v')
        .to_string();

    Ok((version, url))
}

fn score_raylib_asset(name: &str) -> i32 {
    let n = name.to_ascii_lowercase();
    let mut score = 0;
    if n.contains("win") {
        score += 10;
    }
    if n.contains("win64") || n.contains("x64") {
        score += 10;
    }
    if n.contains("msvc") {
        score += 20;
    }
    if n.contains("mingw") {
        score -= 10;
    }
    if n.contains("src") || n.contains("source") {
        score -= 50;
    }
    score
}

#[derive(Clone, Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn github_latest_release(url: &str) -> Result<GhRelease, PkgError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("aura-pkg/0.1")
        .build()
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("failed to build HTTP client: {e}")))?;

    let resp = client
        .get(url)
        .send()
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("failed to fetch {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(pkg_msg(format!(
            "GitHub API returned {} for {url}",
            resp.status()
        )));
    }

    resp.json::<GhRelease>()
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("failed to parse GitHub API JSON: {e}")))
}

fn download_url(url: &str) -> Result<Vec<u8>, PkgError> {
    if !(url.starts_with("https://github.com/") || url.starts_with("https://objects.githubusercontent.com/")) {
        return Err(pkg_msg(format!(
            "refusing to download from unexpected host: {url}"
        )));
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent("aura-pkg/0.1")
        .build()
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("failed to build HTTP client: {e}")))?;

    let mut resp = client
        .get(url)
        .send()
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("download failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(pkg_msg(format!(
            "download HTTP {} from {url}",
            resp.status()
        )));
    }

    let mut buf = Vec::new();
    resp.copy_to(&mut buf)
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("download read failed: {e}")))?;
    Ok(buf)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    hex::encode(digest)
}

fn sanitize_component(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
        .collect::<String>()
}

fn read_lock(path: &Path) -> Result<AuraLock, PkgError> {
    if !path.exists() {
        return Ok(AuraLock::default());
    }
    let raw = fs::read_to_string(path).into_diagnostic()?;
    toml::from_str(&raw)
        .map_err(|e| pkg_msg(format!("failed to parse {}: {e}", path.display())))
}

fn write_lock(path: &Path, lock: &AuraLock) -> Result<(), PkgError> {
    let s = toml::to_string_pretty(lock).into_diagnostic()?;
    fs::write(path, s).into_diagnostic()?;
    Ok(())
}

fn extract_zip_selective(
    zip_bytes: &[u8],
    layout: &ProjectLayout,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>), PkgError> {
    let reader = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .into_diagnostic()
        .map_err(|e| pkg_msg(format!("zip open failed: {e}")))?;

    let mut libs = Vec::new();
    let mut dlls = Vec::new();
    let mut headers = Vec::new();

    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .into_diagnostic()
            .map_err(|e| pkg_msg(format!("zip entry read failed: {e}")))?;

        if f.is_dir() {
            continue;
        }

        let name = f.name().replace('\\', "/");
        let lower = name.to_ascii_lowercase();

        // Headers
        if lower.ends_with(".h") {
            let file_name = Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| pkg_msg("zip entry has invalid filename"))?;
            let out_path = layout.include_dir.join(file_name);
            write_zip_file(&mut f, &out_path)?;
            headers.push(out_path);
            continue;
        }

        // Binaries
        if lower.ends_with(".lib") {
            let file_name = Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| pkg_msg("zip entry has invalid filename"))?;
            let out_path = layout.deps_dir.join(file_name);
            write_zip_file(&mut f, &out_path)?;
            libs.push(out_path);
            continue;
        }

        if lower.ends_with(".dll") {
            let file_name = Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| pkg_msg("zip entry has invalid filename"))?;
            let out_path = layout.deps_dir.join(file_name);
            write_zip_file(&mut f, &out_path)?;
            dlls.push(out_path);
            continue;
        }
    }

    Ok((libs, dlls, headers))
}

fn write_zip_file<R: Read>(mut src: R, out_path: &Path) -> Result<(), PkgError> {
    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut buf = Vec::new();
    src.read_to_end(&mut buf).into_diagnostic()?;

    // Write atomically.
    let tmp = out_path.with_extension("tmp");
    {
        let mut w = fs::File::create(&tmp).into_diagnostic()?;
        w.write_all(&buf).into_diagnostic()?;
        w.sync_all().ok();
    }
    fs::rename(tmp, out_path).into_diagnostic()?;
    Ok(())
}
