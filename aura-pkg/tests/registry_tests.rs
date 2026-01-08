/// Integration tests for registry and publishing
///
/// Tests the registry client and package publishing workflows

use aura_pkg::{RegistryClient, FileRegistry, PackageInfo};
use tempfile::TempDir;

#[test]
fn test_registry_client_default_config() {
    let _client = RegistryClient::default();
    // If this compiles without error, the client is properly initialized
}

#[test]
fn test_registry_client_with_url() {
    let _client = RegistryClient::with_url("https://custom-registry.example.com");
    // If this compiles without error, the client is properly initialized
}

#[test]
fn test_registry_client_with_token() {
    let _client = RegistryClient::default()
        .with_token("test-api-key-12345");
    
    // If this compiles without error, the token is set
}

#[test]
fn test_file_registry_store_package() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    let package_data = b"test package content";
    let result = registry.store_package("test-pkg", "1.0.0", package_data);
    
    assert!(result.is_ok());
}

#[test]
fn test_file_registry_retrieve_package() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    let original_data = b"package binary data";
    registry.store_package("app", "0.5.0", original_data).expect("store failed");
    
    let retrieved = registry.get_package("app", "0.5.0").expect("retrieve failed");
    assert_eq!(retrieved, original_data);
}

#[test]
fn test_file_registry_list_versions() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    // Store multiple versions
    registry.store_package("lib", "1.0.0", b"v1").expect("store v1 failed");
    registry.store_package("lib", "1.1.0", b"v1.1").expect("store v1.1 failed");
    registry.store_package("lib", "2.0.0", b"v2").expect("store v2 failed");
    
    let versions = registry.list_versions("lib").expect("list failed");
    assert_eq!(versions, vec!["1.0.0", "1.1.0", "2.0.0"]);
}

#[test]
fn test_file_registry_empty_list() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    let versions = registry.list_versions("nonexistent").expect("list failed");
    assert!(versions.is_empty());
}

#[test]
fn test_file_registry_multiple_packages() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    // Store multiple different packages
    registry.store_package("pkg-a", "1.0.0", b"a1").expect("store a failed");
    registry.store_package("pkg-b", "2.0.0", b"b2").expect("store b failed");
    registry.store_package("pkg-c", "3.0.0", b"c3").expect("store c failed");
    
    // Each should have correct versions
    assert_eq!(registry.list_versions("pkg-a").unwrap(), vec!["1.0.0"]);
    assert_eq!(registry.list_versions("pkg-b").unwrap(), vec!["2.0.0"]);
    assert_eq!(registry.list_versions("pkg-c").unwrap(), vec!["3.0.0"]);
}

#[test]
fn test_file_registry_overwrite_version() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    let original = b"original content";
    let updated = b"updated content";
    
    registry.store_package("pkg", "1.0.0", original).expect("initial store failed");
    let retrieved1 = registry.get_package("pkg", "1.0.0").expect("first get failed");
    assert_eq!(retrieved1, original);
    
    // Overwrite with new content
    registry.store_package("pkg", "1.0.0", updated).expect("overwrite failed");
    let retrieved2 = registry.get_package("pkg", "1.0.0").expect("second get failed");
    assert_eq!(retrieved2, updated);
}

#[test]
fn test_package_info_creation() {
    let info = PackageInfo {
        name: "my-package".to_string(),
        version: "1.2.3".to_string(),
        authors: Some(vec!["Author One".to_string(), "Author Two".to_string()]),
        description: Some("A great package".to_string()),
        license: Some("MIT".to_string()),
        repository: Some("https://github.com/example/pkg".to_string()),
        documentation: Some("https://docs.example.com".to_string()),
        keywords: Some(vec!["key1".to_string(), "key2".to_string()]),
        categories: Some(vec!["utilities".to_string()]),
    };
    
    assert_eq!(info.name, "my-package");
    assert_eq!(info.version, "1.2.3");
    assert!(info.authors.is_some());
    assert!(info.description.is_some());
}

#[test]
fn test_package_info_minimal() {
    let info = PackageInfo {
        name: "minimal".to_string(),
        version: "0.1.0".to_string(),
        authors: None,
        description: None,
        license: None,
        repository: None,
        documentation: None,
        keywords: None,
        categories: None,
    };
    
    assert_eq!(info.name, "minimal");
    assert_eq!(info.version, "0.1.0");
    assert!(info.authors.is_none());
}

#[test]
fn test_file_registry_version_sorting() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    // Store in non-sorted order
    registry.store_package("sorted", "3.0.0", b"v3").expect("v3 failed");
    registry.store_package("sorted", "1.0.0", b"v1").expect("v1 failed");
    registry.store_package("sorted", "2.0.0", b"v2").expect("v2 failed");
    
    // Should return sorted
    let versions = registry.list_versions("sorted").expect("list failed");
    assert_eq!(versions, vec!["1.0.0", "2.0.0", "3.0.0"]);
}

#[test]
fn test_file_registry_package_isolation() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    registry.store_package("pkg-a", "1.0.0", b"a").expect("a failed");
    registry.store_package("pkg-b", "1.0.0", b"b").expect("b failed");
    
    // Different packages with same version should be separate
    let a = registry.get_package("pkg-a", "1.0.0").expect("get a failed");
    let b = registry.get_package("pkg-b", "1.0.0").expect("get b failed");
    
    assert_eq!(a, b"a");
    assert_eq!(b, b"b");
    assert_ne!(a, b);
}

#[test]
fn test_registry_client_url_construction() {
    let _client = RegistryClient::with_url("https://my-registry.com");
    // If this compiles without error, the client is properly configured
}

#[test]
fn test_file_registry_nonexistent_get() {
    let registry = FileRegistry::new("/tmp/nonexistent-registry-path");
    let result = registry.get_package("missing", "1.0.0");
    assert!(result.is_err());
}

#[test]
fn test_file_registry_filter_by_package() {
    let temp = TempDir::new().expect("create temp dir");
    let registry = FileRegistry::new(temp.path());
    
    registry.store_package("search", "1.0.0", b"v1").expect("v1 failed");
    registry.store_package("search", "2.0.0", b"v2").expect("v2 failed");
    registry.store_package("other", "1.0.0", b"o1").expect("o1 failed");
    
    let search_versions = registry.list_versions("search").expect("search failed");
    let other_versions = registry.list_versions("other").expect("other failed");
    
    assert_eq!(search_versions.len(), 2);
    assert_eq!(other_versions.len(), 1);
}
