/// Performance Optimization & Profiling
///
/// Provides performance monitoring, caching, and optimization
/// strategies for debugger operations

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation_name: String,
    pub duration: Duration,
    pub memory_used: usize,
    pub cache_hit: bool,
    pub timestamp: Instant,
}

/// Cache entry
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: usize,
    pub ttl: Option<Duration>,
}

impl<T: Clone> CacheEntry<T> {
    fn new(value: T, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            self.created_at.elapsed() > ttl
        } else {
            false
        }
    }
}

/// Response cache for debugger commands
pub struct ResponseCache<K, V> {
    cache: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    max_entries: usize,
    default_ttl: Option<Duration>,
    hits: Arc<RwLock<usize>>,
    misses: Arc<RwLock<usize>>,
}

impl<K: std::cmp::Eq + std::hash::Hash + Clone, V: Clone> ResponseCache<K, V> {
    pub fn new(max_entries: usize, default_ttl: Option<Duration>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            default_ttl,
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }

    /// Get value from cache
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get_mut(key) {
            if !entry.is_expired() {
                entry.last_accessed = Instant::now();
                entry.access_count += 1;
                *self.hits.write().await += 1;
                return Some(entry.value.clone());
            } else {
                cache.remove(key);
            }
        }

        *self.misses.write().await += 1;
        None
    }

    /// Put value in cache
    pub async fn put(&self, key: K, value: V) {
        let mut cache = self.cache.write().await;

        // Evict oldest entry if cache is full
        if cache.len() >= self.max_entries && !cache.contains_key(&key) {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(key, CacheEntry::new(value, self.default_ttl));
    }

    /// Clear cache
    pub async fn clear(&self) {
        self.cache.write().await.clear();
        *self.hits.write().await = 0;
        *self.misses.write().await = 0;
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let hits = *self.hits.read().await;
        let misses = *self.misses.read().await;
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        CacheStats {
            hits,
            misses,
            hit_rate,
            entries: self.cache.read().await.len(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f64,
    pub entries: usize,
}

/// Profiler for tracking operation performance
pub struct OperationProfiler {
    metrics: Arc<RwLock<Vec<PerformanceMetrics>>>,
    current_operations: Arc<RwLock<HashMap<String, Instant>>>,
}

impl OperationProfiler {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
            current_operations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start profiling operation
    pub async fn start_operation(&self, name: &str) {
        let mut ops = self.current_operations.write().await;
        ops.insert(name.to_string(), Instant::now());
    }

    /// End profiling operation
    pub async fn end_operation(&self, name: &str, memory_used: usize) {
        let mut ops = self.current_operations.write().await;

        if let Some(start_time) = ops.remove(name) {
            let duration = start_time.elapsed();
            let cache_hit = false; // Would be determined by actual cache lookup

            let metric = PerformanceMetrics {
                operation_name: name.to_string(),
                duration,
                memory_used,
                cache_hit,
                timestamp: Instant::now(),
            };

            self.metrics.write().await.push(metric);
        }
    }

    /// Get performance report
    pub async fn get_report(&self) -> PerformanceReport {
        let metrics = self.metrics.read().await;

        if metrics.is_empty() {
            return PerformanceReport::default();
        }

        let total_operations = metrics.len();
        let cache_hits = metrics.iter().filter(|m| m.cache_hit).count();
        let avg_duration = metrics
            .iter()
            .map(|m| m.duration.as_millis() as f64)
            .sum::<f64>()
            / metrics.len() as f64;

        let total_memory = metrics.iter().map(|m| m.memory_used).sum();
        let avg_memory = total_memory / metrics.len();

        let slowest = metrics
            .iter()
            .max_by_key(|m| m.duration)
            .map(|m| (m.operation_name.clone(), m.duration));

        let fastest = metrics
            .iter()
            .min_by_key(|m| m.duration)
            .map(|m| (m.operation_name.clone(), m.duration));

        PerformanceReport {
            total_operations,
            cache_hits,
            avg_duration_ms: avg_duration,
            total_memory_bytes: total_memory,
            avg_memory_bytes: avg_memory,
            slowest_operation: slowest,
            fastest_operation: fastest,
        }
    }

    /// Clear metrics
    pub async fn clear_metrics(&self) {
        self.metrics.write().await.clear();
    }

    /// Get operation count
    pub async fn operation_count(&self) -> usize {
        self.metrics.read().await.len()
    }
}

/// Performance report
#[derive(Debug, Clone, Default)]
pub struct PerformanceReport {
    pub total_operations: usize,
    pub cache_hits: usize,
    pub avg_duration_ms: f64,
    pub total_memory_bytes: usize,
    pub avg_memory_bytes: usize,
    pub slowest_operation: Option<(String, Duration)>,
    pub fastest_operation: Option<(String, Duration)>,
}

/// Memory pool for reusing MI value allocations
pub struct ValuePool {
    available: Arc<RwLock<Vec<Vec<u8>>>>,
    max_pool_size: usize,
    value_size: usize,
}

impl ValuePool {
    pub fn new(max_pool_size: usize, value_size: usize) -> Self {
        Self {
            available: Arc::new(RwLock::new(Vec::with_capacity(max_pool_size))),
            max_pool_size,
            value_size,
        }
    }

    /// Get a buffer from pool or allocate new
    pub async fn get_buffer(&self) -> Vec<u8> {
        let mut pool = self.available.write().await;
        pool.pop().unwrap_or_else(|| Vec::with_capacity(self.value_size))
    }

    /// Return buffer to pool
    pub async fn return_buffer(&self, mut buffer: Vec<u8>) {
        let mut pool = self.available.write().await;
        if pool.len() < self.max_pool_size {
            buffer.clear();
            pool.push(buffer);
        }
    }

    /// Get pool statistics
    pub async fn pool_stats(&self) -> PoolStats {
        let available = self.available.read().await.len();
        PoolStats {
            available_buffers: available,
            max_pool_size: self.max_pool_size,
            utilization_percent: (available as f64 / self.max_pool_size as f64) * 100.0,
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available_buffers: usize,
    pub max_pool_size: usize,
    pub utilization_percent: f64,
}

/// Lazy parser for deferred MI value parsing
pub struct LazyMIParser {
    raw_response: String,
    parsed_cache: Arc<RwLock<Option<ParsedValue>>>,
}

#[derive(Debug, Clone)]
pub struct ParsedValue {
    pub data: String,
}

impl LazyMIParser {
    pub fn new(response: String) -> Self {
        Self {
            raw_response: response,
            parsed_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Parse on demand
    pub async fn parse(&self) -> ParsedValue {
        let mut cache = self.parsed_cache.write().await;

        if let Some(parsed) = cache.as_ref() {
            return parsed.clone();
        }

        // Parse the raw response (simplified)
        let parsed = ParsedValue {
            data: self.raw_response.clone(),
        };

        *cache = Some(parsed.clone());
        parsed
    }

    /// Get cached value without parsing
    pub async fn get_cached(&self) -> Option<ParsedValue> {
        self.parsed_cache.read().await.clone()
    }
}

/// Performance optimization settings
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    pub enable_response_caching: bool,
    pub cache_ttl: Option<Duration>,
    pub max_cache_entries: usize,
    pub enable_lazy_parsing: bool,
    pub enable_memory_pooling: bool,
    pub pool_size: usize,
    pub profile_operations: bool,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_response_caching: true,
            cache_ttl: Some(Duration::from_secs(60)),
            max_cache_entries: 1000,
            enable_lazy_parsing: true,
            enable_memory_pooling: true,
            pool_size: 100,
            profile_operations: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_response_cache_hit() {
        let cache: ResponseCache<String, String> = ResponseCache::new(10, None);

        cache.put("key1".to_string(), "value1".to_string()).await;
        let result = cache.get(&"key1".to_string()).await;

        assert_eq!(result, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_response_cache_miss() {
        let cache: ResponseCache<String, String> = ResponseCache::new(10, None);

        let result = cache.get(&"nonexistent".to_string()).await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let cache: ResponseCache<String, String> = ResponseCache::new(10, None);

        cache.put("key1".to_string(), "value1".to_string()).await;
        let _ = cache.get(&"key1".to_string()).await; // Hit
        let _ = cache.get(&"key2".to_string()).await; // Miss

        let stats = cache.stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let ttl = Duration::from_millis(100);
        let cache: ResponseCache<String, String> =
            ResponseCache::new(10, Some(ttl));

        cache.put("key1".to_string(), "value1".to_string()).await;

        // Immediate get should work
        let result = cache.get(&"key1".to_string()).await;
        assert!(result.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Now should be expired
        let result = cache.get(&"key1".to_string()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_operation_profiler() {
        let profiler = OperationProfiler::new();

        profiler.start_operation("test_op").await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        profiler.end_operation("test_op", 1000).await;

        let report = profiler.get_report().await;
        assert_eq!(report.total_operations, 1);
        assert!(report.avg_duration_ms > 0.0);
    }

    #[tokio::test]
    async fn test_memory_pool() {
        let pool = ValuePool::new(5, 256);

        let buf1 = pool.get_buffer().await;
        assert_eq!(buf1.capacity(), 256);

        pool.return_buffer(buf1).await;

        let stats = pool.pool_stats().await;
        assert_eq!(stats.available_buffers, 1);
    }

    #[tokio::test]
    async fn test_lazy_parser() {
        let parser = LazyMIParser::new("parsed_value".to_string());

        let cached_before = parser.get_cached().await;
        assert!(cached_before.is_none());

        let parsed = parser.parse().await;
        assert_eq!(parsed.data, "parsed_value");

        let cached_after = parser.get_cached().await;
        assert!(cached_after.is_some());
    }

    #[test]
    fn test_optimization_config_defaults() {
        let config = OptimizationConfig::default();

        assert!(config.enable_response_caching);
        assert!(config.enable_lazy_parsing);
        assert!(config.enable_memory_pooling);
        assert_eq!(config.max_cache_entries, 1000);
    }
}
