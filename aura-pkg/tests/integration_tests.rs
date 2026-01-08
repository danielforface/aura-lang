/// Integration tests for aura-pkg CLI
///
/// Tests the full command-line interface with realistic workflows

use aura_pkg::{Cli, PackageMetadata};
use clap::Parser;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper to run CLI commands in a test project
#[allow(dead_code)]
struct TestProject {
    root: TempDir,
    manifest_path: PathBuf,
}

#[allow(dead_code)]
impl TestProject {
    /// Create a new test project
    fn new() -> Self {
        let root = TempDir::new().expect("create temp dir");
        let manifest_path = root.path().join("Package.toml");
        
        TestProject {
            root,
            manifest_path,
        }
    }

    /// Initialize project with metadata
    fn init(&self, name: &str, edition: &str) {
        aura_pkg::init_project(
            name,
            self.root.path(),
            edition.to_string(),
            None,
            "MIT".to_string(),
            false,
        ).expect("init failed");
    }

    /// Add a dependency
    fn add(&self, package: &str, version: &str, dev: bool) {
        aura_pkg::add_dependency(
            &self.manifest_path.parent().unwrap().join(package).parent().unwrap().join("Package.toml"),
            package.to_string(),
            Some(version.to_string()),
            None,
            dev,
            false,
            false,
        ).expect("add failed");
    }

    /// Remove a dependency
    fn remove(&self, package: &str, dev: bool) {
        aura_pkg::remove_dependency(
            &self.manifest_path,
            package.to_string(),
            dev,
        ).expect("remove failed");
    }

    /// Get the manifest
    fn manifest(&self) -> PackageMetadata {
        // Adjust for the actual project structure
        let actual_manifest = self.root.path().walk_up_find("Package.toml");
        if let Some(actual) = actual_manifest {
            PackageMetadata::from_file(&actual).expect("read manifest failed")
        } else {
            panic!("No Package.toml found");
        }
    }

    /// Path to the root
    fn path(&self) -> &Path {
        self.root.path()
    }
}

#[allow(dead_code)]
trait PathExt {
    fn walk_up_find(&self, name: &str) -> Option<PathBuf>;
}

#[allow(dead_code)]
impl PathExt for Path {
    fn walk_up_find(&self, name: &str) -> Option<PathBuf> {
        let mut current = self.to_path_buf();
        loop {
            let candidate = current.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
            if !current.pop() {
                return None;
            }
        }
    }
}

#[test]
fn test_cli_parse_init() {
    let args = vec!["aura-pkg", "init", "test-app"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Init(init_args) => {
            assert_eq!(init_args.name, "test-app");
            assert_eq!(init_args.edition, "2024");
        }
        _ => panic!("Expected Init command"),
    }
}

#[test]
fn test_cli_parse_add() {
    let args = vec!["aura-pkg", "add", "serde", "--version", "1.0"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Add(add_args) => {
            assert_eq!(add_args.package, "serde");
            assert_eq!(add_args.version, Some("1.0".to_string()));
            assert!(!add_args.dev);
        }
        _ => panic!("Expected Add command"),
    }
}

#[test]
fn test_cli_parse_add_dev() {
    let args = vec!["aura-pkg", "add", "pytest", "--dev"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Add(add_args) => {
            assert_eq!(add_args.package, "pytest");
            assert!(add_args.dev);
        }
        _ => panic!("Expected Add command"),
    }
}

#[test]
fn test_cli_parse_remove() {
    let args = vec!["aura-pkg", "remove", "serde"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Remove(remove_args) => {
            assert_eq!(remove_args.package, "serde");
            assert!(!remove_args.dev);
        }
        _ => panic!("Expected Remove command"),
    }
}

#[test]
fn test_cli_parse_list() {
    let args = vec!["aura-pkg", "list", "--tree", "--versions"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::List(list_args) => {
            assert!(list_args.tree);
            assert!(list_args.versions);
        }
        _ => panic!("Expected List command"),
    }
}

#[test]
fn test_cli_parse_verify() {
    let args = vec!["aura-pkg", "verify", "--lockfile", "--signatures"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Verify(verify_args) => {
            assert!(verify_args.lockfile);
            assert!(verify_args.signatures);
        }
        _ => panic!("Expected Verify command"),
    }
}

#[test]
fn test_cli_parse_publish() {
    let args = vec!["aura-pkg", "publish", "--dry-run"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Publish(publish_args) => {
            assert!(publish_args.dry_run);
        }
        _ => panic!("Expected Publish command"),
    }
}

