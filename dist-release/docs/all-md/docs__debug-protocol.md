# Aura Debug Protocol (Sentinel)

This is a lightweight, Sentinel↔Aura-specific debugging/profiling protocol.

It is **not** DAP.

## Transport

- Commands: **newline-delimited JSON** written to Aura process **stdin**.
- Events: **newline-delimited JSON** written to Aura process **stdout** as a single line:

`AURA_DEBUG_EVENT {json}`

Sentinel should strip these lines from the regular console view and use them to drive Debug/Perf panels.

## Enablement

Sentinel should set `AURA_DEBUG_PROTOCOL=1` in the Aura environment and ensure stdin is piped.

When enabled, Aura emits a `hello` event once at startup.

## Compatibility / versioning policy

- `protocol` is a monotonically increasing integer.
- **Backward compatible changes** (no version bump):
	- Adding new capabilities.
	- Adding new events.
	- Adding new optional fields to existing commands/events.
- **Breaking changes** (require a version bump):
	- Removing/renaming a command/event/field.
	- Changing the meaning or requiredness of an existing field.

Clients should:

- Ignore unknown events.
- Ignore unknown fields on known events/commands.
- Prefer capability checks over protocol version checks for feature gating.

## Commands

All commands use `{"cmd": "..."}` with `camelCase` names.

### `enable`

```json
{"cmd":"enable","start_paused":false,"perf":true}
```

- `start_paused`: if true, Aura starts paused before executing statements.
- `perf`: if true, Aura records a statement-level perf timeline and emits a perf report at the end.

### `pause`

```json
{"cmd":"pause"}
```

### `continue`

```json
{"cmd":"continue"}
```

### `step`

```json
{"cmd":"step"}
```

Statement-level step (MVP).

### `setBreakpoints`

```json
{"cmd":"setBreakpoints","breakpoints":[{"line":12},{"line":20,"condition":"x > 0"}]}
```

Breakpoints are line-based (current source file). Conditions are parsed as single expressions.

### `setWatches`

```json
{"cmd":"setWatches","watches":[{"expr":"x"},{"expr":"x + 1"}]}
```

Watches are parsed as single expressions and evaluated in a restricted “pure” mode in the Dev-VM.

### `terminate`

```json
{"cmd":"terminate"}
```

Requests termination of the currently running program.

- For **native** runs, Aura will attempt to kill the supervised child process.
- For **Dev-VM** runs, Aura will cooperatively cancel at the next statement boundary.

It is valid to send even when no native child is running (it becomes a no-op).

## Events

All events are `{"event": "..."}` with `camelCase` names.

### `hello`

```json
{
	"event": "hello",
	"protocol": 1,
	"capabilities": [
		"devvm.pause",
		"devvm.step",
		"devvm.breakpoints",
		"devvm.watches",
		"native.launch",
		"native.terminate",
		"native.exit",
		"perf.timeline",
		"perf.flame.folded",
		"perf.memory"
	]
}
```

### `stopped`

Emitted when the Dev-VM stops (pause/step/breakpoint).

### `perfReport`

Emitted at the end of a Dev-VM run when perf is enabled.

Current `memory` keys are best-effort runtime stats (not allocator-level), including:

- `values_total`: total number of live values tracked by the VM
- `values_int`: number of live integer values
- `values_bool`: number of live bool values
- `values_str`: number of live string values
- `values_style`: number of live style map values
- `style_entries`: total entries across all live style maps
- `string_bytes`: total UTF-8 bytes across live strings
- `callbacks`: number of live callback values
- `verify_cache_entries`: number of entries in the verifier cache (if present)
- `perf_total_ns`: total time measured for the run (ns)

### `nativeLaunch`

Emitted before launching a native executable.

### `nativeExit`

Emitted when a launched native executable exits.

### `terminated`

Emitted when Aura acts on a `terminate` request.

```json
{"event":"terminated","target":"devvm"}
```

## Notes / Limitations (current)

- Dev-VM stepping is statement-level (no step-in/over/out yet).
- Native debugging is hooks-only (launch/exit/terminate), not attach/stepping.
- Memory reporting is a live, reachable-value breakdown, not allocator-level tracking.
