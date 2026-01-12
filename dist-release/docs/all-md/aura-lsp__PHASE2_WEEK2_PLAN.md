# Phase 2 Week 2 Planning - LSP Integration

## Overview

Phase 2 Week 1 completed the differential testing infrastructure. Phase 2 Week 2 will integrate this with the actual proof verification results from the LSP and Aura core.

## Integration Architecture

```
┌──────────────────────────────────────┐
│  User writes proof in VS Code        │
│  (aura-lsp LSP Client)               │
└────────────────┬─────────────────────┘
                 ↓ (textDocument/didChange)
┌──────────────────────────────────────┐
│  aura-lsp LSP Server (src/main.rs)   │
│                                      │
│  - File parsing                      │
│  - Proof verification                │
│  - Counterexample extraction         │
└────────────────┬─────────────────────┘
                 ↓ (aura-verify proof results)
┌──────────────────────────────────────┐
│  CI Gate Integration (Phase 2 Week 2)│
│                                      │
│  - Load proof results                │
│  - Convert to test cases             │
│  - Run differential tests            │
│  - Gate decision                     │
└────────────────┬─────────────────────┘
                 ↓ (diagnostics)
┌──────────────────────────────────────┐
│  VS Code Editor                      │
│                                      │
│  - Display gate status               │
│  - Show backend agreement/disagreement│
│  - Display detailed comparison       │
└──────────────────────────────────────┘
```

## Week 2 Tasks

### Task 1: Proof Result Loading (Days 1-2)

#### What We Need
Extract proof verification results from LSP diagnostics and aura-verify output.

#### Implementation Steps

1. **Create Proof Result Adapter** (`src/proof_result_adapter.rs`)
   ```rust
   pub struct ProofResultAdapter;
   
   impl ProofResultAdapter {
       pub fn from_verify_error(error: &VerifyError) -> ProofResult {
           // Extract witness, condition, expected output
           // Return structured proof result
       }
       
       pub fn from_diagnostic_data(data: &serde_json::Value) -> ProofResult {
           // Parse counterexample from LSP diagnostic
           // Extract variable bindings
       }
   }
   ```

2. **Load from aura-verify**
   - Extract VerifyError from proof verification
   - Get counterexample data structure
   - Map to ProofResult format

3. **Parse Counterexample Data**
   - Extract witness values
   - Get variable bindings
   - Collect expected output
   - Store in standardized format

#### Files to Create
- `src/proof_result_adapter.rs` - Conversion logic
- `tests/proof_result_tests.rs` - Unit tests

#### Expected Output
```rust
ProofResult {
    program_path: "my_proof.aura",
    expected_output: "proof verified",
    variable_state: {
        "witness": "42",
        "condition": "true",
        "x": "10"
    }
}
```

### Task 2: LSP Diagnostic Integration (Days 2-3)

#### What We Need
Wire the CI gate into the LSP's diagnostic publishing pipeline.

#### Implementation Steps

1. **Extend AuraLanguageServer** (in `src/main.rs`)
   ```rust
   struct AuraLanguageServer {
       // ... existing fields ...
       ci_gate_driver: Arc<Mutex<CIGateDriver>>,
   }
   
   impl LanguageServer for AuraLanguageServer {
       async fn did_change(&self, params: DidChangeTextDocumentParams) {
           // 1. Parse and verify file
           // 2. Extract proof results
           // 3. Run CI gate
           // 4. Update diagnostics with gate status
       }
   }
   ```

2. **Add Gate Status to Diagnostics**
   ```rust
   #[derive(Debug, Clone, serde::Serialize)]
   struct GateStatusInfo {
       passed: bool,
       total_tests: usize,
       passed_tests: usize,
       agreement: f32,  // 0.0 to 1.0
       backend_results: HashMap<String, BackendStatus>,
   }
   ```

3. **Publish Gate Results**
   - Add gate status to Diagnostic.data
   - Send diagnostics to client
   - Include comparison details

#### Files to Modify
- `src/main.rs` - Wire CI gate into LSP server

#### Expected Behavior
```
User edits file with failing proof
  ↓
LSP runs verification
  ↓
Gets counterexample from VerifyError
  ↓
Runs CI gate on proof
  ↓
Publishes diagnostic with gate status:
  "Gate Status: Backend Agreement 100% (GDB: pass, LLDB: pass)"
```

### Task 3: UI/Visualization (Days 3-4)

#### What We Need
Display CI gate status in VS Code with clear visualization.

#### Implementation Steps

1. **Create Custom Panel** (in VS Code extension)
   ```typescript
   // editors/aura-vscode/src/explainPanel.ts
   
   export class GateStatusPanel {
       public static show(gateResult: GateResult) {
           // Display gate status
           // Show per-backend results
           // Highlight disagreements
           // Link to comparison details
       }
   }
   ```

