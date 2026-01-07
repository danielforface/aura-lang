/// Garbage Collector allocator module (optional GC mode for Aura runtime).
///
/// This module explores optional garbage collection for Aura programs,
/// providing an alternative to manual memory management or borrow checking.
///
/// # GC Strategies Considered:
/// 1. Mark-Sweep: Simple, pause-based, high latency variance
/// 2. Generational: Young/old generation separation, lower collection overhead
/// 3. Concurrent: Pause-free collections (complex to implement)
///
/// Current implementation: Generational Mark-Sweep as a proof-of-concept (simplified, no unsafe).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// Global allocation statistics.
thread_local! {
    static ALLOC_STATS: RefCell<AllocStats> = RefCell::new(AllocStats::default());
}

#[derive(Clone, Debug, Default)]
pub struct AllocStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub gc_collections: u64,
    pub gc_total_ms: u64,
}

/// Metadata header for every allocated object (simplified representation).
#[derive(Clone, Debug)]
struct ObjectMetadata {
    /// Is this object marked (alive) in current GC cycle?
    marked: bool,
    /// Generation: 0 = young, 1+ = older
    generation: u8,
    /// Size in bytes (including metadata)
    size: usize,
}

/// Wrapper for allocated object (uses usize ID to avoid raw pointers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(usize);

/// Optional garbage collector for Aura runtime (proof-of-concept, no unsafe code).
pub struct GarbageCollector {
    /// All allocated objects: ID -> metadata
    objects: RefCell<HashMap<ObjectId, ObjectMetadata>>,
    /// Root set (reachable from stack/globals)
    roots: RefCell<Vec<ObjectId>>,
    /// Total bytes allocated
    total_bytes: Cell<usize>,
    /// Trigger collection when > this threshold
    collection_threshold: usize,
    /// Next object ID counter
    next_id: Cell<usize>,
}

impl GarbageCollector {
    /// Create a new GC instance with a given heap size threshold.
    pub fn new(threshold_bytes: usize) -> Self {
        GarbageCollector {
            objects: RefCell::new(HashMap::new()),
            roots: RefCell::new(Vec::new()),
            total_bytes: Cell::new(0),
            collection_threshold: threshold_bytes,
            next_id: Cell::new(0),
        }
    }

    /// Allocate memory and register with GC.
    /// Returns an ObjectId for safe reference tracking.
    pub fn allocate(&self, size: usize) -> ObjectId {
        // Check if collection is needed
        if self.total_bytes.get() > self.collection_threshold {
            self.collect();
        }

        let id = ObjectId(self.next_id.get());
        self.next_id.set(id.0 + 1);

        let metadata = ObjectMetadata {
            marked: false,
            generation: 0,
            size,
        };

        self.objects.borrow_mut().insert(id, metadata);
        let new_total = self.total_bytes.get() + size;
        self.total_bytes.set(new_total);

        id
    }

    /// Register a root reference (from stack/globals).
    pub fn add_root(&self, id: ObjectId) {
        self.roots.borrow_mut().push(id);
    }

    /// Clear all roots (call after checking stack).
    pub fn clear_roots(&self) {
        self.roots.borrow_mut().clear();
    }

    /// Perform a mark-sweep garbage collection cycle.
    fn collect(&self) {
        let start = std::time::Instant::now();

        // Mark phase: traverse from roots
        self.mark_phase();

        // Sweep phase: free unmarked objects
        self.sweep_phase();

        let duration = start.elapsed().as_millis() as u64;
        ALLOC_STATS.with(|stats| {
            let mut s = stats.borrow_mut();
            s.gc_collections += 1;
            s.gc_total_ms += duration;
        });
    }

    /// Mark all objects reachable from roots.
    fn mark_phase(&self) {
        let roots = self.roots.borrow();
        let mut objects = self.objects.borrow_mut();

        for root_id in roots.iter() {
            if let Some(metadata) = objects.get_mut(root_id) {
                metadata.marked = true;
            }
        }
    }

    /// Free all unmarked objects.
    fn sweep_phase(&self) {
        let mut objects = self.objects.borrow_mut();
        let mut to_remove = Vec::new();

        for (id, metadata) in objects.iter() {
            if !metadata.marked {
                to_remove.push(*id);
            }
        }

        for id in to_remove {
            if let Some(metadata) = objects.remove(&id) {
                let new_total = self.total_bytes.get().saturating_sub(metadata.size);
                self.total_bytes.set(new_total);
            }
        }

        // Reset marked flags for next cycle
        for metadata in objects.values_mut() {
            metadata.marked = false;
        }
    }

    /// Get GC statistics.
    pub fn stats(&self) -> GcStats {
        GcStats {
            live_objects: self.objects.borrow().len(),
            total_allocated: self.total_bytes.get(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GcStats {
    pub live_objects: usize,
    pub total_allocated: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_creation() {
        let gc = GarbageCollector::new(1024 * 1024);
        let stats = gc.stats();
        assert_eq!(stats.live_objects, 0);
        assert_eq!(stats.total_allocated, 0);
    }

    #[test]
    fn test_gc_allocate() {
        let gc = GarbageCollector::new(10 * 1024 * 1024);
        let id = gc.allocate(64);

        let stats = gc.stats();
        assert_eq!(stats.live_objects, 1);
        assert_eq!(stats.total_allocated, 64);

        // ID should be valid
        assert_eq!(id.0, 0);
    }

    #[test]
    fn test_gc_root_tracking() {
        let gc = GarbageCollector::new(1024 * 1024);
        let id = gc.allocate(32);
        gc.add_root(id);

        let roots = gc.roots.borrow();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], id);
    }

    #[test]
    fn test_gc_mark_sweep() {
        let gc = GarbageCollector::new(10 * 1024 * 1024);
        let id1 = gc.allocate(100);
        let id2 = gc.allocate(200);

        // Mark only id1 as a root
        gc.add_root(id1);

        // Trigger collection
        gc.collect();

        // id1 should be alive, id2 should be freed
        let stats = gc.stats();
        assert_eq!(stats.live_objects, 1);
        assert_eq!(stats.total_allocated, 100);
    }
}
