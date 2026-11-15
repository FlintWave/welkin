# RealmWalker

**Status:** Ready for build (37 tasks across 6 phases)
**Priority:** High (enables automated testing pipeline)
**Depends on:** welkin-net, welkin-msg, welkin-types (all exist)

## Summary

RealmWalker is a Rust-native bot and client SDK for interacting with
OpenSimulator-compatible grids. It replaces the C# LibreMetaverse library as the
primary tool for automated testing, bot scripting, and programmatic grid
interaction in the Welkin ecosystem.

The name is one word, chosen by the project owner. The crate lives in the Welkin
workspace as `welkin-realmwalker` and publishes to crates.io as `realmwalker`.

RealmWalker is both part of Welkin and a standalone product. Development happens
in the Welkin monorepo workspace for shared CI and protocol crate access, but it
publishes independently to crates.io with its own README, documentation, and
crate page. Its transitive dependencies (welkin-net, welkin-msg, welkin-types)
also publish to crates.io so downstream users pull everything automatically.
Anyone can use RealmWalker against any OpenSim-compatible grid without needing
the rest of Welkin.

## License

LibreMetaverse (cinderblocks/libremetaverse on GitHub) is BSD 3-Clause. Porting
its API design and logic to Rust and releasing under Apache 2.0 is
license-compatible. The only restriction: the derived work cannot use the
"LibreMetaverse" name to endorse or promote itself.

RealmWalker is Apache 2.0, consistent with the rest of Welkin.

## Architecture

RealmWalker sits on top of the existing protocol crates:

```
realmwalker (high-level bot API)
    ├── welkin-net   (LLUDP transport, circuit management, acks)
    ├── welkin-msg   (message definitions, serialization)
    ├── welkin-types (UUIDs, vectors, quaternions, common types)
    ├── tokio        (async runtime)
    └── reqwest      (HTTP for XMLRPC login, caps, asset transfers)
```

The SDK exposes an async `GridClient` (or similar top-level struct) that manages
one agent session. Multiple clients can run concurrently in the same process for
multi-bot scenarios.

### Core modules

| Module | Responsibility |
|--------|---------------|
| `login` | XMLRPC and LLSD login flows, credential handling, grid URI resolution |
| `network` | Circuit lifecycle, packet dispatch, ack management (delegates to welkin-net) |
| `agent` | Movement, teleport, sit/stand, fly, animation triggers |
| `chat` | Local chat, whisper, shout, instant messages, group messages |
| `objects` | Rez, derez, edit properties, select/deselect, link/unlink |
| `inventory` | Folder/item fetch, give/receive, create, move, delete |
| `appearance` | Bake request, wear/detach, rebake, visual params |
| `assets` | Fetch by UUID, upload (mesh, texture, script, notecard) |
| `grid` | Region info queries, grid map, neighbor lookup |
| `events` | Async event stream (tokio broadcast channels) for all inbound activity |

### Event system

All inbound events (chat received, object update, teleport offer, inventory
offer, etc.) flow through a unified async event stream. Consumers select events
by type using pattern matching on an `Event` enum. This replaces
LibreMetaverse's C# delegate/callback model with idiomatic Rust async.

```rust
let mut events = client.events();
while let Some(event) = events.recv().await {
    match event {
        Event::Chat(msg) => println!("{}: {}", msg.from_name, msg.message),
        Event::TeleportOffer(offer) => offer.accept().await?,
        _ => {}
    }
}
```

## What to port

From LibreMetaverse's public API surface:

- GridClient / login flow: XMLRPC authentication, session establishment,
  seed capability fetch, circuit setup
- AgentManager: movement packets, autopilot, teleport, sit, fly, crouch,
  animation overrides
- ChatManager: say, whisper, shout on channels; IM send/receive; group
  chat join/send
- ObjectManager: rez from inventory, create new, edit properties (name,
  description, permissions, flags), select, derez, take, link sets
- InventoryManager: folder tree fetch, item details, give to agent, receive
  offers, create folders/items, trash, purge
- AppearanceManager: bake layers, set visual params, wear/detach
  attachments, rebake on demand
- AssetManager: fetch by UUID and type, upload via caps (mesh, texture,
  animation, sound, script, notecard)
- GridManager: region name/handle lookups, map block requests, neighbor info
- EventQueueGet: long-poll event queue (caps-based), parse and dispatch
  structured events

## What NOT to port

- Rendering, UI, or viewer logic. RealmWalker is headless: no OpenGL, no
  window management, no scene graph.
- Platform-specific code: no Windows registry access, no macOS keychain, no
  platform media plugins.
- Voice (Vivox/WebRTC). Out of scope for a bot SDK.
- HTTP texture pipeline internals. Asset fetch covers it; the viewer-side
  decode and cache layer stays out.
- LibreMetaverse's internal threading model. Replaced by tokio async.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `welkin-net` | LLUDP transport, packet framing, circuit acks |
| `welkin-msg` | Message type definitions and serialization |
| `welkin-types` | Shared types (UUID, Vector3, Quaternion, etc.) |
| `tokio` | Async runtime, channels, timers |
| `reqwest` | XMLRPC login, capability HTTP, asset upload/download |
| `serde` / `serde_json` | Structured data for caps responses |
| `quick-xml` | XMLRPC and LLSD-XML parsing |

No new external protocol libraries required. The existing crates already handle
LLUDP and message serialization.

## Use cases

1. Automated testing: a bot logs in, walks, chats, rezzes objects, and verifies
   server responses. Runs in CI with no viewer installed.
2. Grid monitoring: a bot sits in-world and reports region health, script time,
   agent counts, physics stats.
3. NPC automation: scripted NPCs that greet visitors, give inventory, guide
   tours, respond to chat.
4. Load testing: spin up N bots in one process, ramp concurrent agents to
   stress-test the region.
5. Third-party integrations: Discord/Slack bridge bots, web dashboard agents,
   external service connectors.
6. CI test harness: the Welkin test suite uses RealmWalker to run end-to-end
   integration tests against a live server.

## Relationship to LibreMetaverse

RealmWalker replaces LibreMetaverse for Rust-native use. LibreMetaverse C# bot
tests continue to run as a separate compatibility check, verifying that
third-party C# bots work against Welkin. Both test paths matter:

- RealmWalker tests validate Welkin's protocol implementation through our
  own networking stack.
- LibreMetaverse tests validate that an independent C# client (the standard
  in the OpenSim ecosystem) can interoperate with Welkin.

If RealmWalker tests pass but LibreMetaverse tests fail, we have a
compatibility bug. If both pass, we have confidence.

## Next steps

1. Design doc: module boundaries, public API surface, error handling
   strategy, connection state machine.
2. Task breakdown: sized tasks per module, ordered by dependency
   (login first, then agent/chat, then objects/inventory, then appearance/assets).
3. Build: implement per the task list, commit per module, tests per module.
4. Integration: wire RealmWalker into the Welkin CI pipeline as the primary
   bot test harness.
