# firestorm-session tasks

## Setup
- [x] Install Firestorm viewer configured for local grid — OpenSim build installed
- [x] Set up LibreMetaverse bot for automated session testing
- [x] Create test user account via ROBUST

## Compatibility fixes
- [x] Audit Firestorm login XMLRPC params vs Aurora (inventory-root, buddy-list, gestures, etc.)
- [x] Fix any login response fields Firestorm requires that Aurora doesn't (real_id added)
- [x] Verify UseCircuitCode + CompleteAgentMovement + RegionHandshake sequence
- [x] Confirm AgentAnimation default stand exits T-pose (AvatarAnimation fix applied)
- [x] Confirm ImprovedTerseObjectUpdate echo enables movement
- [x] Verify textures load (ROBUST asset schema fix applied)
- [ ] Test chat send/receive — BROKEN: ChatFromViewer stub in handlers.rs masks real impl in main.rs
- [x] Test inventory fetch and basic operations — Firestorm inventory panel opens with correct folder tree
- [ ] Test local teleport
- [ ] Test map/minimap rendering — BROKEN: MapBlockRequest not sending reply data (blue void)

## Protocol fixes (identified via audit)
- [x] Fix PacketAck processing (server was never clearing acked packets)
- [x] Add LLSD login support (LibreMetaverse 3.x sends LLSD not XMLRPC)
- [x] Add FetchInventoryDescendents2 cap — fully wired to ROBUST xinventory
- [x] Add FetchInventory2 cap — fully wired to ROBUST xinventory
- [x] Add UploadBakedTexture cap (full two-phase implementation, stores in ROBUST)
- [x] Wire inventory caps to actual DB data via ROBUST xinventory
- [x] Add map-server-url to login response
- [x] Add ViewerAsset cap
- [x] Add FetchLib2 cap
- [x] Add GetDisplayNames cap
- [x] Add HomeLocation cap
- [x] Add CreateInventoryCategory cap

## Firestorm live test results (2026-11-14)
### Working
- Login (Test User / password)
- Inventory panel with correct folder tree (16 categories)
- AgentUpdate, ViewerEffect, baked textures flowing
- Region handle formula fixed: ((loc_x * 256) << 32) | (loc_y * 256)
- ROBUST config separation (robust.toml → robust.db, welkin.toml → region.db)
- Inventory creation idempotency guard

### Issues found
- Chat echo not working — ChatFromViewer stub in handlers.rs masks real impl in main.rs
- Map shows blue void — MapBlockRequest handler not sending reply data
- Avatar orange cloud — expected (empty inventory, no baked appearance items)
- "Could not put on outfit" / "Could not load settings" — Firestorm-internal, empty inventory

### Action items
- [x] Audit and remove stub-only handlers that mask real implementations — commit 9d82a90
- [x] Fix ChatFromViewer routing (also fix relay to include sender for single-user test) — commit 9d82a90
- [x] Fix MapBlockRequest to return region land data — commit 9d82a90
- [x] Remove all 21 log-only stub handlers — commit 6f04835 (-297 lines)
- [x] Implement AgentSetAppearance handler (orange cloud fix) — commit f4ec89f
- [ ] Re-test chat, map, and avatar appearance in Firestorm (awaiting user)

## Stability
- [ ] 10-minute soak test with no crashes or disconnects
- [ ] Verify graceful disconnect and presence cleanup
- [ ] Test reconnection after viewer restart

## Sign-off
- [ ] Full Firestorm session documented with working/broken matrix
- [ ] All P0 items passing before sign-off
