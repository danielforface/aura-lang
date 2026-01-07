/// Package metadata parsing from aura.toml (Package.toml)
/// Handles the manifest format, validation, and serialization

use serde::{Deserialize, Serialize};
use semver::{Version, VersionReq};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use miette::{IntoDiagnostic, Report};

pub type MetadataError = Report;

fn metadata_msg(message: impl Into<String>) -> MetadataError {
    Report::msg(message.into())
}

/// Complete package metadata as defined in aura.toml
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Package section: name, version, edition
    pub package: PackageInfo,

    /// Dependencies section: all package dependencies
    #[serde(default)]
    pub dependencies: BTreeMap<String, DependencySpec>,

    /// Dev dependencies for testing
    #[serde(default)]
    pub dev_dependencies: BTreeMap<String, DependencySpec>,

    /// Build dependencies (future)
    #[serde(default)]
    pub build_dependencies: BTreeMap<String, DependencySpec>,

    /// Metadata about the package
    #[serde(default)]
    pub metadata: Option<PackageMetadataExt>,

    /// Profile settings (optimization levels, etc)
    #[serde(default)]
    pub profile: Option<BTreeMap<String, ProfileConfig>>,
}

/// Basic package info: name, version, edition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package name (alphanumeric + - + _)
    pub name: String,

    /// Version in SemVer format (e.g., "1.2.3")
    pub version: String,

    /// Aura edition (2024, 2025, etc.)
    #[serde(default = "default_edition")]
    pub edition: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// Optional authors
    #[serde(default)]
    pub authors: Option<Vec<String>>,

    /// Optional license
    #[serde(default)]
    pub license: Option<String>,

    /// Optional repository URL
    #[serde(default)]
    pub repository: Option<String>,

    /// Optional homepage URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Optional documentation URL
    #[serde(default)]
    pub documentation: Option<String>,

    /// Keywords for discovery
    #[serde(default)]
    pub keywords: Option<Vec<String>>,

    /// Categories for organization
    #[serde(default)]
    pub categories: Option<Vec<String>>,
}

fn default_edition() -> String {
    "2024".to_string()
}

/// Dependency specification in aura.toml
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Simple version string: "1.2" or "1.2.3" or "^1.2"
    Simple(String),
    /// Detailed specification
    Detailed {
        #[serde(default)]
        version: Option<String>,

        #[serde(default)]
        registry: Option<String>,

        #[serde(default)]
        optional: Option<bool>,

        #[serde(default)]
        features: Option<Vec<String>>,
    },
}

impl DependencySpec {
    /// Extract version requirement from dependency spec
    pub fn version(&self) -> Option<String> {
        match self {
            DependencySpec::Simple(v) => Some(v.clone()),
            DependencySpec::Detailed { version, .. } => version.clone(),
        }
    }

    /// Extract registry override if present
    pub fn registry(&self) -> Option<String> {
        match self {
            DependencySpec::Simple(_) => None,
            DependencySpec::Detailed { registry, .. } => registry.clone(),
        }
    }

    /// Check if this is an optional dependency
    pub fn is_optional(&self) -> bool {
        match self {
            DependencySpec::Simple(_) => false,
            DependencySpec::Detailed { optional, .. } => optional.unwrap_or(false),
        }
    }
}

/// Extended package metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageMetadataExt {
    /// Whether package requires signature verification
    #[serde(default)]
    pub require_signature: Option<bool>,

    /// Public key ID for signature verification
    #[serde(default)]
    pub trusted_key_id: Option<String>,

    /// Custom registry URL override
    #[serde(default)]
    pub registry_url: Option<String>,
}

/// Profile configuration (debug, release, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub opt_level: Option<u32>,

    #[serde(default)]
    pub lto: Option<bool>,

    #[serde(default)]
    pub codegen_units: Option<u32>,
}

impl PackageMetadata {
    /// Parse package metadata from aura.toml file
    pub fn from_file(path: &Path) -> Result<Self, MetadataError> {
        let content = fs::read_to_string(path)
            .into_diagnostic()
            .map_err(|e| metadata_msg(format!("failed to read {}: {e}", path.display())))?;

        Self::from_str(&content)
    }

    /// Parse package metadata from TOML string
    pub fn from_str(content: &str) -> Result<Self, MetadataError> {
        let metadata: PackageMetadata = toml::from_str(content)
            .map_err(|e| metadata_msg(format!("invalid aura.toml: {e}")))?;

        // Validate the metadata
        metadata.validate()?;
        Ok(metadata)
    }

    /// Write metadata to TOML file
    pub fn to_file(&self, path: &Path) -> Result<(), MetadataError> {
        let content = toml::to_string_pretty(self)
            .into_diagnostic()?;
        fs::write(path, content)
            .into_diagnostic()
            .map_err(|e| metadata_msg(format!("failed to write {}: {e}", path.display())))
    }

    /// Convert metadata to TOML string
    pub fn to_string(&self) -> Result<String, MetadataError> {
        toml::to_string_pretty(self)
            .into_diagnostic()
            .map_err(|e| metadata_msg(format!("failed to serialize metadata: {e}")))
    }

