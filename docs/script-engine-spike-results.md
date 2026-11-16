# Script Engine Spike Results

**Date:** 2024-11  
**Spike goal:** Determine whether Luau (via mlua 0.12) is a viable runtime for
LSL script execution in Welkin, and identify the work needed to make it
production-ready (spike document; see the README for current status).

---

## Spike task results

### 1.1 -- mlua + Luau basic capabilities: PASS

mlua 0.12 with the `luau` feature (wrapping upstream Luau 740) works correctly.
Eight tests confirm:

| Test | Result |
|------|--------|
| Compile / load / run Luau source | Pass |
| Type annotations | Pass (Luau-native, ignored at runtime) |
| Coroutine yield and resume | Pass |
| Rust functions callable from Lua | Pass |
| Sandbox (no io/os/debug) | Pass |
| Table creation and manipulation | Pass |
| Multiple independent scripts | Pass (separate `Lua` instances or `Scope`s) |
| Resume coroutine with arguments | Pass |

**Conclusion:** mlua + Luau provides a solid, well-tested scripting runtime
with the features we need.

### 1.2 -- Thread/coroutine serialization: PARTIAL (workaround found)

**Finding:** Luau does not expose coroutine stack internals.  There is no API
to serialize a suspended coroutine's stack frames and resume from a byte
buffer.  This means we cannot implement YEngine-style mid-function state
capture.

**Workaround -- checkpoint pattern:**
- `ScriptState` saves: compiled bytecode + global variables as JSON.
- Between events, the runtime serializes globals; on restore it recompiles from
  cached bytecode and reinjects globals.
- For long-running operations that must survive sim restarts, scripts use
  explicit checkpoint yields (an `os*` extension function).
- This is sufficient because >99% of LSL event handlers are short-lived (run
  to completion within a single time slice).

**Impact:** Scripts that call `llSleep()` or perform very long loops cannot be
frozen and migrated mid-execution.  They must either complete or be restarted.
This is an acceptable trade-off; SL itself moved away from mid-function
migration in Mono-based script engines.

### 1.3 -- LSL corpus analysis: COMPLETE

Analysed 18 LSL scripts (780 lines) from the OpenSimulator asset set.  10
unique scripts exercise 8 event types, 35 `ll*` functions, and 2 `os*`
functions.  Full analysis in `docs/lsl-corpus-analysis.md`.

**Key findings for the transpiler:**

- **Trivial to transpile** (~40% of constructs): basic control flow, user
  functions, arithmetic, string concatenation, comments, constants.
- **Medium complexity** (~45%): state machine structure, type system mapping
  (especially `vector`/`rotation`/`key`/`list`), C-style casts, angle-bracket
  and square-bracket literal syntax, global variables, key-as-boolean
  truthiness.
- **Careful handling needed** (~15%): state transitions with event queue
  flush, implicit type coercions, integer overflow wrapping semantics,
  detected-event context functions.

The transpiler is a well-scoped piece of work: the LSL language is small
(~50 keywords, ~7 types, C-like syntax) and the mapping to Luau is largely
mechanical.

### 1.4 -- YEngine pipeline comparison: COMPLETE

Studied the YEngine compilation pipeline (tokenizer -> parser/reducer -> CIL
code generator -> runtime).  Full comparison in `docs/lsl-corpus-analysis.md`.

**Key takeaways:**

1. YEngine compiles LSL directly to CIL bytecode (~5600 lines of code
   generator).  We replace this with a transpiler (LSL -> Luau source) plus
   Luau's own compiler (Luau source -> bytecode).  Net effect: we write a
   transpiler (~1500-2500 lines estimated) instead of a compiler backend.

2. YEngine's preemption (`CheckRun()` inserted at back-edges) maps directly
   to Luau's `set_interrupt` mechanism, which fires at the same points.

3. YEngine's state capture (full CIL stack serialization) is the one feature
   we cannot replicate exactly.  Our checkpoint pattern is the pragmatic
   replacement.

4. YEngine's state/event handler table (SEHT) maps to our Luau module
   structure: each state is a table of `{event_name = handler_function}`.

