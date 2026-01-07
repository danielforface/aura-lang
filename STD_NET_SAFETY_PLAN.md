# std::net Safety Formalization - Enterprise Adoption Blocker #3

## Problem Statement

Network code in systems applications is infamously difficult to get right. Common issues:
- **Race conditions**: Concurrent socket access without proper synchronization
- **Deadlocks**: Circular dependencies in lock acquisition (listener holds lock, handler waits)
- **Resource leaks**: Unclosed sockets on all code paths
- **Order violations**: Sends before initialization, receives after close
- **Timing bugs**: Network timeouts without proper cleanup

**Current Aura State**: No formal network safety verification
- Socket operations exist but aren't verified
- No deadlock detection
- No race condition analysis
- Enterprise users reject code without proof

**Enterprise Requirement**: "Prove to us that your network code will never deadlock or race"

## Architecture: Z3-based Network Safety Verifier

### Design Philosophy
Leverage the existing Z3 verifier infrastructure (already in aura-core) to formally verify network safety properties.

```
aura-core/src/verifier.rs (existing 500+ LOC)
    ↓ (extends)
aura-core/src/net_verifier.rs (NEW - 400+ LOC)
    ├─ DeadlockDetector
    │   ├─ Lock acquisition graph (tracks lock order)
    │   └─ Cycle detection (identifies deadlock potential)
    ├─ RaceDetitionDetector  
    │   ├─ Memory access tracking (who reads/writes what)
    │   └─ Synchronization analysis (what guards access)
    └─ ResourceLifecycleTracker
        ├─ Initialization/finalization tracking
        └─ State machine enforcement (open → read/write → close)
```

### Key Components

#### 1. Network Type Annotations
```aura
# Acquire lock order for deadlock avoidance
@lock_order(["listener_lock", "handler_lock"])
cell accept_connection(listener: &Socket) ->:
  lock(listener_lock)
  ...
  lock(handler_lock)  # OK: respects order
  ...
```

#### 2. Socket State Machine
```
Socket States:
  Uninitialized
    ↓ (after socket::new or socket::connect)
  Open
    ├→ Readable (after listen/accept)
    ├→ Writable (connected, can send)
    ├→ Closed (after socket::close or drop)
    └→ Error (after failed operation)
```

#### 3. Synchronization Requirements
```aura
# Socket can be accessed safely by multiple threads only with synchronization

# BAD: Data race
mut socket = socket::new()
spawn { socket::read(...) }  # Thread 1
spawn { socket::write(...) } # Thread 2 - RACE!

# GOOD: Synchronized access
mut socket = socket::new()
lock = mutex::new()
spawn { lock.acquire(); socket::read(...); lock.release() }
spawn { lock.acquire(); socket::write(...); lock.release() }
```

#### 4. Deadlock Detection Rules
```
Rule 1: Lock Ordering - All threads must acquire locks in same order
  Violation: Thread A waits on B while holding C; Thread B waits on C

Rule 2: No Circular Dependencies
  Violation: Listener holds lock, then waits on handler that needs listener lock

Rule 3: No Unbounded Waits
  Violation: Waiting for socket::read without timeout + no cancellation

Rule 4: Socket Lifecycle Compliance
  Violation: Reading from closed socket, or not closing after open
```

## Implementation Plan

### Phase 1: Foundation (4-5 hours)

#### Step 1.1: Create net_verifier.rs module
- `NetworkProperty` enum (NoDeadlock, NoRaceCondition, SocketLifecycleValid)
- `NetworkState` for tracking socket lifecycle
- `LockGraph` for deadlock detection

#### Step 1.2: Integrate with existing Verifier
- Add `verify_network_safety()` method
- Reuse Z3 solver infrastructure
- Generate Z3 constraints from Aura network code

#### Step 1.3: Implement Deadlock Detection
- Build lock acquisition order graph
- Detect cycles (indicates potential deadlock)
- Track which locks each function holds

#### Step 1.4: Implement Race Detection  
- Track which variables are accessed in parallel (via spawn)
- Identify unsynchronized accesses
- Check mutex/lock protection

### Phase 2: Enforcement (2-3 hours)

#### Step 2.1: Add Network-Specific Type Checking
- Socket type with invariants
- Lock types with ordering constraints
- Spawn signature validation

#### Step 2.2: Create Socket Lifecycle Checker
- Enforce open→read/write→close sequence
- Detect double-closes and use-after-close
- Track all code paths

#### Step 2.3: Add Annotations Support
- @lock_order for manual deadlock prevention
- @synchronized for guarded access
- @no_timeout for unbounded operations

### Phase 3: Testing & Examples (2-3 hours)

#### Step 3.1: Test Network Patterns
```aura
# Pattern 1: Safe HTTP server
# Pattern 2: Safe connection pool
# Pattern 3: Safe broadcast with cleanup
```

