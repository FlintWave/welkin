## 1. Spike (needs no simulator)

- [x] 1.1 Build SLua in the container image and call it from Rust: compile, load, run, yield (mlua 0.12 + luau feature, 8 tests passing)
- [x] 1.2 Serialise a script suspended mid-event, restore it in a second process, and continue (3fb1cbd: ScriptState bytecode+globals JSON round-trip; Luau cannot snapshot coroutine stack — checkpoint pattern documented)
- [ ] 1.3 Compile an LSL corpus taken from OAR files; report what fails to compile
- [ ] 1.4 Run the corpus with stubbed functions on SLua and on YEngine; compare event and call traces
- [x] 1.5 Confirm memory limits and pre-emption stop a runaway script (3fb1cbd: new_vm_with_memory_limit + VmState interrupt callback)
- [x] 1.6 Check with Aurora Viewer how a script is marked as Luau on upload (finding: no Luau support in OSim 0.9.3 or Aurora crates; language detection would be server-side at compile time)
- [ ] 1.7 Write up results and record go or no-go in this change

## 2. VM crate

- [ ] 2.1 Safe Rust wrapper over SLua's C API; SLua pinned as a git submodule
- [ ] 2.2 Scheduler: event queues, time slices, sleep, timers
- [ ] 2.3 State save, restore and region-crossing transfer

## 3. Script API

- [ ] 3.1 Generate function tables and constants from `lsl-definitions`
- [ ] 3.2 Implement `ll*` functions by area: math and strings, lists, objects and prims, physics, communication, inventory, HTTP, sensors, permissions, land
- [ ] 3.3 `os*` extension library
- [ ] 3.4 Events and detected parameters

## 4. Proof

- [ ] 4.1 Port the script API tests from the C# tree
- [ ] 4.2 A scripted vehicle crosses a region border and keeps running
- [ ] 4.3 A Luau script and an LSL script in one object exchange link messages
- [ ] 4.4 Second review of sandboxing: memory, CPU, HTTP and inventory access
