# Aura CI Gate: Differential Testing System

## Quick Start

The CI Gate ensures proof verification results are consistent across debugging backends before releasing.

### Running the CI Gate

```bash
# Build the system
cd aura-lsp
cargo build --release

# Run CI gate with defaults
cargo run --bin aura-ci-gate --release

# Custom configuration
cargo run --bin aura-ci-gate --release -- --min-passing 95% --backends gdb,lldb --timeout 60

# Run differential tests
cargo run --bin differential-test-runner --release -- tests/config.json
```

### Running Tests

```bash
# Unit tests
cargo test --lib --release

# Integration tests  
cargo test --test ci_gate_tests --release

# All tests with output
cargo test -- --nocapture
```

## Overview

The CI Gate system provides:

1. **Differential Testing**: Run the same tests on GDB and LLDB
2. **Automated Verification**: Ensure backends agree on results
3. **Release Gating**: Block releases if backend disagreement detected
4. **Comprehensive Reports**: Detailed comparison and agreement metrics

## Architecture

```
┌─────────────────────────────────────────┐
│  Proof Verification Results             │
└────────────────┬────────────────────────┘
                 ↓
┌─────────────────────────────────────────┐
│  CI Gate Driver                         │
│  - Load proof results                   │
│  - Convert to test cases                │
└────────────────┬────────────────────────┘
                 ↓
┌─────────────────────────────────────────┐
│  Differential Test Runner               │
│  ┌──────────────┐  ┌──────────────┐    │
│  │ GDB Runner   │  │ LLDB Runner  │    │
│  └──────┬───────┘  └───────┬──────┘    │
│         │                  │           │
│         └─────────┬────────┘           │
│                   ↓                    │
│        Compare Results & Verify        │
│        Backend Agreement               │
└────────────────┬────────────────────────┘
                 ↓
┌─────────────────────────────────────────┐
│  Gate Decision & Report                 │
│  - Pass/Block Release                   │
│  - Detailed Metrics                     │
└─────────────────────────────────────────┘
```

## Configuration

### Command-Line Options

```bash
aura-ci-gate [OPTIONS]

Options:
  --min-passing <PERCENT>     Minimum passing percentage (default: 95%)
  --backends <LIST>           Backends to test (default: gdb,lldb)
  --timeout <SECONDS>         Test timeout (default: 60)
  -h, --help                  Show help
```

### JSON Configuration File

```json
{
  "tests": [
    {
      "name": "proof_test_1",
      "source_file": "test.aura",
      "breakpoint": "verify_witness",
      "commands": ["print witness", "print condition"],
      "expected_variables": {
        "witness": "42",
        "condition": "true"
      },
      "expected_output": "proof verified"
    }
  ],
  "ci_gate_config": {
    "min_passing_tests": 10,
    "allow_warnings": false,
    "backends": ["gdb", "lldb"],
    "timeout_seconds": 60
  }
}
```

## Test Execution Flow

1. **Load Configuration**: Parse test cases and CI gate settings
2. **Extract Proof Results**: Get variable values from proof verification
3. **Run GDB Tests**:
   - Compile with debug symbols
   - Set breakpoints
   - Execute debugger commands
   - Parse output and extract variables
4. **Run LLDB Tests**:
   - Compile with debug symbols
   - Set breakpoints
   - Execute debugger commands
   - Parse output and extract variables
5. **Compare Results**:
   - Check execution status matches
   - Verify variable values are identical
   - Confirm output matches
6. **Gate Decision**:
   - If agreement ✅ → Safe to release
   - If disagreement ❌ → Block release

## Example Test

```rust
#[test]
fn test_differential_agreement() {
    // GDB result
    let gdb_vars = {
        let mut m = HashMap::new();
        m.insert("x".to_string(), "10".to_string());
        m.insert("y".to_string(), "20".to_string());
        m
    };

    // LLDB result
    let lldb_vars = {
        let mut m = HashMap::new();
        m.insert("x".to_string(), "10".to_string());
        m.insert("y".to_string(), "20".to_string());
        m
    };

    // Should agree
    assert_eq!(gdb_vars, lldb_vars);
}
```

