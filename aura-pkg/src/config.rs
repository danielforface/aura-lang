/// Configuration management for Aura package manager
///
/// Handles user configuration, registry settings, and authentication tokens

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use miette::Report;

pub type ConfigError = Report;

fn config_msg(message: impl Into<String>) -> ConfigError {
    Report::msg(message.into())
}

/// Registry configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryEntry {
    pub url: String,
    pub token: Option<String>,
    pub index: Option<String>,
}

/// User configuration for Aura package manager
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuraConfig {
    /// Default registry to use
    pub default_registry: String,

    /// Configured registries
    pub registries: BTreeMap<String, RegistryEntry>,

    /// Global settings
    pub settings: ConfigSettings,
}

/// Global configuration settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigSettings {
    /// Check package signatures
    #[serde(default = "default_check_signatures")]
    pub check_signatures: bool,

    /// Require license file
    #[serde(default = "default_require_license")]
    pub require_license: bool,

    /// Offline mode
    #[serde(default)]
    pub offline: bool,

    /// Verbose output
    #[serde(default)]
    pub verbose: bool,

    /// HTTP timeout in seconds
    #[serde(default = "default_timeout")]
    pub http_timeout: u64,

    /// Custom options
    #[serde(default)]
    pub extras: BTreeMap<String, String>,
}

fn default_check_signatures() -> bool {
    true
}

fn default_require_license() -> bool {
    false
}

fn default_timeout() -> u64 {
    30
}

impl Default for ConfigSettings {
    fn default() -> Self {
        ConfigSettings {
            check_signatures: true,
            require_license: false,
            offline: false,
            verbose: false,
            http_timeout: 30,
            extras: BTreeMap::new(),
        }
    }
}

impl Default for AuraConfig {
    fn default() -> Self {
        let mut registries = BTreeMap::new();
        registries.insert(
            "crates".to_string(),
            RegistryEntry {
                url: "https://registry.aura-lang.org".to_string(),
                token: None,
                index: None,
            },
        );

        AuraConfig {
            default_registry: "crates".to_string(),
            registries,
            settings: ConfigSettings::default(),
        }
    }
}

impl AuraConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from file
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| config_msg(format!("Failed to read config: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| config_msg(format!("Invalid config format: {}", e)))
    }

    /// Save configuration to file
    pub fn to_file(&self, path: &Path) -> Result<(), ConfigError> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| config_msg(format!("Failed to create config directory: {}", e)))?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| config_msg(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| config_msg(format!("Failed to write config: {}", e)))
    }

    /// Get registry configuration
    pub fn get_registry(&self, name: &str) -> Option<&RegistryEntry> {
        self.registries.get(name)
    }

    /// Get default registry
    pub fn default_registry(&self) -> Option<&RegistryEntry> {
        self.get_registry(&self.default_registry)
    }

    /// Add or update a registry
    pub fn set_registry(&mut self, name: String, entry: RegistryEntry) {
        self.registries.insert(name, entry);
    }

    /// Remove a registry
    pub fn remove_registry(&mut self, name: &str) -> Option<RegistryEntry> {
        self.registries.remove(name)
    }

    /// Get a setting value
    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.settings.extras.get(key).cloned()
    }

    /// Set a setting value
    pub fn set_setting(&mut self, key: String, value: String) {
        self.settings.extras.insert(key, value);
    }

    /// Parse TOML from string
    pub fn from_str(content: &str) -> Result<Self, ConfigError> {
        toml::from_str(content)
            .map_err(|e| config_msg(format!("Invalid TOML: {}", e)))
    }

    /// Serialize to TOML string
    pub fn to_string(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|e| config_msg(format!("Failed to serialize: {}", e)))
    }
}

/// Configuration file manager
pub struct ConfigManager {
    config_path: PathBuf,
    config: AuraConfig,
}

