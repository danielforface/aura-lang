# Linear Type Enforcement - Implementation Strategy

## Current State Analysis

### Existing Infrastructure in `aura-core/src/sema.rs` (2730 LOC)

**What We Have:**
- `Checker` struct with:
  - `scopes: Vec<HashMap<String, Type>>` - variable bindings
  - `mut_scopes: Vec<HashSet<String>>` - tracks mutability
  - `cap: CapabilityGraph` - resource lifetime tracking
  - `consume_move_from_value()` - marks value as consumed
  - `lookup_val()`, `lookup_scope_index()` - variable resolution

- CapabilityGraph-based tracking:
  - `ensure_alive()` - checks value is still live
  - `consume_move()` - marks resource as consumed
  - Used for data race detection in async lambdas

**What's Missing:**
- Explicit OwnershipState enum (Consumed, Borrowed, Returned)
- Enforcement that prevents reuse after move
- Control flow analysis for branches/loops
- Type parameter tracking for generic ownership
- Clear error messages for ownership violations

## Implementation Plan

### Phase 1: Core Ownership Tracking (Target: 2-3 hours)

#### Step 1.1: Add OwnershipState Enum to sema.rs
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnershipState {
    /// Value is owned and can be moved/borrowed/used
    Owned,
    
    /// Value has been consumed (moved or used in linear context)
    Consumed,
    
    /// Value is borrowed (immutable reference exists)
    Borrowed,
    
    /// Value is returned (passed to function/return statement)
    Returned,
}
```

#### Step 1.2: Extend Checker with Ownership Tracking
```rust
// Add to Checker struct:
ownership_states: Vec<HashMap<String, OwnershipState>>,
```

Parallel structure to `scopes` and `mut_scopes`:
- Push/pop with scopes
- Track state of each variable
- Enable enforcement on variable use

#### Step 1.3: Implement OwnershipState Methods
```rust
fn track_ownership(&mut self, name: &str, state: OwnershipState) -> Result<(), SemanticError>
fn get_ownership(&self, name: &str) -> OwnershipState
fn set_ownership(&mut self, name: &str, state: OwnershipState)
fn mark_consumed(&mut self, name: &str, span: Span) -> Result<(), SemanticError>
fn check_not_consumed(&self, name: &str, span: Span) -> Result<(), SemanticError>
```

#### Step 1.4: Integrate with Variable Binding
Modify `define_val()` to initialize ownership state:
```rust
fn define_val(&mut self, name: &Ident, ty: Type, mutable: bool) -> Result<(), SemanticError> {
    // ... existing code ...
    self.ownership_states.last_mut().unwrap().insert(name.node.clone(), OwnershipState::Owned);
}
```

### Phase 2: Enforcement Rules (Target: 1-2 hours)

#### Step 2.1: Enforce Move Semantics
When identifier is used in consuming position (function arg, assignment):
1. Check current ownership state is not Consumed
2. If non-copy type: mark as Consumed
3. If copy type (u32, bool): allow reuse

#### Step 2.2: Enforce Return/Function Boundaries
At function end:
1. Check all non-copy local values are either:
   - Consumed/Moved out
   - Returned explicitly
   - Dead code warning

#### Step 2.3: Control Flow Awareness
For if/match expressions:
1. Track state at branch start
2. Each branch must end in same state OR consume consistently
3. Merge states at join points

### Phase 3: Enhanced Error Messages (Target: 30 mins)

Clear error messages like:
```
error[E0381]: value used after move
  --> program.aura:5:8
   |
 3 | let x = model.load();  // x has ownership
   |     - binding created here
 4 | ai.infer(model, data);  // model is moved here
   |          -----
   |          model moved here due to linear ownership
 5 | print(model);           // error: model already consumed
   |       ^^^^^ value used after move
   |
   = note: this is a move-only type (Model has linear ownership)
   = note: model can only be used once; use `borrow` if shared access needed
```

## Key Design Decisions

1. **Non-Copy Types**: Types that require linear ownership enforcement
   - User-defined records with move operations
   - AI models, tensors, resources
   - Implement `is_non_copy_type()` to determine

2. **Scope-Aware Tracking**: Ownership state follows scope stack
   - When popping scope, no need to check unconsumed values yet
   - Will add warning system in Phase 3

3. **Integration with CapabilityGraph**: 
   - Both systems track consumption
   - CapabilityGraph focuses on lifetime/deadlock prevention
   - OwnershipState focuses on linear move semantics
   - No conflicts; complementary

4. **Copy Type Handling**:
   - u32, bool, primitive types never consumed
   - Avoid ownership tracking for these
   - Simplify error messages

## Testing Strategy

Test cases to add:
1. ✅ Simple move: `x = model; f(model); print(model)` → ERROR
2. ✅ Copy type: `a = 5; b = a; c = a;` → OK
3. ✅ Explicit borrow: `f(borrow model)` → allows reuse
4. ✅ Function return: `fn get_model() -> Model { model }` → OK
5. ✅ Constructor move: `MyType(model)` → consumes model
6. ✅ Branch merge: If both branches move/both don't move → OK
7. ✅ Mutable move: `mut x = load(); use(x); use(x);` → ERROR

## Integration Points

- **parser**: Already supports type annotations
- **lower**: Will use ownership info for code generation
- **verifier**: Can use ownership facts for Z3 constraints
- **ir**: May need to annotate moves in IR

## Success Criteria

✅ Type-checker rejects double-use of non-copy types
✅ Type-checker accepts valid move patterns
✅ Clear error messages guide users to fixes
✅ Integrates with existing CapabilityGraph system
✅ All new tests pass
✅ No regression in existing tests

## Estimated Progress After Completion

- **Bottleneck #2 (Linear Types)**: 70-80% complete
  - ✅ Ownership enforcement in type-checker
  - ❌ Generic ownership parameters (future enhancement)
  - ❌ Borrow checker improvements (future enhancement)

- **Overall Path to v1.0**:
  - aura-pkg: 60% (metadata, signing, resolver done)
  - Linear types: 75% (core enforcement done)
  - std::net safety: 0% (not started)
  - Critical adoption blockers: ~70% resolved

