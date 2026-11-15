# firestorm-session tasks

## Setup
- [ ] Install Firestorm viewer configured for local grid
- [x] Set up LibreMetaverse bot for automated session testing
- [x] Create test user account via ROBUST

## Compatibility fixes
- [x] Audit Firestorm login XMLRPC params vs Aurora (inventory-root, buddy-list, gestures, etc.)
- [x] Fix any login response fields Firestorm requires that Aurora doesn't (real_id added)
- [x] Verify UseCircuitCode + CompleteAgentMovement + RegionHandshake sequence
- [x] Confirm AgentAnimation default stand exits T-pose (AvatarAnimation fix applied)
- [x] Confirm ImprovedTerseObjectUpdate echo enables movement
- [x] Verify textures load (ROBUST asset schema fix applied)
- [ ] Test chat send/receive between two sessions (LibreMetaverse + Firestorm)
- [ ] Test inventory fetch and basic operations
- [ ] Test local teleport
- [ ] Test map/minimap rendering

## Protocol fixes (identified via audit)
- [x] Fix PacketAck processing (server was never clearing acked packets)
- [x] Add LLSD login support (LibreMetaverse 3.x sends LLSD not XMLRPC)
- [x] Add FetchInventoryDescendents2 cap (stub)
- [x] Add FetchInventory2 cap (stub)
- [x] Add UploadBakedTexture cap (full two-phase implementation)
- [x] Wire inventory caps to actual DB data via ROBUST xinventory
- [ ] Add map-server-url to login response
- [ ] Add ViewerAsset cap
- [ ] Add FetchLib2 cap
- [ ] Add GetDisplayNames cap
- [ ] Add HomeLocation cap
- [ ] Add CreateInventoryCategory cap

## Stability
- [ ] 10-minute soak test with no crashes or disconnects
- [ ] Verify graceful disconnect and presence cleanup
- [ ] Test reconnection after viewer restart

## Sign-off
- [ ] Full Firestorm session documented with working/broken matrix
- [ ] All P0 items passing before sign-off
