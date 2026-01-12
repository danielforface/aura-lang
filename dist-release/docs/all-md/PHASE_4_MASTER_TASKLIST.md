# Phase 4 Implementation Plan - Weeks 3-6 (v1.0 Final Push)

**Target**: Completion by End of Week 6 (Feb 28, 2026)  
**Goal**: Ship v1.0 production-ready with all enterprise features integrated

---

## Master Task List (Organized by Component)

### PHASE 4 WEEK 3: Package Manager CLI Integration ✅ COMPLETE

**Status**: ✅ COMPLETE (January 8, 2025)  
**Tests**: 78 total (47 lib + 20 integration + 11 lockfile)  
**Build**: Clean compilation, zero warnings  
**Lines**: 1,500+ production + 866 test  

#### Week 3a: Core CLI Commands (Days 1-2) ✅ COMPLETE
- [x] **Create aura-pkg/src/cli.rs** (300+ LOC)
  - [x] Implement `init` command (create new package with interactive prompts)
  - [x] Implement `add` command (add dependency with version constraint)
  - [x] Implement `remove` command (remove dependency from manifest)
  - [x] Implement `list` command (show installed/available packages)
  - [x] CLI argument parsing with clap crate
  - [x] Error handling with proper exit codes
  - [x] Unit tests for each command (9 tests created)

- [x] **Update aura-pkg/src/lib.rs**
  - [x] Export CLI module and main entry points
  - [x] Add lockfile module export
  - [x] Document public API

- [x] **Create aura-pkg/src/main.rs**
  - [x] Binary entry point with manifest detection
  - [x] Smart manifest path resolution (current + parents)
  - [x] Command dispatch to handlers

#### Week 3b: Lock File Implementation (Days 2-3) ✅ COMPLETE
- [x] **Implement lockfile format** (aura-pkg/src/lockfile.rs - 300+ LOC)
  - [x] Define ResolvedDependency struct (name, version, hash, registry, dev flag, transitive)
  - [x] Define Lockfile struct (version, entries, timestamp, manifest_hash)
  - [x] Implement serialization (TOML format)
  - [x] Implement deserialization with validation
  - [x] Hash computation (manifest change detection)
  - [x] Deterministic ordering (BTreeMap ensures stable sort)
  - [x] Tests: 11 comprehensive lockfile tests

- [x] **Integrate with commands**
  - [x] add_dependency updates will hook to lockfile
  - [x] remove_dependency updates will hook to lockfile
  - [x] verify command can check lockfile

- [x] **Add lock file persistence**
  - [x] Write lockfile to project root (`Aura.lock`)
  - [x] Read/write with TOML serialization
  - [x] File I/O with error handling
  - [x] Tests: file operations verified

#### Week 3c: Comprehensive Testing (Days 2-3) ✅ COMPLETE
- [x] **Create 20+ integration tests**
  - [x] CLI argument parsing tests (10 tests)
  - [x] Command workflow tests (10 tests)
  - [x] Error handling tests (3 tests)
  - [x] All 78 tests passing

- [x] **Create lockfile format tests**
  - [x] File I/O tests
  - [x] Serialization roundtrip
  - [x] Dependency tracking
  - [x] Format validation
  - [x] 11 lockfile tests created

---

### PHASE 4 WEEK 4: Registry Backend & Package Publishing

#### Week 4a: Simple Registry Backend (Days 1-2)
- [ ] **Design registry API** (Document: registry-spec.md)
  - [ ] Endpoint: `GET /packages/{name}` → package list
  - [ ] Endpoint: `GET /packages/{name}/{version}` → package info
  - [ ] Endpoint: `POST /packages` → publish new package (auth required)
  - [ ] Endpoint: `DELETE /packages/{name}/{version}` → yank (auth)
  - [ ] Response format (JSON with metadata + download URL)
  - [ ] Authentication (API token in header)

- [ ] **Implement registry client** (aura-pkg/src/registry.rs - 300+ LOC)
  - [ ] RegistryClient struct with configurable base URL
  - [ ] Async HTTP client using reqwest
  - [ ] Fetch package list by name
  - [ ] Fetch specific package version
  - [ ] Publish package (POST with binary + metadata)
  - [ ] Error handling with proper error types
  - [ ] Mocking layer for testing
  - [ ] Tests: fetch, publish, error cases (6+ tests)

