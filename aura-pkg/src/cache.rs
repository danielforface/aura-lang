use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use semver::Version;

/// Cache entry with expiration time
#[derive(Clone, Debug)]
pub struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

impl<T: Clone> CacheEntry<T> {
    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }

    fn get(&self) -> Option<T> {
        if self.is_valid() {
            Some(self.data.clone())
        } else {
            None
        }
    }
}

/// High-performance cache for dependency resolution results
pub struct DependencyCache {
    /// Cache for version lookups (package_name -> versions)
    version_cache: Arc<RwLock<HashMap<String, CacheEntry<Vec<Version>>>>>,
    /// Cache for resolved dependencies (package:version -> dependencies)
    resolution_cache: Arc<RwLock<HashMap<String, CacheEntry<HashMap<String, Version>>>>>,
    /// TTL for cache entries
    ttl: Duration,
}

impl DependencyCache {
    /// Create a new cache with the given TTL
    pub fn new(ttl: Duration) -> Self {
        DependencyCache {
            version_cache: Arc::new(RwLock::new(HashMap::new())),
            resolution_cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Create a cache with default 5 minute TTL
    pub fn default() -> Self {
        DependencyCache::new(Duration::from_secs(300))
    }

    /// Get cached versions for a package
    pub fn get_versions(&self, package: &str) -> Option<Vec<Version>> {
        let cache = self.version_cache.read().ok()?;
        cache.get(package).and_then(|entry| entry.get())
    }

    /// Cache versions for a package
    pub fn set_versions(&self, package: String, versions: Vec<Version>) {
        if let Ok(mut cache) = self.version_cache.write() {
            cache.insert(
                package,
                CacheEntry {
                    data: versions,
                    expires_at: Instant::now() + self.ttl,
                },
            );
        }
    }

    /// Get cached resolution result
    pub fn get_resolution(&self, key: &str) -> Option<HashMap<String, Version>> {
        let cache = self.resolution_cache.read().ok()?;
        cache.get(key).and_then(|entry| entry.get())
    }

    /// Cache a resolution result
    pub fn set_resolution(&self, key: String, result: HashMap<String, Version>) {
        if let Ok(mut cache) = self.resolution_cache.write() {
            cache.insert(
                key,
                CacheEntry {
                    data: result,
                    expires_at: Instant::now() + self.ttl,
                },
            );
        }
    }

    /// Clear expired entries from version cache
    pub fn cleanup_version_cache(&self) {
        if let Ok(mut cache) = self.version_cache.write() {
            cache.retain(|_, entry| entry.is_valid());
        }
    }

    /// Clear expired entries from resolution cache
    pub fn cleanup_resolution_cache(&self) {
        if let Ok(mut cache) = self.resolution_cache.write() {
            cache.retain(|_, entry| entry.is_valid());
        }
    }

    /// Clear all caches
    pub fn clear(&self) {
        if let Ok(mut cache) = self.version_cache.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.resolution_cache.write() {
            cache.clear();
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let version_count = self.version_cache.read().map(|c| c.len()).unwrap_or(0);
        let resolution_count = self.resolution_cache.read().map(|c| c.len()).unwrap_or(0);

        CacheStats {
            version_entries: version_count,
            resolution_entries: resolution_count,
            ttl_secs: self.ttl.as_secs(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub version_entries: usize,
    pub resolution_entries: usize,
    pub ttl_secs: u64,
}

/// Lazy-evaluated computation with caching
pub struct LazyCache<T: Clone> {
    value: Arc<RwLock<Option<T>>>,
    ttl: Duration,
    created_at: Instant,
}

impl<T: Clone> LazyCache<T> {
    /// Create a new lazy cache
    pub fn new() -> Self {
        LazyCache {
            value: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(300),
            created_at: Instant::now(),
        }
    }

    /// Create with custom TTL
    pub fn with_ttl(ttl: Duration) -> Self {
        LazyCache {
            value: Arc::new(RwLock::new(None)),
            ttl,
            created_at: Instant::now(),
        }
    }

    /// Check if cache is still valid
    fn is_valid(&self) -> bool {
        Instant::now() < self.created_at + self.ttl
    }

    /// Get or compute value
    pub fn get_or_compute<F>(&self, compute: F) -> Result<T, String>
    where
        F: FnOnce() -> Result<T, String>,
    {
        // Try to get cached value
        if let Ok(guard) = self.value.read() {
            if let Some(value) = guard.as_ref() {
                if self.is_valid() {
                    return Ok(value.clone());
                }
            }
        }

        // Compute and cache new value
        let result = compute()?;
        if let Ok(mut guard) = self.value.write() {
            *guard = Some(result.clone());
        }

        Ok(result)
    }

    /// Invalidate cache
    pub fn invalidate(&self) {
        if let Ok(mut guard) = self.value.write() {
            *guard = None;
        }
    }

    /// Get cached value without computing
    pub fn get(&self) -> Option<T> {
        let guard = self.value.read().ok()?;
        guard.as_ref().cloned()
    }
}

impl<T: Clone> Default for LazyCache<T> {
    fn default() -> Self {
        LazyCache::new()
    }
}

/// Parallel dependency resolution cache using thread-safe primitives
pub struct ParallelResolutionCache {
    /// Per-package version caches
    caches: HashMap<String, DependencyCache>,
}

impl ParallelResolutionCache {
    /// Create a new parallel cache
    pub fn new() -> Self {
        ParallelResolutionCache {
            caches: HashMap::new(),
        }
    }

    /// Get or create cache for a package
    pub fn get_package_cache(&mut self, package: &str) -> &DependencyCache {
        self.caches
            .entry(package.to_string())
            .or_insert_with(DependencyCache::default)
    }

    /// Clear all caches
    pub fn clear_all(&mut self) {
        for cache in self.caches.values() {
            cache.clear();
        }
    }
}

impl Default for ParallelResolutionCache {
    fn default() -> Self {
        ParallelResolutionCache::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_validity() {
        let entry = CacheEntry {
            data: "test".to_string(),
            expires_at: Instant::now() + Duration::from_secs(10),
        };
        assert!(entry.is_valid());
        assert!(entry.get().is_some());
    }

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry {
            data: "test".to_string(),
            expires_at: Instant::now() - Duration::from_secs(1),
        };
        assert!(!entry.is_valid());
        assert!(entry.get().is_none());
    }

    #[test]
    fn test_version_cache_set_get() {
        let cache = DependencyCache::new(Duration::from_secs(100));
        let versions = vec![
            Version::parse("1.0.0").unwrap(),
            Version::parse("1.1.0").unwrap(),
        ];

        cache.set_versions("serde".to_string(), versions.clone());
        let retrieved = cache.get_versions("serde");

        assert_eq!(retrieved, Some(versions));
    }

    #[test]
    fn test_resolution_cache_set_get() {
        let cache = DependencyCache::new(Duration::from_secs(100));
        let mut resolution = HashMap::new();
        resolution.insert("pkg1".to_string(), Version::parse("1.0.0").unwrap());
        resolution.insert("pkg2".to_string(), Version::parse("2.0.0").unwrap());

        cache.set_resolution("app:1.0.0".to_string(), resolution.clone());
        let retrieved = cache.get_resolution("app:1.0.0");

        assert_eq!(retrieved, Some(resolution));
    }

    #[test]
    fn test_cache_cleanup() {
        let cache = DependencyCache::new(Duration::from_millis(10));
        cache.set_versions("test".to_string(), vec![Version::parse("1.0.0").unwrap()]);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(20));
        cache.cleanup_version_cache();

        assert!(cache.get_versions("test").is_none());
    }

    #[test]
    fn test_cache_clear() {
        let cache = DependencyCache::new(Duration::from_secs(100));
        cache.set_versions("pkg1".to_string(), vec![Version::parse("1.0.0").unwrap()]);
        cache.set_versions("pkg2".to_string(), vec![Version::parse("2.0.0").unwrap()]);

        cache.clear();

        assert!(cache.get_versions("pkg1").is_none());
        assert!(cache.get_versions("pkg2").is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = DependencyCache::new(Duration::from_secs(100));
        cache.set_versions("pkg1".to_string(), vec![Version::parse("1.0.0").unwrap()]);
        cache.set_versions("pkg2".to_string(), vec![Version::parse("2.0.0").unwrap()]);

        let stats = cache.stats();
        assert_eq!(stats.version_entries, 2);
        assert_eq!(stats.resolution_entries, 0);
    }

    #[test]
    fn test_lazy_cache_compute() {
        let cache: LazyCache<String> = LazyCache::new();

        let result = cache
            .get_or_compute(|| Ok("computed_value".to_string()))
            .unwrap();

        assert_eq!(result, "computed_value");
        assert_eq!(cache.get(), Some("computed_value".to_string()));
    }

    #[test]
    fn test_lazy_cache_uses_cached_value() {
        let cache: LazyCache<i32> = LazyCache::new();

        // Use Arc to share mutable state across closures
        let call_count = std::sync::Arc::new(std::sync::Mutex::new(0));

        let count_clone = call_count.clone();
        let _result1 = cache
            .get_or_compute(|| {
                let mut count = count_clone.lock().unwrap();
                *count += 1;
                Ok(42)
            })
            .unwrap();

        let count_clone = call_count.clone();
        let _result2 = cache
            .get_or_compute(|| {
                let mut count = count_clone.lock().unwrap();
                *count += 1;
                Ok(42)
            })
            .unwrap();

        // Should only call once because of caching
        let final_count = call_count.lock().unwrap();
        assert_eq!(*final_count, 1);
    }

    #[test]
    fn test_lazy_cache_invalidate() {
        let cache: LazyCache<String> = LazyCache::new();
        cache
            .get_or_compute(|| Ok("test".to_string()))
            .unwrap();

        assert!(cache.get().is_some());

        cache.invalidate();
        assert!(cache.get().is_none());
    }

    #[test]
    fn test_parallel_resolution_cache() {
        let mut cache = ParallelResolutionCache::new();

        let pkg_cache = cache.get_package_cache("serde");
        pkg_cache.set_versions(
            "serde".to_string(),
            vec![Version::parse("1.0.0").unwrap()],
        );

        let pkg_cache = cache.get_package_cache("serde");
        assert!(pkg_cache.get_versions("serde").is_some());
    }

    #[test]
    fn test_dependency_cache_default_ttl() {
        let cache = DependencyCache::default();
        let stats = cache.stats();
        assert_eq!(stats.ttl_secs, 300); // 5 minutes default
    }

    #[test]
    fn test_lazy_cache_error_propagation() {
        let cache: LazyCache<String> = LazyCache::new();

        let result = cache.get_or_compute(|| Err("test_error".to_string()));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "test_error");
    }
}
