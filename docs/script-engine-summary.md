# script-engine summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md#v1) · Previous: [Prim meshing](prim-meshing-summary.md) · Next: [OSSL API](ossl-api-summary.md)

Status: accepted (all tracked tasks checked in `openspec/changes/script-engine/tasks.md`).
Commits `cdb1913` through `b57c2db`. Second review verdict: CONDITIONAL PASS.
Tests: 172 in `opensim-script-vm`.

## What was built

Scripts run on Luau through mlua 0.12 (`luau` feature, upstream Luau 740), one
VM per script. LSL reaches the VM by transpiling to Luau (see go/no-go below).
All code lives in `crates/opensim-script-vm`; `opensim-script-api` and
`opensim-script-compiler` are still skeletons.

### Spike (`cdb1913`, `3fb1cbd`, `d413580`)

- **VM spike:** compile, load, run and yield from Rust (8 tests).
- **State serialization:** `ScriptState` holds bytecode plus globals as JSON and
  round-trips into a second VM. Luau cannot snapshot a coroutine stack, so
  scripts persist through a checkpoint pattern (state lives in globals, the
  event handler is restarted).
- **Memory limits and pre-emption:** `new_vm_with_memory_limit` uses the
  allocator hook; an interrupt callback (`VmState`) stops runaway scripts.
- **LSL corpus analysis** (`docs/lsl-corpus-analysis.md`): 18 scripts from OAR
  files, 10 unique, with a feature inventory and transpiler complexity breakdown.
- **YEngine comparison:** the 4-stage CIL pipeline analysed against the Luau
  approach (`docs/script-engine-spike-results.md`).
- **Go/no-go: GO.** Caveats: an LSL-to-Luau transpiler is needed, persistence is
  checkpoint based, about 400 host functions need implementing.
- **Luau marking on upload:** neither OpenSim 0.9.3 nor the Aurora crates have
  Luau support; language detection would be server-side at compile time.

### VM crate (`6efdaef`)

- `ScriptInstance`: one Luau VM, bounded memory, event queue and dispatch.
- `ScriptScheduler`: round-robin with an instruction limit per slice, timer
  heap, `post_event` and `broadcast_event`.
- `RegionTransferBundle`: serialises scheduler scripts to bytes and applies them
  in another scheduler, for region crossing.

### Script API (`5357247`, `60241c6`, `a5539f5`, `ee60389`)

- Generated function tables, constants, events and the `LslType`/`LslValue`
  type system (100+ functions, 120+ constants, 35 events).
- **82 `ll*` functions** over a shared `ScriptContext` (`Arc<RwLock>`): 49
  implemented, 33 stubs. Areas: math, string, list, object, physics,
  communication (`builtins/`).
- **27 `os*` functions** (`ossl.rs`) gated by `OsslThreatLevel`: None, Nuisance,
  VeryLow, Low, Moderate, High, VeryHigh, Severe. `OsslConfig` sets the maximum
  allowed level and a per-function allow override.
- **Events and detected parameters** (`events.rs`): 16 `llDetected*` functions,
  `DetectedInfo`, `EventContext`, `dispatch_event`.

### Proof tests (`3d0e331`)

- 32 tests ported from the C# `LSL_Types` and `LSL_Api` suites (math, string,
  list, constants, conversions, compile API).
- 5 tests: vehicle script state survives a cross-region transfer.
- 6 tests: a Luau script and an LSL script in one object exchange link
  messages, including broadcast and survival across transfer.

## Security review (`dd1177d`)

`docs/security-review-script-sandboxing.md`. CONDITIONAL PASS: 0 CRITICAL,
2 HIGH.

- **H1:** `ScriptInstance` never calls `sandbox(true)` after registering
  functions (`new` and `restore_from`), so globals are not frozen.
- **H2:** the event queue is unbounded. Needs a cap (C# uses 300 per script).
- MEDIUM: `restore_script_state` returns an unsandboxed VM; transfer bytecode
  has no version check; transfer bundle size and `globals_json` size are
  uncapped, and the memory limit is applied after globals are injected.

Strengths: one VM per script, no `io`/`os`/`debug`, no network or file
functions registered.

## Known gaps

- The `os*` functions are stubs with threat-level gating (no inventory or file I/O yet), and 33 `ll*` functions are stubs (inventory, HTTP, sensors, permissions, land and
  others). Roughly 370 of the 454 C# `ll*` functions are not bound yet.
- A coroutine stack cannot be serialised; mid-event state is lost on transfer
  unless the script uses the checkpoint pattern.
- The LSL-to-Luau transpiler (`opensim-script-compiler`) is not written.
- No scene integration: `ScriptContext` is not yet wired to `opensim-scene`.
- Both HIGH findings must be fixed before production use.
- YEngine script state cannot be read; scripts restart once at migration.