2. **Diagnostic Display**
   - Green check for passed gate ✅
   - Red X for failed gate ❌
   - Yellow warning for disagreement ⚠️

3. **Comparison View**
   ```
   Test: proof_001
   
   GDB:  ✅ PASS  x=10, y=20
   LLDB: ✅ PASS  x=10, y=20
   
   Agreement: ✅ 100%
   ```

#### Files to Create
- `editors/aura-vscode/src/gateStatusPanel.ts` - Gate status display
- `editors/aura-vscode/src/comparisonView.ts` - Detailed comparison view

### Task 4: Testing and Validation (Days 4-5)

#### What We Need
End-to-end tests proving the system works.

#### Test Cases

1. **Test Successful Proof with Gate Pass**
   ```rust
   #[test]
   async fn test_proof_passes_gate() {
       // Create proof in LSP
       // Get counterexample
       // Run gate
       // Assert gate.passed == true
   }
   ```

2. **Test Failed Proof with Gate Fail**
   ```rust
   #[test]
   async fn test_proof_fails_gate() {
       // Create failing proof
       // Get counterexample
       // Run gate
       // Assert gate.passed == false
   }
   ```

3. **Test Backend Disagreement**
   ```rust
   #[test]
   async fn test_backend_disagreement() {
       // Mock GDB returning x=10
       // Mock LLDB returning x=15
       // Run gate
       // Assert disagreement detected
   }
   ```

4. **Test UI Integration**
   ```typescript
   test("Gate status displays correctly", async () => {
       // Open proof file
       // Wait for verification
       // Check panel shows gate status
       // Verify agreement metrics
   });
   ```

#### Files to Create
- `tests/lsp_integration_tests.rs` - LSP integration tests
- `editors/aura-vscode/src/test/gateStatus.test.ts` - UI tests

## Implementation Order

### Week 2 Day 1
- [ ] Create proof result adapter
- [ ] Implement proof result parsing
- [ ] Unit tests for adapter

### Week 2 Day 2
- [ ] Extend LSP server with CI gate driver
- [ ] Wire proof results to gate
- [ ] Add gate status to diagnostics

### Week 2 Day 3
- [ ] Implement gate status panel
- [ ] Create comparison view
- [ ] Test diagnostic display

### Week 2 Day 4-5
- [ ] End-to-end testing
- [ ] VS Code UI testing
- [ ] Performance validation

## Dependencies

### Required from Phase 2 Week 1
- ✅ CI gate core (src/ci_gate.rs)
- ✅ CI gate driver (src/ci_gate_driver.rs)
- ✅ Differential test runner (src/differential_test_runner.rs)

### From Existing Codebase
- `aura-verify::VerifyError` - Proof verification results
- `aura-verify::Counterexample` - Witness data
- `tower-lsp::Client` - Diagnostic publishing
- VS Code LSP client capabilities

## Success Criteria

- [ ] Proof results load from LSP verification
- [ ] CI gate runs automatically on proof changes
- [ ] Gate status publishes to VS Code diagnostics
- [ ] UI displays gate results clearly
- [ ] Backend disagreements highlighted
- [ ] End-to-end tests pass
- [ ] No performance regression
- [ ] Documentation updated

## Estimated Effort

- **Days to Complete**: 5 days
- **Lines of Code**: ~1500 (excluding tests)
- **Test Coverage**: 20+ integration tests

## Key Challenges & Solutions

### Challenge 1: Async/Await Complexity
**Solution**: Use tokio channels for CI gate execution, don't block LSP

### Challenge 2: Large Variable Sets
**Solution**: Only extract relevant variables from diagnostics

### Challenge 3: Timeout Management
**Solution**: Run differential tests in background tasks

### Challenge 4: Backend Availability
**Solution**: Gracefully degrade if GDB/LLDB not available

## Rollout Plan

1. **Phase 2 Week 2**: Complete integration
2. **Phase 2 Week 3**: Beta testing with team
3. **Phase 3**: Full release with documentation
4. **Ongoing**: Monitor gate pass rates and performance

## Definition of Done

- [ ] All Week 2 tasks completed
- [ ] No regressions in existing LSP functionality
- [ ] Gate status visible in VS Code
- [ ] Detailed comparison available on demand
- [ ] Performance acceptable (< 100ms overhead)
- [ ] Documentation complete
- [ ] Team code review approved

---

## Phase 2 Week 2 Start Checklist

Before starting Week 2:

- [ ] Review Phase 1 Week 1 completion checklist
- [ ] Pull latest main branch
- [ ] Build aura-lsp successfully
- [ ] Run Phase 1 Week 1 tests (all pass)
- [ ] Understand LSP server architecture
- [ ] Understand aura-verify error structure
- [ ] Understand VS Code extension capabilities

## Ready to Begin?

✅ **Phase 2 Week 1 Complete**
🚀 **Phase 2 Week 2 Ready to Start**

Begin with "Task 1: Proof Result Loading" when ready.
