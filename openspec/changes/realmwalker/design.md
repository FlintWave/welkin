# RealmWalker: Design

## Crate structure

`welkin-realmwalker` lives in the workspace at `crates/welkin-realmwalker/`.
Internal module layout:

```
src/
├── lib.rs          // Public re-exports: RealmWalker, Event, Error, Config
├── client.rs       // RealmWalker struct, top-level entry point
├── config.rs       // LoginCredentials, GridConfig, timeouts
├── login.rs        // XMLRPC login, seed cap fetch, circuit params
├── network.rs      // Wraps welkin-net LLUDPClient, packet pump, ack loop
├── caps.rs         // Capability URL management, EventQueueGet long-poll
├── agent.rs        // Movement, teleport, sit/stand, fly, animations
├── chat.rs         // Local chat, whisper, shout, IM, group chat
├── objects.rs      // Rez, derez, edit, select, link/unlink, properties
├── inventory.rs    // Folder tree, item CRUD, give/receive, trash
├── appearance.rs   // Visual params, bake requests, wear/detach
├── assets.rs       // Fetch by UUID, upload via caps
├── grid.rs         // Region lookup, map blocks, neighbor queries
├── parcels.rs      // Parcel info, access lists (read-only for bots)
├── events.rs       // Event enum, EventBus (broadcast channels)
└── error.rs        // Error types
```

Each module corresponds to one of LibreMetaverse's manager classes but uses
Rust idioms rather than translating C# patterns.

## Connection lifecycle

```
Disconnected → Logging In → Circuit Setup → Connected → Disconnecting → Disconnected
                                                ↑            |
                                                └── Reconnecting (optional)
```

States:
- Disconnected: no active session.
- Logging In: XMLRPC POST to login URI, waiting for session_id/circuit_code/seed_cap.
- Circuit Setup: sends UseCircuitCode over UDP, waits for ack, fetches seed cap.
- Connected: packet pump running, event queue polling, agent active in-world.
- Disconnecting: sends LogoutRequest, drains pending acks, closes socket.
- Reconnecting: on timeout or kick, optionally re-login (config-driven).

State transitions are internal. The caller awaits `login()` which resolves
when the client reaches Connected, or returns an error.

## Async model

Built on tokio. The client spawns three background tasks on login:

1. Packet pump: reads UDP datagrams from welkin-net, dispatches to handlers,
   feeds the event bus. Runs until disconnect.
2. Ack/ping loop: calls `circuit.tick()` every 100ms, sends acks and
   keepalive pings. Handles retransmission.
3. EventQueueGet: long-polls the cap URL, parses LLSD events, feeds the
   event bus. Falls back to UDP events if cap unavailable.

All three tasks are joined on disconnect. Panics in any task trigger a
controlled shutdown with an error event.

The packet pump owns mutable state and communicates via channels, so no thread
pools or `Arc<Mutex<>>` appear on the hot path. Shared read-only state (session
info, agent UUID) uses `Arc<SessionInfo>` set once at login.

## API surface

### Config and login

```rust
let config = GridConfig {
    login_uri: "http://localhost:8002".into(),
    first_name: "Test".into(),
    last_name: "User".into(),
    password: "password".into(),
    start: StartLocation::Home,
};

let mut client = RealmWalker::new(config);
client.login().await?;
```

### Primary struct

```rust
pub struct RealmWalker {
    config: GridConfig,
    state: ConnectionState,
    session: Option<Arc<SessionInfo>>,
    // Module accessors grouped as method namespaces
}
```

LibreMetaverse's `GridClient` holds 15+ manager objects as public fields, each
with a back-reference to `GridClient`. RealmWalker uses inherent method groups
instead, which sidesteps the circular-reference problem and satisfies the borrow
checker.

```rust
// LibreMetaverse: client.Self.Chat(...)
// RealmWalker:    client.chat("Hello", 0, ChatType::Normal).await?

// LibreMetaverse: client.Objects.RequestObject(...)
// RealmWalker:    client.request_object(local_id).await?
```

Methods that need internal state take `&self` (reads) or `&mut self` (mutations
that change connection state). Most operations are `&self` because they send a
packet and don't mutate client state; the response arrives as an event.

### Typical bot

```rust
let mut client = RealmWalker::new(config);
client.login().await?;

// Send chat
client.chat("Hello grid", 0, ChatType::Normal).await?;

// Move to a position
client.move_to(Vector3::new(128.0, 128.0, 25.0)).await?;

// Rez an object from inventory
client.rez_object(item_id, position, rotation).await?;

// Listen for events
let mut events = client.subscribe();
while let Ok(event) = events.recv().await {
    match event {
        Event::ChatReceived(msg) => {
            if msg.message == "quit" {
                break;
            }
        }
        _ => {}
    }
}

client.logout().await?;
```

## Event system

All inbound server activity produces events through a tokio `broadcast` channel.

```rust
pub enum Event {
    // Chat
    ChatReceived(ChatMessage),
    ImReceived(InstantMessage),

    // Agent
    TeleportOffer { from: Uuid, region: String },
    TeleportProgress(TeleportStatus),
    HealthChanged(f32),
    MoneyBalance(i32),

    // Objects
    ObjectUpdate(ObjectData),
    ObjectKilled(u32),  // local_id
    ObjectProperties(ObjectProperties),

    // Inventory
    InventoryOffer { from: String, item_id: Uuid },
    InventoryUpdated(FolderUpdate),

    // Appearance
    AppearanceSet,
    RebakeRequired,

    // Connection
    Disconnected(DisconnectReason),
    SimChanged(RegionInfo),
    EventQueueEvent(String, serde_json::Value), // raw cap events
}
```

