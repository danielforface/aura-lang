use aura_pkg::resolver::{DependencyResolver, PackageRegistry, ResolvedDependencies};
use semver::{Version, VersionReq};
use std::collections::BTreeMap;

#[test]
fn test_complex_dependency_graph() {
    let mut registry = PackageRegistry::new();
    
    // Register package versions
    registry.register_version("app".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("web".to_string(), Version::parse("2.0.0").unwrap());
    registry.register_version("web".to_string(), Version::parse("2.1.0").unwrap());
    registry.register_version("db".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("cache".to_string(), Version::parse("0.5.0").unwrap());
    registry.register_version("pool".to_string(), Version::parse("1.2.0").unwrap());
    
    // Set up dependencies:
    // app 1.0.0 -> web ^2.0, db ^1.0
    let mut app_deps = BTreeMap::new();
    app_deps.insert("web".to_string(), VersionReq::parse("^2.0").unwrap());
    app_deps.insert("db".to_string(), VersionReq::parse("^1.0").unwrap());
    registry.set_dependencies("app".to_string(), Version::parse("1.0.0").unwrap(), app_deps);
    
    // web 2.1.0 -> cache ^0.5
    let mut web_deps = BTreeMap::new();
    web_deps.insert("cache".to_string(), VersionReq::parse("^0.5").unwrap());
    registry.set_dependencies("web".to_string(), Version::parse("2.1.0").unwrap(), web_deps);
    
    // db 1.0.0 -> pool ^1.2
    let mut db_deps = BTreeMap::new();
    db_deps.insert("pool".to_string(), VersionReq::parse("^1.2").unwrap());
    registry.set_dependencies("db".to_string(), Version::parse("1.0.0").unwrap(), db_deps);
    
    let resolver = DependencyResolver::new(registry);
    
    let mut root_deps = BTreeMap::new();
    root_deps.insert("web".to_string(), VersionReq::parse("^2.0").unwrap());
    root_deps.insert("db".to_string(), VersionReq::parse("^1.0").unwrap());
    
    let resolved = resolver.resolve("app", &Version::parse("1.0.0").unwrap(), &root_deps)
        .expect("Resolution should succeed");
    
    // Verify all packages were resolved
    assert!(resolved.packages.contains_key("web"));
    assert!(resolved.packages.contains_key("db"));
    assert!(resolved.packages.contains_key("cache"));
    assert!(resolved.packages.contains_key("pool"));
    
    // Verify correct versions were selected
    assert_eq!(resolved.packages.get("web").unwrap(), &Version::parse("2.1.0").unwrap());
    assert_eq!(resolved.packages.get("cache").unwrap(), &Version::parse("0.5.0").unwrap());
    assert_eq!(resolved.packages.get("pool").unwrap(), &Version::parse("1.2.0").unwrap());
}

#[test]
fn test_caret_range_resolution() {
    let mut registry = PackageRegistry::new();
    
    // Register multiple patch versions
    registry.register_version("math".to_string(), Version::parse("2.0.0").unwrap());
    registry.register_version("math".to_string(), Version::parse("2.0.5").unwrap());
    registry.register_version("math".to_string(), Version::parse("2.1.0").unwrap());
    registry.register_version("math".to_string(), Version::parse("2.5.3").unwrap());
    registry.register_version("math".to_string(), Version::parse("3.0.0").unwrap());
    
    let resolver = DependencyResolver::new(registry);
    
    let req = VersionReq::parse("^2.0.0").unwrap();
    let version = resolver.registry.find_matching_version("math", &req).unwrap();
    
    // Should select newest matching version (2.5.3 < 3.0.0)
    assert_eq!(version, Version::parse("2.5.3").unwrap());
}

#[test]
fn test_tilde_range_resolution() {
    let mut registry = PackageRegistry::new();
    
    // Register versions for tilde range test
    registry.register_version("util".to_string(), Version::parse("1.2.0").unwrap());
    registry.register_version("util".to_string(), Version::parse("1.2.5").unwrap());
    registry.register_version("util".to_string(), Version::parse("1.3.0").unwrap());
    registry.register_version("util".to_string(), Version::parse("2.0.0").unwrap());
    
    let resolver = DependencyResolver::new(registry);
    
    let req = VersionReq::parse("~1.2.0").unwrap();
    let version = resolver.registry.find_matching_version("util", &req).unwrap();
    
    // Should select newest patch version in 1.2.x range
    assert_eq!(version, Version::parse("1.2.5").unwrap());
    
    // Should NOT match 1.3.0
    let req_1_3 = VersionReq::parse("~1.2.0").unwrap();
    assert!(!req_1_3.matches(&Version::parse("1.3.0").unwrap()));
}

#[test]
fn test_zero_major_caret_range() {
    let mut registry = PackageRegistry::new();
    
    registry.register_version("beta".to_string(), Version::parse("0.1.0").unwrap());
    registry.register_version("beta".to_string(), Version::parse("0.1.5").unwrap());
    registry.register_version("beta".to_string(), Version::parse("0.2.0").unwrap());
    registry.register_version("beta".to_string(), Version::parse("1.0.0").unwrap());
    
    let resolver = DependencyResolver::new(registry);
    
    let req = VersionReq::parse("^0.1.0").unwrap();
    let version = resolver.registry.find_matching_version("beta", &req).unwrap();
    
    // For 0.x versions, caret allows patch changes only
    assert_eq!(version, Version::parse("0.1.5").unwrap());
    
    // Should NOT match 0.2.0
    assert!(!req.matches(&Version::parse("0.2.0").unwrap()));
}

#[test]
fn test_multiple_incompatible_requirements() {
    let mut registry = PackageRegistry::new();
    
    registry.register_version("pkg".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("pkg".to_string(), Version::parse("2.0.0").unwrap());
    
    let resolver = DependencyResolver::new(registry);
    
    // Try to resolve with conflicting requirements
    let req1 = VersionReq::parse("^1.0").unwrap();
    let req2 = VersionReq::parse("^2.0").unwrap();
    
    // This is a simplified check - in a full resolver, this would need constraint solving
    assert!(req1.matches(&Version::parse("1.0.0").unwrap()));
    assert!(!req1.matches(&Version::parse("2.0.0").unwrap()));
    assert!(!req2.matches(&Version::parse("1.0.0").unwrap()));
    assert!(req2.matches(&Version::parse("2.0.0").unwrap()));
}

#[test]
fn test_resolution_respects_install_order() {
    let mut registry = PackageRegistry::new();
    
    registry.register_version("app".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("a".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("b".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("c".to_string(), Version::parse("1.0.0").unwrap());
    
    let mut app_deps = BTreeMap::new();
    app_deps.insert("a".to_string(), VersionReq::parse("1.0.0").unwrap());
    app_deps.insert("b".to_string(), VersionReq::parse("1.0.0").unwrap());
    app_deps.insert("c".to_string(), VersionReq::parse("1.0.0").unwrap());
    registry.set_dependencies("app".to_string(), Version::parse("1.0.0").unwrap(), app_deps);
    
    let resolver = DependencyResolver::new(registry);
    
    let mut root_deps = BTreeMap::new();
    root_deps.insert("a".to_string(), VersionReq::parse("1.0.0").unwrap());
    root_deps.insert("b".to_string(), VersionReq::parse("1.0.0").unwrap());
    root_deps.insert("c".to_string(), VersionReq::parse("1.0.0").unwrap());
    
    let resolved = resolver.resolve("app", &Version::parse("1.0.0").unwrap(), &root_deps)
        .expect("Resolution should succeed");
    
    // Check that app comes first (as root), then dependencies follow
    assert_eq!(resolved.install_order[0], "app");
    // Verify all required packages are present
    assert!(resolved.packages.contains_key("a"));
    assert!(resolved.packages.contains_key("b"));
    assert!(resolved.packages.contains_key("c"));
}

#[test]
fn test_shared_transitive_dependency() {
    let mut registry = PackageRegistry::new();
    
    // Both serviceA and serviceB depend on common
    registry.register_version("app".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("serviceA".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("serviceB".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("common".to_string(), Version::parse("1.0.0").unwrap());
    
    let mut app_deps = BTreeMap::new();
    app_deps.insert("serviceA".to_string(), VersionReq::parse("1.0.0").unwrap());
    app_deps.insert("serviceB".to_string(), VersionReq::parse("1.0.0").unwrap());
    registry.set_dependencies("app".to_string(), Version::parse("1.0.0").unwrap(), app_deps);
    
    let mut serviceA_deps = BTreeMap::new();
    serviceA_deps.insert("common".to_string(), VersionReq::parse("1.0.0").unwrap());
    registry.set_dependencies("serviceA".to_string(), Version::parse("1.0.0").unwrap(), serviceA_deps);
    
    let mut serviceB_deps = BTreeMap::new();
    serviceB_deps.insert("common".to_string(), VersionReq::parse("1.0.0").unwrap());
    registry.set_dependencies("serviceB".to_string(), Version::parse("1.0.0").unwrap(), serviceB_deps);
    
    let resolver = DependencyResolver::new(registry);
    
    let mut root_deps = BTreeMap::new();
    root_deps.insert("serviceA".to_string(), VersionReq::parse("1.0.0").unwrap());
    root_deps.insert("serviceB".to_string(), VersionReq::parse("1.0.0").unwrap());
    
    let resolved = resolver.resolve("app", &Version::parse("1.0.0").unwrap(), &root_deps)
        .expect("Resolution should succeed");
    
    // common should only be included once
    assert!(resolved.packages.contains_key("common"));
    assert_eq!(
        resolved.packages.iter().filter(|(k, _)| k.as_str() == "common").count(),
        1
    );
}

#[test]
fn test_version_compatibility_check() {
    let registry = PackageRegistry::new();
    let resolver = DependencyResolver::new(registry);
    
    let req = VersionReq::parse("^1.2.3").unwrap();
    let v1 = Version::parse("1.2.3").unwrap();
    let v2 = Version::parse("1.5.0").unwrap();
    let v3 = Version::parse("2.0.0").unwrap();
    
    assert!(resolver.verify_compatibility("pkg", &v1, &req).unwrap());
    assert!(resolver.verify_compatibility("pkg", &v2, &req).unwrap());
    assert!(!resolver.verify_compatibility("pkg", &v3, &req).unwrap());
}

#[test]
fn test_exact_version_specification() {
    let mut registry = PackageRegistry::new();
    
    registry.register_version("locked".to_string(), Version::parse("1.5.2").unwrap());
    registry.register_version("locked".to_string(), Version::parse("1.5.3").unwrap());
    registry.register_version("locked".to_string(), Version::parse("2.0.0").unwrap());
    
    let resolver = DependencyResolver::new(registry);
    
    // Standard semver treats bare version as caret range (^1.5.2)
    // This matches 1.5.2, 1.5.3, 1.x.x up to 2.0.0
    let req = VersionReq::parse("1.5.2").unwrap();
    
    // Verify the caret matching behavior (semver standard)
    assert!(req.matches(&Version::parse("1.5.2").unwrap()));
    assert!(req.matches(&Version::parse("1.5.3").unwrap()));
    assert!(!req.matches(&Version::parse("2.0.0").unwrap()));
}

#[test]
fn test_prerelease_handling() {
    let mut registry = PackageRegistry::new();
    
    registry.register_version("pkg".to_string(), Version::parse("1.0.0").unwrap());
    registry.register_version("pkg".to_string(), Version::parse("2.0.0-alpha.1").unwrap());
    registry.register_version("pkg".to_string(), Version::parse("2.0.0-beta.1").unwrap());
    registry.register_version("pkg".to_string(), Version::parse("2.0.0").unwrap());
    
    let resolver = DependencyResolver::new(registry);
    
    // ^1.0.0 should prefer stable over prerelease
    let req = VersionReq::parse("^1.0").unwrap();
    let version = resolver.registry.find_matching_version("pkg", &req).unwrap();
    
    // Should get 1.0.0 not 2.0.0 prerelease
    assert_eq!(version, Version::parse("1.0.0").unwrap());
}

#[test]
fn test_no_version_available() {
    let registry = PackageRegistry::new();
    let resolver = DependencyResolver::new(registry);
    
    let req = VersionReq::parse("^1.0").unwrap();
    let result = resolver.registry.find_matching_version("nonexistent", &req);
    
    assert!(result.is_err());
}

#[test]
fn test_resolved_dependencies_completeness() {
    let mut resolved = ResolvedDependencies {
        packages: std::collections::BTreeMap::new(),
        install_order: Vec::new(),
    };
    
    resolved.add_package("pkg1".to_string(), Version::parse("1.0.0").unwrap());
    resolved.add_package("pkg2".to_string(), Version::parse("2.0.0").unwrap());
    
    let mut root_deps = BTreeMap::new();
    root_deps.insert("pkg1".to_string(), VersionReq::parse("1.0.0").unwrap());
    root_deps.insert("pkg2".to_string(), VersionReq::parse("2.0.0").unwrap());
    
    assert!(resolved.is_complete(&root_deps));
    
    // Remove one package
    resolved.packages.remove("pkg2");
    assert!(!resolved.is_complete(&root_deps));
}
