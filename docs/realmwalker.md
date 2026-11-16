<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

# RealmWalker bot SDK

[← Back to docs](index.md) · [Script engine](scripting.md) · [Viewer compatibility](viewer-compatibility.md) · [Architecture](architecture.md)

RealmWalker is a Rust-native bot and client SDK for OpenSimulator-compatible
grids. It provides async login, UDP circuit management, chat, movement, and
object interaction through a single `RealmWalker` struct. The crate lives at
`crates/welkin-realmwalker/` and is licensed under Apache 2.0, ported from
LibreMetaverse (BSD 3-Clause).

## Current status

Phases 1 and 2 are complete (commit `c4deaa2`, 84 tests, 21/80 tasks done).
Phases 3 through 11 (59 tasks) cover inventory, appearance, multi-bot stress
testing, friends, groups, terrain, and more. See [what's coming](#whats-coming)
for the full roadmap.

## Capabilities

### Phase 1: login, circuit, chat

- XMLRPC login with MD5 password hashing
- UDP circuit establishment (UseCircuitCode handshake, ack loop, ping keepalive)
- Seed capability fetch and EventQueueGet long-poll
- Chat: say (20m), whisper (10m), shout (100m), channel selection
- Instant messaging to other avatars
- Event bus via `tokio::broadcast` for all inbound server activity

### Phase 2: movement, objects

- Agent movement with 24 control flags (walk, run, strafe, fly, sit, crouch, mouselook, nudge)
- Teleport request and response handling
- Sit on object, stand, fly toggle, animation override
- Object updates: parse ObjectUpdate, ObjectUpdateCompressed, ImprovedTerseObjectUpdate
- Object lifecycle: rez from inventory, derez, select/deselect
- Object editing: name, description, permissions, link/unlink
- Object property queries via RequestObjectPropertiesFamily

## Usage

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
welkin-realmwalker = { path = "crates/welkin-realmwalker" }
tokio = { version = "1", features = ["full"] }
```

### Connect and chat

```rust
use welkin_realmwalker::{GridConfig, RealmWalker, Event, StartLocation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = GridConfig {
        login_uri: "http://localhost:8002".into(),
        first_name: "Bot".into(),
        last_name: "User".into(),
        password: "password".into(),
        start: StartLocation::Last,
        ..GridConfig::default()
    };

    let mut client = RealmWalker::login(config).await?;
    println!("logged in as {}", client.agent_id());

    // Say hello on the public channel
    client.say("Hello from RealmWalker").await?;

    // Listen for events
    let mut rx = client.events();
    loop {
        match rx.recv().await? {
            Event::ChatReceived(msg) => {
                println!("{}: {}", msg.from_name, msg.message);
                if msg.message == "quit" {
                    break;
                }
            }
            Event::Disconnected(reason) => {
                println!("disconnected: {:?}", reason);
                break;
            }
            _ => {}
        }
    }

    client.logout().await?;
    Ok(())
}
```

### Chat on a specific channel

```rust
use welkin_realmwalker::events::ChatType;

// Whisper on channel 42
client.send_chat_on_channel("secret", ChatType::Whisper, 42).await?;

// Send an instant message to another avatar
client.send_im(other_avatar_uuid, "hello").await?;
```

### Start location options

```rust
// Log in at the avatar's home location
let config = GridConfig { start: StartLocation::Home, ..base };

// Log in at a named region
let config = GridConfig { start: StartLocation::Region("MyRegion".into()), ..base };

// Log in at a specific position in a region
let config = GridConfig {
    start: StartLocation::RegionWithPosition("MyRegion".into(), 128, 128, 30),
    ..base
};
```

## Architecture

RealmWalker composes four internal layers:

```
GridConfig ──► login.rs (XMLRPC) ──► network.rs (UDP circuit)
                                         │
                                    caps.rs (HTTP capabilities, EQG)
                                         │
                                    event dispatch
                                    ┌────┴────┐
                                chat.rs   objects.rs   agent.rs
                                    │         │           │
                                    └────┬────┘           │
                                     EventBus ◄───────────┘
                                         │
                                  broadcast::Receiver<Event>
```

On login, the client spawns four background tokio tasks: a UDP packet pump,
an ack/ping loop, an event dispatcher that routes inbound packets to chat and
object handlers, and an EventQueueGet long-poll loop for capability-based
events. All four tasks run until `logout()` is called.

### Event system

All inbound activity produces `Event` variants delivered through a
`tokio::broadcast` channel. Call `client.events()` to get a receiver; multiple
subscribers are supported. Slow subscribers receive a `Lagged` error and skip
to the latest event. Channel capacity defaults to 256 and is adjustable in
`GridConfig`.

Current event variants:

| Variant | Source |
|---|---|
| ChatReceived | Nearby chat from avatars or objects |
| ImReceived | Instant message from another avatar |
| ObjectUpdated | Object created or changed in the scene |
| ObjectKilled | Object removed from the scene |
| ObjectPropertiesReceived | Detailed object properties returned |
| TeleportOffer | Lure from another avatar |
| TeleportStarted / TeleportProgress / TeleportFailed | Teleport lifecycle |
| HealthChanged | Agent health value changed |
| MoneyBalance | L$ balance changed |
| Disconnected | Connection lost or closed |
| SimChanged | Avatar moved to a different simulator |
| EventQueueEvent | Raw cap event with no dedicated variant |

### Error handling

`RealmWalkerError` covers all failure modes: `LoginFailed` (bad credentials,
grid down), `CircuitFailed` (UseCircuitCode not acked), `Disconnected`
(server kicked, timeout), plus wrapped errors from reqwest, welkin-msg, and
std::io. Auto-reconnect is off by default; set `config.auto_reconnect = true`
with `max_reconnect_attempts` and `reconnect_delay` to enable it.

### Crate dependencies

| Crate | Role |
|---|---|
| welkin-msg | LLUDP message types, packet encode/decode |
| welkin-types | UUID, Vector3, Quaternion, asset/folder types |
| tokio | Async runtime, broadcast channels, timers |
| reqwest | XMLRPC login, capability HTTP requests |
| quick-xml | XMLRPC response parsing |
| glam | Vec3, Quat for positions and rotations |

## Testing

84 unit tests cover packet construction, response parsing, event bus delivery,
config validation, and control flag bitmasks. Integration tests (gated behind
`#[cfg(feature = "integration")]`) run against a live Welkin server:

```sh
# Unit tests only (no server needed)
cargo test -p welkin-realmwalker

# Integration tests (requires a running Welkin grid)
cargo test -p welkin-realmwalker --features integration
```

Integration tests verify: full login/logout cycle, chat round-trip, agent
movement with position confirmation via ObjectUpdate, and object rez from
inventory.

## What's coming

Phases 3 through 11 add 59 tasks across nine areas. Here is what each phase
delivers:

| Phase | Focus | Tasks |
|---|---|---|
| 3 | Inventory and assets: folder tree, item CRUD, give/receive, upload via caps | 7 |
| 4 | Appearance and grid: bake textures, wear/detach, region lookup, parcel info | 7 |
| 5 | Multi-bot and stress: concurrent clients, mutual visibility, N-bot load test, reconnect | 5 |
| 6 | Refactor to Option A: move client-side circuit logic into welkin-net | 3 |
| 7 | Friends, groups, avatars: friendship lifecycle, group management, name resolution | 8 |
| 8 | Directory, sound, terrain, environment: grid search, heightmap, Windlight/EEP | 7 |
| 9 | Agent completions: camera, grab/touch, mute, viewer effects, gestures, money, profile, region crossing | 10 |
| 10 | Extended subsystems: task inventory, parcel CRUD, outfit management, multi-sim, asset pipeline | 9 |
| 11 | Optional: OAR/IAR archive reading, experience API, marketplace (low priority) | 3 |

The design document at `openspec/changes/realmwalker/design.md` has the full
API surface and architectural rationale for each phase.
