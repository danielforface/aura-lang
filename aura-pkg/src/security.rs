use miette::Result;
use regex::Regex;
use std::collections::HashSet;

/// Package name validation rules
const VALID_PACKAGE_NAME_PATTERN: &str = r"^[a-zA-Z0-9_\-\.]{1,64}$";
const RESERVED_NAMES: &[&str] = &[
    "aura", "aura-pkg", "aura-core", "std", "core", "alloc", "proc-macro",
];

/// URL validation for registry endpoints
pub fn validate_registry_url(url: &str) -> Result<()> {
    // Check basic URL format
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(miette::miette!(
            "Registry URL must start with http:// or https://: {}",
            url
        ));
    }

    // Require HTTPS in production
    if !url.starts_with("https://") && !url.starts_with("http://127.0.0.1") && !url.starts_with("http://localhost") {
        return Err(miette::miette!(
            "Registry URLs must use HTTPS in production (got: {})",
            url
        ));
    }

    // Check URL length
    if url.len() > 2048 {
        return Err(miette::miette!("Registry URL is too long (max 2048 characters)"));
    }

    Ok(())
}

/// Package name validation
pub fn validate_package_name(name: &str) -> Result<()> {
    // Check if name matches pattern
    let regex = Regex::new(VALID_PACKAGE_NAME_PATTERN)
        .map_err(|e| miette::miette!("Regex error: {}", e))?;

    if !regex.is_match(name) {
        return Err(miette::miette!(
            "Invalid package name '{}'. Must contain only alphanumeric, dash, underscore, and dot characters (1-64 chars)",
            name
        ));
    }

    // Check reserved names
    if RESERVED_NAMES.contains(&name) {
        return Err(miette::miette!(
            "Package name '{}' is reserved and cannot be used",
            name
        ));
    }

    // Check for confusing patterns (prevent package squatting)
    if name.contains("__") || name.starts_with('_') || name.ends_with('_') {
        return Err(miette::miette!(
            "Package name '{}' contains confusing patterns (avoid leading/trailing underscores or double underscores)",
            name
        ));
    }

    Ok(())
}

/// Validate version strings
pub fn validate_version(version: &str) -> Result<()> {
    // Try to parse as semver
    semver::Version::parse(version)
        .map_err(|e| miette::miette!("Invalid version '{}': {}", version, e))?;

    // Reject prerelease versions from being published as stable
    if version.contains('-') {
        // This is OK - just a prerelease
    }

    // Check length
    if version.len() > 20 {
        return Err(miette::miette!("Version string is too long (max 20 characters)"));
    }

    Ok(())
}

/// Validate dependency specification
pub fn validate_dependency_spec(spec: &str) -> Result<()> {
    // Spec format: name[@version]
    let parts: Vec<&str> = spec.splitn(2, '@').collect();

    let name = parts[0];
    validate_package_name(name)?;

    if parts.len() == 2 {
        validate_version(parts[1])?;
    }

    Ok(())
}

/// Validate author email
pub fn validate_author_email(email: &str) -> Result<()> {
    // Basic email validation
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
        .map_err(|e| miette::miette!("Regex error: {}", e))?;

    if !email_regex.is_match(email) {
        return Err(miette::miette!("Invalid email address: {}", email));
    }

    if email.len() > 254 {
        return Err(miette::miette!("Email address is too long (max 254 characters)"));
    }

    Ok(())
}

/// Validate description text
pub fn validate_description(desc: &str) -> Result<()> {
    if desc.is_empty() {
        return Err(miette::miette!("Description cannot be empty"));
    }

    if desc.len() > 500 {
        return Err(miette::miette!(
            "Description is too long (max 500 characters, got {})",
            desc.len()
        ));
    }

    // Check for suspicious characters that could indicate injection attempts
    if desc.contains('\0') {
        return Err(miette::miette!("Description contains null bytes"));
    }

    Ok(())
}

/// Validate file path for security (prevent path traversal)
pub fn validate_file_path(path: &str) -> Result<()> {
    // Reject absolute paths (both Unix and Windows)
    if path.starts_with('/') || std::path::Path::new(path).is_absolute() {
        return Err(miette::miette!("File path must be relative, not absolute: {}", path));
    }

    // Reject path traversal attempts
    if path.contains("..") || path.contains("./..") {
        return Err(miette::miette!(
            "File path contains path traversal attempt (..): {}",
            path
        ));
    }

    // Reject null bytes
    if path.contains('\0') {
        return Err(miette::miette!("File path contains null bytes"));
    }

    Ok(())
}

