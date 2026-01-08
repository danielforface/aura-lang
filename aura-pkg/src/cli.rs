/// CLI command-line interface for the Aura package manager.
///
/// Provides subcommands for:
/// - `aura pkg init`: Initialize a new Aura project
/// - `aura pkg add`: Add a dependency
/// - `aura pkg remove`: Remove a dependency
/// - `aura pkg list`: List current dependencies
/// - `aura pkg publish`: Publish to registry

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Aura package manager CLI
#[derive(Parser, Debug)]
#[command(name = "aura pkg")]
#[command(about = "Aura Package Manager", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Path to the workspace (defaults to current directory)
    #[arg(global = true, short, long)]
    pub manifest_path: Option<PathBuf>,

    /// Enable verbose output
    #[arg(global = true, long)]
    pub verbose: bool,

    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Subcommands for the Aura package manager
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new Aura project
    Init(InitArgs),

    /// Add a dependency to the project
    Add(AddArgs),

    /// Remove a dependency from the project
    Remove(RemoveArgs),

    /// List all dependencies
    List(ListArgs),

    /// Publish the package to the registry
    Publish(PublishArgs),

    /// Verify package integrity and dependencies
    Verify(VerifyArgs),
}

/// Arguments for the `init` subcommand
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Name of the new project
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Edition to use (2024, 2025, 2026)
    #[arg(short, long, default_value = "2024")]
    pub edition: String,

    /// Author(s) of the project
    #[arg(short, long)]
    pub authors: Option<String>,

    /// License for the project
    #[arg(short, long, default_value = "MIT")]
    pub license: String,

    /// Create the project in the current directory instead of a new subdirectory
    #[arg(long)]
    pub inline: bool,
}

/// Arguments for the `add` subcommand
#[derive(Parser, Debug)]
pub struct AddArgs {
    /// Package name (optionally with version: `package@version`)
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Specific version to add
    #[arg(short, long)]
    pub version: Option<String>,

    /// Registry to fetch from (defaults to pkg.auralang.org)
    #[arg(short, long)]
    pub registry: Option<String>,

    /// Add as a dev dependency
    #[arg(long)]
    pub dev: bool,

    /// Mark the dependency as optional
    #[arg(long)]
    pub optional: bool,

    /// Features to enable for this package
    #[arg(long)]
    pub features: Option<Vec<String>>,

    /// Allow pre-release versions
    #[arg(long)]
    pub allow_prerelease: bool,
}

/// Arguments for the `remove` subcommand
#[derive(Parser, Debug)]
pub struct RemoveArgs {
    /// Package name to remove
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Remove from dev dependencies instead of regular dependencies
    #[arg(long)]
    pub dev: bool,
}

/// Arguments for the `list` subcommand
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// List only dev dependencies
    #[arg(long)]
    pub dev: bool,

    /// Show tree structure
    #[arg(short, long)]
    pub tree: bool,

    /// Show only direct dependencies
    #[arg(long)]
    pub direct: bool,

    /// Filter by package name (partial match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Show versions for all dependencies
    #[arg(long)]
    pub versions: bool,
}

/// Arguments for the `publish` subcommand
#[derive(Parser, Debug)]
pub struct PublishArgs {
    /// Registry to publish to (defaults to pkg.auralang.org)
    #[arg(short, long)]
    pub registry: Option<String>,

    /// Don't publish, just perform checks
    #[arg(long)]
    pub dry_run: bool,

    /// Allow publishing a pre-release version
    #[arg(long)]
    pub allow_prerelease: bool,

    /// Path to the signing key (PEM format)
    #[arg(long)]
    pub signing_key: Option<PathBuf>,

    /// Token for registry authentication
    #[arg(long, env = "AURA_REGISTRY_TOKEN", value_name = "TOKEN")]
    pub token: Option<String>,
}

/// Arguments for the `verify` subcommand
#[derive(Parser, Debug)]
pub struct VerifyArgs {
    /// Check that the lock file is up to date
    #[arg(long)]
    pub lockfile: bool,

    /// Verify all dependency signatures
    #[arg(long)]
    pub signatures: bool,

