# Aura Debugging Guide

## Quick Start: Using Sentinel Debugger

### Launch a Debug Session

1. **Open Aura file** in Sentinel IDE
2. **Set breakpoint** — Click margin next to line number
3. **Run debugger** — Press `F5` or `Debug > Start Debugging`
4. **Control execution:**
   - `F10` — Step over (next line in current function)
   - `F11` — Step into (enter function call)
   - `Shift+F11` — Step out (exit current function)
   - `F5` — Continue (resume execution)
   - `Shift+F5` — Stop (terminate debug session)

### Variables Panel

While paused, the **Variables panel** (left sidebar) shows:
- **Local variables** — Values in current scope
- **Parameters** — Function arguments
- **Expanded objects** — Click ▶ to inspect nested structures

Example:

```
function sum_to_n(n: i32) → i32
  Parameters:
    n: i32 = 5

  Locals:
    i: i32 = 3
    total: i32 = 6
    ▶ vec: Vec[i32]
      [0]: 1
      [1]: 2
      [2]: 3
```

### Stack Trace

The **Call Stack panel** shows:
- **Current frame** — Where execution is paused
- **Calling frames** — Full stack history
- **Click to jump** — Click any frame to inspect its locals

Example:

```
 → sum_to_n (line 10, column 8)
   main (line 25, column 4)
   <program entry>
```

---

## Setting Breakpoints

### Line Breakpoint

Click the margin next to the line number. A red dot appears. Execution pauses when that line is reached.

### Conditional Breakpoint

Right-click the margin → **Add Conditional Breakpoint** → Enter condition:

```
i == 10 && x > 0
```

Breaks only when the condition is true. Useful for:
- Breaking on specific loop iterations
- Breaking when a variable reaches a threshold

### Logpoint (Print Without Stopping)

Right-click the margin → **Add Logpoint** → Enter message:

```
i={i}, total={total}
```

Messages appear in the **Debug Console** without pausing execution. Use for:
- Debugging without stopping
- Capturing variable history
- High-frequency monitoring

### Tracepoint (Combined)

Some debuggers support tracepoints: breakpoints that log and continue. In Sentinel, use logpoints for the same effect.

---

## GDB/LLDB Command Reference

### Direct MI Commands (Advanced)

For complex debugging, use the **Debug Console** to send raw GDB/LLDB commands:

#### Execution Control

```gdb
(gdb) run              # Start execution
(gdb) continue         # Resume
(gdb) next             # Next line (step over)
(gdb) step             # Step into function
(gdb) finish           # Run until return
(gdb) until <line>     # Run until specific line
```

#### Breakpoints

```gdb
(gdb) break <file>:<line>           # Set breakpoint
(gdb) break <function>              # Break at function
(gdb) info breakpoints              # List all breakpoints
(gdb) delete <number>               # Remove breakpoint
(gdb) enable/disable <number>       # Toggle breakpoint
(gdb) condition <number> <expr>     # Add condition
```

#### Inspection

```gdb
(gdb) print <variable>              # Inspect value
(gdb) print <variable>@<count>      # Inspect array
(gdb) info locals                   # Show local variables
(gdb) backtrace                     # Full call stack
(gdb) frame <number>                # Switch frame
(gdb) up/down                       # Navigate stack
```

#### Watch Expressions

```gdb
(gdb) watch <variable>              # Break on change
(gdb) print <expr>                  # One-time evaluation
```

### LLDB Equivalents

LLDB syntax is similar but uses different command names:

```lldb
(lldb) run              # Start (gdb: run)
(lldb) c                # Continue (gdb: continue)
(lldb) n                # Next (gdb: next)
(lldb) s                # Step (gdb: step)
(lldb) frame info       # Stack frame (gdb: frame)
(lldb) v                # Show locals (gdb: info locals)
(lldb) p <variable>     # Print (gdb: print)
```

---

## Debugging Strategies

### Strategy 1: Binary Search

Narrow down the failure location:

1. Set breakpoint at function start
2. Step past suspect code sections using `F10`
3. When failure occurs, zoom in on that section
4. Repeat until root cause is found

**Time:** O(log N) for N lines of code

### Strategy 2: Watch Variables

Monitor variable changes:

1. Right-click variable in Variables panel → **Add to Watch**
2. Watch panel appears in sidebar
3. As you step, watch shows how value changes
4. Stop when unexpected change occurs

### Strategy 3: Conditional Breakpoints

For loops, use conditions to skip iterations:

```
i >= 90 && i < 100
```

Useful for:
- Breaking on rare conditions
- Skipping early iterations (faster iteration)
- Debugging specific failing cases

### Strategy 4: Logpoints for History

Instead of stepping, capture variable history:

1. Right-click margin → **Add Logpoint**
2. Message: `step {i}, sum={total}`
3. Run to completion
4. Check **Debug Console** for full history
5. Faster than manual stepping

---

## Debugging Concurrent Code

### Race Conditions

Use **Thread panel** (if available):

```
Threads:
  Thread 1 (main)
    ▶ acquire_lock()
  Thread 2 (worker)
    ▶ release_lock()
  Thread 3 (gc)
    ▶ allocate()
```

Set breakpoints in each thread independently. Execution pauses only in that thread.

### Deadlocks

If execution freezes:

1. **Pause** (Ctrl+Alt+Break or kill with signal)
2. **View all threads** in Thread panel
3. **Inspect stack** of each thread
4. Look for circular lock acquisition

Example: Thread A holds lock1, waits for lock2. Thread B holds lock2, waits for lock1. **Deadlock!**