    /// Validate package metadata
    pub fn validate(&self) -> Result<(), MetadataError> {
        // Validate package name
        self.validate_package_name(&self.package.name)?;

        // Validate version is valid SemVer
        Version::parse(&self.package.version)
            .map_err(|e| metadata_msg(format!("invalid version '{}': {e}", self.package.name)))?;

        // Validate edition
        if !self.is_valid_edition(&self.package.edition) {
            return Err(metadata_msg(format!("unknown edition: {}", self.package.edition)));
        }

        // Validate all dependencies have valid version specs
        for (name, spec) in &self.dependencies {
            self.validate_dependency(name, spec)?;
        }

        for (name, spec) in &self.dev_dependencies {
            self.validate_dependency(name, spec)?;
        }

        Ok(())
    }

    fn validate_package_name(&self, name: &str) -> Result<(), MetadataError> {
        if name.is_empty() {
            return Err(metadata_msg("package name cannot be empty"));
        }

        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(metadata_msg(format!(
                "invalid package name '{}': must contain only alphanumeric, -, _",
                name
            )));
        }

        if name.len() > 64 {
            return Err(metadata_msg(format!(
                "package name too long: max 64 characters, got {}",
                name.len()
            )));
        }

        Ok(())
    }

    fn validate_dependency(&self, name: &str, spec: &DependencySpec) -> Result<(), MetadataError> {
        self.validate_package_name(name)?;

        if let Some(version_str) = spec.version() {
            // Try to parse as version requirement (supports ^, ~, etc.)
            VersionReq::parse(&version_str)
                .map_err(|e| metadata_msg(format!(
                    "invalid version requirement for dependency '{}': {e}",
                    name
                )))?;
        }

        Ok(())
    }

    fn is_valid_edition(&self, edition: &str) -> bool {
        matches!(edition, "2024" | "2025" | "2026")
    }

    /// Get all dependencies including optional
    pub fn all_dependencies(&self) -> BTreeMap<String, &DependencySpec> {
        let mut all = BTreeMap::new();
        for (name, spec) in &self.dependencies {
            all.insert(name.clone(), spec);
        }
        for (name, spec) in &self.dev_dependencies {
            all.insert(format!("{}_dev", name), spec);
        }
        all
    }

    /// Get only required (non-optional) dependencies
    pub fn required_dependencies(&self) -> BTreeMap<String, &DependencySpec> {
        self.dependencies
            .iter()
            .filter(|(_, spec)| !spec.is_optional())
            .map(|(k, v)| (k.clone(), v))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_metadata() {
        let toml = r#"
[package]
name = "my-package"
version = "1.0.0"
edition = "2024"
"#;
        let metadata = PackageMetadata::from_str(toml).expect("parse failed");
        assert_eq!(metadata.package.name, "my-package");
        assert_eq!(metadata.package.version, "1.0.0");
        assert_eq!(metadata.package.edition, "2024");
    }

    #[test]
    fn test_parse_with_dependencies() {
        let toml = r#"
[package]
name = "my-app"
version = "1.0.0"
edition = "2024"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }

[dev-dependencies]
tempfile = "3.0"
"#;
        let metadata = PackageMetadata::from_str(toml).expect("parse failed");
        assert_eq!(metadata.dependencies.len(), 2);
        assert_eq!(metadata.dev_dependencies.len(), 1);
    }

    #[test]
    fn test_validate_invalid_version() {
        let toml = r#"
[package]
name = "bad-version"
version = "not-a-version"
edition = "2024"
"#;
        let result = PackageMetadata::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_package_name() {
        let toml = r#"
[package]
name = "package!@#"
version = "1.0.0"
edition = "2024"
"#;
        let result = PackageMetadata::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_spec_parsing() {
        let toml = r#"
[package]
name = "app"
version = "1.0.0"

[dependencies]
simple = "1.0"
detailed = { version = "2.0", optional = true }
"#;
        let metadata = PackageMetadata::from_str(toml).expect("parse failed");
        
        let simple = metadata.dependencies.get("simple").unwrap();
        assert_eq!(simple.version(), Some("1.0".to_string()));
        assert!(!simple.is_optional());

        let detailed = metadata.dependencies.get("detailed").unwrap();
        assert_eq!(detailed.version(), Some("2.0".to_string()));
        assert!(detailed.is_optional());
    }

    #[test]
    fn test_round_trip_serialization() {
        let toml = r#"
[package]
name = "test-pkg"
version = "1.2.3"
edition = "2024"
description = "A test package"
authors = ["Alice <alice@example.com>"]

[dependencies]
serde = "1.0"
"#;
        let metadata = PackageMetadata::from_str(toml).expect("parse failed");
        let output = metadata.to_string().expect("serialize failed");
        let metadata2 = PackageMetadata::from_str(&output).expect("re-parse failed");
        assert_eq!(metadata.package.name, metadata2.package.name);
        assert_eq!(metadata.package.version, metadata2.package.version);
    }
}