    /// Report detailed information about the verification
    #[arg(short, long)]
    pub detailed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_init_command() {
        let args = vec!["aura pkg", "init", "my-project"];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Init(init_args) = cli.command {
            assert_eq!(init_args.name, "my-project");
            assert_eq!(init_args.edition, "2024");
            assert_eq!(init_args.license, "MIT");
        } else {
            panic!("Expected Init command");
        }
    }

    #[test]
    fn test_parse_init_with_options() {
        let args = vec![
            "aura pkg",
            "init",
            "my-project",
            "--edition",
            "2025",
            "--authors",
            "Alice <alice@example.com>",
            "--license",
            "Apache-2.0",
        ];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Init(init_args) = cli.command {
            assert_eq!(init_args.name, "my-project");
            assert_eq!(init_args.edition, "2025");
            assert_eq!(init_args.license, "Apache-2.0");
            assert_eq!(
                init_args.authors,
                Some("Alice <alice@example.com>".to_string())
            );
        } else {
            panic!("Expected Init command");
        }
    }

    #[test]
    fn test_parse_add_command() {
        let args = vec!["aura pkg", "add", "tensor@1.2.0"];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Add(add_args) = cli.command {
            assert_eq!(add_args.package, "tensor@1.2.0");
            assert!(!add_args.dev);
        } else {
            panic!("Expected Add command");
        }
    }

    #[test]
    fn test_parse_add_dev_dependency() {
        let args = vec![
            "aura pkg",
            "add",
            "test-utils",
            "--dev",
            "--allow-prerelease",
        ];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Add(add_args) = cli.command {
            assert_eq!(add_args.package, "test-utils");
            assert!(add_args.dev);
            assert!(add_args.allow_prerelease);
        } else {
            panic!("Expected Add command");
        }
    }

    #[test]
    fn test_parse_remove_command() {
        let args = vec!["aura pkg", "remove", "old-lib"];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Remove(remove_args) = cli.command {
            assert_eq!(remove_args.package, "old-lib");
            assert!(!remove_args.dev);
        } else {
            panic!("Expected Remove command");
        }
    }

    #[test]
    fn test_parse_list_command() {
        let args = vec!["aura pkg", "list", "--tree", "--versions"];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::List(list_args) = cli.command {
            assert!(list_args.tree);
            assert!(list_args.versions);
            assert!(!list_args.dev);
        } else {
            panic!("Expected List command");
        }
    }

    #[test]
    fn test_parse_publish_command() {
        let args = vec![
            "aura pkg",
            "publish",
            "--dry-run",
            "--registry",
            "https://pkg.example.com",
        ];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Publish(publish_args) = cli.command {
            assert!(publish_args.dry_run);
            assert_eq!(publish_args.registry, Some("https://pkg.example.com".to_string()));
        } else {
            panic!("Expected Publish command");
        }
    }

    #[test]
    fn test_parse_verify_command() {
        let args = vec![
            "aura pkg",
            "verify",
            "--lockfile",
            "--signatures",
            "--detailed",
        ];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Verify(verify_args) = cli.command {
            assert!(verify_args.lockfile);
            assert!(verify_args.signatures);
            assert!(verify_args.detailed);
        } else {
            panic!("Expected Verify command");
        }
    }

    #[test]
    fn test_parse_with_global_options() {
        let args = vec![
            "aura pkg",
            "--verbose",
            "--manifest-path",
            "/path/to/Package.toml",
            "init",
            "my-project",
        ];
        let cli = Cli::try_parse_from(&args).unwrap();
        assert!(cli.verbose);
        assert_eq!(
            cli.manifest_path,
            Some(PathBuf::from("/path/to/Package.toml"))
        );
    }

    #[test]
    fn test_parse_add_with_features() {
        let args = vec![
            "aura pkg",
            "add",
            "crypto",
            "--features",
            "sha256",
            "--features",
            "ed25519",
        ];
        let cli = Cli::try_parse_from(&args).unwrap();
        if let Commands::Add(add_args) = cli.command {
            assert_eq!(add_args.package, "crypto");
            assert_eq!(
                add_args.features,
                Some(vec!["sha256".to_string(), "ed25519".to_string()])
            );
        } else {
            panic!("Expected Add command");
        }
    }
}