### Data Races

Use **Data Breakpoints** (if supported):

```
watch <address>
```

Breaks when memory at that address is written. Use to find which thread modifies shared data unexpectedly.

---

## Debugging Memory Issues

### Out-of-Bounds Access

In Aura, bounds are proven at compile time. If you still get a runtime error:

1. Check **Variables panel** — What was the array size?
2. Check **stack** — Which function accessed it?
3. Review **proof failures** in Sentinel — Did the verifier miss something?

Example:

```aura
fn bad_access() {
    let arr = [1, 2, 3];
    arr[10]  // Verifier should catch this
}
```

If it compiles, the verifier proved it's safe. If runtime fails, file a bug.

### Memory Leaks

Aura's region-based memory model prevents memory leaks. All allocations are tracked by region. To debug allocation patterns:

1. **Profiling panel** — Monitor memory usage over time
2. **Region stats** — `aura debug --memory-stats` shows allocation breakdown
3. **Valgrind** (if needed) — `valgrind ./program` for external verification

### Use-After-Free

Compile-time prevention: Aura's linear type system rejects use-after-free. If you see a runtime error, the type system failed to prove safety. File a bug or add explicit `requires` clauses.

---

## Debugging Proofs

### Proof Failure Workflow

1. **See failure in Sentinel** — Red `✗` on function
2. **Click Explain button** — View detailed counterexample
3. **Read variable trace:**
   - Input values
   - Which branch was taken
   - Output value that failed assertion
4. **Strengthen preconditions** — If input was invalid
5. **Weaken postconditions** — If claim was too strict
6. **Add intermediate assertions** — Break large proofs

### Example: Debugging a Failed `ensures`

```aura
fn absolute(x: i32) -> i32
    ensures return >= 0
{
    if x > 0 { x } else { -x }
}

// Sentinel shows counterexample:
// x = -2147483648 (INT_MIN)
// return = -2147483648  (negation overflows!)
```

**Fix:**

```aura
fn absolute(x: i32) -> i32
    requires x > -2147483648  // Exclude INT_MIN
    ensures return >= 0
{
    if x > 0 { x } else { -x }
}
```

---

## Performance Profiling

### Profiling Panel in Sentinel

1. **Run verification** — Verifier emits telemetry
2. **View Profiling Dashboard** — Shows P50/P95/P99 latencies
3. **Identify slow phases:**
   - Parse: <10ms (syntax)
   - Semantic: <20ms (types)
   - Normalize: <50ms (simplification)
   - Z3: 50–200ms (solving)

### Speeding Up Proofs

| Problem | Solution |
| --- | --- |
| Slow semantic phase | Split into smaller functions |
| Slow normalize phase | Simplify assertion complexity |
| Slow Z3 phase | Add intermediate assertions / use fast profile |
| Cached hit rate low | Check if file deps are correct |

### Profile Commands

```bash
# Fast profile (for interactive development)
aura verify --profile fast file.aura

# Thorough profile (for CI)
aura verify --profile thorough file.aura

# Show timing breakdown
aura verify --profile thorough --timings file.aura
```

---

## Using Sentinel Integration

### Explain Panel

When assertion fails:

1. Click **Explain** in Proofs panel
2. **Counterexample variables** expand as tree
3. **Hover to highlight** — Code is highlighted where variable defined/used
4. **Click variable** — Jump to definition
5. **Repair suggestions** — Auto-generated fix recommendations

### Debug + Verify Together

Sentinel integrates stepping (GDB/LLDB) with proof failures:

1. **Set breakpoint** at assertion
2. **Step to breakpoint** in debugger
3. **View variables** in Variables panel
4. **Compare with proof failure** — Expected vs. actual values
5. **Edit code** — Fix based on insight
6. **Proof re-runs** automatically (cached when possible)

---

## Advanced: Custom Debug Handlers

For library developers:

```aura
#[debug_handler]
fn debug_print_vec(vec: &Vec[T]) -> String {
    // Custom formatter for debugger display
    format!("Vec(len={}, cap={})", vec.len(), vec.capacity())
}
```

Register custom formatters so debugger shows meaningful values instead of raw memory.

---

## Troubleshooting

| Issue | Cause | Fix |
| --- | --- | --- |
| Debugger won't start | GDB/LLDB not installed | Run `aura debug --check` |
| Breakpoint not hit | Optimization removed code | Use `--profile dev` |
| Variables show `<optimized out>` | Release build | Use `--profile dev` for debugging |
| Stack corrupted | Stack overflow or buffer overflow | Check array bounds + loop invariants |
| Deadlock | Circular lock acquisition | Review lock order in threads |

---

## Quick Reference

| Task | Command/Key |
| --- | --- |
| Start debugging | `F5` or `Debug > Start` |
| Step over | `F10` |
| Step into | `F11` |
| Step out | `Shift+F11` |
| Continue | `F5` |
| Stop | `Shift+F5` |
| Toggle breakpoint | `F9` or click margin |
| View variables | Sidebar (left panel) |
| View call stack | Sidebar (left panel) |
| View watches | Sidebar (left panel) |
| Open debug console | `Ctrl+Shift+Y` |
| Conditional breakpoint | Right-click margin → Add Conditional |
| Logpoint | Right-click margin → Add Logpoint |

---

## Next Steps

- **Verification debugging** — See Chapter 10: Verification & Proof-Driven Development
- **Performance profiling** — Use `aura profile` command
- **Concurrent debugging** — Chapter 12: Concurrency & Race-Free Proofs