`client.subscribe()` returns a `broadcast::Receiver<Event>`. Multiple
subscribers allowed, which is useful for running a logger alongside bot logic.
Slow consumers get `Lagged` errors and miss events, which is acceptable for
bots. Channel capacity is adjustable in config.

## Error handling

```rust
pub enum RealmWalkerError {
    LoginFailed(LoginFailure),    // bad creds, grid down, banned
    CircuitFailed(String),         // UseCircuitCode not acked
    Disconnected(DisconnectReason),// server kicked, timeout, network
    PacketError(welkin_msg::DecodeError),
    HttpError(reqwest::Error),
    IoError(std::io::Error),
    Timeout,
}

pub enum LoginFailure {
    BadCredentials,
    GridUnavailable,
    AccountDisabled,
    PresenceConflict,  // already logged in
    Other(String),
}
```

Automatic reconnection is off by default; bots should handle disconnects
explicitly. Optional reconnect behavior via `config.auto_reconnect = true`
with configurable delay and max attempts.

## Integration with existing crates

| Layer | Crate | What RealmWalker uses |
|-------|-------|-----------------------|
| Transport | `welkin-net` | `LLUDPClient` for send/receive, `Circuit` for ack tracking, `ThrottleCategory` for bandwidth |
| Messages | `welkin-msg` | `Msg` trait, typed message structs via `aurora_msg::msgs`, `parse_packet`/`build_packet` |
| Types | `welkin-types` | `Uuid`, `Vector3`, `Quaternion`, `AssetType`, `FolderType`, `PermissionMask` |
| HTTP | `reqwest` | XMLRPC login, cap requests, asset upload/download |

RealmWalker is the first crate that uses welkin-net as a *client* rather than
server-side. The `LLUDPClient` struct is currently server-oriented (it
represents one connected viewer). RealmWalker needs the inverse: a client that
connects *to* a server. This requires either:

**Option A (preferred):** Add a `ClientConnection` type to welkin-net that
handles the client side of circuit setup (sends UseCircuitCode, processes the
ack, manages outbound sequencing). This keeps protocol code in the right crate.

**Option B:** RealmWalker wraps a raw UDP socket and uses welkin-msg for
serialization directly, duplicating some circuit logic. Less clean but faster
to prototype.

Decision: start with Option B for the initial bot-test-capable version, then
refactor to Option A once the API stabilizes.

## Key design decisions

### No manager objects

LibreMetaverse uses `NetworkManager`, `AgentManager`, `ObjectManager`, and
others, each holding a back-reference to `GridClient`. This works in C# (GC
handles cycles) but creates borrow-checker pain in Rust. RealmWalker puts all methods
on the `RealmWalker` struct directly, grouped by module via `impl` blocks in
separate files. The struct owns all state; no circular references.

### Broadcast channels over callbacks

LibreMetaverse uses C# events (multicast delegates). Rust has no equivalent.
Options considered:
- Callbacks (`Box<dyn Fn>`): awkward lifetimes, hard to use across await points
- mpsc channels: single consumer, need fan-out for multiple listeners
- broadcast channels: multiple consumers, each gets every event, `Lagged` on slow readers

Broadcast wins for bot use cases. Log all events while also pattern-matching
in bot logic.

### Flat event enum over typed streams

A single `Event` enum rather than per-type channels (chat channel, object
channel, etc.). Simpler API, one subscription call, pattern matching handles
filtering. Per-type channels add complexity with minimal benefit for bot use.

### Option B bootstrap

Starting with raw UDP + welkin-msg rather than extending welkin-net's server
types. Reason: welkin-net's `LLUDPClient` assumes server-side ownership of the
circuit. Refactoring it to support both directions is the right long-term
answer but blocks bot testing. Option B unblocks testing, Option A follows as
a refactor task.

## Testing strategy

### Unit tests (in-crate)

- Login XMLRPC parsing: mock HTTP responses, verify session extraction
- Packet construction: build known messages, verify byte output against spec
- Event parsing: feed raw packets, verify correct `Event` variants produced
- State machine: verify connection lifecycle transitions

### Integration tests (against live Welkin)

- Full login/logout cycle
- Chat round-trip (send, verify via second bot or server log)
- Movement (send AgentUpdate, verify position via ObjectUpdate)
- Object rez (rez from inventory, verify ObjectUpdate received)
- Inventory fetch (request folder contents, verify response)
- Multi-bot (two clients in same region, verify they see each other)
- Stress: N concurrent logins, verify all circuits established

Integration tests require a running Welkin server. They live in
`tests/integration/` and are gated behind `#[cfg(feature = "integration")]`
so `cargo test` doesn't require a live server by default.

## Phased delivery

1. Login + circuit + chat: minimum viable bot, enough for basic testing
2. Movement + objects: rez, derez, move agent, verify position
3. Inventory + assets: fetch folders, give items, upload
4. Appearance + grid: bake, wear, region queries
5. Multi-bot + stress: concurrent clients, load testing
6. Refactor to Option A: move client-side circuit logic into welkin-net