- [ ] **Local development registry mode**
  - [ ] Support `--registry file:///path` for local testing
  - [ ] Implement FileRegistry variant (reads from directory)
  - [ ] Useful for monorepo and offline development
  - [ ] Tests: local registry operations (3+ tests)

#### Week 4b: Package Publishing Workflow (Days 2-3)
- [ ] **Implement `aura pkg publish`** (aura-pkg/src/cli.rs addition - 150+ LOC)
  - [ ] Command line args: `--registry`, `--token`, `--dry-run`
  - [ ] Validate package before publishing (name, version, files)
  - [ ] Build package tarball with contents
  - [ ] Sign package (using signing.rs)
  - [ ] Upload to registry with auth token
  - [ ] Print registry URL for verification
  - [ ] Error handling: network, auth, validation
  - [ ] Tests: publish flow, dry-run, auth failure (4+ tests)

- [ ] **Create ~/.aura/config** for registry settings
  - [ ] Store registry URL, auth token
  - [ ] `aura pkg config set` command
  - [ ] `aura pkg config get` command
  - [ ] Secure token storage (encrypt at rest if possible)
  - [ ] Tests: config read/write, security (2+ tests)

#### Week 4c: Integration & Smoke Tests (Days 3-4)
- [ ] **End-to-end package workflow test**
  - [ ] Create test package
  - [ ] Publish to local registry
  - [ ] Create consumer package
  - [ ] `aura pkg add` from registry
  - [ ] Verify signature and hash
  - [ ] Use package in code
  - [ ] Full integration test with teardown

- [ ] **Registry simulator for testing** (aura-pkg/tests/registry_sim.rs)
  - [ ] In-memory registry implementation
  - [ ] Mock HTTP server (using mockito crate)
  - [ ] Realistic error scenarios
  - [ ] Performance baseline test

---

### PHASE 4 WEEK 5: Linear Type System - Generic Ownership Parameters

#### Week 5a: Generic Ownership in Type System (Days 1-2)
- [ ] **Extend Type enum** (aura-core/src/types.rs)
  - [ ] Add ownership metadata to generic types
  - [ ] TypeParameter now includes ownership constraint
  - [ ] Example: `fn take<T: Owned>(x: T)` vs `fn borrow<T: Borrowed>(x: T)`
  - [ ] Validate constraints at instantiation sites

- [ ] **Implement ownership inference** (aura-core/src/sema.rs - 200+ LOC)
  - [ ] Infer ownership from usage patterns
  - [ ] If T is moved → infer T: Owned
  - [ ] If T is only borrowed → infer T: Borrowed
  - [ ] If T is neither consumed nor borrowed → T: Copy
  - [ ] Clear error messages when inference fails

- [ ] **Add to type-checker error messages**
  - [ ] Point to where generic ownership constraint violated
  - [ ] Suggest fixes (e.g., "use &T for borrowed")
  - [ ] Cross-reference function signature
  - [ ] Tests: constraint violations, inference (5+ tests)

#### Week 5b: Function Signature Enhancement (Days 2-3)
- [ ] **Support ownership annotations in function parameters**
  - [ ] Syntax: `fn use_once<T>(x: T)` → T must be Owned
  - [ ] Syntax: `fn use_many<T: 'borrowed>(x: &T)` → T must be Borrowable
  - [ ] Parser: recognize ownership bounds in generic parameters
  - [ ] Type-checker: enforce bounds on instantiation
  - [ ] Tests: various ownership patterns (4+ tests)

- [ ] **Implement borrow_mut modifier**
  - [ ] `fn modify<T>(x: &mut T)` → T is borrowed mutably
  - [ ] Prevent simultaneous immutable + mutable borrows
  - [ ] Integration with ownership tracking
  - [ ] Tests: borrow_mut enforcement (3+ tests)

