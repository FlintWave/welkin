# RealmWalker — Tasks

37 tasks across 6 phases. Each task produces working, tested code.
Integration tests are gated behind `#[cfg(feature = "integration")]`.
All protocol constants copied from OpenSim/LibreMetaverse C# source.

## Phase 1: Login + Circuit + Chat (minimum viable bot)

- [ ] 1.1 Scaffold `welkin-realmwalker` crate in workspace with Cargo.toml, dependencies (welkin-net, welkin-msg, welkin-types, tokio, reqwest, quick-xml, serde, serde_json), and empty lib.rs
- [ ] 1.2 Implement `config.rs`: GridConfig, LoginCredentials, StartLocation enum, timeout defaults, channel capacity settings
- [ ] 1.3 Implement `error.rs`: RealmWalkerError enum, LoginFailure enum, Display/Error trait impls, From conversions for reqwest::Error, io::Error, welkin_msg::DecodeError
- [ ] 1.4 Implement `events.rs`: Event enum (start with ChatReceived, ImReceived, Disconnected, SimChanged, EventQueueEvent variants), EventBus wrapping tokio::broadcast, subscribe() method
- [ ] 1.5 Implement `login.rs`: XMLRPC POST to login URI, parse response for session_id, agent_id, circuit_code, seed_capability, sim_ip, sim_port. Copy login response field names from LibreMetaverse's LoginResponseData
- [ ] 1.6 Unit tests for login: mock XMLRPC responses (success, bad credentials, grid unavailable), verify parsed fields match
- [ ] 1.7 Implement `network.rs` (Option B): raw UDP socket, send/receive datagrams, UseCircuitCode packet construction via welkin-msg, ack handling, sequence numbering, packet pump task, ping/ack loop task
- [ ] 1.8 Unit tests for network: verify UseCircuitCode byte layout, sequence number incrementing, ack packet parsing
- [ ] 1.9 Implement `caps.rs`: store capability URLs from seed cap response, EventQueueGet long-poll task, parse LLSD event bodies, feed parsed events to EventBus
- [ ] 1.10 Implement `client.rs`: RealmWalker struct, new(), login() (orchestrates XMLRPC then circuit setup then spawns background tasks), logout() (sends LogoutRequest, joins tasks), connection state machine
- [ ] 1.11 Implement `chat.rs`: say/whisper/shout on numbered channels (ChatFromViewer packet), IM send (ImprovedInstantMessage packet), handle inbound ChatFromSimulator and ImprovedInstantMessage, emit ChatReceived/ImReceived events
- [ ] 1.12 Unit tests for chat: verify outbound packet construction, verify inbound packet parsing produces correct Event variants
- [ ] 1.13 Integration test: full login/logout cycle against live Welkin server, verify Connected state reached and clean disconnect
- [ ] 1.14 Integration test: chat round-trip, send a message on channel 0, verify ChatReceived event arrives (via second bot or server echo)

## Phase 2: Movement + Objects

- [ ] 2.1 Implement `agent.rs`: AgentUpdate packet construction (position, rotation, body rotation, control flags, state), move_to() with autopilot (GenericMessage or AgentUpdate loop), teleport request/response handling, sit/stand (AgentRequestSit, AgentSit, AgentStand packets), fly toggle, animation override (AgentAnimation packet)
- [ ] 2.2 Unit tests for agent: verify AgentUpdate byte layout, control flag bitmask values match OpenSim's AgentManager constants
- [ ] 2.3 Implement `objects.rs`: handle inbound ObjectUpdate/ObjectUpdateCompressed/ImprovedTerseObjectUpdate, emit ObjectUpdate/ObjectKilled events, rez from inventory (RezObject packet), derez (DeRezObject), select/deselect (ObjectSelect/ObjectDeselect), edit properties (ObjectName, ObjectDescription, ObjectPermissions), link/unlink (ObjectLink/ObjectDelink), request object properties (RequestObjectPropertiesFamily)
- [ ] 2.4 Unit tests for objects: verify ObjectUpdate parsing against known packet bytes, verify RezObject packet matches LibreMetaverse's construction
- [ ] 2.5 Add TeleportOffer, TeleportProgress, HealthChanged, MoneyBalance, ObjectUpdate, ObjectKilled, ObjectProperties variants to Event enum
- [ ] 2.6 Integration test: move agent to coordinates, verify position via ObjectUpdate event
- [ ] 2.7 Integration test: rez an object from inventory, verify ObjectUpdate event with matching item

