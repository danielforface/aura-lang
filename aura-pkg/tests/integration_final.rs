use aura_pkg::{
    DependencyResolver, PackageRegistry,
    security::{validate_package_name, validate_version, SecurityValidator},
    cache::DependencyCache,
};
use semver::{Version, VersionReq};
use std::collections::BTreeMap;
use std::time::Duration;

#[test]
fn test_full_workflow_validation() {
    assert!(validate_package_name("my-awesome-lib").is_ok());
    assert!(validate_package_name("test_lib").is_ok());
    assert!(validate_package_name("lib123").is_ok());
    assert!(validate_package_name("_invalid").is_err());
    assert!(validate_package_name("my app").is_err());
    assert!(validate_package_name("std").is_err());
    
    assert!(validate_version("1.0.0").is_ok());
    assert!(validate_version("0.1.0").is_ok());
    assert!(validate_version("1.0").is_err());
}

#[test]
fn test_full_workflow_security() {
    let validator = SecurityValidator::new();
    assert!(validator.validate_package_file("src/main.rs", 1000).is_ok());
    assert!(validator.validate_package_file("script.sh", 1000).is_err());
    assert!(validator.validate_package_file("../../../etc/passwd", 100).is_err());
    assert!(validator.validate_package_size(50_000_000).is_ok());
    assert!(validator.validate_package_size(150_000_000).is_err());
}

#[test]
fn test_end_to_end_dependency_resolution() {
    let mut registry = PackageRegistry::new();
    registry.register_version("axum".to_string(), Version::parse("0.6.0").unwrap());
    registry.register_version("tokio".to_string(), Version::parse("1.27.0").unwrap());
    registry.register_version("serde".to_string(), Version::parse("1.0.188").unwrap());
    registry.register_version("serde_json".to_string(), Version::parse("1.0.105").unwrap());
    
    let mut axum_deps = BTreeMap::new();
    axum_deps.insert("tokio".to_string(), VersionReq::parse("^1.0").unwrap());
    axum_deps.insert("serde".to_string(), VersionReq::parse("^1.0").unwrap());
    registry.set_dependencies("axum".to_string(), Version::parse("0.6.0").unwrap(), axum_deps);
    
    let resolver = DependencyResolver::new(registry);
    let mut app_deps = BTreeMap::new();
    app_deps.insert("axum".to_string(), VersionReq::parse("0.6.0").unwrap());
    
    let resolved = resolver.resolve(
        "my-app",
        &Version::parse("1.0.0").unwrap(),
        &app_deps,
    ).expect("Should resolve");
    
    assert!(resolved.packages.contains_key("axum"));
}

#[test]
fn test_complex_version_constraints() {
    let mut registry = PackageRegistry::new();
    for minor in 0..5 {
        for patch in 0..5 {
            registry.register_version(
                "complex".to_string(),
                Version::parse(&format!("1.{}.{}", minor, patch)).unwrap(),
            );
        }
    }
    
    let resolver = DependencyResolver::new(registry);
    let constraints = vec![
        ("^1.0.0", Version::parse("1.4.4").unwrap()),
        ("~1.2.0", Version::parse("1.2.4").unwrap()),
    ];
    
    for (spec, expected) in constraints {
        let req = VersionReq::parse(spec).unwrap();
        let version = resolver.registry.find_matching_version("complex", &req).unwrap();
        assert_eq!(version, expected);
    }
}

#[test]
fn test_cache_performance() {
    let cache = DependencyCache::new(Duration::from_secs(300));
    let versions = vec![
        Version::parse("0.1.0").unwrap(),
        Version::parse("1.0.0").unwrap(),
    ];
    
    cache.set_versions("test-pkg".to_string(), versions.clone());
    let result = cache.get_versions("test-pkg");
    assert!(result.is_some());
    assert_eq!(result.unwrap().len(), 2);
}
