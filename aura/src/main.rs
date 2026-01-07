#![forbid(unsafe_code)]

use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::Instant,
};

use clap::{Parser, Subcommand};
use miette::{Diagnostic, IntoDiagnostic, NamedSource};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use thiserror::Error;

use sha2::Digest;

use aura_nexus::PluginManifest;
use aura_ir::DebugSource;
use aura_parse::ParseConfig;

use aura_interpret::{DebugCommand, DebugEvent, DebugHandle, DebugSession};

static DEBUG_SESSION: OnceLock<(DebugSession, DebugHandle)> = OnceLock::new();
static DEBUG_STDOUT_GUARD: OnceLock<std::sync::Arc<std::sync::Mutex<()>>> = OnceLock::new();

fn debug_protocol_enabled() -> bool {
    std::env::var("AURA_DEBUG_PROTOCOL").is_ok()
}

fn debug_pair() -> Option<(DebugSession, DebugHandle)> {
    if !debug_protocol_enabled() {
        return None;
    }

    let (sess, handle) = DEBUG_SESSION.get_or_init(|| {
        let guard = DEBUG_STDOUT_GUARD
            .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(())))
            .clone();
        let emit = std::sync::Arc::new(move |ev: DebugEvent| {
            let json = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
            let _g = guard.lock().expect("debug stdout guard poisoned");
            let mut out = io::stdout();
            let _ = writeln!(&mut out, "AURA_DEBUG_EVENT {json}");
            let _ = out.flush();
        });

        let (sess, handle) = DebugSession::new(emit);

        sess.emit(DebugEvent::Hello {
            protocol: DebugSession::PROTOCOL_VERSION,
            capabilities: vec![
                "devvm.pause".to_string(),
                "devvm.step".to_string(),
                "devvm.breakpoints".to_string(),
                "devvm.watches".to_string(),
                "perf.timeline".to_string(),
                "perf.flame.folded".to_string(),
                "perf.memory".to_string(),
                "native.launch".to_string(),
                "native.terminate".to_string(),
                "native.exit".to_string(),
            ],
        });

        // Background stdin reader: accept one JSON command per line.
        let h = handle.clone();
        std::thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines().flatten() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<DebugCommand>(trimmed) {
                    Ok(cmd) => h.send(cmd),
                    Err(e) => {
                        eprintln!("AURA_DEBUG_PROTOCOL: ignored invalid command: {e}: {trimmed}");
                    }
                }
            }
        });

        (sess, handle)
    });

    // Ensure debugger/perf is enabled by default when the protocol is active.
    handle.send(DebugCommand::Enable {
        start_paused: false,
        perf: true,
    });

    Some((sess.clone(), handle.clone()))
}