#### Step 3.2: Test Violation Detection
```aura
# Deadlock case: Should be rejected
# Race condition: Should be rejected  
# Resource leak: Should be rejected
```

## Technical Approach

### Race Condition Detection Algorithm
```
1. Identify all concurrent code paths (spawn/join points)
2. For each variable that escapes to spawned threads:
   - Track all reads/writes
   - Collect synchronization operations (mutex acquire/release)
   - Check: all accesses protected by same lock
3. Generate Z3 constraints:
   - For each pair of accesses in different threads:
     ~(thread1_holds_lock AND thread2_holds_lock) → constraint_violated
4. Check satisfiability - if satisfiable bad state exists
```

### Deadlock Detection Algorithm
```
1. Build lock dependency graph:
   - Nodes: locks
   - Edges: "held during acquisition" (timestamp-based)
   
2. For each function:
   - Record lock acquisition sequence
   - Add edges: lock[i] → lock[i+1] (ordered acquisition)
   
3. Detect cycles:
   - If Thread A: acquire(L1) then acquire(L2)
   - And Thread B: acquire(L2) then acquire(L1)
   - Then Thread A holds L1, waits for L2
   - Thread B holds L2, waits for L1
   - DEADLOCK!
   
4. Generate Z3 model to confirm:
   - Variables: lock_held[t][l] (thread t holds lock l)
   - Variables: lock_waiting[t][l] (thread t waiting for lock l)
   - Constraints: circular dependencies impossible
```

### Socket Lifecycle State Machine
```
Type Socket:
  state: {Uninitialized, Open, Reading, Writing, Closed}
  
Valid transitions:
  Uninitialized --[socket::new]--> Open
  Open --[socket::connect]--> Open
  Open --[socket::listen]--> Open
  Open --[socket::accept]--> Readable
  Open --[socket::read]--> Open
  Open --[socket::write]--> Open
  Any --[socket::close]--> Closed
  Reading --[socket::read]--> Reading
  Writing --[socket::write]--> Writing
  
Invalid transitions (generate errors):
  Readable --[socket::write]--> ERROR
  Closed --[socket::read]--> ERROR
  Closed --[socket::write]--> ERROR
```

## Enterprise Value Proposition

### Pre-Implementation (Current State)
```
Enterprise CTO: "Your network code is unverified. How do we know it won't deadlock?"
Aura Team: "We follow best practices..."
CTO: "Not good enough. We need formal proof."
→ BLOCKED on Enterprise adoption
```

### Post-Implementation (After Phase 3)
```
Enterprise CTO: "Your network code is verified deadlock-free?"
Aura Team: "Yes - proven by Z3 solver. Here's the formal proof:"
CTO: "Show me a network case that would fail..."
Aura Team: "See? Deadlock detected here. Non-deterministic lock order."
CTO: "Excellent. You're Enterprise-grade."
→ UNBLOCKSED - Enterprise adoption possible
```

## Expected Outcomes

### Code Artifacts
- `aura-core/src/net_verifier.rs` (400+ LOC)
- `aura-core/tests/network_safety.rs` (20+ test cases)
- Network module documentation with verified patterns
- Enterprise case study: "Verified HTTP server"

### Formal Properties Guaranteed
1. **No Deadlock** - All network operations eventually progress
2. **No Race Conditions** - All concurrent access properly synchronized
3. **No Resource Leaks** - All sockets properly cleaned up
4. **Valid Lifecycle** - All state transitions valid
5. **Ordering Invariants** - Network protocol order maintained

### Integration Points
- Type-checker: Socket type with invariants
- Verifier: Network-specific Z3 constraints
- Type system: Generic lock ordering with phantom types
- Macros: Network pattern templates (server, client, broker)

## Success Criteria

✅ Deadlock detection algorithm proven on classic cases (lock order, wait-hold)
✅ Race condition detection works for shared sockets
✅ Socket lifecycle enforced (no use-after-close)
✅ All integration tests pass
✅ Enterprise case study verified (HTTP server pattern)
✅ Clear error messages guide users to fixes
✅ Can optionally disable for performance-critical code (@unsafe_net)

## Timeline

- **Phase 1 (Foundation)**: 4-5 hours → core algorithms
- **Phase 2 (Enforcement)**: 2-3 hours → type integration  
- **Phase 3 (Testing)**: 2-3 hours → verification & case study
- **Total**: ~10 hours intensive work
- **Target Completion**: This session or next

## Post-v1.0 Enhancements

- Liveness analysis (socket eventually closes)
- Fairness (all threads eventually acquire lock)
- Real-time constraints (socket ops complete within time bounds)
- Protocol verification (actual HTTP, TLS compliance)
- Symbolic execution for path-sensitive analysis

