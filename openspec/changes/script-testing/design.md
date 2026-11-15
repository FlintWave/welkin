# Script Testing: Design

## Harness architecture

The test harness has two independent runners that share a common script
inventory and output format.

```
tests/script-testing/
├── scripts/              # Committed: 20-30 selected .lsl files
├── corpus/               # Gitignored: full Outworldz clone
├── scanner/              # Rust binary: scans corpus, reports builtin usage
├── runner/               # Rust binary: Layer 1 offline compilation
├── results/              # JSON output from both layers
│   ├── layer1.json
│   └── layer2.json
└── matrix.md             # Generated compatibility matrix
```

Layer 1 (offline compilation) is a standalone Rust binary that calls into
welkin-script-compiler as a library. It needs no running server and produces
results in seconds.

Layer 2 (live runtime) extends the existing LibreMetaverse C# bot harness
with a script-upload test sequence. It needs a live Welkin server and runs
manually or under an integration test flag.

Both layers write JSON results in the same schema. A small generator reads
those JSON files and produces the Markdown compatibility matrix.

## Layer 1: Offline compilation runner

The runner binary lives at `tests/script-testing/runner/` and depends on
`welkin-script-compiler` as a library crate.

For each `.lsl` file in the scripts directory, the runner:

1. Reads the file contents
2. Runs the lexer. Records pass/fail. On failure, captures the error span
   and message.
3. On lex success, runs the parser. Records pass/fail with error details.
4. On parse success, runs the analyzer (type checking, scope resolution,
   builtin resolution). Records pass/fail. On failure, captures which
   builtin or type was unresolved.
5. On analysis success, runs codegen (LSL AST to Luau source). Records
   pass/fail.

Output per script:

```json
{
  "file": "greeter.lsl",
  "category": "chat",
  "ossl": false,
  "stages": {
    "lex": { "pass": true },
    "parse": { "pass": true },
    "analyze": { "pass": false, "error": "unknown builtin: llHTTPRequest" },
    "codegen": { "pass": null }
  },
  "failure_class": "missing_builtin",
  "missing_builtins": ["llHTTPRequest"]
}
```

Failure classes:

- `missing_builtin`: the analyzer could not resolve an `ll*` or `os*` call
- `parse_error`: syntax the parser does not handle
- `type_error`: the analyzer rejected a type mismatch or scope violation
- `codegen_error`: the AST-to-Luau translation failed
- `unsupported_syntax`: valid LSL that the compiler does not yet support

## Layer 2: Live runtime runner

The Layer 2 runner uses the LibreMetaverse C# bot project at
`tests/bot-tests/`. It adds a new test class (or extends the existing one)
with a script-upload sequence:

1. Bot logs in (already working, 7/7 tests passing)
2. For each script that passed Layer 1 compilation:
   a. Rez a fresh prim (ObjectAdd, already proven)
   b. Upload the script via the UpdateScriptAgent capability
   c. Check the server response for compilation success or error
   d. If compiled, copy the script into the prim's task inventory
   e. Wait a configurable timeout (default 5 seconds) for observable effects
   f. Record what happened

Observable effect checks vary by category:

| Category | What to check |
|----------|--------------|
| Chat | Listen on the expected channel for llSay/llWhisper/llShout output |
| Timers | Wait for repeated chat output on a timer interval |
| Sensors | Position the bot near the prim, check for sensor-triggered chat |
| Touch | Send a touch_start event, check for response |
| Particles | Verify ParticleSystem packet received (prim has active particles) |
| Movement | Check ObjectUpdate for position change after script starts |
| Permissions | Verify the permission request dialog arrives |

Scripts with no observable side effects (or side effects that need external
services like HTTP endpoints) are tested only for "compiles and starts" in
Layer 2. The result records whether state_entry fired by checking for any
script activity within the timeout window.

## Script scanner

A small Rust utility at `tests/script-testing/scanner/` that reads the full
Outworldz corpus and produces a frequency report of builtin usage.

The scanner:

1. Walks all `.lsl` files in the corpus directory
2. Extracts function calls matching `ll[A-Z]\w+` and `os[A-Z]\w+` via regex
3. Counts calls per function across the corpus
4. Groups functions by category (chat, timer, sensor, physics, etc.) using
   a hardcoded mapping table derived from the LSL wiki function list
5. Flags scripts that use `os*` functions as OSSL

Output: a JSON report with function frequencies and per-script builtin lists.
This report drives script selection: pick scripts that exercise the most
common builtins in each category, prefer scripts with fewer external
dependencies (no llHTTPRequest to remote servers, no specific inventory items).

## OSSL detection

Scripts that call `os*` functions (osNpcCreate, osSetDynamicTextureURL,
osTeleportAgent, etc.) are OpenSim-specific extensions. The scanner tags
these scripts with `"ossl": true`. They are tracked separately in the
compatibility matrix so OSSL failures don't pollute the LSL compatibility
numbers.

A script that mixes LSL and OSSL calls gets tagged as OSSL. If the LSL
portions are otherwise interesting, a second copy with the OSSL calls
commented out can be added to the selection.

## CI integration

Layer 1 runs as a cargo binary or test:

```
cargo run -p welkin-script-test-runner -- --scripts tests/script-testing/scripts/
```

Or gated behind a feature flag in the test suite:

```
cargo test -p welkin-script-test-runner --features ci
```

The runner exits with a non-zero status if any script that previously compiled
now fails (regression detection). New failures in scripts that were already
failing are not regressions.

Layer 2 does not run in CI. It requires a live server and runs as part of
manual integration testing or a dedicated integration test job with a server
container.

## Compatibility matrix generation

A small function (in the runner binary or a separate script) reads the JSON
results from both layers and produces a Markdown table:

```markdown
| Category    | Scripts | Compile | Run  | Blocked by                        |
|-------------|---------|---------|------|-----------------------------------|
| Chat        | 3       | 3/3     | 3/3  |                                   |
| Timers      | 2       | 2/2     | 2/2  |                                   |
| HTTP        | 2       | 0/2     | n/a  | llHTTPRequest (lsl-compiler #X.Y) |
| Vehicles    | 1       | 0/1     | n/a  | llSetVehicleType (lsl-compiler)   |
```

The "Blocked by" column links each failure to a specific lsl-compiler task
when the failure class is `missing_builtin`. This makes the matrix a direct
input for prioritizing the remaining 11 lsl-compiler tasks.