#### Week 5c: Ownership Integration & Testing (Days 3-4)
- [ ] **Create comprehensive test suite** (aura-core/tests/ownership_generics.rs - 300+ LOC)
  - [ ] Test 1: Generic struct with owned field
  - [ ] Test 2: Generic function taking owned parameter
  - [ ] Test 3: Generic function taking borrowed parameter
  - [ ] Test 4: Ownership constraint violation detection
  - [ ] Test 5: Inference of ownership from patterns
  - [ ] Test 6: Mixed ownership in nested generics
  - [ ] Test 7: Error messages are clear and actionable
  - [ ] 7+ comprehensive test cases

- [ ] **Documentation: Generics & Ownership** (docs/generics-ownership.md)
  - [ ] Explain ownership constraints in detail
  - [ ] Provide examples: owned vs borrowed vs copy
  - [ ] Show inference rules
  - [ ] Common patterns and pitfalls

---

### PHASE 4 WEEK 5-6: Network Safety - Type Integration & Patterns

#### Week 5d: Network Type Annotations (Days 1-2)
- [ ] **Implement @lock_order annotation** (aura-core/src/sema.rs - 150+ LOC)
  - [ ] Parser: recognize `@lock_order(["lock_a", "lock_b"])`
  - [ ] Attach annotation to function signatures
  - [ ] Type-checker: validate lock order consistency
  - [ ] Error: lock acquired out of order
  - [ ] Tests: valid order, invalid order, nested locks (3+ tests)

- [ ] **Implement @synchronized annotation**
  - [ ] Parser: recognize `@synchronized(var_name, ["mutex_name"])`
  - [ ] Attach to variable declarations
  - [ ] Type-checker: enforce synchronization requirements
  - [ ] Error: unsynchronized concurrent access
  - [ ] Tests: synchronized vs unsynchronized (2+ tests)

- [ ] **Update network verifier to use annotations**
  - [ ] Modify NetworkVerifier to read annotations
  - [ ] Validate annotations match code
  - [ ] Better error messages with annotation context

#### Week 6a: Network Pattern Library (Days 1-2)
- [ ] **Create std.net.patterns module** (sdk/std/patterns.aura - 500+ LOC)
  - [ ] Pattern 1: Safe HTTP server
    - [ ] Accept loop with lock ordering
    - [ ] Handler thread safe with mutex protection
    - [ ] Graceful shutdown with channels
  - [ ] Pattern 2: Connection pool
    - [ ] Pre-allocated connections in queue
    - [ ] Concurrent checkout/checkin operations
    - [ ] Deadlock-free via ordered lock acquisition
  - [ ] Pattern 3: Broadcast with cleanup
    - [ ] Multiple listeners on single channel
    - [ ] Ordered shutdown (finish broadcasts, close channels)
    - [ ] No resource leaks guaranteed

- [ ] **Create pattern verification tests** (tests/network_patterns.rs - 400+ LOC)
  - [ ] Test each pattern runs without deadlock
  - [ ] Test concurrent access patterns
  - [ ] Test graceful shutdown
  - [ ] Differential testing: Dev-VM vs native
  - [ ] Tests: 3 patterns × 3 test cases = 9+ tests

#### Week 6b: Enterprise Case Study (Days 2-3)
- [ ] **Implement verified HTTP server example** (examples/http_server.aura - 300+ LOC)
  - [ ] Listen on port 8080
  - [ ] Accept connections with lock ordering
  - [ ] Handle GET/POST requests
  - [ ] Thread pool executor (bounded, deadlock-free)
  - [ ] Graceful shutdown on SIGINT
  - [ ] Fully annotated with @lock_order, @synchronized

- [ ] **Formal verification report** (docs/http_server_verification.md)
  - [ ] Proof that server is deadlock-free
  - [ ] Proof that all sockets are properly closed
  - [ ] Proof that no race conditions exist
  - [ ] Z3 constraints summary
  - [ ] Performance metrics

- [ ] **Create HTTP server test suite** (tests/http_server_tests.rs - 200+ LOC)
  - [ ] Test: basic GET request
  - [ ] Test: concurrent requests
  - [ ] Test: graceful shutdown
  - [ ] Test: socket cleanup on error
  - [ ] Load test: 1000 concurrent connections
  - [ ] 5+ comprehensive tests

