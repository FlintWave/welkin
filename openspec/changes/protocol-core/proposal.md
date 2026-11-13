## Why

Everything a viewer sees passes through the Linden protocol: LLUDP messages,
LLSD documents and HTTP capabilities. The C# server gets these from
libopenmetaverse (`OpenMetaverse*.dll`) and from `Region/ClientStack` (30,923
lines). They must exist in Rust before any service or simulator can be tested
against Aurora Viewer.

## What Changes

- Reuse from Aurora Viewer (GPL-3.0-or-later, pinned git dependency), each
  behind a thin wrapper crate so its changing API touches one place:
  - `aurora-llsd` for LLSD,
  - `aurora-msg` for packet framing, zero-coding and typed messages,
  - `aurora-prim` for prim parameters, texture entries and extra parameters.
- An inventory of those crates from the server's side. Aurora is a viewer, so
  some paths exist in one direction only: `aurora-net` decodes object updates
  that the server must encode, for example. Each gap is filled in this
  workspace and offered upstream.
- `types`: region handles, permission masks, asset and inventory type enums
  and whatever the Aurora crates do not define. Vector and quaternion types
  are `glam`, as in Aurora, with conversion at the physics boundary.
- `net`: the server side of the LLUDP circuit: sequence numbers, reliable delivery and resend,
  acknowledgements, ping, throttles by category, and packet splitting, with
  the behaviour of `LLUDPServer` and `LLUDPClient`.
- XML-RPC encoding for login and the legacy service calls that use it.
- A test bot built on `aurora-net`, the viewer's own client code, so tests
  exercise the server with the same requests Aurora Viewer makes.

## Impact

- Replaces libopenmetaverse. The message codec and LLSD are no longer
  written from scratch.
- Depends on `port-foundation`.
- The Aurora crates are internal to a viewer, at version 0.1.0, and changed
  on the day this was written. Updating the pinned commit is a reviewed
  change with the replay tests as its gate.
- Risk: MEDIUM. Aurora Viewer matches the reference viewer's request rates
  and message use, so throttle and resend behaviour must match the C# server
  closely or the viewer will stall or flood.