### 1.5 -- Memory limits and preemption: PASS

| Feature | API | Behaviour |
|---------|-----|-----------|
| Memory limit | `Lua::set_memory_limit(bytes)` | Luau tracks allocations natively; exceeding the limit raises an error |
| Preemption | `Lua::set_interrupt(callback)` | Callback fires at function calls and loop back-edges; can yield or error |

Both mechanisms work as documented.  Memory limits are per-`Lua` instance,
giving us per-script isolation.  The interrupt callback can enforce time slices
(count instructions or check wall-clock) and yield the script cooperatively.

### 1.6 -- Luau support in upstream OpenSimulator / Aurora: NONE

Neither OpenSimulator 0.9.3 nor the Aurora-Sim codebase has any Luau support.
Their script engines are:

- **YEngine** (OpenSim): LSL -> CIL, as studied in 1.4.
- **XEngine** (OpenSim legacy): LSL -> C# source -> .NET compiler -> CIL.
- **Aurora ScriptEngine**: similar pipeline to XEngine.

Language detection in Welkin would be server-side at compile time:
inspect the first line for a `//!luau` directive, or assume LSL and transpile.

---

## Go / No-Go recommendation

### GO -- with caveats

The spike confirms that Luau via mlua is a viable and attractive runtime for
LSL script execution in Welkin.

**Strengths:**

1. **Mature VM**: Luau (developed by Roblox) is battle-tested at massive scale
   with excellent sandboxing, memory tracking, and performance.
2. **Good Rust integration**: mlua 0.12 provides a safe, ergonomic Rust API
   with full support for Luau-specific features.
3. **Preemption parity**: `set_interrupt` provides equivalent preemption to
   YEngine's `CheckRun()` mechanism.
4. **Memory isolation**: per-instance memory limits give us the same safety as
   YEngine's heap tracking.
5. **Simpler architecture**: transpiling to Luau lets us use the Luau
   compiler and VM instead of writing and maintaining a CIL backend.

**Caveats (required work):**

1. **LSL-to-Luau transpiler**: must be written.  Estimated 1500-2500 lines of
   Rust.  The LSL language is small and well-specified; the mapping is largely
   mechanical.  This is the critical-path deliverable.

2. **Host function stubs**: ~400 `ll*` functions and ~200 `os*` functions must
   be implemented as Rust functions registered in mlua.  These can be
   stubbed incrementally (return defaults, log calls) and fleshed out as
   simulator subsystems come online.

3. **Checkpoint-based state persistence**: replaces YEngine's full stack
   serialization.  Adequate for the event-driven LSL execution model.
   Scripts that call blocking functions (`llSleep`, `llHTTPRequest` callback
   waits) will resume from the start of the next event, not mid-function.

4. **Source maps for error reporting**: transpiled Luau line numbers won't
   match LSL source lines.  A simple line-mapping table emitted during
   transpilation will provide acceptable diagnostics.

5. **Integer overflow semantics**: Luau uses f64 for all numbers.  Integer
   wrapping must be enforced by the transpiler (emit `bit32.band` wrappers)
   or the runtime (check on assignment to integer-typed variables).

**Risks (low):**

- **Luau language evolution**: Luau is actively developed; API changes could
  require mlua updates.  Mitigated by pinning mlua version.
- **Performance**: transpiled Luau may be slower than native CIL for
  compute-heavy scripts.  Mitigated by Luau's JIT-like optimisations and the
  fact that most LSL scripts are I/O-bound (waiting for events).

### Recommended next steps

1. **Write the LSL tokenizer/parser** in Rust (can reference YEngine's
   `MMRScriptTokenize.cs` and `MMRScriptReduce.cs` for the grammar).
2. **Write the Luau emitter** that walks the AST and outputs Luau source.
3. **Build the runtime harness**: event queue, state machine dispatcher,
   preemption interrupt, memory limits.
4. **Stub the LSL API**: start with the 35 functions found in the corpus,
   expand from there.
5. **Integration tests**: transpile and run the 18-script corpus; compare
   output against expected behaviour.