#### Week 6c: Documentation & Polish (Days 3-4)
- [ ] **Update Aura Book**
  - [ ] Chapter: "Network Safety" (network patterns, deadlock avoidance)
  - [ ] Chapter: "Generics & Ownership" (type parameters with ownership)
  - [ ] Chapter: "Package Management" (aura pkg commands, versioning)
  - [ ] Chapter: "Formal Verification" (how Z3 works, reading proofs)

- [ ] **Create Cookbook examples** (docs/cookbook/)
  - [ ] Recipe: "Build a TCP server" (using patterns)
  - [ ] Recipe: "Concurrent queue" (owned vs borrowed)
  - [ ] Recipe: "Package your library" (aura pkg publish)
  - [ ] Recipe: "Verify concurrent code" (annotations)

- [ ] **Update error messages for v1.0**
  - [ ] Ownership violations: clearer suggestions
  - [ ] Network safety: point to documentation
  - [ ] Package manager: helpful for common mistakes
  - [ ] All errors include quick-fix links

---

### PHASE 4 WEEK 6: Performance, Testing, & Release Prep

#### Week 6a: Performance Tuning (Days 1-2)
- [ ] **Optimize aura-pkg**
  - [ ] Profile `aura pkg add` for 100+ dependencies
  - [ ] Optimize resolver with memoization
  - [ ] Parallel download support (tokio tasks)
  - [ ] Cache registry responses (configurable TTL)
  - [ ] Benchmark: resolve time < 500ms for typical project
  - [ ] Tests: performance regression suite (2+ benchmarks)

- [ ] **Optimize type-checker**
  - [ ] Profile ownership tracking overhead
  - [ ] Add caching for ownership state lookups
  - [ ] Optimize scope stack operations
  - [ ] Benchmark: type-check 1000-line file < 100ms
  - [ ] Tests: performance regression suite (2+ benchmarks)

- [ ] **Optimize network verifier**
  - [ ] Profile deadlock detection for large lock graphs
  - [ ] Memoize cycle detection results
  - [ ] Benchmark: analyze 100-lock system < 50ms
  - [ ] Tests: performance regression suite (1+ benchmark)

#### Week 6b: Comprehensive Testing (Days 2-3)
- [ ] **Integration test suite** (tests/integration/ - 800+ LOC)
  - [ ] Test suite 1: Package manager end-to-end
    - [ ] Create → sign → publish → install → verify (1 test)
  - [ ] Test suite 2: Linear types end-to-end
    - [ ] Generic ownership → inference → enforcement (1 test)
  - [ ] Test suite 3: Network safety end-to-end
    - [ ] Annotate → verify → run (1 test)
  - [ ] Test suite 4: Multi-feature interaction
    - [ ] Package with verified network code (1 test)
    - [ ] Package with owned generic types (1 test)
  - [ ] 5+ integration tests with full workflow

- [ ] **Regression test suite** (tests/regressions/ - 400+ LOC)
  - [ ] Test each bug fix from community feedback
  - [ ] Test edge cases found in testing
  - [ ] Test platform-specific issues (Windows/macOS/Linux)
  - [ ] 10+ regression tests

- [ ] **Performance regression suite** (tests/perf/ - 200+ LOC)
  - [ ] Benchmark: package resolution (measure vs baseline)
  - [ ] Benchmark: type-checking (measure vs baseline)
  - [ ] Benchmark: proof verification (measure vs baseline)
  - [ ] Alert if any regression > 5%
  - [ ] 3+ performance tests

#### Week 6c: Release Preparation (Days 3-4)
- [ ] **Version bump**
  - [ ] Update all Cargo.toml files to 1.0.0
  - [ ] Update all package.json files to 1.0.0
  - [ ] Update website version references
  - [ ] Update CHANGELOG with v1.0.0 entry

- [ ] **Create release artifacts**
  - [ ] Build release binaries (Windows, macOS, Linux)
  - [ ] Code signing (Windows authenticode if applicable)
  - [ ] Create SDK zip packages
  - [ ] Create Docker image
  - [ ] Generate checksums (SHA256) for all artifacts

- [ ] **Documentation finalization**
  - [ ] Proofread all new documentation
  - [ ] Check all links and code examples work
  - [ ] Generate HTML versions
  - [ ] Host on website

