# Garbage Collection in Aura: Design Exploration

## Motivation
Aura currently relies on manual memory management and (in Option B) borrow checking for memory safety. However, for certain use cases (rapid prototyping, systems with abundant heap space, real-time constraint relaxation), optional garbage collection could provide a simpler programming model.

## Design Alternatives

### Option 1: Mark-Sweep Collector (Pause-Based)
**Pros:**
- Simple to implement and understand
- Works with pointer graphs of any shape
- No write barriers or read barriers needed

**Cons:**
- Pause latencies proportional to heap size (O(live objects))
- Not suitable for real-time / low-latency workloads
- GC pauses can be unpredictable

**Estimated Implementation:** 2-3 weeks (mark phase, sweep, root tracking)

### Option 2: Generational Collector (Young/Old Split)
**Pros:**
- Focuses collection on young generation (high mortality rate)
- Typical pause latencies much lower than mark-sweep
- Amortized throughput better than mark-sweep

**Cons:**
- Cross-generation pointers require write barriers
- More complex root tracking (need to track inter-generational pointers)
- Old generation collections still expensive

**Estimated Implementation:** 4-6 weeks (write barrier instrumentation, promotion strategy, tuning)

### Option 3: Concurrent GC (Background Collection)
**Pros:**
- True pause-free collection (or near-pause-free)
- Best latency profile for interactive workloads
- Suitable for real-time systems

**Cons:**
- Very complex (concurrent marking, tri-color marking, CAS loops)
- Race conditions and synchronization overhead
- Testing and debugging extremely difficult

**Estimated Implementation:** 8-12 weeks (if attempted)

## Current Proof-of-Concept

The `aura-rt-native/src/allocator.rs` module implements a **skeletal generational Mark-Sweep collector** with:

1. **Object metadata:** Each allocation carries a header with:
   - `marked` flag (for mark phase)
   - `generation` (0 = young, 1+ = old)
   - `size` (for sweep phase)

2. **Root tracking:** External API to register stack/global roots via `add_root()`.

3. **Collection trigger:** Automatic collection when heap exceeds threshold.

4. **Allocation statistics:** Track total allocations, collections, time spent in GC.

## Integration Path

To fully enable GC mode in Aura:

1. **Compile flag:** Add `--gc` or environment variable to enable GC allocator.
2. **Type system:** Mark types as `GC<T>` for heap allocation, vs. `Stack<T>` or `Box<T>`.
3. **Runtime initialization:** Initialize GC on first allocation.
4. **Root discovery:** Integrate with stack scanner to find roots automatically (alternative: conservative GC).
5. **Benchmarking:** Compare pause latencies, throughput vs. manual allocation.

## Recommendation

**For Aura v0.3:** 
- Implement generational collector (Option 2) to gain 80/20 benefit
- Keep option configurable (compile-time feature gate)
- Measure latency/throughput against manual allocation benchmarks
- Do NOT pursue concurrent GC unless workload demands < 10ms p99 pause

## Code Structure

```
aura-rt-native/
  src/
    allocator.rs       ← GC implementation (this file)
    gc_config.rs       ← GC tuning parameters (not yet created)
    conservative_gc.rs ← Conservative stack scanning (planned)
```

## References

- "Garbage Collection Algorithms for Automatic Dynamic Memory Management" (Zeller)
- HotSpot GC tuning guide (Oracle)
- ZGC pause-less GC (for concurrent inspiration)