/// Validate license identifier
pub fn validate_license(license: &str) -> Result<()> {
    // Simple validation - could be SPDX identifier
    let valid_licenses = HashSet::from([
        "MIT", "Apache-2.0", "GPL-3.0", "BSD-3-Clause", "BSD-2-Clause", 
        "ISC", "LGPL-3.0", "MPL-2.0", "UNLICENSE",
    ]);

    if !valid_licenses.contains(license) && !license.contains(" OR ") && !license.contains(" AND ") {
        // Allow SPDX expressions, but warn about unknown licenses
    }

    if license.len() > 50 {
        return Err(miette::miette!("License identifier is too long (max 50 characters)"));
    }

    Ok(())
}

/// Security validation context for package operations
#[derive(Debug)]
pub struct SecurityValidator {
    max_package_size: u64,
    allowed_file_extensions: HashSet<String>,
    reject_executables: bool,
}

impl SecurityValidator {
    /// Create a new security validator with default settings
    pub fn new() -> Self {
        let mut extensions = HashSet::new();
        extensions.insert("rs".to_string());
        extensions.insert("toml".to_string());
        extensions.insert("txt".to_string());
        extensions.insert("md".to_string());
        extensions.insert("yaml".to_string());
        extensions.insert("yml".to_string());
        extensions.insert("json".to_string());

        SecurityValidator {
            max_package_size: 100 * 1024 * 1024, // 100MB default
            allowed_file_extensions: extensions,
            reject_executables: true,
        }
    }

    /// Set maximum package size (in bytes)
    pub fn set_max_size(&mut self, bytes: u64) {
        self.max_package_size = bytes;
    }

    /// Validate package size
    pub fn validate_package_size(&self, size: u64) -> Result<()> {
        if size > self.max_package_size {
            return Err(miette::miette!(
                "Package is too large ({} bytes, max {} bytes)",
                size,
                self.max_package_size
            ));
        }
        Ok(())
    }

    /// Validate file within package
    pub fn validate_package_file(&self, filename: &str, size: u64) -> Result<()> {
        // Validate path
        validate_file_path(filename)?;

        // Check file size (individual file limit)
        const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50MB per file
        if size > MAX_FILE_SIZE {
            return Err(miette::miette!(
                "File '{}' is too large ({} bytes, max {} bytes)",
                filename,
                size,
                MAX_FILE_SIZE
            ));
        }

        // Check file extension
        let path = std::path::Path::new(filename);
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if self.reject_executables && is_executable_extension(&ext_str) {
                return Err(miette::miette!(
                    "Cannot include executable files in package: {}",
                    filename
                ));
            }
        }

        Ok(())
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        SecurityValidator::new()
    }
}

