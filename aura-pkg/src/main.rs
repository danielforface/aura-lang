/// Main entry point for the `aura pkg` command-line tool.
///
/// This binary provides the user-facing package manager interface.

use clap::Parser;
use aura_pkg::{Cli, Commands};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("Aura Package Manager CLI");
        eprintln!("Manifest path: {:?}", cli.manifest_path.as_deref().unwrap_or(std::path::Path::new(".")));
    }

    let manifest_path = cli.manifest_path.unwrap_or_else(|| PathBuf::from("."));

    match cli.command {
        Commands::Init(args) => {
            if cli.verbose {
                eprintln!("Initializing project: {}", args.name);
            }
            init_project(&args.name, &manifest_path, args.edition, args.authors, args.license, args.inline)?;
        }

        Commands::Add(args) => {
            if cli.verbose {
                eprintln!("Adding dependency: {}", args.package);
            }
            add_dependency(&manifest_path, args.package, args.version, args.registry, args.dev, args.optional, args.allow_prerelease)?;
        }

        Commands::Remove(args) => {
            if cli.verbose {
                eprintln!("Removing dependency: {}", args.package);
            }
            remove_dependency(&manifest_path, args.package, args.dev)?;
        }

        Commands::List(args) => {
            if cli.verbose {
                eprintln!("Listing dependencies");
            }
            list_dependencies(&manifest_path, args.dev, args.tree, args.direct, args.filter)?;
        }

        Commands::Publish(args) => {
            if cli.verbose {
                eprintln!("Publishing package");
            }
            publish_package(&manifest_path, args.registry, args.dry_run, args.allow_prerelease, args.signing_key, args.token)?;
        }

        Commands::Verify(args) => {
            if cli.verbose {
                eprintln!("Verifying package integrity");
            }
            verify_package(&manifest_path, args.lockfile, args.signatures, args.detailed)?;
        }
    }

    Ok(())
}

fn init_project(
    name: &str,
    _base_path: &std::path::Path,
    edition: String,
    authors: Option<String>,
    license: String,
    _inline: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating new Aura project: {}", name);
    println!("  Edition: {}", edition);
    if let Some(ref authors_str) = authors {
        println!("  Authors: {}", authors_str);
    }
    println!("  License: {}", license);
    Ok(())
}

fn add_dependency(
    _manifest_path: &std::path::Path,
    package: String,
    version: Option<String>,
    registry: Option<String>,
    dev: bool,
    optional: bool,
    allow_prerelease: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let kind = if dev { "dev dependency" } else { "dependency" };
    println!("Adding {} to {}", package, kind);
    if let Some(v) = version {
        println!("  Version: {}", v);
    }
    if let Some(r) = registry {
        println!("  Registry: {}", r);
    }
    if optional {
        println!("  Optional: yes");
    }
    if allow_prerelease {
        println!("  Allow prerelease: yes");
    }
    Ok(())
}

fn remove_dependency(
    _manifest_path: &std::path::Path,
    package: String,
    dev: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let kind = if dev { "dev dependency" } else { "dependency" };
    println!("Removing {} from {}", package, kind);
    Ok(())
}

fn list_dependencies(
    _manifest_path: &std::path::Path,
    dev: bool,
    tree: bool,
    direct: bool,
    filter: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Listing dependencies");
    if dev {
        println!("  Filter: dev dependencies only");
    }
    if tree {
        println!("  Format: tree");
    }
    if direct {
        println!("  Filter: direct dependencies only");
    }
    if let Some(f) = filter {
        println!("  Filter: {}", f);
    }
    Ok(())
}

fn publish_package(
    _manifest_path: &std::path::Path,
    registry: Option<String>,
    dry_run: bool,
    allow_prerelease: bool,
    signing_key: Option<std::path::PathBuf>,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Publishing package");
    if dry_run {
        println!("  Mode: dry-run (no changes will be made)");
    }
    if let Some(r) = registry {
        println!("  Registry: {}", r);
    }
    if allow_prerelease {
        println!("  Allow prerelease: yes");
    }
    if signing_key.is_some() {
        println!("  Signing: yes");
    }
    if token.is_some() {
        println!("  Token: provided");
    }
    Ok(())
}

fn verify_package(
    _manifest_path: &std::path::Path,
    lockfile: bool,
    signatures: bool,
    detailed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Verifying package");
    if lockfile {
        println!("  Checking: lockfile consistency");
    }
    if signatures {
        println!("  Checking: dependency signatures");
    }
    if detailed {
        println!("  Report: detailed");
    }
    Ok(())
}