#[test]
fn test_cli_parse_with_manifest_path() {
    let args = vec!["aura-pkg", "--manifest-path", "/home/user/project", "init", "app"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    assert_eq!(cli.manifest_path, Some(PathBuf::from("/home/user/project")));
}

#[test]
fn test_cli_parse_verbose_flag() {
    let args = vec!["aura-pkg", "--verbose", "init", "app"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    assert!(cli.verbose);
}

#[test]
fn test_cli_init_project_workflow() {
    let temp = TempDir::new().expect("create temp dir");
    let project_root = temp.path().join("my-app");

    aura_pkg::init_project(
        "my-app",
        temp.path(),
        "2024".to_string(),
        Some("Alice <alice@example.com>".to_string()),
        "MIT".to_string(),
        false,
    ).expect("init failed");

    // Verify structure
    assert!(project_root.exists());
    assert!(project_root.join("Package.toml").exists());
    assert!(project_root.join("src").exists());
    assert!(project_root.join("src/main.aura").exists());
    assert!(project_root.join(".gitignore").exists());

    // Verify manifest
    let manifest = PackageMetadata::from_file(&project_root.join("Package.toml"))
        .expect("read manifest failed");
    assert_eq!(manifest.package.name, "my-app");
    assert_eq!(manifest.package.version, "0.1.0");
    assert_eq!(manifest.package.edition, "2024");
    assert_eq!(manifest.package.authors, Some(vec!["Alice <alice@example.com>".to_string()]));
}

#[test]
fn test_cli_add_dependency_workflow() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Add dependency
    aura_pkg::add_dependency(
        &manifest_path,
        "serde".to_string(),
        Some("1.0".to_string()),
        None,
        false,
        false,
        false,
    ).expect("add failed");

    // Verify dependency was added
    let manifest = PackageMetadata::from_file(&manifest_path).expect("read failed");
    assert!(manifest.dependencies.contains_key("serde"));
}

#[test]
fn test_cli_add_multiple_dependencies() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Add multiple dependencies
    for (pkg, ver) in &[("serde", "1.0"), ("tokio", "1.35"), ("regex", "1.10")] {
        aura_pkg::add_dependency(
            &manifest_path,
            pkg.to_string(),
            Some(ver.to_string()),
            None,
            false,
            false,
            false,
        ).expect("add failed");
    }

    // Verify all dependencies were added
    let manifest = PackageMetadata::from_file(&manifest_path).expect("read failed");
    assert_eq!(manifest.dependencies.len(), 3);
    assert!(manifest.dependencies.contains_key("serde"));
    assert!(manifest.dependencies.contains_key("tokio"));
    assert!(manifest.dependencies.contains_key("regex"));
}

#[test]
fn test_cli_remove_dependency_workflow() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Add and remove dependency
    aura_pkg::add_dependency(
        &manifest_path,
        "serde".to_string(),
        Some("1.0".to_string()),
        None,
        false,
        false,
        false,
    ).expect("add failed");

    aura_pkg::remove_dependency(
        &manifest_path,
        "serde".to_string(),
        false,
    ).expect("remove failed");

    // Verify dependency was removed
    let manifest = PackageMetadata::from_file(&manifest_path).expect("read failed");
    assert!(!manifest.dependencies.contains_key("serde"));
}

#[test]
fn test_cli_dev_vs_regular_dependencies() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Add regular and dev dependencies
    aura_pkg::add_dependency(
        &manifest_path,
        "serde".to_string(),
        Some("1.0".to_string()),
        None,
        false,
        false,
        false,
    ).expect("add serde failed");

    aura_pkg::add_dependency(
        &manifest_path,
        "tokio".to_string(),
        Some("1.35".to_string()),
        None,
        true,
        false,
        false,
    ).expect("add tokio failed");

    // Verify both types were added
    let manifest = PackageMetadata::from_file(&manifest_path).expect("read failed");
    assert!(manifest.dependencies.contains_key("serde"));
    assert!(manifest.dev_dependencies.contains_key("tokio"));
    assert!(!manifest.dependencies.contains_key("tokio"));
    assert!(!manifest.dev_dependencies.contains_key("serde"));
}

#[test]
fn test_cli_list_dependencies() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Add dependencies
    aura_pkg::add_dependency(
        &manifest_path,
        "serde".to_string(),
        Some("1.0".to_string()),
        None,
        false,
        false,
        false,
    ).expect("add failed");

    // List should succeed
    let result = aura_pkg::list_dependencies(&manifest_path, false, false, false, None);
    assert!(result.is_ok());
}

#[test]
fn test_cli_verify_package() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Verify should succeed
    let result = aura_pkg::verify_package(&manifest_path, false, false, false);
    assert!(result.is_ok());
}

#[test]
fn test_cli_invalid_package_name() {
    let temp = TempDir::new().expect("create temp dir");

    let result = aura_pkg::init_project(
        "my app",  // Space - invalid
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    );

    assert!(result.is_err());
}

#[test]
fn test_cli_duplicate_dependency_error() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Add first time should succeed
    aura_pkg::add_dependency(
        &manifest_path,
        "serde".to_string(),
        Some("1.0".to_string()),
        None,
        false,
        false,
        false,
    ).expect("first add failed");

    // Add second time should fail
    let result = aura_pkg::add_dependency(
        &manifest_path,
        "serde".to_string(),
        Some("1.0".to_string()),
        None,
        false,
        false,
        false,
    );

    assert!(result.is_err());
}

#[test]
fn test_cli_remove_nonexistent_dependency_error() {
    let temp = TempDir::new().expect("create temp dir");
    
    aura_pkg::init_project(
        "test-app",
        temp.path(),
        "2024".to_string(),
        None,
        "MIT".to_string(),
        false,
    ).expect("init failed");

    let project_root = temp.path().join("test-app");
    let manifest_path = project_root.join("Package.toml");

    // Remove non-existent dependency should fail
    let result = aura_pkg::remove_dependency(
        &manifest_path,
        "nonexistent".to_string(),
        false,
    );

    assert!(result.is_err());
}

#[test]
fn test_cli_package_at_version_format() {
    let args = vec!["aura-pkg", "add", "serde@1.0"];
    let cli = Cli::try_parse_from(&args).expect("parse failed");
    match cli.command {
        aura_pkg::Commands::Add(add_args) => {
            assert_eq!(add_args.package, "serde@1.0");
        }
        _ => panic!("Expected Add command"),
    }
}