/// Check if file extension indicates executable
fn is_executable_extension(ext: &str) -> bool {
    matches!(
        ext,
        "exe" | "dll" | "so" | "dylib" | "bin" | "bat" | "cmd" | "sh" | "com" | "msi"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_registry_urls() {
        assert!(validate_registry_url("https://registry.aura.dev").is_ok());
        assert!(validate_registry_url("https://crates.io").is_ok());
        assert!(validate_registry_url("http://localhost:8080").is_ok());
        assert!(validate_registry_url("http://127.0.0.1:3000").is_ok());
    }

    #[test]
    fn test_invalid_registry_urls() {
        assert!(validate_registry_url("registry.aura.dev").is_err()); // No protocol
        assert!(validate_registry_url("ftp://registry.aura.dev").is_err()); // Wrong protocol
        assert!(validate_registry_url("http://example.com").is_err()); // HTTP not localhost
    }

    #[test]
    fn test_valid_package_names() {
        assert!(validate_package_name("my-app").is_ok());
        assert!(validate_package_name("my_app").is_ok());
        assert!(validate_package_name("MyApp").is_ok());
        assert!(validate_package_name("app123").is_ok());
        assert!(validate_package_name("a-b.c_d").is_ok());
    }

    #[test]
    fn test_invalid_package_names() {
        assert!(validate_package_name("my app").is_err()); // Space
        assert!(validate_package_name("my/app").is_err()); // Slash
        assert!(validate_package_name("").is_err()); // Empty
        assert!(validate_package_name("_app").is_err()); // Leading underscore
        assert!(validate_package_name("app_").is_err()); // Trailing underscore
        assert!(validate_package_name("app__name").is_err()); // Double underscore
    }

    #[test]
    fn test_reserved_names() {
        assert!(validate_package_name("aura").is_err());
        assert!(validate_package_name("std").is_err());
        assert!(validate_package_name("core").is_err());
    }

    #[test]
    fn test_valid_versions() {
        assert!(validate_version("1.0.0").is_ok());
        assert!(validate_version("0.1.0").is_ok());
        assert!(validate_version("1.0.0-alpha.1").is_ok());
        assert!(validate_version("1.0.0-beta+build.1").is_ok());
    }

    #[test]
    fn test_invalid_versions() {
        assert!(validate_version("1.0").is_err()); // Not semver
        assert!(validate_version("v1.0.0").is_err()); // v prefix
        assert!(validate_version("1.0.0.0").is_err()); // Too many parts
    }

    #[test]
    fn test_valid_dependency_specs() {
        assert!(validate_dependency_spec("serde").is_ok());
        assert!(validate_dependency_spec("serde@1.0.0").is_ok());
        assert!(validate_dependency_spec("my-lib@0.5.0").is_ok());
    }

    #[test]
    fn test_invalid_dependency_specs() {
        assert!(validate_dependency_spec("my app").is_err());
        assert!(validate_dependency_spec("@1.0.0").is_err()); // No name
    }

    #[test]
    fn test_valid_emails() {
        assert!(validate_author_email("user@example.com").is_ok());
        assert!(validate_author_email("john.doe@company.co.uk").is_ok());
        assert!(validate_author_email("a+b@test.org").is_ok());
    }

    #[test]
    fn test_invalid_emails() {
        assert!(validate_author_email("not-an-email").is_err());
        assert!(validate_author_email("@example.com").is_err());
        assert!(validate_author_email("user@").is_err());
        assert!(validate_author_email("user @example.com").is_err());
    }

    #[test]
    fn test_valid_descriptions() {
        assert!(validate_description("A great package").is_ok());
        assert!(validate_description("Multi-purpose library for Rust applications").is_ok());
    }

    #[test]
    fn test_invalid_descriptions() {
        assert!(validate_description("").is_err()); // Empty
        assert!(validate_description(&"x".repeat(501)).is_err()); // Too long
        assert!(validate_description("Test\0Injection").is_err()); // Null byte
    }

    #[test]
    fn test_valid_file_paths() {
        assert!(validate_file_path("src/main.rs").is_ok());
        assert!(validate_file_path("Cargo.toml").is_ok());
        assert!(validate_file_path("lib/module.rs").is_ok());
    }

    #[test]
    fn test_invalid_file_paths() {
        assert!(validate_file_path("/etc/passwd").is_err()); // Absolute
        assert!(validate_file_path("../../../etc/passwd").is_err()); // Path traversal
        assert!(validate_file_path("src/../../../etc/passwd").is_err()); // Path traversal
        assert!(validate_file_path("file\0null").is_err()); // Null byte
    }

    #[test]
    fn test_valid_licenses() {
        assert!(validate_license("MIT").is_ok());
        assert!(validate_license("Apache-2.0").is_ok());
        assert!(validate_license("MIT OR Apache-2.0").is_ok());
    }

    #[test]
    fn test_security_validator_default() {
        let validator = SecurityValidator::new();
        assert_eq!(validator.max_package_size, 100 * 1024 * 1024);
        assert!(validator.reject_executables);
    }

    #[test]
    fn test_security_validator_package_size() {
        let validator = SecurityValidator::new();
        assert!(validator.validate_package_size(50 * 1024 * 1024).is_ok());
        assert!(validator.validate_package_size(101 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_security_validator_file_validation() {
        let validator = SecurityValidator::new();
        assert!(validator.validate_package_file("src/main.rs", 1000).is_ok());
        assert!(validator.validate_package_file("../../../etc/passwd", 1000).is_err());
    }

    #[test]
    fn test_reject_executable_files() {
        let validator = SecurityValidator::new();
        assert!(validator.validate_package_file("script.sh", 1000).is_err());
        assert!(validator.validate_package_file("program.exe", 1000).is_err());
        assert!(validator.validate_package_file("lib.so", 1000).is_err());
    }

    #[test]
    fn test_allow_source_files() {
        let validator = SecurityValidator::new();
        assert!(validator.validate_package_file("main.rs", 1000).is_ok());
        assert!(validator.validate_package_file("config.toml", 1000).is_ok());
        assert!(validator.validate_package_file("README.md", 1000).is_ok());
    }
}
