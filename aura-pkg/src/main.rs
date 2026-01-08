/// Main entry point for the `aura pkg` command-line tool.
///
/// This binary provides the user-facing package manager interface.

use clap::Parser;
use aura_pkg::Cli;
use aura_pkg::{Commands, init_project, add_dependency, remove_dependency, list_dependencies, verify_package};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("Aura Package Manager CLI");
        eprintln!("Manifest path: {:?}", cli.manifest_path.as_deref().unwrap_or(std::path::Path::new(".")));
    }

    // Determine the manifest path
    let manifest_path = determine_manifest_path(cli.manifest_path.as_deref())?;

    match cli.command {
        Commands::Init(args) => {
            if cli.verbose {
                eprintln!("Initializing project: {}", args.name);
            }
            init_project(&args.name, &manifest_path, args.edition, args.authors, args.license, args.inline)
                .map_err(|e| Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )) as Box<dyn std::error::Error>)?;
        }

        Commands::Add(args) => {
            if cli.verbose {
                eprintln!("Adding dependency: {}", args.package);
            }
            add_dependency(&manifest_path, args.package, args.version, args.registry, args.dev, args.optional, args.allow_prerelease)
                .map_err(|e| Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )) as Box<dyn std::error::Error>)?;
        }

        Commands::Remove(args) => {
            if cli.verbose {
                eprintln!("Removing dependency: {}", args.package);
            }
            remove_dependency(&manifest_path, args.package, args.dev)
                .map_err(|e| Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )) as Box<dyn std::error::Error>)?;
        }

        Commands::List(args) => {
            if cli.verbose {
                eprintln!("Listing dependencies");
            }
            list_dependencies(&manifest_path, args.dev, args.tree, args.direct, args.filter)
                .map_err(|e| Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )) as Box<dyn std::error::Error>)?;
        }

        Commands::Publish(_args) => {
            if cli.verbose {
                eprintln!("Publishing package");
            }
            eprintln!("Publishing is not yet implemented (Week 4)");
        }

        Commands::Verify(args) => {
            if cli.verbose {
                eprintln!("Verifying package integrity");
            }
            verify_package(&manifest_path, args.lockfile, args.signatures, args.detailed)
                .map_err(|e| Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )) as Box<dyn std::error::Error>)?;
        }
    }

    Ok(())
}

/// Determine the correct manifest path
fn determine_manifest_path(explicit_path: Option<&std::path::Path>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = explicit_path {
        // If explicitly provided, check if it's a directory or file
        if path.is_dir() {
            Ok(path.join("Package.toml"))
        } else {
            Ok(path.to_path_buf())
        }
    } else {
        // Look for Package.toml in current directory, then parent directories
        let mut current = std::env::current_dir()?;
        loop {
            let candidate = current.join("Package.toml");
            if candidate.exists() {
                return Ok(candidate);
            }
            
            if !current.pop() {
                // Not found in any parent, use current directory as fallback
                return Ok(std::env::current_dir()?.join("Package.toml"));
            }
        }
    }
}
