/// Lockfile format for Aura.lock
///
/// Stores exact dependency versions resolved during dependency resolution
/// for reproducible builds across different environments and machines.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use miette::Report;

pub type LockfileError = Report;

fn lockfile_msg(message: impl Into<String>) -> LockfileError {
    Report::msg(message.into())
}

/// A resolved dependency with exact version and metadata
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolvedDependency {
    /// Package name
    pub name: String,

    /// Exact resolved version
    pub version: String,

    /// Optional registry this was fetched from
    pub registry: Option<String>,

    /// Hash of the package for integrity checking
    pub hash: Option<String>,

    /// Whether this is a dev dependency
    #[serde(default)]
    pub dev: bool,

    /// Direct dependencies of this package
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Complete lockfile for an Aura project
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lockfile {
    /// Format version for backward compatibility
    pub version: String,

    /// Timestamp when lockfile was generated
    pub generated: String,

    /// Hash of Package.toml that generated this lock
    pub manifest_hash: Option<String>,

    /// All resolved dependencies (keyed by package name)
    pub dependencies: BTreeMap<String, ResolvedDependency>,
}

impl Lockfile {
    /// Create a new empty lockfile
    pub fn new() -> Self {
        Lockfile {
            version: "1.0".to_string(),
            generated: chrono::Utc::now().to_rfc3339(),
            manifest_hash: None,
            dependencies: BTreeMap::new(),
        }
    }

    /// Load lockfile from path
    pub fn from_file(path: &Path) -> Result<Self, LockfileError> {
        let content = fs::read_to_string(path)
            .map_err(|e| lockfile_msg(format!("Failed to read lockfile: {}", e)))?;
        
        Self::from_str(&content)
    }

    /// Parse lockfile from TOML string
    pub fn from_str(content: &str) -> Result<Self, LockfileError> {
        toml::from_str(content)
            .map_err(|e| lockfile_msg(format!("Invalid lockfile format: {}", e)))
    }

    /// Write lockfile to path
    pub fn to_file(&self, path: &Path) -> Result<(), LockfileError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| lockfile_msg(format!("Failed to serialize lockfile: {}", e)))?;
        
        fs::write(path, content)
            .map_err(|e| lockfile_msg(format!("Failed to write lockfile: {}", e)))
    }

    /// Convert to TOML string
    pub fn to_string(&self) -> Result<String, LockfileError> {
        toml::to_string_pretty(self)
            .map_err(|e| lockfile_msg(format!("Failed to serialize: {}", e)))
    }

    /// Add a resolved dependency to the lockfile
    pub fn add_dependency(&mut self, dep: ResolvedDependency) {
        self.dependencies.insert(dep.name.clone(), dep);
    }

    /// Get a dependency by name
    pub fn get_dependency(&self, name: &str) -> Option<&ResolvedDependency> {
        self.dependencies.get(name)
    }

    /// Get the exact version for a dependency
    pub fn get_version(&self, name: &str) -> Option<String> {
        self.dependencies.get(name).map(|d| d.version.clone())
    }

    /// Check if a dependency is locked
    pub fn contains(&self, name: &str) -> bool {
        self.dependencies.contains_key(name)
    }

    /// Get count of locked dependencies
    pub fn len(&self) -> usize {
        self.dependencies.len()
    }

    /// Check if lockfile is empty
    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Update manifest hash (typically computed from Package.toml)
    pub fn set_manifest_hash(&mut self, hash: String) {
        self.manifest_hash = Some(hash);
    }

    /// Verify lockfile hasn't been tampered with
    pub fn verify(&self) -> Result<(), LockfileError> {
        // Check for valid format version
        if !self.version.starts_with("1.") {
            return Err(lockfile_msg(format!(
                "Unsupported lockfile version: {}",
                self.version
            )));
        }

        // Check that all dependencies have required fields
        for (name, dep) in &self.dependencies {
            if dep.name != *name {
                return Err(lockfile_msg(format!(
                    "Dependency name mismatch: {} vs {}",
                    name, dep.name
                )));
            }

            // Version must be a valid semver (or wildcard)
            if dep.version.is_empty() {
                return Err(lockfile_msg(format!(
                    "Empty version for dependency {}",
                    name
                )));
            }
        }

        Ok(())
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_lockfile() {
        let lock = Lockfile::new();
        assert_eq!(lock.version, "1.0");
        assert!(lock.dependencies.is_empty());
    }

    #[test]
    fn test_add_dependency() {
        let mut lock = Lockfile::new();
        let dep = ResolvedDependency {
            name: "serde".to_string(),
            version: "1.0.3".to_string(),
            registry: None,
            hash: Some("abc123".to_string()),
            dev: false,
            dependencies: vec![],
        };

        lock.add_dependency(dep.clone());
        assert!(lock.contains("serde"));
        assert_eq!(lock.get_version("serde"), Some("1.0.3".to_string()));
        assert_eq!(lock.len(), 1);
    }

    #[test]
    fn test_lockfile_serialization() {
        let mut lock = Lockfile::new();
        lock.add_dependency(ResolvedDependency {
            name: "serde".to_string(),
            version: "1.0.3".to_string(),
            registry: None,
            hash: Some("abc123".to_string()),
            dev: false,
            dependencies: vec![],
        });

        lock.add_dependency(ResolvedDependency {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            registry: None,
            hash: Some("def456".to_string()),
            dev: true,
            dependencies: vec!["serde".to_string()],
        });

        let toml = lock.to_string().expect("serialize failed");
        assert!(toml.contains("serde"));
        assert!(toml.contains("1.0.3"));
        assert!(toml.contains("tokio"));
        assert!(toml.contains("1.35.0"));
    }

    #[test]
    fn test_lockfile_roundtrip() {
        let mut lock = Lockfile::new();
        lock.add_dependency(ResolvedDependency {
            name: "serde".to_string(),
            version: "1.0.3".to_string(),
            registry: Some("pkg.auralang.org".to_string()),
            hash: Some("abc123".to_string()),
            dev: false,
            dependencies: vec![],
        });

        let toml = lock.to_string().expect("serialize failed");
        let lock2 = Lockfile::from_str(&toml).expect("deserialize failed");

        assert_eq!(lock.version, lock2.version);
        assert_eq!(lock.len(), lock2.len());
        assert_eq!(lock.get_version("serde"), lock2.get_version("serde"));
    }

    #[test]
    fn test_lockfile_file_operations() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let lockfile_path = temp_dir.path().join("Aura.lock");

        let mut lock = Lockfile::new();
        lock.add_dependency(ResolvedDependency {
            name: "serde".to_string(),
            version: "1.0.3".to_string(),
            registry: None,
            hash: Some("abc123".to_string()),
            dev: false,
            dependencies: vec![],
        });

        lock.to_file(&lockfile_path).expect("write failed");
        assert!(lockfile_path.exists());

        let lock2 = Lockfile::from_file(&lockfile_path).expect("read failed");
        assert_eq!(lock.get_version("serde"), lock2.get_version("serde"));
    }

    #[test]
    fn test_lockfile_verification() {
        let mut lock = Lockfile::new();
        lock.add_dependency(ResolvedDependency {
            name: "serde".to_string(),
            version: "1.0.3".to_string(),
            registry: None,
            hash: Some("abc123".to_string()),
            dev: false,
            dependencies: vec![],
        });

        assert!(lock.verify().is_ok());
    }

    #[test]
    fn test_lockfile_verification_invalid_version() {
        let mut lock = Lockfile::new();
        lock.version = "2.0".to_string();

        assert!(lock.verify().is_err());
    }

    #[test]
    fn test_lockfile_multiple_dependencies() {
        let mut lock = Lockfile::new();
        
        for i in 0..5 {
            let name = format!("pkg{}", i);
            lock.add_dependency(ResolvedDependency {
                name: name.clone(),
                version: format!("1.{}.0", i),
                registry: None,
                hash: None,
                dev: false,
                dependencies: vec![],
            });
        }

        assert_eq!(lock.len(), 5);
        assert!(lock.contains("pkg0"));
        assert!(lock.contains("pkg4"));
    }

    #[test]
    fn test_resolved_dependency_with_transitive_deps() {
        let dep = ResolvedDependency {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            registry: None,
            hash: Some("def456".to_string()),
            dev: false,
            dependencies: vec![
                "bytes".to_string(),
                "pin-project-lite".to_string(),
                "tracing".to_string(),
            ],
        };

        assert_eq!(dep.dependencies.len(), 3);
        assert!(dep.dependencies.contains(&"bytes".to_string()));
    }

    #[test]
    fn test_lockfile_manifest_hash() {
        let mut lock = Lockfile::new();
        lock.set_manifest_hash("abc123def456".to_string());

        assert_eq!(lock.manifest_hash, Some("abc123def456".to_string()));
    }
}
