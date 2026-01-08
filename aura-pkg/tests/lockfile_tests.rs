/// Integration tests for lockfile functionality
///
/// Tests lockfile creation, updates, and verification

use aura_pkg::{Lockfile, ResolvedDependency};
use tempfile::TempDir;

#[test]
fn test_lockfile_create_new() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let lockfile = Lockfile::new();
    lockfile.to_file(&lockfile_path).expect("write failed");

    assert!(lockfile_path.exists());
    
    // Verify we can read it back
    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    assert_eq!(loaded.version, "1.0");
}

#[test]
fn test_lockfile_add_and_verify() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let mut lockfile = Lockfile::new();
    lockfile.add_dependency(ResolvedDependency {
        name: "serde".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile.to_file(&lockfile_path).expect("write failed");

    // Verify it can be read and has the dependency
    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    assert!(loaded.get_dependency("serde").is_some());
}

#[test]
fn test_lockfile_multiple_dependencies() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let mut lockfile = Lockfile::new();
    lockfile.add_dependency(ResolvedDependency {
        name: "serde".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile.add_dependency(ResolvedDependency {
        name: "tokio".to_string(),
        version: "1.35.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile.add_dependency(ResolvedDependency {
        name: "regex".to_string(),
        version: "1.10.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });

    lockfile.to_file(&lockfile_path).expect("write failed");

    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    assert_eq!(loaded.dependencies.len(), 3);
    assert!(loaded.get_dependency("serde").is_some());
    assert!(loaded.get_dependency("tokio").is_some());
    assert!(loaded.get_dependency("regex").is_some());
}

#[test]
fn test_lockfile_serialization_roundtrip() {
    let mut lockfile = Lockfile::new();
    lockfile.add_dependency(ResolvedDependency {
        name: "serde".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile.add_dependency(ResolvedDependency {
        name: "tokio".to_string(),
        version: "1.35.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });

    let serialized = lockfile.to_string().expect("serialize failed");
    let deserialized = Lockfile::from_str(&serialized).expect("deserialize failed");

    assert_eq!(deserialized.dependencies.len(), 2);
    assert!(deserialized.get_dependency("serde").is_some());
    assert!(deserialized.get_dependency("tokio").is_some());
}

#[test]
fn test_lockfile_dev_dependencies() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let mut lockfile = Lockfile::new();
    lockfile.add_dependency(ResolvedDependency {
        name: "serde".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile.add_dependency(ResolvedDependency {
        name: "tokio".to_string(),
        version: "1.35.0".to_string(),
        registry: None,
        hash: None,
        dev: true,  // dev dependency
        dependencies: vec![],
    });

    lockfile.to_file(&lockfile_path).expect("write failed");

    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    
    let serde = loaded.get_dependency("serde").unwrap();
    assert!(!serde.dev);

    let tokio = loaded.get_dependency("tokio").unwrap();
    assert!(tokio.dev);
}

#[test]
fn test_lockfile_manifest_hash() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let mut lockfile = Lockfile::new();
    lockfile.set_manifest_hash("abc123".to_string());

    lockfile.to_file(&lockfile_path).expect("write failed");

    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    assert_eq!(loaded.manifest_hash, Some("abc123".to_string()));
}

#[test]
fn test_lockfile_verify() {
    let lockfile = Lockfile::new();
    let result = lockfile.verify();
    assert!(result.is_ok());
}

#[test]
fn test_lockfile_invalid_version() {
    let lockfile_str = r#"
version = "2.0"
generated = "2024-01-01T00:00:00Z"
"#;

    let result = Lockfile::from_str(lockfile_str);
    assert!(result.is_err());
}

#[test]
fn test_lockfile_transitive_dependencies() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let mut lockfile = Lockfile::new();
    lockfile.add_dependency(ResolvedDependency {
        name: "serde".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec!["serde_derive".to_string()],
    });
    lockfile.add_dependency(ResolvedDependency {
        name: "serde_json".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec!["serde".to_string()],
    });

    lockfile.to_file(&lockfile_path).expect("write failed");

    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    assert!(loaded.get_dependency("serde").is_some());
    assert!(loaded.get_dependency("serde_json").is_some());
    
    // Verify transitive dependency tracking
    let serde = loaded.get_dependency("serde").unwrap();
    assert!(serde.dependencies.contains(&"serde_derive".to_string()));
}

#[test]
fn test_lockfile_deterministic_output() {
    let mut lockfile1 = Lockfile::new();
    lockfile1.add_dependency(ResolvedDependency {
        name: "z-package".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile1.add_dependency(ResolvedDependency {
        name: "a-package".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile1.add_dependency(ResolvedDependency {
        name: "m-package".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });

    let mut lockfile2 = Lockfile::new();
    lockfile2.add_dependency(ResolvedDependency {
        name: "a-package".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile2.add_dependency(ResolvedDependency {
        name: "z-package".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });
    lockfile2.add_dependency(ResolvedDependency {
        name: "m-package".to_string(),
        version: "1.0.0".to_string(),
        registry: None,
        hash: None,
        dev: false,
        dependencies: vec![],
    });

    // Verify both have the same dependencies in the same order (BTreeMap ensures consistent ordering)
    // Note: timestamps will differ slightly, so we just verify the dependencies are ordered deterministically
    assert_eq!(lockfile1.dependencies.keys().collect::<Vec<_>>(),
               lockfile2.dependencies.keys().collect::<Vec<_>>());
}

#[test]
fn test_lockfile_empty() {
    let temp = TempDir::new().expect("create temp dir");
    let lockfile_path = temp.path().join("Aura.lock");

    let lockfile = Lockfile::new();
    lockfile.to_file(&lockfile_path).expect("write failed");

    let loaded = Lockfile::from_file(&lockfile_path).expect("read failed");
    assert_eq!(loaded.dependencies.len(), 0);
}