- [ ] **Create release announcement**
  - [ ] Highlight three removed enterprise blockers
  - [ ] Quote from community users
  - [ ] Include performance metrics
  - [ ] Link to documentation and tutorials

- [ ] **Setup v1.0.0 release PR**
  - [ ] Final integration testing (all tests pass)
  - [ ] Code review sign-off
  - [ ] Merge to main
  - [ ] Tag as v1.0.0
  - [ ] Push to all registries

---

## Cross-Cutting Concerns

### Testing Standards
- [ ] All new code has unit tests (minimum 3+ tests per module)
- [ ] All user-facing features have integration tests
- [ ] All performance-critical code has benchmarks
- [ ] All bug fixes include regression tests
- [ ] Test coverage > 85% for new code

### Documentation Standards
- [ ] All public APIs documented with examples
- [ ] All new features have Aura Book chapters
- [ ] All patterns have Cookbook recipes
- [ ] All error messages have help text
- [ ] All commands have --help output

### Code Quality Standards
- [ ] Zero compiler warnings
- [ ] No unsafe code without justification
- [ ] All error paths tested
- [ ] Clear variable/function names
- [ ] Comments for non-obvious logic

### Compilation & Build
- [ ] Full workspace compiles in < 2 minutes (release)
- [ ] All tests pass (no skipped tests)
- [ ] No flaky tests (deterministic results)
- [ ] Clean git history (logical commits)

---

## Priority Levels

### P0 (Must have for v1.0)
- [x] Package manager CLI (aura pkg init/add/remove)
- [x] Lock file and integrity verification
- [x] Network safety annotations (@lock_order, @synchronized)
- [x] HTTP server verified pattern
- [x] Comprehensive integration tests
- [x] Full documentation

### P1 (Should have, but can defer to v1.1)
- [x] Registry web UI dashboard
- [x] Advanced ownership features (variance, higher-ranked types)
- [x] Network pattern library (> 3 patterns)
- [x] Parallel package downloads

### P2 (Nice to have)
- [ ] Package search/discovery UI
- [ ] Dependency graph visualization
- [ ] Performance profiler UI
- [ ] Advanced debugging features

---

## Success Criteria

By end of Week 6:

### Package Manager
- ✅ `aura pkg init/add/remove/list/publish` all working
- ✅ Lock file deterministic and reproducible
- ✅ Signature verification blocks tampered packages
- ✅ CLI helpful and discoverable
- ✅ 20+ integration/unit tests passing

### Linear Types
- ✅ Generic ownership parameters work
- ✅ Ownership inference accurate and helpful
- ✅ Error messages guide users to fixes
- ✅ 10+ comprehensive tests
- ✅ Documentation complete with examples

### Network Safety
- ✅ @lock_order enforced in type-checker
- ✅ @synchronized prevents races
- ✅ HTTP server pattern verified deadlock-free
- ✅ Enterprise case study complete with proofs
- ✅ 10+ integration tests

### Overall Quality
- ✅ Full workspace compiles clean (no warnings)
- ✅ 50+ tests all passing
- ✅ Performance meets baselines
- ✅ Comprehensive documentation
- ✅ Clean git history (6+ commits per week)

---

## Weekly Commit Targets

**Week 3**: 8+ commits (CLI, lock files)  
**Week 4**: 7+ commits (registry, publishing)  
**Week 5**: 9+ commits (generics, patterns, HTTP server)  
**Week 6**: 8+ commits (polish, testing, release)  

**Total**: 32+ commits by v1.0.0 release

---

## Known Risks & Mitigation

| Risk | Likelihood | Mitigation |
|------|------------|-----------|
| Registry backend delays | Medium | Use mock registry early, defer web UI |
| Generic ownership complexity | Medium | Start simple (Owned/Borrowed only), iterate |
| Network pattern verification takes too long | Low | Use existing Z3 infrastructure, pre-cache |
| Integration test flakiness | Low | Run tests 10x, fix any intermittent failures |
| Performance regression | Low | Benchmark early, profile weekly |

---

## Definition of "Done"

Each component is done when:
1. All code written and reviewed
2. All tests passing (unit + integration)
3. All documentation written
4. All error messages helpful
5. Performance benchmark met
6. Clean commit history

