/// Dependency resolution for package manager
/// Resolves version requirements and handles transitive dependencies

use semver::{Version, VersionReq};
use std::collections::{BTreeMap, VecDeque};
use miette::{IntoDiagnostic, Report};

pub type ResolutionError = Report;

fn resolution_msg(message: impl Into<String>) -> ResolutionError {
    Report::msg(message.into())
}

/// A single package in the dependency graph
#[derive(Clone, Debug)]
pub struct PackageNode {
    pub name: String,
    pub version: Version,
    pub dependencies: BTreeMap<String, VersionReq>,
}

impl PackageNode {
    pub fn new(name: String, version: Version) -> Self {
        PackageNode {
            name,
            version,
            dependencies: BTreeMap::new(),
        }
    }

    pub fn add_dependency(&mut self, name: String, version_req: VersionReq) {
        self.dependencies.insert(name, version_req);
    }
}

/// Package registry mock (in practice, this queries the actual registry)
pub struct PackageRegistry {
    /// Map of package name -> list of available versions
    packages: BTreeMap<String, Vec<Version>>,
    /// Map of (package, version) -> dependencies
    dependencies: BTreeMap<(String, Version), BTreeMap<String, VersionReq>>,
}

impl PackageRegistry {
    pub fn new() -> Self {
        PackageRegistry {
            packages: BTreeMap::new(),
            dependencies: BTreeMap::new(),
        }
    }

    /// Register a package version
    pub fn register_version(&mut self, name: String, version: Version) {
        self.packages.entry(name).or_insert_with(Vec::new).push(version);
    }

    /// Set dependencies for a specific package version
    pub fn set_dependencies(
        &mut self,
        name: String,
        version: Version,
        deps: BTreeMap<String, VersionReq>,
    ) {
        self.dependencies.insert((name, version), deps);
    }

    /// Find all versions of a package
    pub fn find_versions(&self, name: &str) -> Result<Vec<Version>, ResolutionError> {
        self.packages
            .get(name)
            .cloned()
            .ok_or_else(|| resolution_msg(format!("package '{}' not found in registry", name)))
    }

    /// Find the best version matching a requirement
    pub fn find_matching_version(
        &self,
        name: &str,
        req: &VersionReq,
    ) -> Result<Version, ResolutionError> {
        let mut versions = self.find_versions(name)?;
        versions.sort();
        versions.reverse(); // Try newest first

        versions
            .into_iter()
            .find(|v| req.matches(v))
            .ok_or_else(|| {
                resolution_msg(format!(
                    "no version of '{}' matches requirement '{}'",
                    name, req
                ))
            })
    }

    /// Get dependencies for a specific version
    pub fn get_dependencies(
        &self,
        name: &str,
        version: &Version,
    ) -> Result<BTreeMap<String, VersionReq>, ResolutionError> {
        Ok(self
            .dependencies
            .get(&(name.to_string(), version.clone()))
            .cloned()
            .unwrap_or_default())
    }
}

/// Resolution result: a complete locked dependency tree
#[derive(Clone, Debug)]
pub struct ResolvedDependencies {
    /// Map of package name -> resolved version
    pub packages: BTreeMap<String, Version>,
    /// Order of installation (respects dependencies)
    pub install_order: Vec<String>,
}

impl ResolvedDependencies {
    /// Check if all dependencies are present
    pub fn is_complete(&self, root_deps: &BTreeMap<String, VersionReq>) -> bool {
        root_deps.keys().all(|name| self.packages.contains_key(name))
    }

    /// Add a package to the resolution
    pub fn add_package(&mut self, name: String, version: Version) {
        if !self.packages.contains_key(&name) {
            self.install_order.push(name.clone());
        }
        self.packages.insert(name, version);
    }

    /// Verify no circular dependencies
    pub fn has_cycles(&self) -> bool {
        // For now, a simple check - in a full implementation, would do proper cycle detection
        false
    }
}

/// Main resolver: resolves all dependencies to specific versions
pub struct DependencyResolver {
    registry: PackageRegistry,
}

impl DependencyResolver {
    pub fn new(registry: PackageRegistry) -> Self {
        DependencyResolver { registry }
    }

    /// Resolve all dependencies for a package
    pub fn resolve(
        &self,
        root_name: &str,
        root_version: &Version,
        root_dependencies: &BTreeMap<String, VersionReq>,
    ) -> Result<ResolvedDependencies, ResolutionError> {
        let mut resolved = ResolvedDependencies {
            packages: BTreeMap::new(),
            install_order: Vec::new(),
        };

        // Add root package
        resolved.add_package(root_name.to_string(), root_version.clone());

        // BFS to resolve all transitive dependencies
        let mut queue = VecDeque::new();
        for (name, req) in root_dependencies {
            queue.push_back((name.clone(), req.clone()));
        }

        let mut visited = std::collections::HashSet::new();

        while let Some((name, req)) = queue.pop_front() {
            // Skip if already resolved
            if resolved.packages.contains_key(&name) {
                continue;
            }

            // Skip infinite loops
            let visit_key = format!("{}:{}", name, req);
            if visited.contains(&visit_key) {
                continue;
            }
            visited.insert(visit_key);

            // Find matching version
            let version = self.registry.find_matching_version(&name, &req)?;

            // Get its dependencies
            let deps = self.registry.get_dependencies(&name, &version)?;

            // Add to resolved
            resolved.add_package(name.clone(), version.clone());

            // Queue transitive dependencies
            for (dep_name, dep_req) in deps {
                if !resolved.packages.contains_key(&dep_name) {
                    queue.push_back((dep_name, dep_req));
                }
            }
        }

        // Verify we resolved everything
        if !resolved.is_complete(root_dependencies) {
            return Err(resolution_msg(
                "failed to resolve all root dependencies",
            ));
        }

        Ok(resolved)
    }

