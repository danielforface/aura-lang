/// Registry client for package distribution and discovery
///
/// Handles communication with package registries via HTTP,
/// supporting package publishing, retrieval, and metadata querying.

use serde::{Deserialize, Serialize};
use std::path::Path;
use miette::Report;

pub type RegistryError = Report;

fn registry_msg(message: impl Into<String>) -> RegistryError {
    Report::msg(message.into())
}

/// Package metadata in registry
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub authors: Option<Vec<String>>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
}

/// Package version info from registry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub published_at: String,
    pub downloads: u64,
    pub yanked: bool,
    pub hash: Option<String>,
    pub size: u64,
    pub download_url: String,
}

/// Registry API responses
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageListResponse {
    pub packages: Vec<PackageInfo>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionListResponse {
    pub versions: Vec<VersionInfo>,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishResponse {
    pub success: bool,
    pub message: String,
    pub url: Option<String>,
}

/// Registry client configuration
#[derive(Clone, Debug)]
pub struct RegistryConfig {
    pub base_url: String,
    pub auth_token: Option<String>,
    pub timeout_secs: u64,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        RegistryConfig {
            base_url: "https://registry.aura-lang.org".to_string(),
            auth_token: None,
            timeout_secs: 30,
        }
    }
}

/// HTTP-based registry client
pub struct RegistryClient {
    config: RegistryConfig,
    client: Option<reqwest::blocking::Client>,
}

impl RegistryClient {
    /// Create a new registry client
    pub fn new(config: RegistryConfig) -> Self {
        // Build HTTP client
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .ok();

        RegistryClient {
            config,
            client,
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(RegistryConfig::default())
    }

    /// Create with custom base URL
    pub fn with_url(url: impl Into<String>) -> Self {
        let mut config = RegistryConfig::default();
        config.base_url = url.into();
        Self::new(config)
    }

    /// Set authentication token
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.config.auth_token = Some(token.into());
        self
    }

    /// Fetch package list by name
    pub fn search(&self, query: &str, page: u32) -> Result<PackageListResponse, RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let url = format!(
            "{}/api/packages/search?q={}&page={}",
            self.config.base_url, query, page
        );

        let response = client
            .get(&url)
            .send()
            .map_err(|e| registry_msg(format!("Failed to search registry: {}", e)))?;

        response
            .json()
            .map_err(|e| registry_msg(format!("Invalid registry response: {}", e)))
    }

    /// Get all versions of a package
    pub fn get_versions(&self, package: &str) -> Result<VersionListResponse, RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let url = format!(
            "{}/api/packages/{}/versions",
            self.config.base_url, package
        );

        let response = client
            .get(&url)
            .send()
            .map_err(|e| registry_msg(format!("Failed to fetch versions: {}", e)))?;

        response
            .json()
            .map_err(|e| registry_msg(format!("Invalid version list: {}", e)))
    }

    /// Get specific package version info
    pub fn get_version(
        &self,
        package: &str,
        version: &str,
    ) -> Result<VersionInfo, RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let url = format!(
            "{}/api/packages/{}/versions/{}",
            self.config.base_url, package, version
        );

        let response = client
            .get(&url)
            .send()
            .map_err(|e| registry_msg(format!("Failed to fetch version info: {}", e)))?;

        response
            .json()
            .map_err(|e| registry_msg(format!("Invalid version info: {}", e)))
    }

    /// Download package tarball
    pub fn download_package(
        &self,
        package: &str,
        version: &str,
        dest: &Path,
    ) -> Result<(), RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let url = format!(
            "{}/api/packages/{}/versions/{}/download",
            self.config.base_url, package, version
        );

        let mut response = client
            .get(&url)
            .send()
            .map_err(|e| registry_msg(format!("Failed to download: {}", e)))?;

        let mut file = std::fs::File::create(dest)
            .map_err(|e| registry_msg(format!("Failed to create file: {}", e)))?;

        std::io::copy(&mut response, &mut file)
            .map_err(|e| registry_msg(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Publish a package
    pub fn publish(
        &self,
        package_info: &PackageInfo,
        _package_data: &[u8],
        signature: Option<&str>,
    ) -> Result<PublishResponse, RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let token = self.config.auth_token.as_ref()
            .ok_or_else(|| registry_msg("Authentication token required for publishing"))?;

        let url = format!("{}/api/packages/publish", self.config.base_url);

        let mut request = client
            .post(&url)
            .bearer_auth(token);

        // Add package metadata as JSON
        let metadata = serde_json::json!({
            "name": package_info.name,
            "version": package_info.version,
            "authors": package_info.authors,
            "description": package_info.description,
            "license": package_info.license,
            "repository": package_info.repository,
            "documentation": package_info.documentation,
            "keywords": package_info.keywords,
            "categories": package_info.categories,
            "signature": signature,
        });

        request = request
            .json(&metadata);

        let response = request
            .send()
            .map_err(|e| registry_msg(format!("Failed to publish: {}", e)))?;

        response
            .json()
            .map_err(|e| registry_msg(format!("Invalid publish response: {}", e)))
    }

    /// Yank a version (mark as unsafe)
    pub fn yank_version(&self, package: &str, version: &str) -> Result<(), RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let token = self.config.auth_token.as_ref()
            .ok_or_else(|| registry_msg("Authentication required"))?;

        let url = format!(
            "{}/api/packages/{}/versions/{}/yank",
            self.config.base_url, package, version
        );

        let response = client
            .delete(&url)
            .bearer_auth(token)
            .send()
            .map_err(|e| registry_msg(format!("Failed to yank version: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(registry_msg(format!(
                "Failed to yank version: {}",
                response.status()
            )))
        }
    }

    /// Check if registry is available
    pub fn health_check(&self) -> Result<(), RegistryError> {
        let client = self.client.as_ref()
            .ok_or_else(|| registry_msg("HTTP client not available"))?;

        let url = format!("{}/health", self.config.base_url);

        let response = client
            .get(&url)
            .send()
            .map_err(|e| registry_msg(format!("Registry unavailable: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(registry_msg("Registry health check failed"))
        }
    }
}

/// File-based registry for local development
pub struct FileRegistry {
    base_path: std::path::PathBuf,
}

impl FileRegistry {
    /// Create a file-based registry
    pub fn new(path: impl AsRef<Path>) -> Self {
        FileRegistry {
            base_path: path.as_ref().to_path_buf(),
        }
    }

    /// Get package from filesystem
    pub fn get_package(&self, package: &str, version: &str) -> Result<Vec<u8>, RegistryError> {
        let path = self.base_path.join(format!("{}-{}.aura", package, version));

        std::fs::read(&path)
            .map_err(|e| registry_msg(format!("Failed to read package: {}", e)))
    }

    /// Store package in filesystem
    pub fn store_package(
        &self,
        package: &str,
        version: &str,
        data: &[u8],
    ) -> Result<(), RegistryError> {
        std::fs::create_dir_all(&self.base_path)
            .map_err(|e| registry_msg(format!("Failed to create registry dir: {}", e)))?;

        let path = self.base_path.join(format!("{}-{}.aura", package, version));

        std::fs::write(&path, data)
            .map_err(|e| registry_msg(format!("Failed to write package: {}", e)))
    }

    /// List available versions
    pub fn list_versions(&self, package: &str) -> Result<Vec<String>, RegistryError> {
        let mut versions = Vec::new();

        if !self.base_path.exists() {
            return Ok(versions);
        }

        for entry in std::fs::read_dir(&self.base_path)
            .map_err(|e| registry_msg(format!("Failed to read registry: {}", e)))?
        {
            let entry = entry
                .map_err(|e| registry_msg(format!("Error reading entry: {}", e)))?;

            let filename = entry.file_name();
            let name = filename.to_string_lossy();

            let prefix = format!("{}-", package);
            if name.starts_with(&prefix) && name.ends_with(".aura") {
                let version = name[prefix.len()..name.len() - 5].to_string();
                versions.push(version);
            }
        }

        versions.sort();
        Ok(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_config_default() {
        let config = RegistryConfig::default();
        assert_eq!(config.base_url, "https://registry.aura-lang.org");
        assert_eq!(config.timeout_secs, 30);
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_registry_client_creation() {
        let client = RegistryClient::default();
        assert!(client.client.is_some());
    }

    #[test]
    fn test_registry_client_with_token() {
        let client = RegistryClient::default().with_token("test-token");
        assert_eq!(client.config.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_file_registry_new() {
        let registry = FileRegistry::new("/tmp/registry");
        assert_eq!(registry.base_path, std::path::PathBuf::from("/tmp/registry"));
    }

    #[test]
    fn test_file_registry_store_and_list() {
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let registry = FileRegistry::new(temp.path());

        let data = b"package data";
        registry.store_package("serde", "1.0.0", data).expect("store failed");

        let versions = registry.list_versions("serde").expect("list failed");
        assert_eq!(versions, vec!["1.0.0"]);
    }

    #[test]
    fn test_file_registry_get_package() {
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let registry = FileRegistry::new(temp.path());

        let data = b"test package content";
        registry.store_package("test", "0.1.0", data).expect("store failed");

        let retrieved = registry.get_package("test", "0.1.0").expect("get failed");
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_file_registry_nonexistent() {
        let registry = FileRegistry::new("/nonexistent/path");
        let result = registry.get_package("missing", "1.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_package_info() {
        let info = PackageInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            authors: Some(vec!["Author".to_string()]),
            description: Some("Test package".to_string()),
            license: Some("MIT".to_string()),
            repository: None,
            documentation: None,
            keywords: None,
            categories: None,
        };

        assert_eq!(info.name, "test");
        assert_eq!(info.version, "1.0.0");
    }

    #[test]
    fn test_version_info() {
        let version = VersionInfo {
            version: "1.0.0".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            downloads: 100,
            yanked: false,
            hash: Some("abc123".to_string()),
            size: 1024,
            download_url: "https://example.com/test-1.0.0.tar.gz".to_string(),
        };

        assert_eq!(version.downloads, 100);
        assert!(!version.yanked);
    }
}
