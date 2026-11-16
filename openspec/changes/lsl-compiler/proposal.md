# LSL Compiler — Proposal

## Problem

Welkin has a working Luau VM (mlua), a ScriptScheduler with round-robin and
instruction limits, 67 real ll* builtins, and state serialization. But there
is no way to compile LSL source into Luau bytecode. The `welkin-script-compiler`
crate contains only a type system (200 lines) — no parser, no AST, no
transpiler.

Every object in OpenSim can contain LSL scripts. Without a compiler, no
user-created scripts run. This is the single largest remaining gap for
OSgrid parity.

## Scope

Build a complete LSL-to-Luau compilation pipeline and wire it into the
region server so scripts execute when objects are rezzed or edited.

### In scope

1. **LSL Lexer/Tokenizer** — all LSL tokens (types, operators, keywords,
   string/key/vector/rotation literals, state/event syntax)
2. **LSL Parser → AST** — full LSL grammar including: states, events, all
   types (integer, float, string, key, vector, rotation, list), control
   flow (if/else, for, while, do-while, jump/label), function declarations,
   global variables, implicit casts
3. **LSL → Luau Transpiler** — AST to Luau source code generation:
   - State machine emulation (current_state variable, event dispatch table)
   - Type coercions (LSL implicit casts: integer→float, string→key, etc.)
   - Operator overloading for vector/rotation math
   - LSL list semantics (heterogeneous, stride operations)
   - LSL string semantics (concatenation with +, type coercions in concat)
   - Default parameter values and return values per LSL spec
4. **Region integration** — wire ScriptScheduler into region main loop:
   - Compile scripts on rez (RezObject, RezScript)
   - Compile scripts on edit (UpdateScriptAgent cap)
   - Fire scene events → script events (touch_start, collision, etc.)
   - Timer dispatch from scheduler
   - Listen/chat dispatch
   - Script state save/restore on region crossing
5. **Remaining ll* builtins** — implement the ~387 missing ll* functions
   with real behavior (not stubs)
6. **os* stub replacement** — replace ~141 os* stubs with real
   implementations

### Out of scope

- Mono/CIL compilation (OpenSim's YEngine path — we use Luau instead)
- Visual scripting or node-based scripting
- LSL preprocessor (#include — not standard LSL)
- Experience tools API (Linden-only)

### Reference

- OpenSim ScriptEngine: 89,668 lines C# (YEngine compiler, runtime)
- LSL spec: wiki.secondlife.com/wiki/LSL_Portal
- Luau spec: luau-lang.org
- Existing Welkin code: welkin-script-compiler (200 lines), welkin-script-vm
  (11,535 lines), welkin-script-api (1,894 lines)