fn run_native_supervised(exe: &Path, sess: Option<&DebugSession>) -> miette::Result<()> {
    use std::process::Stdio;
    use std::time::Duration;

    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().into_diagnostic()?;

    // Stream stdout/stderr back through Aura so Sentinel can see output even
    // when we're supervising the child (and also for logging in terminals).
    let mut out_threads = Vec::new();

    let stdout_guard = DEBUG_STDOUT_GUARD
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(())))
        .clone();

    if let Some(stdout) = child.stdout.take() {
        let guard = stdout_guard.clone();
        out_threads.push(std::thread::spawn(move || {
            let mut r = io::BufReader::new(stdout);
            let mut buf: Vec<u8> = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let _g = guard.lock().expect("debug stdout guard poisoned");
                        let mut w = io::stdout();
                        let _ = w.write_all(&buf);
                        let _ = w.flush();
                    }
                    Err(_) => break,
                }
            }
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        out_threads.push(std::thread::spawn(move || {
            let mut w = io::stderr();
            let _ = io::copy(&mut io::BufReader::new(stderr), &mut w);
            let _ = w.flush();
        }));
    }

    // Poll for completion and debug commands.
    let mut emitted_terminated = false;
    loop {
        if let Some(sess) = sess {
            for c in sess.take_pending_commands() {
                sess.apply_command(c);
            }
            if sess.take_terminate_requested() {
                if !emitted_terminated {
                    sess.emit(DebugEvent::Terminated {
                        target: "native".to_string(),
                    });
                    emitted_terminated = true;
                }
                let _ = child.kill();
            }
        }

        match child.try_wait().into_diagnostic()? {
            Some(status) => {
                for t in out_threads {
                    let _ = t.join();
                }

                if let Some(sess) = sess {
                    sess.emit(DebugEvent::NativeExit {
                        code: status.code(),
                    });
                }

                if !status.success() {
                    return Err(miette::miette!("program exited with {}", status));
                }
                return Ok(());
            }
            None => {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn augment_with_sdk_std(src: &str) -> miette::Result<String> {
    // Best-effort stdlib injection for SDK installs.
    // Keep original offsets stable by appending std modules at EOF.
    aura_sdk::augment_source_with_default_std(src).into_diagnostic()
}

mod linker;
mod manifest;
mod report;

#[derive(Parser, Debug)]
#[command(name = "aura", version, about = "Aura (2026 Edition) prototype compiler")]
struct Cli {
    /// Language edition (e.g. "2026"). Overrides `aura.toml`.
    #[arg(long, global = true)]
    edition: Option<String>,

    /// Enable an unstable language feature (repeatable). Also configurable via `aura.toml`.
    #[arg(long, global = true)]
    feature: Vec<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

fn build_parse_config(
    cli_edition: &Option<String>,
    cli_features: &[String],
    resolved: &manifest::ResolvedManifest,
) -> ParseConfig {
    let mut cfg = ParseConfig::default();
    cfg.edition = cli_edition.clone().or_else(|| resolved.edition.clone());
    for f in resolved.features.iter().chain(cli_features.iter()) {
        cfg.features.insert(f.clone());
    }
    cfg
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Aura Virtual Machine (AVM) interpreter (development)
    Avm,
    /// Native compilation via LLVM backend (production)
    Llvm,
    /// AVM for `run`, LLVM for `build` (default)
    Hybrid,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
enum SmtProfileArg {
    Fast,
    Ci,
    Thorough,
}

impl From<SmtProfileArg> for aura_verify::SmtProfile {
    fn from(v: SmtProfileArg) -> Self {
        match v {
            SmtProfileArg::Fast => aura_verify::SmtProfile::Fast,
            SmtProfileArg::Ci => aura_verify::SmtProfile::Ci,
            SmtProfileArg::Thorough => aura_verify::SmtProfile::Thorough,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum BuildProfileArg {
    Dev,
    Release,
    Verify,
}

impl BuildProfileArg {
    fn default_optimize(&self) -> &'static str {
        match self {
            BuildProfileArg::Dev => "none",
            BuildProfileArg::Release => "full",
            BuildProfileArg::Verify => "none",
        }
    }

    fn default_smt_profile(&self) -> SmtProfileArg {
        match self {
            BuildProfileArg::Dev => SmtProfileArg::Fast,
            BuildProfileArg::Release => SmtProfileArg::Ci,
            BuildProfileArg::Verify => SmtProfileArg::Thorough,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Type-check + lower + emit backend artifacts
    Build {
        /// Input .aura file
        #[arg(default_value = "main.aura")]
        path: PathBuf,

        /// Build profile: `dev`, `release`, or `verify`
        #[arg(long, value_enum, default_value_t = BuildProfileArg::Dev)]
        profile: BuildProfileArg,

        /// Execution mode: `avm`, `llvm`, or `hybrid`
        #[arg(long, value_enum, default_value_t = Mode::Hybrid)]
        mode: Mode,

        /// Backend: `c` (C23 transpiler), `llvm` (LLVM IR), or `wasm` (wasm32-wasi via clang)
        #[arg(long, default_value = "c")]
        backend: String,

        /// One or more C/C++ headers to bridge into Aura (bootstrap parser)
        #[arg(long)]
        bridge: Vec<PathBuf>,

        /// Extra library search dirs to pass to the native linker (repeatable)
        #[arg(long = "link-dir")]
        link_dirs: Vec<PathBuf>,

        /// Extra libraries to link (repeatable). Accepts `foo` or `foo.lib`.
        #[arg(long = "link-lib")]
        link_libs: Vec<String>,

        /// Optimization level for LLVM post-pass: `none` or `full`
        #[arg(long)]
        optimize: Option<String>,

        /// SMT solver profile for verification: `fast`, `ci`, or `thorough`
        #[arg(long, value_enum)]
        smt_profile: Option<SmtProfileArg>,
    },
    /// Build and execute (when toolchain is available)
    Run {
        /// Input .aura file
        #[arg(default_value = "main.aura")]
        path: PathBuf,

        /// Build profile: `dev`, `release`, or `verify`
        #[arg(long, value_enum, default_value_t = BuildProfileArg::Dev)]
        profile: BuildProfileArg,

        /// Execution mode: `avm`, `llvm`, or `hybrid`
        #[arg(long, value_enum, default_value_t = Mode::Hybrid)]
        mode: Mode,

        /// Backend: `c` or `llvm`
        #[arg(long, default_value = "c")]
        backend: String,

        /// One or more C/C++ headers to bridge into Aura (bootstrap parser)
        #[arg(long)]
        bridge: Vec<PathBuf>,

        /// Extra library search dirs to pass to the native linker (repeatable)
        #[arg(long = "link-dir")]
        link_dirs: Vec<PathBuf>,

        /// Extra libraries to link (repeatable). Accepts `foo` or `foo.lib`.
        #[arg(long = "link-lib")]
        link_libs: Vec<String>,

        /// Optimization level for LLVM post-pass: `none` or `full`
        #[arg(long)]
        optimize: Option<String>,

        /// SMT solver profile for verification: `fast`, `ci`, or `thorough`
        #[arg(long, value_enum)]
        smt_profile: Option<SmtProfileArg>,

        /// Hot-reload monitoring: rebuild + restart when sources change
        #[arg(long, default_value_t = false)]
        hot: bool,
    },

    /// Verify a program (parse + sema + Z3 gate when enabled)
    Verify {
        /// Input .aura file or a directory containing `aura.toml`
        #[arg(default_value = "main.aura")]
        path: PathBuf,

        /// Build profile: `dev`, `release`, or `verify`
        #[arg(long, value_enum, default_value_t = BuildProfileArg::Verify)]
        profile: BuildProfileArg,

        /// SMT solver profile for verification: `fast`, `ci`, or `thorough`
        #[arg(long, value_enum)]
        smt_profile: Option<SmtProfileArg>,

        /// Write a machine-readable trusted core report (JSON).
        ///
        /// - If verifying a single file, this may be a file path.
        /// - If verifying a workspace (multiple targets), this must be a directory.
        #[arg(long)]
        report: Option<PathBuf>,
    },

    /// Run Aura tests (verifies all `tests/**/*.aura`)
    Test {
        /// Project directory (or any path inside it)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// SMT solver profile for verification: `fast`, `ci`, or `thorough`
        #[arg(long, value_enum, default_value_t = SmtProfileArg::Ci)]
        smt_profile: SmtProfileArg,
    },

    /// Lint Aura source (format check + parse/sema)
    Lint {
        /// Input .aura file or a project directory
        #[arg(default_value = "main.aura")]
        path: PathBuf,
    },

    /// Package manager (artifact discovery + install)
    Pkg {
        #[command(subcommand)]
        cmd: PkgCmd,
    },

    /// Initialize a new Aura project
    Init {
        /// Project directory to create (default: ./aura-project)
        #[arg(default_value = "aura-project")]
        path: PathBuf,
    },

    /// Format Aura source (canonical formatter)
    Fmt {
        /// Input .aura file
        #[arg(default_value = "main.aura")]
        path: PathBuf,

        /// Check formatting (exits non-zero if changes are needed)
        #[arg(long, default_value_t = false)]
        check: bool,

        /// Write formatted output back to the file
        #[arg(long, default_value_t = false)]
        write: bool,
    },

    /// Generate Aura bindings from C headers (Aura-Bindgen; bootstrap)
    Bindgen {
        /// One or more C/C++ headers to parse
        #[arg(long = "header", required = true)]
        headers: Vec<PathBuf>,

        /// Output directory for generated Aura shim + report
        #[arg(long, default_value = "build/bindgen")]
        out: PathBuf,

        /// Extra include dirs for future bindgen versions (currently unused)
        #[arg(long = "include-dir")]
        include_dirs: Vec<PathBuf>,

        /// Extra library search dirs to record in the generated report (repeatable)
        #[arg(long = "link-dir")]
        link_dirs: Vec<PathBuf>,

        /// Extra libraries to record in the generated report (repeatable)
        #[arg(long = "link-lib")]
        link_libs: Vec<String>,

        /// Disable caching of bindgen outputs
        #[arg(long, default_value_t = false)]
        no_cache: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PkgCmd {
    /// Add a dependency (downloads artifacts, updates aura.toml)
    Add {
        /// Package name (e.g., raylib)
        package: String,

        /// Optional version override (otherwise uses latest)
        #[arg(long)]
        version: Option<String>,

        /// Optional direct URL to a .zip artifact (bypasses discovery)
        #[arg(long)]
        url: Option<String>,

        /// Registry root (local directory path or http(s) URL)
        #[arg(long)]
        registry: Option<String>,

        /// Fail if the chosen registry version is deprecated
        #[arg(long, default_value_t = false)]
        deny_deprecated: bool,

        /// Require the registry entry to be signed
        #[arg(long, default_value_t = false)]
        require_signature: bool,

        /// Trusted ed25519 public key (hex-encoded 32 bytes) for signature verification
        #[arg(long)]
        trusted_key: Option<PathBuf>,

        /// Overwrite cached artifacts and lock entries
        #[arg(long, default_value_t = false)]
        force: bool,

        /// Disable post-install smoke test
        #[arg(long, default_value_t = false)]
        no_smoke: bool,
    },

    /// Publish a package artifact to a local registry directory
    Publish {
        /// Package id (supports namespacing like `aura/raylib`)
        package: String,

        /// Package version (SemVer)
        version: String,

        /// Registry directory to write into
        #[arg(long)]
        registry: PathBuf,

        /// Source directory containing `deps/` and/or `include/`
        #[arg(long)]
        from: PathBuf,

        /// Optional ed25519 signing key (hex-encoded 32 bytes)
        #[arg(long)]
        signing_key: Option<PathBuf>,

        /// Optional key id to record alongside the signature
        #[arg(long)]
        key_id: Option<String>,
    },

    /// Deprecate a published version in a local registry directory
    Deprecate {
        /// Package id (supports namespacing like `aura/raylib`)
        package: String,

        /// Package version (SemVer)
        version: String,

        /// Registry directory to edit
        #[arg(long)]
        registry: PathBuf,

        /// Deprecation message
        #[arg(long)]
        message: String,

        /// Optional replacement suggestion (package id)
        #[arg(long)]
        replaced_by: Option<String>,
    },
}

#[derive(Debug)]
struct BuildOutputs {
    out_dir: PathBuf,
    module_c: Option<PathBuf>,
    llvm_ll: Option<PathBuf>,
    llvm_opt_ll: Option<PathBuf>,
    link: aura_bridge::LinkInputs,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Build {
            path,
            profile,
            mode,
            backend: backend_cli,
            bridge,
            link_dirs,
            link_libs,
            optimize,
            smt_profile,
        } => {
            let resolved = resolve_manifest_config(&path, &bridge, &link_dirs, &link_libs)?;
            let parse_cfg = build_parse_config(&cli.edition, &cli.feature, &resolved);

            let optimize = optimize.unwrap_or_else(|| profile.default_optimize().to_string());
            let smt_profile: aura_verify::SmtProfile = smt_profile
                .unwrap_or_else(|| profile.default_smt_profile())
                .into();

            let targets = expand_workspace_targets(&path, &resolved);
            for t in targets {
                build_one(
                    &t,
                    &parse_cfg,
                    &profile,
                    mode,
                    &backend_cli,
                    &resolved,
                    &optimize,
                    smt_profile,
                )?;
            }
            Ok(())
        }
        Cmd::Run {
            path,
            profile,
            mode,
            backend: backend_cli,
            bridge,
            link_dirs,
            link_libs,
            optimize,
            smt_profile,
            hot,
        } => {
            let resolved = resolve_manifest_config(&path, &bridge, &link_dirs, &link_libs)?;
            let parse_cfg = build_parse_config(&cli.edition, &cli.feature, &resolved);
            let optimize = optimize.unwrap_or_else(|| profile.default_optimize().to_string());
            let smt_profile: aura_verify::SmtProfile = smt_profile
                .unwrap_or_else(|| profile.default_smt_profile())
                .into();
            match mode {
                Mode::Llvm => run(
                    &path,
                    &parse_cfg,
                    &backend_cli,
                    &resolved.bridge_headers,
                    &resolved.lib_dirs,
                    &resolved.libs,
                    &resolved.nexus_plugins,
                    &optimize,
                    smt_profile,
                    hot,
                ),
                Mode::Avm | Mode::Hybrid => {
                    if hot {
                        run_avm_hot(&path, smt_profile)
                    } else {
                        // Hybrid run starts in AVM. Promotion to LLVM is handled inside the AVM
                        // (prototype: not yet implemented).
                        run_avm(&path, smt_profile)
                    }
                }
            }
        }

        Cmd::Verify {
            path,
            profile,
            smt_profile,
            report,
        } => {
            let resolved = resolve_manifest_config(&path, &[], &[], &[])?;
            let parse_cfg = build_parse_config(&cli.edition, &cli.feature, &resolved);
            let smt_profile: aura_verify::SmtProfile = smt_profile
                .unwrap_or_else(|| profile.default_smt_profile())
                .into();

            let targets = expand_workspace_targets(&path, &resolved);
            if let Some(report_path) = report {
                if targets.len() == 1 {
                    verify_file_with_report(
                        &targets[0],
                        &parse_cfg,
                        &resolved.nexus_plugins,
                        smt_profile,
                        &report_path,
                    )?;
                } else {
                    if report_path.exists() && !report_path.is_dir() {
                        return Err(miette::miette!(
                            "--report must be a directory when verifying multiple targets"
                        ));
                    }
                    fs::create_dir_all(&report_path).into_diagnostic()?;
                    for t in targets {
                        let stem = t
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("module");
                        let out = report_path.join(format!("{stem}.trusted-core.json"));
                        verify_file_with_report(
                            &t,
                            &parse_cfg,
                            &resolved.nexus_plugins,
                            smt_profile,
                            &out,
                        )?;
                    }
                }
            } else {
                for t in targets {
                    verify_file(&t, &parse_cfg, &resolved.nexus_plugins, smt_profile)?;
                }
            }
            Ok(())
        }

        Cmd::Test { path, smt_profile } => {
            let resolved = resolve_manifest_config(&path, &[], &[], &[])?;
            let parse_cfg = build_parse_config(&cli.edition, &cli.feature, &resolved);
            let smt_profile: aura_verify::SmtProfile = smt_profile.into();

            let roots = expand_workspace_roots(&resolved);
            let mut files: Vec<PathBuf> = Vec::new();
            for r in roots {
                let tests_dir = r.join("tests");
                if tests_dir.exists() {
                    collect_aura_files(&tests_dir, &mut files)?;
                }
            }
            files.sort();
            files.dedup();

            if files.is_empty() {
                println!("aura test: no tests found");
                return Ok(());
            }

            let mut failed = 0usize;
            for f in files {
                if let Err(e) = verify_file(&f, &parse_cfg, &resolved.nexus_plugins, smt_profile) {
                    eprintln!("test failed: {}", f.display());
                    eprintln!("{e:?}");
                    failed += 1;
                }
            }
            if failed > 0 {
                return Err(miette::miette!("{failed} test file(s) failed"));
            }
            println!("aura test: ok");
            Ok(())
        }

        Cmd::Lint { path } => {
            let resolved = resolve_manifest_config(&path, &[], &[], &[])?;
            let parse_cfg = build_parse_config(&cli.edition, &cli.feature, &resolved);
            let targets = expand_workspace_targets(&path, &resolved);
            let mut failed = 0usize;
            for t in targets {
                if let Err(e) = lint_file(&t, &parse_cfg) {
                    eprintln!("lint failed: {}", t.display());
                    eprintln!("{e:?}");
                    failed += 1;
                }
            }
            if failed > 0 {
                return Err(miette::miette!("lint failed"));
            }
            println!("aura lint: ok");
            Ok(())
        }

        Cmd::Pkg { cmd } => match cmd {
            PkgCmd::Add {
                package,
                version,
                url,
                registry,
                deny_deprecated,
                require_signature,
                trusted_key,
                force,
                no_smoke,
            } => pkg_add(
                &package,
                version.as_deref(),
                url.as_deref(),
                registry.as_deref(),
                deny_deprecated,
                require_signature,
                trusted_key.as_deref(),
                force,
                !no_smoke,
            ),

            PkgCmd::Publish {
                package,
                version,
                registry,
                from,
                signing_key,
                key_id,
            } => {
                let (_sha256, _sig) = aura_pkg::publish_package(&aura_pkg::PublishOptions {
                    package,
                    version,
                    registry_dir: registry,
                    from_dir: from,
                    signing_key,
                    signature_key_id: key_id,
                })?;
                println!("published");
                Ok(())
            }

            PkgCmd::Deprecate {
                package,
                version,
                registry,
                message,
                replaced_by,
            } => {
                aura_pkg::deprecate_version(&aura_pkg::DeprecateOptions {
                    package,
                    version,
                    registry_dir: registry,
                    message,
                    replaced_by,
                })?;
                println!("deprecated");
                Ok(())
            }
        },

        Cmd::Init { path } => init_project(&path),

        Cmd::Fmt { path, check, write } => {
            let resolved = manifest::load_resolved_manifest(&path)
                .unwrap_or_else(|_| manifest::ResolvedManifest::empty(PathBuf::from(".")));
            let parse_cfg = build_parse_config(&cli.edition, &cli.feature, &resolved);

            let src = fs::read_to_string(&path).into_diagnostic()?;
            let src_aug = augment_with_sdk_std(&src)?;
            let program = aura_parse::parse_source_with_config(&src_aug, &parse_cfg)?;
            let formatted = aura_parse::format_program(&program);

            if check {
                if formatted != src_aug {
                    return Err(miette::miette!("formatting differs"));
                }
                return Ok(());
            }

            if write {
                // Preserve original file contents style by writing the formatted output.
                fs::write(&path, formatted).into_diagnostic()?;
                return Ok(());
            }

            print!("{formatted}");
            Ok(())
        }

        Cmd::Bindgen {
            headers,
            out,
            include_dirs,
            link_dirs,
            link_libs,
            no_cache,
        } => bindgen(&headers, &out, &include_dirs, &link_dirs, &link_libs, !no_cache),
    }
}

#[derive(Debug, Serialize)]
struct BindgenLinkInputsReport {
    lib_dirs: Vec<String>,
    libs: Vec<String>,
    c_sources: Vec<String>,
    runtime_dlls: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BindgenDiscoveredFnReport {
    name: String,
    params: Vec<(String, String)>,
    ret: String,
}

#[derive(Debug, Serialize)]
struct BindgenTrustedBoundaryReport {
    tool: &'static str,
    headers: Vec<String>,
    generated_shim: String,
    link: BindgenLinkInputsReport,
    discovered_fns: Vec<BindgenDiscoveredFnReport>,
    notes: Vec<String>,
}

fn bindgen(
    headers: &[PathBuf],
    out_dir: &Path,
    include_dirs: &[PathBuf],
    link_dirs: &[PathBuf],
    link_libs: &[String],
    enable_cache: bool,
) -> miette::Result<()> {
    let _ = include_dirs; // reserved

    fs::create_dir_all(out_dir).into_diagnostic()?;

    let mut hasher = sha2::Sha256::new();
    for h in headers {
        hasher.update(h.to_string_lossy().as_bytes());
        let bytes = fs::read(h).into_diagnostic()?;
        hasher.update(&bytes);
    }
    for d in link_dirs {
        hasher.update(d.to_string_lossy().as_bytes());
    }
    for l in link_libs {
        hasher.update(l.as_bytes());
    }
    let key = hex::encode(hasher.finalize());

    let shim_name = "bridge.aura";
    let report_name = "bindgen.trusted-boundary.json";
    let cache_dir = PathBuf::from(".aura")
        .join("cache")
        .join("bindgen")
        .join(&key);

    if enable_cache && cache_dir.exists() {
        let cached_shim = cache_dir.join(shim_name);
        let cached_report = cache_dir.join(report_name);
        if cached_shim.exists() {
            fs::copy(&cached_shim, out_dir.join(shim_name)).into_diagnostic()?;
        }
        if cached_report.exists() {
            fs::copy(&cached_report, out_dir.join(report_name)).into_diagnostic()?;
        }
        println!("bindgen cache hit: {}", key);
        return Ok(());
    }

    let outputs = aura_bridge::run_bridge(
        &aura_bridge::BridgeConfig {
            headers: headers.to_vec(),
            include_dirs: include_dirs.to_vec(),
            lib_dirs: link_dirs.to_vec(),
            libs: link_libs.to_vec(),
        },
        out_dir,
    )?;

    // Copy/rename to a stable output name for tooling.
    fs::copy(&outputs.aura_shim_path, out_dir.join(shim_name)).into_diagnostic()?;
    println!("wrote {}", out_dir.join(shim_name).display());

    let report = BindgenTrustedBoundaryReport {
        tool: "aura bindgen (bootstrap)",
        headers: headers.iter().map(|p| p.to_string_lossy().to_string()).collect(),
        generated_shim: out_dir.join(shim_name).to_string_lossy().to_string(),
        link: BindgenLinkInputsReport {
            lib_dirs: outputs
                .link
                .lib_dirs
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            libs: outputs.link.libs.clone(),
            c_sources: outputs
                .link
                .c_sources
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            runtime_dlls: outputs
                .link
                .runtime_dlls
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
        },
        discovered_fns: outputs
            .discovered
            .iter()
            .map(|f| BindgenDiscoveredFnReport {
                name: f.name.clone(),
                params: f.params.clone(),
                ret: f.ret.clone(),
            })
            .collect(),
        notes: vec![
            "All symbols discovered from C headers are treated as an FFI trust boundary.".to_string(),
            "The generated shim is heuristic (regex-based header parsing) in this phase.".to_string(),
        ],
    };
    let report_path = out_dir.join(report_name);
    fs::write(&report_path, serde_json::to_string_pretty(&report).into_diagnostic()?)
        .into_diagnostic()?;
    println!("wrote {}", report_path.display());

    if enable_cache {
        fs::create_dir_all(&cache_dir).into_diagnostic()?;
        let _ = fs::copy(out_dir.join(shim_name), cache_dir.join(shim_name));
        let _ = fs::copy(&report_path, cache_dir.join(report_name));
        println!("bindgen cache store: {}", key);
    }

    Ok(())
}

fn expand_workspace_roots(resolved: &manifest::ResolvedManifest) -> Vec<PathBuf> {
    if resolved.workspace_members.is_empty() {
        return vec![resolved.project_root.clone()];
    }
    resolved.workspace_members.clone()
}

fn expand_workspace_targets(input: &Path, resolved: &manifest::ResolvedManifest) -> Vec<PathBuf> {
    if !resolved.workspace_members.is_empty() {
        return resolved
            .workspace_members
            .iter()
            .map(|m| if m.is_dir() { m.join("main.aura") } else { m.clone() })
            .collect();
    }
    if input.is_dir() {
        return vec![input.join("main.aura")];
    }
    vec![input.to_path_buf()]
}

fn collect_aura_files(dir: &Path, out: &mut Vec<PathBuf>) -> miette::Result<()> {
    for entry in fs::read_dir(dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let p = entry.path();
        if p.is_dir() {
            collect_aura_files(&p, out)?;
        } else if p.extension().and_then(|e| e.to_str()) == Some("aura") {
            out.push(p);
        }
    }
    Ok(())
}

fn verify_file(
    path: &Path,
    parse_cfg: &ParseConfig,
    nexus_plugins: &[PluginManifest],
    smt_profile: aura_verify::SmtProfile,
) -> miette::Result<()> {
    let src = fs::read_to_string(path).into_diagnostic()?;
    let src = augment_with_sdk_std(&src)?;
    let source = NamedSource::new(display_path(path), src.clone());

    let program =
        aura_parse::parse_source_with_config(&src, parse_cfg).map_err(|e| e.with_source_code(source.clone()))?;

    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    checker
        .check_program(&program)
        .map_err(|e| miette::Report::new(e).with_source_code(source.clone()))?;

    #[cfg(feature = "z3")]
    {
        let mut prover = aura_verify::Z3Prover::new();
        verify_program_z3_with_manifest_plugins(&program, &mut prover, nexus_plugins, smt_profile)
            .map_err(|e| miette::Report::new(e).with_source_code(source.clone()))?;
    }

    #[cfg(not(feature = "z3"))]
    {
        let _ = nexus_plugins;
        let _ = smt_profile;
    }

    Ok(())
}

fn verify_file_with_report(
    path: &Path,
    parse_cfg: &ParseConfig,
    nexus_plugins: &[PluginManifest],
    smt_profile: aura_verify::SmtProfile,
    report_out: &Path,
) -> miette::Result<()> {
    let src = fs::read_to_string(path).into_diagnostic()?;
    let src = augment_with_sdk_std(&src)?;
    let source = NamedSource::new(display_path(path), src.clone());

    let program = match aura_parse::parse_source_with_config(&src, parse_cfg)
        .map_err(|e| e.with_source_code(source.clone()))
    {
        Ok(p) => p,
        Err(e) => {
            let _ = report::write_verify_report(
                path,
                false,
                Some(format!("{e:?}")),
                None,
                None,
                report_out,
            );
            return Err(e);
        }
    };

    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    if let Err(e) = checker
        .check_program(&program)
        .map_err(|e| miette::Report::new(e).with_source_code(source.clone()))
    {
        let _ = report::write_verify_report(
            path,
            false,
            Some(format!("{e:?}")),
            Some(&program),
            None,
            report_out,
        );
        return Err(e);
    }

    #[cfg(feature = "z3")]
    {
        let mut prover = aura_verify::Z3Prover::new();
        let verify = match verify_program_z3_report_with_manifest_plugins(
            &program,
            &mut prover,
            nexus_plugins,
            smt_profile,
        ) {
            Ok(rep) => Some(report::analyze_verify_evidence(&program, &rep.proofs)),
            Err(e) => {
                let e = miette::Report::new(e).with_source_code(source.clone());
                let _ = report::write_verify_report(
                    path,
                    false,
                    Some(format!("{e:?}")),
                    Some(&program),
                    None,
                    report_out,
                );
                return Err(e);
            }
        };

        report::write_verify_report(path, true, None, Some(&program), verify, report_out)?;
        return Ok(());
    }

    #[cfg(not(feature = "z3"))]
    {
        let _ = nexus_plugins;
        let _ = smt_profile;
    }

    report::write_verify_report(path, true, None, Some(&program), None, report_out)?;
    Ok(())
}

fn lint_file(path: &Path, parse_cfg: &ParseConfig) -> miette::Result<()> {
    let src = fs::read_to_string(path).into_diagnostic()?;
    let src_aug = augment_with_sdk_std(&src)?;
    let program = aura_parse::parse_source_with_config(&src_aug, parse_cfg)?;
    let formatted = aura_parse::format_program(&program);
    if formatted != src_aug {
        return Err(miette::miette!("formatting differs"));
    }
    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    checker.check_program(&program).map_err(miette::Report::new)?;
    Ok(())
}

fn build_one(
    path: &Path,
    parse_cfg: &ParseConfig,
    profile: &BuildProfileArg,
    mode: Mode,
    backend_cli: &str,
    resolved: &manifest::ResolvedManifest,
    optimize: &str,
    smt_profile: aura_verify::SmtProfile,
) -> miette::Result<BuildOutputs> {
    if mode == Mode::Avm {
        verify_file(path, parse_cfg, &resolved.nexus_plugins, smt_profile)?;
        println!("avm: verified {}", path.display());
        return Ok(BuildOutputs {
            out_dir: build_dir(path),
            module_c: None,
            llvm_ll: None,
            llvm_opt_ll: None,
            link: aura_bridge::LinkInputs::default(),
        });
    }

    // Verify profile enforces verification regardless of backend.
    if *profile == BuildProfileArg::Verify {
        verify_file(path, parse_cfg, &resolved.nexus_plugins, smt_profile)?;
    }

    let backend = backend_cli.to_string();

    build_cached(
        path,
        parse_cfg,
        &backend,
        &resolved.bridge_headers,
        &resolved.lib_dirs,
        &resolved.libs,
        &resolved.nexus_plugins,
        optimize,
        smt_profile,
    )
}

fn build_cached(
    path: &Path,
    parse_cfg: &ParseConfig,
    backend: &str,
    bridge_headers: &[PathBuf],
    link_dirs: &[PathBuf],
    link_libs: &[String],
    nexus_plugins: &[PluginManifest],
    optimize: &str,
    smt_profile: aura_verify::SmtProfile,
) -> miette::Result<BuildOutputs> {
    let cache_root = PathBuf::from(".aura").join("cache");
    fs::create_dir_all(&cache_root).into_diagnostic()?;

    // Hash the effective build inputs (best effort): augmented source + backend options.
    let src = fs::read_to_string(path).into_diagnostic()?;
    let mut combined = src;
    if !bridge_headers.is_empty() {
        combined.push_str("\n# bridge_headers:\n");
        for h in bridge_headers {
            combined.push_str(&h.to_string_lossy());
            combined.push('\n');
        }
    }
    combined = augment_with_sdk_std(&combined)?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(combined.as_bytes());
    if let Some(ed) = &parse_cfg.edition {
        hasher.update(b"edition=");
        hasher.update(ed.as_bytes());
    }
    for f in &parse_cfg.features {
        hasher.update(b"feature=");
        hasher.update(f.as_bytes());
    }
    hasher.update(backend.as_bytes());
    hasher.update(optimize.as_bytes());
    hasher.update(format!("{:?}", smt_profile).as_bytes());
    for d in link_dirs {
        hasher.update(d.to_string_lossy().as_bytes());
    }
    for l in link_libs {
        hasher.update(l.as_bytes());
    }
    for p in nexus_plugins {
        hasher.update(p.name.as_bytes());
        hasher.update(format!("{}", p.trusted).as_bytes());
    }
    let key = hex::encode(hasher.finalize());
    let entry_dir = cache_root.join(&key);

    let out_dir = build_dir(path);
    fs::create_dir_all(&out_dir).into_diagnostic()?;

    // Cache hit: restore known artifacts.
    if entry_dir.exists() {
        let mut restored_any = false;
        for f in [
            "module.c",
            "aura_runtime.h",
            "module.ll",
            "module.opt.ll",
            "combined.aura",
        ] {
            let srcp = entry_dir.join(f);
            if srcp.exists() {
                let dstp = out_dir.join(f);
                let _ = fs::copy(&srcp, &dstp);
                restored_any = true;
            }
        }

        if restored_any {
            println!("cache hit: {}", key);
            let module_c = if backend == "c" || backend == "wasm" {
                let p = out_dir.join("module.c");
                if p.exists() { Some(p) } else { None }
            } else {
                None
            };
            let llvm_ll = if backend == "llvm" {
                let p = out_dir.join("module.ll");
                if p.exists() { Some(p) } else { None }
            } else {
                None
            };
            let llvm_opt_ll = if backend == "llvm" {
                let p = out_dir.join("module.opt.ll");
                if p.exists() { Some(p) } else { None }
            } else {
                None
            };
            return Ok(BuildOutputs {
                out_dir,
                module_c,
                llvm_ll,
                llvm_opt_ll,
                // LinkInputs are only needed for full native linking; rebuild would be required.
                link: aura_bridge::LinkInputs::default(),
            });
        }
    }

    // Cache miss: run the real build, then copy artifacts into cache.
    let out = build(
        path,
        parse_cfg,
        backend,
        bridge_headers,
        link_dirs,
        link_libs,
        nexus_plugins,
        optimize,
        smt_profile,
    )?;

    fs::create_dir_all(&entry_dir).into_diagnostic()?;
    for f in [
        out.out_dir.join("module.c"),
        out.out_dir.join("aura_runtime.h"),
        out.out_dir.join("module.ll"),
        out.out_dir.join("module.opt.ll"),
        out.out_dir.join("combined.aura"),
    ] {
        if f.exists() {
            let dst = entry_dir.join(f.file_name().unwrap());
            let _ = fs::copy(&f, &dst);
        }
    }

    println!("cache store: {}", key);
    Ok(out)
}

fn pkg_add(
    package: &str,
    version: Option<&str>,
    url: Option<&str>,
    registry: Option<&str>,
    deny_deprecated: bool,
    require_signature: bool,
    trusted_key: Option<&Path>,
    force: bool,
    smoke: bool,
) -> miette::Result<()> {
    // Resolve project root via manifest if present; otherwise use CWD.
    let cwd = std::env::current_dir().into_diagnostic()?;
    let resolved = manifest::load_resolved_manifest(&cwd).unwrap_or_else(|_| manifest::ResolvedManifest::empty(cwd.clone()));
    let project_root = resolved
        .manifest_path
        .as_ref()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or(cwd);

    // Install artifacts.
    let result = aura_pkg::add_package(
        &project_root,
        &aura_pkg::AddOptions {
            package: package.to_string(),
            version: version.map(|s| s.to_string()),
            url: url.map(|s| s.to_string()),
            smoke_test: smoke,
            force,
            registry: registry.map(|s| s.to_string()),
            require_signature,
            trusted_public_key: trusted_key.map(|p| p.to_path_buf()),
            deny_deprecated,
        },
    )?;

    // Update aura.toml (create if missing).
    update_manifest_for_install(&project_root, &result)?;

    println!(
        "installed {} {} ({}; sha256 {})",
        result.package, result.version, result.checksum_status, result.sha256
    );

    if smoke {
        pkg_smoke_test(&project_root, &result.package)?;
        println!("smoke test: ok");
    }

    Ok(())
}

fn update_manifest_for_install(project_root: &Path, install: &aura_pkg::InstallResult) -> miette::Result<()> {
    let manifest_path = project_root.join("aura.toml");
    let raw = if manifest_path.exists() {
        fs::read_to_string(&manifest_path).into_diagnostic()?
    } else {
        "[project]\nname = \"AuraProject\"\n".to_string()
    };

    let mut doc: toml::Value = raw
        .parse::<toml::Value>()
        .map_err(|e| miette::miette!("failed to parse aura.toml: {e}"))?;

    // Ensure tables exist.
    ensure_table(&mut doc, "bridge");
    ensure_table(&mut doc, "linking");

    // Bridge: keep our stable shim header as the default for raylib.
    if install.package == "raylib" {
        push_string_array_unique(&mut doc, &["bridge"], "headers", "tools/raylib_bridge.h");
    }

    // Bridge: stable shim header for onnxruntime.
    if install.package == "onnxruntime" {
        push_string_array_unique(
            &mut doc,
            &["bridge"],
            "headers",
            "tools/onnxruntime_bridge.h",
        );
    }

    // Linking.
    push_string_array_unique(&mut doc, &["linking"], "lib_dirs", "./deps");
    push_string_array_unique(&mut doc, &["linking"], "lib_dirs", "./tools");

    // Raylib + Windows system libs.
    if install.package == "raylib" {
        push_string_array_unique(&mut doc, &["linking"], "libs", "raylib.lib");
        for sys in ["user32.lib", "gdi32.lib", "winmm.lib", "shell32.lib"] {
            push_string_array_unique(&mut doc, &["linking"], "libs", sys);
        }
    }

    // ONNX Runtime import library (DLL is copied post-link from ./deps).
    if install.package == "onnxruntime" {
        push_string_array_unique(&mut doc, &["linking"], "libs", "onnxruntime.lib");
    }

    let out = toml::to_string_pretty(&doc).into_diagnostic()?;
    fs::write(&manifest_path, out).into_diagnostic()?;
    Ok(())
}

fn ensure_table(doc: &mut toml::Value, key: &str) {
    if !doc.get(key).is_some_and(|v| v.is_table()) {
        doc[key] = toml::Value::Table(toml::map::Map::new());
    }
}

fn push_string_array_unique(doc: &mut toml::Value, table_path: &[&str], key: &str, value: &str) {
    let mut cur = doc;
    for seg in table_path {
        if !cur.get(seg).is_some_and(|v| v.is_table()) {
            cur[*seg] = toml::Value::Table(toml::map::Map::new());
        }
        cur = cur.get_mut(seg).expect("table exists");
    }

    let arr = cur.get_mut(key);
    match arr {
        Some(toml::Value::Array(a)) => {
            if !a.iter().any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(value))) {
                a.push(toml::Value::String(value.to_string()));
            }
        }
        _ => {
            cur[key] = toml::Value::Array(vec![toml::Value::String(value.to_string())]);
        }
    }
}

fn pkg_smoke_test(project_root: &Path, package: &str) -> miette::Result<()> {
    // Build-only + link-only test: does not execute.
    let smoke_dir = project_root.join("build").join("pkg_smoke");
    fs::create_dir_all(&smoke_dir).into_diagnostic()?;
    let smoke_file = smoke_dir.join("smoke.aura");

    // Minimal program that references the package shim externs.
    // (Link-only smoke test: this code is not executed.)
    let src = match package {
        "raylib" => "import raylib\n\ncell main() ->:\n    raylib_set_target_fps(1)\n",
        "onnxruntime" => "import onnxruntime\n\ncell main() ->:\n    onnxruntime_available()\n",
        _ => return Err(miette::miette!("no smoke test defined for package '{package}'")),
    };
    fs::write(&smoke_file, src).into_diagnostic()?;

    let resolved = resolve_manifest_config(&smoke_file, &[], &[], &[])?;
    let parse_cfg = build_parse_config(&None, &[], &resolved);
    let out = build(
        &smoke_file,
        &parse_cfg,
        "llvm",
        &resolved.bridge_headers,
        &resolved.lib_dirs,
        &resolved.libs,
        &resolved.nexus_plugins,
        "none",
        aura_verify::SmtProfile::Ci,
    )?;

    let ll = out
        .llvm_opt_ll
        .as_ref()
        .or(out.llvm_ll.as_ref())
        .expect("LLVM backend produces module.ll");

    let exe = out.out_dir.join(exe_name(&smoke_file));
    linker::link_with_clang(
        ll,
        &exe,
        &out.link.lib_dirs,
        &out.link.libs,
        &out.link.c_sources,
        &out.link.runtime_dlls,
    )
    .map_err(miette::Report::new)?;

    Ok(())
}

fn resolve_manifest_config(
    aura_file: &Path,
    cli_bridge: &[PathBuf],
    cli_link_dirs: &[PathBuf],
    cli_link_libs: &[String],
) -> miette::Result<manifest::ResolvedManifest> {
    // Start searching from the aura file directory.
    let resolved = manifest::load_resolved_manifest(aura_file).map_err(miette::Report::new)?;

    // Manifest provides defaults; CLI flags append/override.
    let mut out = resolved;

    if !cli_bridge.is_empty() {
        out.bridge_headers.extend(cli_bridge.iter().cloned());
    }
    if !cli_link_dirs.is_empty() {
        out.lib_dirs.extend(cli_link_dirs.iter().cloned());
    }
    if !cli_link_libs.is_empty() {
        out.libs.extend(cli_link_libs.iter().cloned());
    }

    // De-dupe.
    out.bridge_headers = {
        let mut v = Vec::new();
        let mut seen = std::collections::HashSet::<PathBuf>::new();
        for p in out.bridge_headers.drain(..) {
            let key = p.canonicalize().unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                v.push(p);
            }
        }
        v
    };
    out.lib_dirs = {
        let mut v = Vec::new();
        let mut seen = std::collections::HashSet::<PathBuf>::new();
        for p in out.lib_dirs.drain(..) {
            let key = p.canonicalize().unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                v.push(p);
            }
        }
        v
    };
    out.libs = {
        let mut v = Vec::new();
        let mut seen = std::collections::HashSet::<String>::new();
        for s in out.libs.drain(..) {
            let key = s.to_ascii_lowercase();
            if seen.insert(key) {
                v.push(s);
            }
        }
        v
    };

    // Zero-config native deps: if the source imports a known native package
    // and its artifacts are missing, install it automatically.
    out = maybe_auto_install_native_deps(aura_file, out, cli_bridge, cli_link_dirs, cli_link_libs)?;

    Ok(out)
}

fn maybe_auto_install_native_deps(
    aura_file: &Path,
    resolved: manifest::ResolvedManifest,
    cli_bridge: &[PathBuf],
    cli_link_dirs: &[PathBuf],
    cli_link_libs: &[String],
) -> miette::Result<manifest::ResolvedManifest> {
    let src = fs::read_to_string(aura_file).into_diagnostic()?;

    let wants_raylib = src
        .lines()
        .any(|l| l.trim_start().starts_with("import ") && l.contains("raylib"));
    let wants_onnxruntime = src
        .lines()
        .any(|l| l.trim_start().starts_with("import ") && (l.contains("onnxruntime") || l.contains("aura::ai")));

    if !wants_raylib && !wants_onnxruntime {
        return Ok(resolved);
    }

    let has_raylib = resolved
        .project_root
        .join("deps")
        .join("raylib.lib")
        .exists();
    let has_onnxruntime = resolved
        .project_root
        .join("deps")
        .join("onnxruntime.lib")
        .exists();

    if wants_raylib && !has_raylib {
        let t0 = Instant::now();
        eprintln!("auto-installing raylib (ACPM)...");
        let install = aura_pkg::add_package(
            &resolved.project_root,
            &aura_pkg::AddOptions {
                package: "raylib".to_string(),
                version: None,
                url: None,
                smoke_test: false,
                force: false,
                registry: None,
                require_signature: false,
                trusted_public_key: None,
                deny_deprecated: false,
            },
        )?;
        update_manifest_for_install(&resolved.project_root, &install)?;
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "auto-installed {} {} in {:.2}s ({}; sha256 {})",
            install.package,
            install.version,
            secs,
            install.checksum_status,
            install.sha256
        );
    }

    if wants_onnxruntime && !has_onnxruntime {
        let t0 = Instant::now();
        eprintln!("auto-installing onnxruntime (ACPM)...");
        let install = aura_pkg::add_package(
            &resolved.project_root,
            &aura_pkg::AddOptions {
                package: "onnxruntime".to_string(),
                version: None,
                url: None,
                smoke_test: false,
                force: false,
                registry: None,
                require_signature: false,
                trusted_public_key: None,
                deny_deprecated: false,
            },
        )?;
        update_manifest_for_install(&resolved.project_root, &install)?;
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "auto-installed {} {} in {:.2}s ({}; sha256 {})",
            install.package,
            install.version,
            secs,
            install.checksum_status,
            install.sha256
        );
    }

    // Reload manifest and re-apply CLI overrides.
    let mut out = manifest::load_resolved_manifest(aura_file).map_err(miette::Report::new)?;
    if !cli_bridge.is_empty() {
        out.bridge_headers.extend(cli_bridge.iter().cloned());
    }
    if !cli_link_dirs.is_empty() {
        out.lib_dirs.extend(cli_link_dirs.iter().cloned());
    }
    if !cli_link_libs.is_empty() {
        out.libs.extend(cli_link_libs.iter().cloned());
    }

    out.bridge_headers = {
        let mut v = Vec::new();
        let mut seen = std::collections::HashSet::<PathBuf>::new();
        for p in out.bridge_headers.drain(..) {
            let key = p.canonicalize().unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                v.push(p);
            }
        }
        v
    };
    out.lib_dirs = {
        let mut v = Vec::new();
        let mut seen = std::collections::HashSet::<PathBuf>::new();
        for p in out.lib_dirs.drain(..) {
            let key = p.canonicalize().unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                v.push(p);
            }
        }
        v
    };
    out.libs = {
        let mut v = Vec::new();
        let mut seen = std::collections::HashSet::<String>::new();
        for s in out.libs.drain(..) {
            let key = s.to_ascii_lowercase();
            if seen.insert(key) {
                v.push(s);
            }
        }
        v
    };

    Ok(out)
}

fn build(
    path: &Path,
    parse_cfg: &ParseConfig,
    backend: &str,
    bridge_headers: &[PathBuf],
    link_dirs: &[PathBuf],
    link_libs: &[String],
    nexus_plugins: &[PluginManifest],
    optimize: &str,
    smt_profile: aura_verify::SmtProfile,
) -> miette::Result<BuildOutputs> {
    let _ = nexus_plugins;
    let _ = optimize;
    let _ = smt_profile;

    let src = fs::read_to_string(path).into_diagnostic()?;

    let out_dir = build_dir(path);
    fs::create_dir_all(&out_dir).into_diagnostic()?;

    // Stage 6: optional bridge generation.
    let mut combined_src = String::new();
    let mut link = aura_bridge::LinkInputs::default();
    link.lib_dirs.extend(link_dirs.iter().cloned());
    link.libs.extend(link_libs.iter().cloned());

    if !bridge_headers.is_empty() {
        let bridge_dir = out_dir.join("bridge");
        let outputs = aura_bridge::run_bridge(
            &aura_bridge::BridgeConfig {
                headers: bridge_headers.to_vec(),
                include_dirs: vec![],
                lib_dirs: link_dirs.to_vec(),
                libs: link_libs.to_vec(),
            },
            &bridge_dir,
        )?;

        let shim_src = fs::read_to_string(&outputs.aura_shim_path).into_diagnostic()?;
        combined_src.push_str(&shim_src);
        combined_src.push_str("\n\n");
        combined_src.push_str(&src);

        // Keep a copy for debugging.
        let combined_path = out_dir.join("combined.aura");
        fs::write(&combined_path, &combined_src).into_diagnostic()?;
        println!("wrote {}", outputs.aura_shim_path.display());
        println!("wrote {}", combined_path.display());

        link = outputs.link;
    } else {
        combined_src = src.clone();
    }

    // Stage 0: SDK stdlib augmentation.
    combined_src = augment_with_sdk_std(&combined_src)?;

    let source = NamedSource::new(display_path(path), combined_src.clone());

    let program = aura_parse::parse_source_with_config(&combined_src, parse_cfg)
        .map_err(|e| e.with_source_code(source.clone()))?;

    let mut checker = aura_core::Checker::new();
    // Prototype: defer non-trivial constrained-range proofs to `aura-verify`.
    // This is required for loop-local facts (e.g., `i = i + 1` under `while i < 10`).
    checker.set_defer_range_proofs(true);
    checker
        .check_program(&program)
        .map_err(|e| miette::Report::new(e).with_source_code(source.clone()))?;

    let module_ir = aura_core::lower_program(&program)
        .map_err(|e| miette::Report::new(e).with_source_code(source.clone()))?;

    let debug = DebugSource::new(display_path(path), &combined_src);

    match backend {
        "c" => {
            let artifacts = aura_backend_c::emit_module(&module_ir, Some(&debug)).into_diagnostic()?;
            let runtime_h = out_dir.join("aura_runtime.h");
            let module_c = out_dir.join("module.c");
            fs::write(&runtime_h, artifacts.runtime_h).into_diagnostic()?;
            fs::write(&module_c, artifacts.module_c).into_diagnostic()?;
            println!("wrote {}", runtime_h.display());
            println!("wrote {}", module_c.display());
            Ok(BuildOutputs {
                out_dir,
                module_c: Some(module_c),
                llvm_ll: None,
                llvm_opt_ll: None,
                link,
            })
        }
        "wasm" => {
            let artifacts = aura_backend_c::emit_module(&module_ir, Some(&debug)).into_diagnostic()?;
            let runtime_h = out_dir.join("aura_runtime.h");
            let module_c = out_dir.join("module.c");
            fs::write(&runtime_h, artifacts.runtime_h).into_diagnostic()?;
            fs::write(&module_c, artifacts.module_c).into_diagnostic()?;
            println!("wrote {}", runtime_h.display());
            println!("wrote {}", module_c.display());

            // Best-effort: compile to a WASI module when clang is available.
            // This provides sandboxable execution artifacts without requiring the LLVM backend.
            let clang = linker::find_clang().ok_or_else(|| {
                miette::miette!(
                    "WASM backend requires clang.exe in PATH (or installed at C:\\Program Files\\LLVM\\bin)."
                )
            })?;
            let wasm = out_dir.join(wasm_name(path));
            compile_wasm_wasi(&clang, &module_c, &wasm)?;
            println!("wrote {}", wasm.display());

            Ok(BuildOutputs {
                out_dir,
                module_c: Some(module_c),
                llvm_ll: None,
                llvm_opt_ll: None,
                link,
            })
        }
        "llvm" => {
            #[cfg(not(all(feature = "z3", feature = "llvm")))]
            {
                return Err(miette::miette!(
                    "LLVM backend requires both features: z3 + llvm. Re-run with `cargo run -p aura --features z3,llvm -- ...` and ensure Z3 is installed."
                ));
            }

            #[cfg(all(feature = "z3", feature = "llvm"))]
            {
                // Phase 3 requirement: verify with Z3 before any native emission.
                let mut prover = aura_verify::Z3Prover::new();
                verify_program_z3_with_manifest_plugins(
                    &program,
                    &mut prover,
                    nexus_plugins,
                    smt_profile,
                )
                    .map_err(|e| miette::Report::new(e).with_source_code(source.clone()))?;

                let artifacts = aura_backend_llvm::emit_llvm_ir(&module_ir, Some(&debug)).into_diagnostic()?;
                let ll = out_dir.join("module.ll");
                fs::write(&ll, artifacts.llvm_ir).into_diagnostic()?;
                println!("wrote {}", ll.display());

                let llvm_opt_ll = match optimize {
                    "none" => None,
                    "full" => {
                        let opt_ll = out_dir.join("module.opt.ll");
                        aura_ai_opt::optimize_ll_file(&ll, &opt_ll)?;
                        println!("wrote {}", opt_ll.display());
                        Some(opt_ll)
                    }
                    other => {
                        return Err(miette::miette!(
                            "unknown --optimize value: {other} (expected 'none' or 'full')"
                        ));
                    }
                };

                Ok(BuildOutputs {
                    out_dir,
                    module_c: None,
                    llvm_ll: Some(ll),
                    llvm_opt_ll,
                    link,
                })
            }
        }
        other => Err(miette::miette!(
            "unknown backend: {other} (expected 'c', 'llvm', or 'wasm')"
        )),
    }
}

fn run(
    path: &Path,
    parse_cfg: &ParseConfig,
    backend: &str,
    bridge_headers: &[PathBuf],
    link_dirs: &[PathBuf],
    link_libs: &[String],
    nexus_plugins: &[PluginManifest],
    optimize: &str,
    smt_profile: aura_verify::SmtProfile,
    hot: bool,
) -> miette::Result<()> {
    // Zero-config convenience: if the default C backend is selected but there's no
    // C compiler available, fall back to LLVM when enabled.
    let backend = if backend == "c" && find_c_compiler().is_none() {
        #[cfg(feature = "llvm")]
        {
            "llvm"
        }
        #[cfg(not(feature = "llvm"))]
        {
            backend
        }
    } else {
        backend
    };

    if hot {
        return run_hot(
            path,
            parse_cfg,
            backend,
            bridge_headers,
            link_dirs,
            link_libs,
            nexus_plugins,
            optimize,
            smt_profile,
        );
    }

    let out = build(
        path,
        parse_cfg,
        backend,
        bridge_headers,
        link_dirs,
        link_libs,
        nexus_plugins,
        optimize,
        smt_profile,
    )?;

    match backend {
        "c" => {
            let module_c = out.module_c.as_ref().expect("C backend produces module.c");
            let exe = out.out_dir.join(exe_name(path));
            if let Some((cc, kind)) = find_c_compiler() {
                compile_c(&cc, kind, module_c, &exe)?;

                if let Some((sess, _handle)) = debug_pair() {
                    sess.emit(DebugEvent::NativeLaunch {
                        exe: exe.display().to_string(),
                    });
                    for c in sess.take_pending_commands() {
                        sess.apply_command(c);
                    }
                    sess.wait_while_paused();

                    return run_native_supervised(&exe, Some(&sess));
                }

                run_native_supervised(&exe, None)
            } else {
                Err(miette::miette!(
                    "No C compiler found in PATH. Install LLVM clang or GCC and re-run `aura run`."
                ))
            }
        }
        "llvm" => {
            let ll = out
                .llvm_opt_ll
                .as_ref()
                .or(out.llvm_ll.as_ref())
                .expect("LLVM backend produces module.ll");

            let exe = out.out_dir.join(exe_name(path));
            linker::link_with_clang(
                ll,
                &exe,
                &out.link.lib_dirs,
                &out.link.libs,
                &out.link.c_sources,
                &out.link.runtime_dlls,
            )
            .map_err(miette::Report::new)?;

            if let Some((sess, _handle)) = debug_pair() {
                sess.emit(DebugEvent::NativeLaunch {
                    exe: exe.display().to_string(),
                });
                for c in sess.take_pending_commands() {
                    sess.apply_command(c);
                }
                sess.wait_while_paused();

                return run_native_supervised(&exe, Some(&sess));
            }

            run_native_supervised(&exe, None)
        }
        "wasm" => Err(miette::miette!(
            "cannot execute a wasm artifact. Use `aura build --backend wasm` and run the output with a WASI runtime."
        )),
        other => Err(miette::miette!(
            "unknown backend: {other} (expected 'c' or 'llvm')"
        )),
    }
}

fn run_hot(
    path: &Path,
    parse_cfg: &ParseConfig,
    backend: &str,
    bridge_headers: &[PathBuf],
    link_dirs: &[PathBuf],
    link_libs: &[String],
    nexus_plugins: &[PluginManifest],
    optimize: &str,
    smt_profile: aura_verify::SmtProfile,
) -> miette::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher: RecommendedWatcher =
        RecommendedWatcher::new(tx, notify::Config::default()).into_diagnostic()?;

    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .into_diagnostic()?;

    for h in bridge_headers {
        watcher.watch(h, RecursiveMode::NonRecursive).into_diagnostic()?;
    }

    if let Ok(m) = manifest::load_resolved_manifest(path) {
        if let Some(mp) = m.manifest_path {
            let _ = watcher.watch(&mp, RecursiveMode::NonRecursive);
        }
    }

    println!("hot reload enabled: watching {}", path.display());

    let mut child: Option<std::process::Child> = None;

    loop {
        // Build + run.
        let out = build(
            path,
            parse_cfg,
            backend,
            bridge_headers,
            link_dirs,
            link_libs,
            nexus_plugins,
            optimize,
            smt_profile,
        )?;
        if backend != "llvm" {
            return Err(miette::miette!("--hot is currently supported only for --backend llvm"));
        }

        let ll = out
            .llvm_opt_ll
            .as_ref()
            .or(out.llvm_ll.as_ref())
            .expect("LLVM backend produces module.ll");

        let exe = out.out_dir.join(exe_name(path));
        linker::link_with_clang(
            ll,
            &exe,
            &out.link.lib_dirs,
            &out.link.libs,
            &out.link.c_sources,
            &out.link.runtime_dlls,
        )
        .map_err(miette::Report::new)?;

        if let Some(mut c) = child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }

        child = Some(Command::new(&exe).spawn().into_diagnostic()?);

        // Wait for change event.
        loop {
            match rx.recv() {
                Ok(Ok(_evt)) => break,
                Ok(Err(_)) => break,
                Err(_) => return Ok(()),
            }
        }
    }
}

#[cfg(feature = "z3")]
fn verify_program_z3_with_manifest_plugins(
    program: &aura_ast::Program,
    prover: &mut aura_verify::Z3Prover,
    nexus_plugins: &[PluginManifest],
    profile: aura_verify::SmtProfile,
) -> Result<(), aura_verify::VerifyError> {
    let mut nexus = aura_nexus::NexusContext::default();
    let dummy_span = aura_ast::Span::new(miette::SourceOffset::from(0usize), 0usize);

    let mut requested: Vec<String> = Vec::new();
    for p in nexus_plugins {
        if !p.trusted {
            return Err(aura_verify::VerifyError {
                message: format!(
                    "Nexus plugin '{}' is not trusted. Set `trusted = true` in aura.toml to enable it.",
                    p.name
                ),
                span: dummy_span,
                model: None,
                meta: None,
            });
        }
        requested.push(p.name.to_ascii_lowercase());
    }
    requested.sort();
    requested.dedup();

    // If no plugins are listed, default to the built-in set.
    if requested.is_empty() {
        requested.push("aura-ai".to_string());
        requested.push("aura-iot".to_string());
        requested.sort();
    }

    match requested.as_slice() {
        [a] if a == "aura-ai" => {
            let plugins = (aura_plugin_ai::AuraAiPlugin::new(),);
            aura_verify::verify_program_z3_profile(program, prover, &plugins, &mut nexus, profile)
        }
        [a] if a == "aura-iot" => {
            let plugins = (aura_plugin_iot::AuraIotPlugin::new(),);
            aura_verify::verify_program_z3_profile(program, prover, &plugins, &mut nexus, profile)
        }
        [a, b]
            if (a == "aura-ai" && b == "aura-iot") || (a == "aura-iot" && b == "aura-ai") =>
        {
            let plugins = (aura_plugin_iot::AuraIotPlugin::new(), aura_plugin_ai::AuraAiPlugin::new());
            aura_verify::verify_program_z3_profile(program, prover, &plugins, &mut nexus, profile)
        }
        other => Err(aura_verify::VerifyError {
            message: format!(
                "unsupported Nexus plugin set: {:?}. Supported built-ins: ['aura-iot', 'aura-ai']",
                other
            ),
            span: dummy_span,
            model: None,
            meta: None,
        }),
    }
}

#[cfg(feature = "z3")]
fn verify_program_z3_report_with_manifest_plugins(
    program: &aura_ast::Program,
    prover: &mut aura_verify::Z3Prover,
    nexus_plugins: &[PluginManifest],
    profile: aura_verify::SmtProfile,
) -> Result<aura_verify::VerificationReport, aura_verify::VerifyError> {
    let mut nexus = aura_nexus::NexusContext::default();
    let dummy_span = aura_ast::Span::new(miette::SourceOffset::from(0usize), 0usize);

    let mut requested: Vec<String> = Vec::new();
    for p in nexus_plugins {
        if !p.trusted {
            return Err(aura_verify::VerifyError {
                message: format!(
                    "Nexus plugin '{}' is not trusted. Set `trusted = true` in aura.toml to enable it.",
                    p.name
                ),
                span: dummy_span,
                model: None,
                meta: None,
            });
        }
        requested.push(p.name.to_ascii_lowercase());
    }
    requested.sort();
    requested.dedup();

    if requested.is_empty() {
        requested.push("aura-ai".to_string());
        requested.push("aura-iot".to_string());
        requested.sort();
    }

    match requested.as_slice() {
        [a] if a == "aura-ai" => {
            let plugins = (aura_plugin_ai::AuraAiPlugin::new(),);
            aura_verify::verify_program_z3_profile(program, prover, &plugins, &mut nexus, profile)?;
            let proofs = aura_nexus::drain_proofs(&mut nexus);
            Ok(aura_verify::VerificationReport { status: aura_verify::VerificationStatus::Success, proofs })
        }
        [a] if a == "aura-iot" => {
            let plugins = (aura_plugin_iot::AuraIotPlugin::new(),);
            aura_verify::verify_program_z3_profile(program, prover, &plugins, &mut nexus, profile)?;
            let proofs = aura_nexus::drain_proofs(&mut nexus);
            Ok(aura_verify::VerificationReport { status: aura_verify::VerificationStatus::Success, proofs })
        }
        [a, b]
            if (a == "aura-ai" && b == "aura-iot") || (a == "aura-iot" && b == "aura-ai") =>
        {
            let plugins = (aura_plugin_iot::AuraIotPlugin::new(), aura_plugin_ai::AuraAiPlugin::new());
            aura_verify::verify_program_z3_profile(program, prover, &plugins, &mut nexus, profile)?;
            let proofs = aura_nexus::drain_proofs(&mut nexus);
            Ok(aura_verify::VerificationReport { status: aura_verify::VerificationStatus::Success, proofs })
        }
        other => Err(aura_verify::VerifyError {
            message: format!(
                "unsupported Nexus plugin set: {:?}. Supported built-ins: ['aura-iot', 'aura-ai']",
                other
            ),
            span: dummy_span,
            model: None,
            meta: None,
        }),
    }
}

fn run_avm(path: &Path, smt_profile: aura_verify::SmtProfile) -> miette::Result<()> {
    let src = fs::read_to_string(path).into_diagnostic()?;
    let src = augment_with_sdk_std(&src)?;

    let mut cfg = aura_interpret::AvmConfig::default();
    // Test/CI escape hatch: allow disabling the Z3 safety gate for fast smoke checks.
    if std::env::var("AURA_AVM_NO_Z3").is_ok() {
        cfg.enable_z3_gate = false;
    }
    cfg.smt_profile = smt_profile;

    if let Some((sess, _handle)) = debug_pair() {
        sess.set_source(path.display().to_string(), &src);
        cfg.debug = Some(sess);
    }

    let mut avm = aura_interpret::Avm::new(cfg);
    let mut nexus = aura_nexus::NexusContext::default();
    let ui_plugins = (aura_plugin_lumina::AuraLuminaPlugin::new(),);
    let out = match avm.exec_entry_cell_with_ui_plugins(&src, "main", &ui_plugins, &mut nexus) {
        Ok(out) => out,
        Err(e) => {
            if e.downcast_ref::<aura_interpret::AvmTerminated>().is_some() {
                return Ok(());
            }
            return Err(e);
        }
    };

    if !out.stdout.is_empty() {
        print!("{}", out.stdout);
    }

    // Only print the value if it's not unit.
    if out.value != aura_interpret::AvmValue::Unit {
        println!("{:?}", out.value);
    }

    if !out.verified {
        if let Some(msg) = out.gate_error {
            if let Some(span) = out.gate_span {
                let named = NamedSource::new(path.display().to_string(), src);
                return Err(miette::Report::new(AvmGateRejected { message: msg, span })
                    .with_source_code(named));
            }
            return Err(miette::miette!("avm: Z3 gate rejected execution: {msg}"));
        }
        return Err(miette::miette!("avm: Z3 gate rejected execution"));
    }

    Ok(())
}

fn run_avm_hot(path: &Path, smt_profile: aura_verify::SmtProfile) -> miette::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher: RecommendedWatcher =
        RecommendedWatcher::new(tx, notify::Config::default()).into_diagnostic()?;

    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .into_diagnostic()?;

    if let Ok(m) = manifest::load_resolved_manifest(path) {
        if let Some(mp) = m.manifest_path {
            let _ = watcher.watch(&mp, RecursiveMode::NonRecursive);
        }
    }

    println!("hot reload enabled (avm): watching {}", path.display());

    loop {
        if let Err(e) = run_avm(path, smt_profile) {
            eprintln!("avm run failed: {e:?}");
        }

        // Wait for change event.
        loop {
            match rx.recv() {
                Ok(Ok(_evt)) => break,
                Ok(Err(_)) => break,
                Err(_) => return Ok(()),
            }
        }
    }
}

