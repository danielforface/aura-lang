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

### Interactive Explanation Engine

The **Explanation Engine** provides step-by-step breakdowns of proof failures and concurrent code behavior. This is the primary way to understand verification failures and debug proofs.

#### Understanding Proof Failures with Explanations

When a proof fails in Sentinel:

1. **Red ✗ appears** — Function doesn't meet its contract
2. **Click "Explain"** — Panel opens showing detailed breakdown
3. **Read counterexample:**
   - **Main claim** — What was being proven
   - **Proof steps** — Logical chain up to failure point
   - **Variable trace** — Concrete values that triggered failure
   - **Repair hints** — Suggested fixes

**Example: Loop Invariant Failure**

```
Main Claim: return == (n * (n + 1)) / 2

Loop Analysis (iteration 1):
├─ Before: i=0, sum=0
├─ Add i:  sum = 0 + 0 = 0
├─ Increment: i = 0 + 1 = 1
├─ Check invariant: sum == (i * (i + 1)) / 2
│  Expected: 0 == (1 * 2) / 2 = 1
│  Actual:   0 != 1
└─ ❌ INVARIANT FAILED at line 13
```

**Repair Hint:** "Looks like you're adding the old value of i. Either:
- Add BEFORE incrementing, or
- Update invariant to (i-1)*i/2"

#### Interactive Variable Inspection

In the Explanation panel:

- **Click variable** — Shows all assignments in trace
- **Hover source** — Code line highlights
- **Show all** — Expands full proof tree
- **Copy trace** — Export for documentation
- **Proof timeline** — Visual Z3 solver timeline

### Concurrent Code Explanation

The explanation engine analyzes concurrent code for race conditions and deadlocks:

#### Data Race Explanation

```aura
fn buggy_concurrent() {
    let mut x = 0;
    
    spawn { x = 1; }
    spawn { x = 2; }
}
```

**Explanation:**

```
Race Condition Detected: data race on 'x'

Thread 1 (line 5):
  └─ Unprotected write: x = 1

Thread 2 (line 6):
  └─ Unprotected write: x = 2

Happens-Before Analysis:
  ✗ No synchronization between writes
  ✗ Both threads access same memory
  ✓ At least one is a write

Concurrency Risk: CRITICAL

Repair Suggestions:
  [1] Use Mutex:
      let x = Mutex::new(0);
      spawn { *x.lock() = 1; }
      spawn { *x.lock() = 2; }
      
  [2] Use Atomic:
      let x = Atomic::new(0);
      spawn { x.store(1); }
      spawn { x.store(2); }
      
  [3] Separate variables:
      spawn { let x1 = 1; ... }
      spawn { let x2 = 2; ... }
```

#### Deadlock Explanation

```aura
fn deadlock_risk() {
    let lock_a = Mutex::new(0);
    let lock_b = Mutex::new(0);
    
    spawn {
        let _a = lock_a.lock();  // T1 acquires A
        let _b = lock_b.lock();  // T1 waits for B
    }
    spawn {
        let _b = lock_b.lock();  // T2 acquires B  
        let _a = lock_a.lock();  // T2 waits for A
    }
}
```

**Explanation:**

```
Deadlock Detected: Circular Lock Dependency

Lock Dependency Graph:
  Thread 1: A → B
  Thread 2: B → A
  
Cycle: A ← T1 → B ← T2 → A

Trace:
  Timeline 1:
    T1: acquire(A) ✓
    T2: acquire(B) ✓
    T1: acquire(B) 🔒 (blocked by T2)
    T2: acquire(A) 🔒 (blocked by T1)
    
  Result: Circular wait → DEADLOCK

Repair Strategies:
  [1] Enforce lock order globally:
      Always acquire locks in order: A, then B
      
      Thread 1:
        let _a = lock_a.lock();
        let _b = lock_b.lock();
        
      Thread 2:
        let _a = lock_a.lock();  // Same order!
        let _b = lock_b.lock();
      
  [2] Use timeouts:
      if let Ok(b) = lock_b.try_lock_timeout(Duration::ms(100)) {
          // proceed
      } else {
          // retry or abort
      }
      
  [3] Refactor to avoid nested locks:
      Move B acquisition outside A's critical section
```

#### Memory Ordering Explanation

For atomic operations and memory barriers:

```aura
fn memory_ordering_issue() {
    let x = Atomic::new(0);
    let y = Atomic::new(0);
    
    spawn {
        x.store(1, Release);  // T1: store x with Release
        // ??? What about y?
    }
    
    spawn {
        let y_val = y.load(Acquire);  // T2: load y with Acquire
        // Does T2 see x = 1?
    }
}
```

**Explanation:**

```
Memory Ordering Analysis:

Claim: T2 sees x = 1 after y.load()
Status: ❌ NOT PROVEN

Explanation:
  Release-Acquire synchronizes through the same atomic!
  
  T1 does:  store(x, Release) — no synchronization!
  T2 does:  load(y, Acquire) — synchronizes with release of y, not x
  
  ✗ x.store() and y.load() are different atomics
  ✗ No happens-before relationship
  
  Possible outcomes:
    - T2 sees x = 0 (reordering)
    - T2 sees x = 1 (luck)

Fix: Use fence for sequential consistency
  T1: x.store(1, Relaxed); fence(Release);  // Synchronizes all prior stores
  T2: fence(Acquire); let y = y.load(Relaxed);  // Synchronizes all later loads
```

### Explain Panel Features

**Visual Timeline:**
- Shows order of events across threads
- Color-codes synchronization points
- Marks where ordering is guaranteed vs. possible

**Drill-Down:**
- Click any event to see source line
- Expand variables to see captured values
- Show proof of safety (if proven) or counterexample

**Copy Output:**
- Export explanation as markdown
- Include in bug reports or design docs
- Share with team for code review



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