## Phase 3: Inventory + Assets

- [ ] 3.1 Implement `inventory.rs`: fetch folder contents (FetchInventoryDescendents packet or caps), parse InventoryDescendents response, folder tree representation, item CRUD (CreateInventoryItem, UpdateInventoryItem, MoveInventoryItem, RemoveInventoryItem packets), give to agent (ImprovedInstantMessage with inventory offer dialog), receive offers (accept/decline via ImprovedInstantMessage), trash (MoveInventoryItem to Trash folder)
- [ ] 3.2 Unit tests for inventory: verify folder parse from known response, verify CreateInventoryItem packet fields
- [ ] 3.3 Implement `assets.rs`: fetch asset by UUID and type (TransferRequest packet or GetMesh/GetTexture caps), upload via caps (NewFileAgentInventory cap for textures/sounds/animations, UpdateScriptAgent cap for scripts), parse transfer response
- [ ] 3.4 Unit tests for assets: verify TransferRequest packet construction, verify cap URL selection by asset type
- [ ] 3.5 Add InventoryOffer, InventoryUpdated variants to Event enum
- [ ] 3.6 Integration test: fetch root inventory folder, verify at least the standard system folders exist
- [ ] 3.7 Integration test: upload a test texture via caps, fetch it back by UUID, verify bytes match

## Phase 4: Appearance + Grid

- [ ] 4.1 Implement `appearance.rs`: set visual params (AgentSetAppearance packet), bake layer request, wear attachment (RezSingleAttachmentFromInv), detach (DetachAttachmentIntoInv), rebake on RebakeRequired event, parse AgentWearablesUpdate and AgentCachedTextureResponse
- [ ] 4.2 Unit tests for appearance: verify AgentSetAppearance byte layout, verify visual param indices match LibreMetaverse's constants
- [ ] 4.3 Implement `grid.rs`: region lookup by name (GridLayerRequest or caps), map block requests (MapBlockRequest/MapBlockReply), neighbor info from RegionHandshake data, parse region handle to grid coordinates
- [ ] 4.4 Implement `parcels.rs`: parcel info request (ParcelInfoRequest), parse ParcelInfoReply, parcel properties (ParcelPropertiesRequest/ParcelProperties), access list query (read-only)
- [ ] 4.5 Unit tests for grid: verify region handle to grid coordinate conversion, verify MapBlockRequest packet fields
- [ ] 4.6 Add AppearanceSet, RebakeRequired variants to Event enum (if not already present from Phase 1 skeleton)
- [ ] 4.7 Integration test: query grid for the test region by name, verify correct region handle returned

## Phase 5: Multi-bot + Stress

- [ ] 5.1 Multi-client support: verify multiple RealmWalker instances run concurrently in one tokio runtime, each with independent UDP sockets and sessions
- [ ] 5.2 Integration test: two bots log in to the same region, each receives ObjectUpdate for the other's avatar
- [ ] 5.3 Integration test: bot A sends chat, bot B receives ChatReceived event with correct message and sender
- [ ] 5.4 Stress test: spawn N bots (configurable, default 10), verify all circuits establish, all logins complete, all logouts clean. Report timing per bot.
- [ ] 5.5 Add reconnection logic: on Disconnected event with auto_reconnect enabled, attempt re-login with configurable delay and max retries

## Phase 6: Refactor to Option A

- [ ] 6.1 Add `ClientConnection` type to welkin-net: client-side circuit setup (send UseCircuitCode, wait for ack, manage outbound sequence numbers), symmetric with existing server-side LLUDPClient
- [ ] 6.2 Migrate RealmWalker's network.rs from raw UDP to welkin-net ClientConnection, remove duplicated circuit logic
- [ ] 6.3 Verify all existing unit and integration tests still pass after migration