fn init_project(path: &Path) -> miette::Result<()> {
    fs::create_dir_all(path).into_diagnostic()?;

    let main_path = path.join("main.aura");
    if !main_path.exists() {
        fs::write(
            &main_path,
            r#"# welcome.aura - תוכנת ההדגמה הראשונה של המשתמש
import aura::lumina

cell main():
    layout:
        VStack(alignment: "center") {
            render: Text(text: "Welcome to the Aura Universe!", style: PixarModern())
            render: Button(label: "Start Creating") {
                on_click: ~> { log("Aura SDK is Active.") }
            }
        }
"#,
        )
        .into_diagnostic()?;
    }

    let toml_path = path.join("aura.toml");
    if !toml_path.exists() {
        fs::write(
            &toml_path,
            r#"# aura.toml - Aura project manifest

# Plugins are optional; if omitted, Aura defaults to built-ins.
#
# [[plugins]]
# name = "aura-iot"
# trusted = true

# [[plugins]]
# name = "aura-ai"
# trusted = true
"#,
        )
        .into_diagnostic()?;
    }

    println!("initialized Aura project at {}", path.display());
    Ok(())
}

#[derive(Debug, Error, Diagnostic)]
#[error("avm: Z3 gate rejected execution: {message}")]
#[diagnostic(code(aura::avm::z3_gate))]
#[allow(unused_assignments)]
struct AvmGateRejected {
    message: String,
    #[label]
    span: aura_ast::Span,
}

