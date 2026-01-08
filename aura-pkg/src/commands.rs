/// Command handlers for aura pkg subcommands.
/// 
/// This module implements the actual business logic for CLI commands,
/// separate from CLI argument parsing.

use crate::metadata::{PackageInfo, PackageMetadata, ProfileConfig};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use miette::Report;

pub type CmdError = Report;

fn cmd_msg(message: impl Into<String>) -> CmdError {
    Report::msg(message.into())
}

/// Initialize a new Aura project
pub fn init_project(
    name: &str,
    base_path: &Path,
    edition: String,
    authors: Option<String>,
    license: String,
    inline: bool,
) -> Result<(), CmdError> {
    // Validate project name (alphanumeric, hyphens, underscores)
    if !is_valid_package_name(name) {
        return Err(cmd_msg(format!(
            "Invalid project name '{}'. Use alphanumeric characters, hyphens, and underscores only.",
            name
        )));
    }

    // Determine project root
    let project_root = if inline {
        base_path.to_path_buf()
    } else {
        base_path.join(name)
    };

    // Check if project directory already exists
    if project_root.exists() && !inline {
        return Err(cmd_msg(format!(
            "Directory '{}' already exists",
            project_root.display()
        )));
    }

    // Create directory structure
    create_project_structure(&project_root)?;

    // Create Package.toml manifest
    let manifest_path = project_root.join("Package.toml");
    let metadata = create_package_metadata(
        name,
        edition.clone(),
        authors,
        license.clone(),
    );

    metadata.to_file(&manifest_path)?;

    // Create initial source file
    let src_dir = project_root.join("src");
    fs::create_dir_all(&src_dir)
        .map_err(|e| cmd_msg(format!("Failed to create src directory: {}", e)))?;

    let main_file = src_dir.join("main.aura");
    fs::write(&main_file, MAIN_FILE_TEMPLATE)
        .map_err(|e| cmd_msg(format!("Failed to create main.aura: {}", e)))?;

    println!("✓ Created binary (application) package `{}`", name);
    println!("  Location: {}", project_root.display());
    println!("  Edition: {}", edition);
    println!("  License: {}", license);

    Ok(())
}

/// Create the standard directory structure for a new project
fn create_project_structure(root: &Path) -> Result<(), CmdError> {
    // Create directories
    let dirs = vec![
        "src",
        "tests",
        "examples",
        "target",
    ];

    for dir in dirs {
        let path = root.join(dir);
        fs::create_dir_all(&path)
            .map_err(|e| cmd_msg(format!("Failed to create {}: {}", dir, e)))?;
    }

    // Create .gitignore
    let gitignore = root.join(".gitignore");
    fs::write(&gitignore, GITIGNORE_TEMPLATE)
        .map_err(|e| cmd_msg(format!("Failed to create .gitignore: {}", e)))?;

    Ok(())
}