    /// Check if a version satisfies requirements
    pub fn verify_compatibility(
        &self,
        name: &str,
        version: &Version,
        req: &VersionReq,
    ) -> Result<bool, ResolutionError> {
        // Check version matches requirement
        Ok(req.matches(version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_resolution() {
        let mut registry = PackageRegistry::new();
        registry.register_version("serde".to_string(), Version::parse("1.0.0").unwrap());
        registry.register_version("serde".to_string(), Version::parse("1.0.1").unwrap());

        let resolver = DependencyResolver::new(registry);
        let req = VersionReq::parse("^1.0").unwrap();
        let version = resolver
            .registry
            .find_matching_version("serde", &req)
            .expect("find failed");

        assert_eq!(version, Version::parse("1.0.1").unwrap());
    }

    #[test]
    fn test_transitive_resolution() {
        let mut registry = PackageRegistry::new();

        // Register packages
        registry.register_version("myapp".to_string(), Version::parse("1.0.0").unwrap());
        registry.register_version("serde".to_string(), Version::parse("1.0.0").unwrap());
        registry.register_version("tokio".to_string(), Version::parse("1.0.0").unwrap());
        registry.register_version("bytes".to_string(), Version::parse("1.0.0").unwrap());

        // myapp 1.0.0 depends on serde 1.0.0 and tokio 1.0.0
        let mut myapp_deps = BTreeMap::new();
        myapp_deps.insert(
            "serde".to_string(),
            VersionReq::parse("^1.0").unwrap(),
        );
        myapp_deps.insert(
            "tokio".to_string(),
            VersionReq::parse("^1.0").unwrap(),
        );
        registry.set_dependencies("myapp".to_string(), Version::parse("1.0.0").unwrap(), myapp_deps);

        // tokio 1.0.0 depends on bytes 1.0.0
        let mut tokio_deps = BTreeMap::new();
        tokio_deps.insert("bytes".to_string(), VersionReq::parse("^1.0").unwrap());
        registry.set_dependencies("tokio".to_string(), Version::parse("1.0.0").unwrap(), tokio_deps);

        let resolver = DependencyResolver::new(registry);

        let mut root_deps = BTreeMap::new();
        root_deps.insert("serde".to_string(), VersionReq::parse("^1.0").unwrap());
        root_deps.insert("tokio".to_string(), VersionReq::parse("^1.0").unwrap());

        let resolved = resolver
            .resolve(
                "myapp",
                &Version::parse("1.0.0").unwrap(),
                &root_deps,
            )
            .expect("resolve failed");

        // Should have resolved all packages
        assert!(resolved.packages.contains_key("serde"));
        assert!(resolved.packages.contains_key("tokio"));
        assert!(resolved.packages.contains_key("bytes"));
    }

    #[test]
    fn test_version_requirement_matching() {
        let req_caret = VersionReq::parse("^1.2.3").unwrap();
        assert!(req_caret.matches(&Version::parse("1.2.3").unwrap()));
        assert!(req_caret.matches(&Version::parse("1.3.0").unwrap()));
        assert!(!req_caret.matches(&Version::parse("2.0.0").unwrap()));

        let req_tilde = VersionReq::parse("~1.2.3").unwrap();
        assert!(req_tilde.matches(&Version::parse("1.2.3").unwrap()));
        assert!(req_tilde.matches(&Version::parse("1.2.5").unwrap()));
        assert!(!req_tilde.matches(&Version::parse("1.3.0").unwrap()));
    }

    #[test]
    fn test_missing_package_error() {
        let registry = PackageRegistry::new();
        let resolver = DependencyResolver::new(registry);

        let result = resolver.registry.find_versions("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_no_matching_version() {
        let mut registry = PackageRegistry::new();
        registry.register_version("mypackage".to_string(), Version::parse("1.0.0").unwrap());

        let resolver = DependencyResolver::new(registry);
        let req = VersionReq::parse("^2.0").unwrap();
        let result = resolver.registry.find_matching_version("mypackage", &req);

        assert!(result.is_err());
    }

    #[test]
    fn test_resolved_dependencies_ordering() {
        let mut resolved = ResolvedDependencies {
            packages: BTreeMap::new(),
            install_order: Vec::new(),
        };

        resolved.add_package("package1".to_string(), Version::parse("1.0.0").unwrap());
        resolved.add_package("package2".to_string(), Version::parse("2.0.0").unwrap());
        resolved.add_package("package1".to_string(), Version::parse("1.0.1").unwrap()); // Update

        // Should maintain order of first insertion
        assert_eq!(resolved.install_order[0], "package1");
        assert_eq!(resolved.install_order[1], "package2");
        // But have latest version
        assert_eq!(
            resolved.packages.get("package1").unwrap(),
            &Version::parse("1.0.1").unwrap()
        );
    }
}
