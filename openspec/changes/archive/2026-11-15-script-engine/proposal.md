## Why

The script engine is the largest part of the port (89,668 lines of C#, with
454 `ll*` and 290 `os*` functions) and the hardest: a script must be
suspended mid-event, saved, and resumed in another region. The earlier plan
was to write a bytecode VM for this. Linden Lab has published one.

## What Changes

- Embed **SLua** (`secondlife/slua`, MIT, C++), Linden Lab's fork of Luau.
  Read from its README on 2026-11-13: a yielded script's execution state can
  be serialised with its global environment; many cheap forks of a base
  script run in one VM with isolated state; per-script memory limits; hooks
  for pre-emptive scheduling; the Luau JIT works, including serialising from
  inside JIT-compiled code; and LSL is executed on the same VM through
  **Tailslide** (MIT), which lives in the SLua tree.
- One Rust crate wraps SLua's C API (`script-vm`). A second (`script-api`)
  implements the script functions against the scene, once, for both
  languages.
- Function names, signatures and constants are generated from
  `secondlife/lsl-definitions` (MIT): `lsl_definitions.yaml` and
  `slua_definitions.yaml`.
- **Luau scripting comes with the VM.** Aurora Viewer's script code already
  refers to Luau and SLua (`aurora-net/src/scripts.rs`).
- `os*` functions are added as an OpenSimulator extension library available
  to both languages.
- YEngine is not ported.

## Impact

- Adds a C++ build (CMake) to the container image. The tree already needs a
  C toolchain for `openjpeg-sys`.
- Scripts are recompiled from source at migration. Saved YEngine script
  state cannot be read, so running scripts restart once when a region moves
  to the Rust simulator.
- LSL behaviour follows Second Life where it differs from OpenSimulator.
  YEngine's own language extensions are not supported.
- Risk: HIGH. Not yet established: how complete LSL-on-SLua is, whether
  SLua's C API exposes everything needed without patching, and how many
  existing OpenSimulator scripts depend on YEngine behaviour. The spike
  answers these before any commitment. The fallback is a Rust VM fed by
  Tailslide's parser, which keeps the generated API bindings.