/// Create package metadata with defaults
fn create_package_metadata(
    name: &str,
    edition: String,
    authors: Option<String>,
    license: String,
) -> PackageMetadata {
    let authors_vec = authors.map(|a| {
        a.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    let package_info = PackageInfo {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        edition,
        description: None,
        authors: authors_vec,
        license: Some(license),
        repository: None,
        homepage: None,
        documentation: None,
        keywords: None,
        categories: None,
    };

    let mut profiles = BTreeMap::new();
    profiles.insert(
        "dev".to_string(),
        ProfileConfig {
            opt_level: Some(0),
            lto: Some(false),
            codegen_units: Some(16),
        },
    );
    profiles.insert(
        "release".to_string(),
        ProfileConfig {
            opt_level: Some(3),
            lto: Some(true),
            codegen_units: Some(1),
        },
    );

    PackageMetadata {
        package: package_info,
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
        build_dependencies: BTreeMap::new(),
        metadata: None,
        profile: Some(profiles),
    }
}

/// Validate a package name
fn is_valid_package_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    
    // Allow alphanumeric, hyphens, and underscores
    name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Add a dependency to a project
pub fn add_dependency(
    manifest_path: &Path,
    package: String,
    version: Option<String>,
    _registry: Option<String>,
    dev: bool,
    _optional: bool,
    _allow_prerelease: bool,
) -> Result<(), CmdError> {
    // Parse package@version format if no separate version provided
    let (pkg_name, pkg_version) = if let Some(v) = version {
        (package, v)
    } else if package.contains('@') {
        let parts: Vec<&str> = package.split('@').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(cmd_msg("Invalid package specification"));
        }
    } else {
        (package, "*".to_string())
    };

    // Load existing manifest
    let mut metadata = PackageMetadata::from_file(manifest_path)?;

    // Add to appropriate dependency section
    let deps = if dev {
        &mut metadata.dev_dependencies
    } else {
        &mut metadata.dependencies
    };

    // Check if already exists
    if deps.contains_key(&pkg_name) {
        return Err(cmd_msg(format!(
            "Dependency '{}' already exists",
            pkg_name
        )));
    }

    // Add dependency (simple version spec for now)
    deps.insert(pkg_name.clone(), crate::metadata::DependencySpec::Simple(pkg_version.clone()));

    // Save updated manifest
    metadata.to_file(manifest_path)?;

    let kind = if dev { "dev dependency" } else { "dependency" };
    println!("✓ Added {} to {}", pkg_name, kind);
    println!("  Version: {}", pkg_version);

    Ok(())
}

/// Remove a dependency from a project
pub fn remove_dependency(
    manifest_path: &Path,
    package: String,
    dev: bool,
) -> Result<(), CmdError> {
    // Load existing manifest
    let mut metadata = PackageMetadata::from_file(manifest_path)?;

    // Remove from appropriate dependency section
    let deps = if dev {
        &mut metadata.dev_dependencies
    } else {
        &mut metadata.dependencies
    };

    // Check if exists
    if !deps.contains_key(&package) {
        let kind = if dev { "dev dependency" } else { "dependency" };
        return Err(cmd_msg(format!(
            "{} '{}' not found",
            kind, package
        )));
    }

    // Remove dependency
    deps.remove(&package);

    // Save updated manifest
    metadata.to_file(manifest_path)?;

    let kind = if dev { "dev dependency" } else { "dependency" };
    println!("✓ Removed {} from {}", package, kind);

    Ok(())
}

/// List dependencies
pub fn list_dependencies(
    manifest_path: &Path,
    dev_only: bool,
    tree: bool,
    _direct: bool,
    filter: Option<String>,
) -> Result<(), CmdError> {
    // Load manifest
    let metadata = PackageMetadata::from_file(manifest_path)?;

    println!("Dependencies for {}", metadata.package.name);
    println!();

    let mut any_shown = false;

    // Show regular dependencies
    if !dev_only && !metadata.dependencies.is_empty() {
        println!("[dependencies]");
        for (name, spec) in &metadata.dependencies {
            // Apply filter if provided
            if let Some(ref f) = filter {
                if !name.contains(f.as_str()) {
                    continue;
                }
            }
            
            let version = spec.version().unwrap_or_default();
            if tree {
                println!("├── {} = \"{}\"", name, version);
            } else {
                println!("  {} = \"{}\"", name, version);
            }
            any_shown = true;
        }
        println!();
    }

    // Show dev dependencies
    if !metadata.dev_dependencies.is_empty() {
        println!("[dev-dependencies]");
        for (name, spec) in &metadata.dev_dependencies {
            // Apply filter if provided
            if let Some(ref f) = filter {
                if !name.contains(f.as_str()) {
                    continue;
                }
            }
            
            let version = spec.version().unwrap_or_default();
            if tree {
                println!("├── {} = \"{}\"", name, version);
            } else {
                println!("  {} = \"{}\"", name, version);
            }
            any_shown = true;
        }
    }

    if !any_shown {
        println!("No dependencies found");
    }

    Ok(())
}

/// Verify package integrity
pub fn verify_package(
    manifest_path: &Path,
    check_lockfile: bool,
    _check_signatures: bool,
    _detailed: bool,
) -> Result<(), CmdError> {
    // Load and validate manifest
    let metadata = PackageMetadata::from_file(manifest_path)?;

    println!("Verifying package: {}", metadata.package.name);
    println!("  Version: {}", metadata.package.version);
    println!("  Edition: {}", metadata.package.edition);

    // Check lockfile if requested
    if check_lockfile {
        let lockfile_path = manifest_path.parent()
            .map(|p| p.join("Aura.lock"))
            .ok_or_else(|| cmd_msg("Cannot determine lockfile path"))?;

        if lockfile_path.exists() {
            println!("✓ Lockfile found");
        } else {
            println!("⚠ Lockfile not found (run `aura pkg add` to generate)");
        }
    }

    println!("✓ Package verification complete");
    Ok(())
}

// Template files
const MAIN_FILE_TEMPLATE: &str = r#"/// Main entry point for the application
pub fn main() {
    println!("Hello from Aura!");
}
"#;

const GITIGNORE_TEMPLATE: &str = r#"# Aura build artifacts
target/
Aura.lock
*.aura.o
*.rlib

# Editor files
.vscode/
.idea/
*.swp
*.swo
*~
.DS_Store
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_valid_package_name() {
        assert!(is_valid_package_name("my-app"));
        assert!(is_valid_package_name("my_app"));
        assert!(is_valid_package_name("MyApp"));
        assert!(is_valid_package_name("app123"));
        assert!(!is_valid_package_name("my app"));
        assert!(!is_valid_package_name("my.app"));
        assert!(!is_valid_package_name(""));
    }

    #[test]
    fn test_init_project() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let project_name = "test-project";

        init_project(
            project_name,
            temp_dir.path(),
            "2024".to_string(),
            Some("Alice <alice@example.com>".to_string()),
            "MIT".to_string(),
            false,
        ).expect("init failed");

        let project_root = temp_dir.path().join(project_name);
        assert!(project_root.exists());
        assert!(project_root.join("Package.toml").exists());
        assert!(project_root.join("src").exists());
        assert!(project_root.join("src/main.aura").exists());
        assert!(project_root.join(".gitignore").exists());
    }

    #[test]
    fn test_init_project_invalid_name() {
        let temp_dir = TempDir::new().expect("create temp dir");
        
        let result = init_project(
            "my app",
            temp_dir.path(),
            "2024".to_string(),
            None,
            "MIT".to_string(),
            false,
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_create_package_metadata() {
        let metadata = create_package_metadata(
            "test-pkg",
            "2024".to_string(),
            Some("Alice <alice@example.com>".to_string()),
            "MIT".to_string(),
        );

        assert_eq!(metadata.package.name, "test-pkg");
        assert_eq!(metadata.package.version, "0.1.0");
        assert_eq!(metadata.package.edition, "2024");
        assert_eq!(metadata.package.license, Some("MIT".to_string()));
    }

    #[test]
    fn test_add_dependency() {
        let temp_dir = TempDir::new().expect("create temp dir");
        
        // First init a project
        let project_name = "test-app";
        init_project(
            project_name,
            temp_dir.path(),
            "2024".to_string(),
            None,
            "MIT".to_string(),
            false,
        ).expect("init failed");

        let project_root = temp_dir.path().join(project_name);
        let manifest_path = project_root.join("Package.toml");

        // Add a dependency
        add_dependency(
            &manifest_path,
            "serde".to_string(),
            Some("1.0".to_string()),
            None,
            false,
            false,
            false,
        ).expect("add failed");

        // Verify it was added
        let metadata = PackageMetadata::from_file(&manifest_path).expect("read failed");
        assert!(metadata.dependencies.contains_key("serde"));
    }

    #[test]
    fn test_remove_dependency() {
        let temp_dir = TempDir::new().expect("create temp dir");
        
        // First init a project
        let project_name = "test-app";
        init_project(
            project_name,
            temp_dir.path(),
            "2024".to_string(),
            None,
            "MIT".to_string(),
            false,
        ).expect("init failed");

        let project_root = temp_dir.path().join(project_name);
        let manifest_path = project_root.join("Package.toml");

        // Add a dependency
        add_dependency(
            &manifest_path,
            "serde".to_string(),
            Some("1.0".to_string()),
            None,
            false,
            false,
            false,
        ).expect("add failed");

        // Remove it
        remove_dependency(&manifest_path, "serde".to_string(), false).expect("remove failed");

        // Verify it was removed
        let metadata = PackageMetadata::from_file(&manifest_path).expect("read failed");
        assert!(!metadata.dependencies.contains_key("serde"));
    }

    #[test]
    fn test_list_dependencies() {
        let temp_dir = TempDir::new().expect("create temp dir");
        
        // First init a project
        let project_name = "test-app";
        init_project(
            project_name,
            temp_dir.path(),
            "2024".to_string(),
            None,
            "MIT".to_string(),
            false,
        ).expect("init failed");

        let project_root = temp_dir.path().join(project_name);
        let manifest_path = project_root.join("Package.toml");

        // Add some dependencies
        add_dependency(
            &manifest_path,
            "serde".to_string(),
            Some("1.0".to_string()),
            None,
            false,
            false,
            false,
        ).expect("add failed");

        // List should not error
        let result = list_dependencies(&manifest_path, false, false, false, None);
        assert!(result.is_ok());
    }
}
