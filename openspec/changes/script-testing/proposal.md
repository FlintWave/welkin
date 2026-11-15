# Script Testing

**Status:** Ready for build (18 tasks across 4 phases)
**Priority:** Medium-high (validates the scripting stack against real content)
**Depends on:** lsl-compiler, script-engine (both in progress or complete)

## Summary

Build a test harness that runs real-world LSL scripts against Welkin's
compiler and runtime. The harness clones the Outworldz/LSL-Scripts collection
(1,079 projects, 1,796 scripts), selects 20 to 30 scripts across distinct
function categories, and validates them at two levels: offline compilation
through welkin-script-compiler, and live in-world execution via a
LibreMetaverse bot.

The output is a compatibility matrix showing which LSL function categories
work today, which need the remaining lsl-compiler builtins (11 tasks left),
and which need new work.

## Motivation

The scripting engine (18/18 tasks) and LSL-to-Luau compiler (58/69 tasks)
exist and pass unit tests, but no real user scripts have been run through
them. Unit tests verify individual builtins; this change verifies that
scripts people actually wrote for OpenSim compile and behave correctly on
Welkin. That gap matters because real scripts combine builtins in ways unit
tests don't cover, and they expose missing constants, edge cases in the
parser, and runtime interactions between subsystems.

## Test corpus

**Outworldz/LSL-Scripts** on GitHub. 1,079 projects, roughly 1,796 scripts,
about 600 contributors. Covers doors, vehicles, particle effects, vendors,
NPCs, animation controllers, teleporters, building tools, chat relays,
sensors, HTTP systems, and more. License permits free use with author credits
preserved.

Four smaller collections also exist as supplements if coverage gaps appear:
nuxy/LSL-OS-Library, peterhost/lsl-library, AvaCon/LSL-OpenSim-Script-Library,
sanchorelaxo/opensim-tools (OSSL coverage).

sLua/Lua has no usable OpenSim corpus. Only LSL scripts are testable today.

## Two testing layers

### Layer 1: Offline compilation

Feed selected scripts through welkin-script-compiler outside of a running
server. For each script, record:

- Whether it lexes and parses without error
- Whether the analyzer passes (type checking, scope resolution)
- Whether codegen produces Luau bytecode
- If it fails, which stage fails and why (missing builtin, unsupported
  syntax, type error, parser limitation)

This layer runs fast, needs no server, and can process the full corpus in
CI. It produces the raw compatibility numbers.

### Layer 2: Live runtime execution

Using the existing LibreMetaverse C# bot harness (7/7 tests already passing),
a bot logs in, rezzes a prim, uploads a script via the UpdateScriptAgent
capability, copies it into the prim's task inventory, and checks:

- The script compiles on the server (no compile error returned)
- The script starts (state_entry fires)
- Observable side effects occur where expected (chat output, timer events,
  sensor responses, prim property changes)

This layer validates the full pipeline: upload, compile, VM instantiation,
scheduler dispatch, and builtin execution in the context of a live region.

## Script selection strategy

Pick 20 to 30 scripts covering these function categories. Each category gets
at least one script, chosen for clarity and minimal external dependencies
(no scripts that require specific inventory items, external HTTP endpoints,
or multiple linked prims unless testing that category specifically).

| Category | Example script type | Key builtins exercised |
|----------|-------------------|----------------------|
| Chat | Chat relay, greeter | llSay, llListen, llWhisper, llShout |
| Timers | Periodic announcer | llSetTimerEvent, timer() |
| Sensors | Visitor detector | llSensor, llSensorRepeat, sensor() |
| Touch | Interactive object | touch_start, llDetectedName, llDetectedKey |
| Movement | Door, elevator | llSetPos, llSetRot, llMoveToTarget |
| Physics | Physical ball, cannon | llApplyImpulse, llSetBuoyancy, llSetPhysicsMaterial |
| Particles | Particle emitter | llParticleSystem |
| Prim params | Color changer, resizer | llSetPrimitiveParams, llGetPrimitiveParams |
| Inventory | Inventory giver | llGiveInventory, llGetInventoryName |
| Notecard | Config reader | llGetNotecardLine, llGetNumberOfNotecardLines |
| HTTP | Web fetcher | llHTTPRequest, http_response() |
| Email | Email sender | llEmail, llGetNextEmail |
| Vehicles | Simple car or boat | llSetVehicleType, llSetVehicleFloatParam |
| Animations | AO controller | llStartAnimation, llStopAnimation |
| Link messages | Multi-script object | llMessageLinked, link_message() |
| Textures | Texture cycler | llSetTexture, llSetTextureAnim |
| Permissions | Debit handler | llRequestPermissions, run_time_permissions() |
| Collision | Bump detector | collision_start, llDetectedName |

Scripts that depend on builtins in the remaining 11 lsl-compiler tasks
(email, HTTP requests, inbound HTTP URLs, rez, link prim params, attachments,
vehicles, position/rotation targets, agent data queries, notecard reading)
are still selected. They will fail at compilation, and those failures confirm
which remaining tasks need to land before that category works.

## Output

A compatibility matrix in Markdown, updated after each test run:

```
| Category     | Compiles | Runs | Notes                          |
|-------------|----------|------|--------------------------------|
| Chat         | yes      | yes  |                                |
| Timers       | yes      | yes  |                                |
| HTTP         | no       | n/a  | needs llHTTPRequest (task X.Y) |
| Vehicles     | no       | n/a  | needs llSetVehicleType (task X.Y) |
```

This matrix becomes the acceptance gate for the lsl-compiler change: when
all selected scripts compile and run, the scripting stack is validated
against real content.

## Dependencies

- **welkin-script-compiler** (58/69 complete): the LSL-to-Luau pipeline.
  Scripts that use unimplemented builtins will fail at compilation; those
  failures are expected and tracked.
- **welkin-script-vm** (18 builtin modules, scheduler, events): the runtime
  that executes compiled scripts.
- **LibreMetaverse bot harness**: for Layer 2 live testing. Already works
  (7/7 bot tests passing, including rez prim).
- **UpdateScriptAgent capability**: the server endpoint that accepts script
  uploads. Needs verification that it's wired up and calls into
  welkin-script-compiler.

## What this change does NOT include

- Fixing compiler bugs found by testing. Those fixes go back into the
  lsl-compiler change as new tasks or patches to existing code.
- Implementing the remaining 11 builtins. Those belong to lsl-compiler.
- Testing sLua/Lua scripts (no corpus exists).
- Performance benchmarking of the script VM. That's a separate concern.

## Risks

The UpdateScriptAgent capability may not be fully wired to the Luau compiler
pipeline yet. If the upload path doesn't reach welkin-script-compiler, Layer 2
testing is blocked until that integration is done. Layer 1 (offline
compilation) works regardless.

Some Outworldz scripts may use OSSL functions (OpenSim-specific extensions)
that aren't in the LSL standard. The harness should detect and tag these
separately so they don't pollute the LSL compatibility numbers.

## Next steps

1. Design doc: harness architecture, CI integration, test runner structure
2. Task breakdown: clone corpus, build selection tool, build Layer 1 runner,
   build Layer 2 runner, generate matrix, CI hookup
3. Build and run