#[derive(Clone, Copy, Debug)]
enum CcKind {
    ClangOrGcc,
}

fn compile_c(cc: &str, kind: CcKind, module_c: &Path, exe: &Path) -> miette::Result<()> {
    match kind {
        CcKind::ClangOrGcc => {
            let stdlib_c = aura_stdlib::stdlib_c_path();
            let stdlib_h = aura_stdlib::stdlib_h_path();
            let stdlib_include_dir = stdlib_h
                .parent()
                .ok_or_else(|| miette::miette!("stdlib include dir missing"))?;

            let status = Command::new(cc)
                .arg("-std=c2x")
                .arg("-g")
                .arg("-O2")
                .arg(format!("-I{}", stdlib_include_dir.display()))
                .arg(module_c)
                .arg(stdlib_c)
                .arg("-o")
                .arg(exe)
                .current_dir(module_c.parent().unwrap())
                .status()
                .into_diagnostic()?;

            if !status.success() {
                return Err(miette::miette!("C compilation failed: {status}"));
            }
        }
    }
    Ok(())
}

fn compile_wasm_wasi(clang: &Path, module_c: &Path, wasm: &Path) -> miette::Result<()> {
    let status = Command::new(clang)
        .arg("--target=wasm32-wasi")
        .arg("-std=c2x")
        .arg("-O2")
        .arg(module_c)
        .arg("-o")
        .arg(wasm)
        .current_dir(module_c.parent().unwrap())
        .status()
        .into_diagnostic()?;

    if !status.success() {
        return Err(miette::miette!(
            "WASM compilation failed: {status}. You may need a WASI sysroot (e.g. wasi-sdk) configured for clang."
        ));
    }
    Ok(())
}

fn find_c_compiler() -> Option<(String, CcKind)> {
    // Prefer clang then gcc.
    if Command::new("clang").arg("--version").output().is_ok() {
        return Some(("clang".to_string(), CcKind::ClangOrGcc));
    }
    if Command::new("gcc").arg("--version").output().is_ok() {
        return Some(("gcc".to_string(), CcKind::ClangOrGcc));
    }
    None
}

fn build_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("aura");
    PathBuf::from("build").join(stem)
}

fn exe_name(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("aura");
    format!("{stem}.exe")
}

fn wasm_name(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("aura");
    format!("{stem}.wasm")
}

fn display_path(p: &Path) -> String {
    p.to_string_lossy().to_string()
}