impl ConfigManager {
    /// Create a new config manager
    pub fn new(config_path: PathBuf) -> Self {
        let config = if config_path.exists() {
            AuraConfig::from_file(&config_path).unwrap_or_default()
        } else {
            AuraConfig::default()
        };

        ConfigManager {
            config_path,
            config,
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &AuraConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut AuraConfig {
        &mut self.config
    }

    /// Save changes to file
    pub fn save(&self) -> Result<(), ConfigError> {
        self.config.to_file(&self.config_path)
    }

    /// Reload configuration from file
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        self.config = AuraConfig::from_file(&self.config_path)?;
        Ok(())
    }

    /// Get config file path
    pub fn path(&self) -> &Path {
        &self.config_path
    }
}

/// Get standard Aura config directory
pub fn get_config_dir() -> Result<PathBuf, ConfigError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| config_msg("Cannot determine home directory"))?;

    let config_dir = PathBuf::from(home).join(".aura");
    Ok(config_dir)
}

/// Get standard Aura config file path
pub fn get_config_path() -> Result<PathBuf, ConfigError> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = AuraConfig::default();
        assert_eq!(config.default_registry, "crates");
        assert!(config.registries.contains_key("crates"));
    }

    #[test]
    fn test_config_settings_default() {
        let settings = ConfigSettings::default();
        assert!(settings.check_signatures);
        assert!(!settings.require_license);
        assert_eq!(settings.http_timeout, 30);
    }

    #[test]
    fn test_registry_entry() {
        let entry = RegistryEntry {
            url: "https://example.com".to_string(),
            token: Some("secret".to_string()),
            index: None,
        };
        assert_eq!(entry.url, "https://example.com");
        assert_eq!(entry.token, Some("secret".to_string()));
    }

    #[test]
    fn test_config_get_registry() {
        let config = AuraConfig::default();
        let registry = config.get_registry("crates");
        assert!(registry.is_some());
        assert_eq!(registry.unwrap().url, "https://registry.aura-lang.org");
    }

    #[test]
    fn test_config_set_registry() {
        let mut config = AuraConfig::default();
        config.set_registry(
            "custom".to_string(),
            RegistryEntry {
                url: "https://custom.example.com".to_string(),
                token: None,
                index: None,
            },
        );
        assert!(config.registries.contains_key("custom"));
    }

    #[test]
    fn test_config_remove_registry() {
        let mut config = AuraConfig::default();
        config.set_registry(
            "temp".to_string(),
            RegistryEntry {
                url: "https://temp.example.com".to_string(),
                token: None,
                index: None,
            },
        );
        let removed = config.remove_registry("temp");
        assert!(removed.is_some());
        assert!(!config.registries.contains_key("temp"));
    }

    #[test]
    fn test_config_serialization() {
        let config = AuraConfig::default();
        let toml = config.to_string().expect("serialize failed");
        assert!(toml.contains("default_registry"));
        assert!(toml.contains("crates"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml = r#"
default_registry = "crates"

[registries.crates]
url = "https://registry.aura-lang.org"

[settings]
check_signatures = true
http_timeout = 30
"#;
        let config = AuraConfig::from_str(toml).expect("deserialize failed");
        assert_eq!(config.default_registry, "crates");
        assert!(config.settings.check_signatures);
    }

    #[test]
    fn test_config_get_set_setting() {
        let mut config = AuraConfig::default();
        config.set_setting("custom_key".to_string(), "value".to_string());
        assert_eq!(config.get_setting("custom_key"), Some("value".to_string()));
    }

    #[test]
    fn test_config_file_operations() {
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let config_path = temp.path().join("config.toml");

        let config = AuraConfig::default();
        config.to_file(&config_path).expect("write failed");
        assert!(config_path.exists());

        let loaded = AuraConfig::from_file(&config_path).expect("read failed");
        assert_eq!(loaded.default_registry, config.default_registry);
    }

    #[test]
    fn test_config_manager_new() {
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let config_path = temp.path().join("config.toml");
        let manager = ConfigManager::new(config_path);
        assert_eq!(manager.config().default_registry, "crates");
    }

    #[test]
    fn test_config_manager_save_and_reload() {
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let config_path = temp.path().join("config.toml");

        let mut manager = ConfigManager::new(config_path.clone());
        manager.config_mut().default_registry = "custom".to_string();
        manager.save().expect("save failed");

        let mut manager2 = ConfigManager::new(config_path);
        manager2.reload().expect("reload failed");
        assert_eq!(manager2.config().default_registry, "custom");
    }

    #[test]
    fn test_config_manager_path() {
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let config_path = temp.path().join("config.toml");
        let manager = ConfigManager::new(config_path.clone());
        assert_eq!(manager.path(), config_path);
    }
}
