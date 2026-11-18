# Script Testing: Tasks

18 tasks across 4 phases. Each task produces a concrete, testable artifact.

## Phase 1: Corpus and tooling

- [x] 1.1 Clone Outworldz/LSL-Scripts into `tests/script-testing/corpus/`, add the directory to `.gitignore`
- [x] 1.2 Build the script scanner: a Rust binary at `tests/script-testing/scanner/` that reads `.lsl` files, extracts `ll*` and `os*` function calls via regex, groups by category using a hardcoded LSL function-to-category mapping, flags OSSL scripts
- [x] 1.3 Run the scanner on the full corpus, produce a JSON frequency report showing call counts per builtin and per category
- [x] 1.4 Select 20 to 30 scripts based on the frequency report: at least one per category from the proposal table (chat, timers, sensors, touch, movement, physics, particles, prim params, inventory, notecard, HTTP, email, vehicles, animations, link messages, textures, permissions, collision), prefer scripts with minimal external dependencies
- [x] 1.5 Copy selected scripts to `tests/script-testing/scripts/` (committed to the repo), with a `manifest.json` listing each script's file, category, expected builtins, and whether it uses OSSL

## Phase 2: Layer 1 (offline compilation)

- [x] 2.1 Build the Layer 1 test runner: a Rust binary at `tests/script-testing/runner/` that depends on `welkin-script-compiler`, loads each `.lsl` file from the scripts directory, runs it through lex/parse/analyze/codegen stages, records pass/fail per stage with error details
- [x] 2.2 Define the JSON output schema for results (file, category, ossl flag, per-stage pass/fail, failure class, missing builtins list)
- [x] 2.3 Run Layer 1 on all selected scripts, produce `results/layer1.json`
- [x] 2.4 Build the matrix generator: reads `layer1.json` (and later `layer2.json`), produces `matrix.md` with columns for category, script count, compile status, run status, and blocker notes linking to lsl-compiler tasks
- [x] 2.5 Categorize all compilation failures by root cause (missing builtin, parse error, type error, unsupported syntax), update the matrix with failure counts per class

## Phase 3: Layer 2 (live runtime)

- [ ] 3.1 Extend the LibreMetaverse C# bot harness with a `ScriptUploadTest` class: rez prim, upload a `.lsl` file via the UpdateScriptAgent capability, read the server's compile response
- [ ] 3.2 Add observable-effect checks: listen on chat channels for script output, detect timer-driven repeated messages, verify prim property changes via ObjectUpdate, check for particle system packets
- [ ] 3.3 Run Layer 2 on all scripts that passed Layer 1 compilation, produce `results/layer2.json` in the same schema (with an added `runtime` field for observable-effect results)
- [ ] 3.4 Update the compatibility matrix with Layer 2 results (compile-on-server status, runtime status, observed effects)

## Phase 4: CI and reporting

- [ ] 4.1 Wire Layer 1 into CI: the runner binary exits non-zero on regressions (a script that previously compiled now fails), exits zero when only known failures remain
- [ ] 4.2 Add a regression baseline file (`results/baseline.json`) that records the current pass/fail state, so future runs detect regressions against it
- [ ] 4.3 Generate the final compatibility matrix with notes linking each `missing_builtin` failure to the specific lsl-compiler task that would fix it
- [ ] 4.4 Write a one-page findings summary: which categories work end-to-end, which are blocked on the remaining 11 lsl-compiler builtins, which need work beyond those 11 tasks, and any parser or codegen bugs found