## Output Parsing

### GDB Output Example

```
Breakpoint 1 at 0x401030
Running test...
x = 10
y = 20
sum = 30
```

Parsed variables: `{"x": "10", "y": "20", "sum": "30"}`

### LLDB Output Example

```
Breakpoint 1: where = test`main + 0x12 at test.c:5:5
Process 12345 launched: './test' (x86_64)
Process 12345 stopped
(int) x = 10
(int) y = 20
(int) sum = 30
```

Parsed variables: `{"x": "10", "y": "20", "sum": "30"}`

## Integration with CI/CD

### GitHub Actions

The system integrates with GitHub Actions via `.github/workflows/ci-gate.yml`:

```yaml
- name: Run CI gate
  run: |
    cargo run --bin aura-ci-gate --release \
      -- --min-passing 95% --backends gdb,lldb

- name: Comment on PR
  uses: actions/github-script@v7
  with:
    script: |
      // Post results to PR
      github.rest.issues.createComment({...})
```

### Status Checks

- ✅ **PASS**: All tests agree across backends → PR can merge
- ❌ **FAIL**: Backend disagreement detected → PR blocked

## Troubleshooting

### Backend Not Found

```
Error: gdb is not installed
```

Solution: Install GDB and/or LLDB

```bash
# Ubuntu/Debian
sudo apt-get install gdb lldb

# macOS
brew install gdb lldb

# Windows
choco install gdb lldb
```

### Timeout Exceeded

If tests exceed the timeout:

1. Increase timeout: `--timeout 120`
2. Check system resources
3. Profile individual tests
4. Optimize proof verification

### Variable Parsing Failure

If variables aren't being extracted correctly:

1. Check debugger output format matches parser expectations
2. Verify variable names match expected names
3. Check for whitespace issues in output
4. Enable verbose mode for debugging

## Performance Tips

1. **Parallelize Tests**: Run multiple test cases concurrently
2. **Cache Results**: Reuse GDB/LLDB output when possible
3. **Optimize Compilation**: Use `-O0` for faster builds
4. **Selective Testing**: Only test changed modules

## Advanced Usage

### Custom Backends

To add a new backend (e.g., VSCode Debugger):

```rust
pub struct VSCodeDebuggerRunner;

impl VSCodeDebuggerRunner {
    pub fn run_test(test: &DifferentialTest) -> Result<DebuggerTestResult, String> {
        // Implement VSCode Debugger protocol
    }
}
```

### Automated Bisection

To debug backend disagreements:

```bash
# Find which test case causes disagreement
cargo run --bin diagnostic-bisector -- tests/config.json
```

### Historical Analysis

Track CI gate results over time:

```bash
# Analyze trends
cargo run --bin ci-gate-analyzer -- --days 30
```

## Files

- `src/ci_gate.rs` - Core CI gate logic
- `src/ci_gate_driver.rs` - Proof result loading and orchestration
- `src/differential_test_runner.rs` - GDB/LLDB execution and comparison
- `src/bin/aura-ci-gate.rs` - CLI binary
- `src/bin/differential-test-runner.rs` - Test runner binary
- `tests/ci_gate_tests.rs` - Integration tests
- `.github/workflows/ci-gate.yml` - GitHub Actions workflow
- `scripts/run_differential_tests.sh` - Test execution script

## References

- [CI Gate Documentation](docs/CI_GATE.md)
- [GDB Documentation](https://sourceware.org/gdb/onlinedocs/)
- [LLDB Documentation](https://lldb.llvm.org/)
- [Aura Verification](../aura-verify/README.md)

## Contributing

To add new differential tests:

1. Create test case in `tests/config.json`
2. Add test program in `tests/programs/`
3. Run `cargo run --bin differential-test-runner -- tests/config.json`
4. Verify results match across backends
5. Submit PR with results

## License

Part of the Aura Language project. See main LICENSE file.
