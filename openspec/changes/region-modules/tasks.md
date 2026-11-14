## 1. Module framework

- [x] 1.1 RegionModule trait, ModuleType enum, ModuleRegistry with TypeId-keyed interface lookup, SceneEvents typed event bus (d3f148c: 4 source files, 14 tests)
- [x] 1.2 Module lifecycle integration: Scene calls initialise/add_region/remove_region/close; CapsModule, DynAttrsModule, ServiceThrottleModule (d3f148c: skeleton implementations with config reading)

## 2. Land management

- [x] 2.1 LandParcel struct, LandData (~40 fields), ParcelGrid (4x4m cells), ParcelFlags bitflags, add/remove/subdivide/join, access/ban lists (1f5d627: 7 source files, 21 new tests)
- [x] 2.2 LandManagementModule implementing RegionModule with viewer protocol stubs, bitmap operations, default parcel creation on add_region (1f5d627)
- [x] 2.3 PrimCountModule (per-parcel prim accounting), DwellModule (1f5d627)

## 3. Permissions (HIGH RISK)

- [x] 3.1 DefaultPermissionsModule: config loading, bypass/propagate/debug flags, admin set rules, god mode and estate manager escalation (652d801: PermissionsConfig, UserSet, admin/god detection, Opus reviewed)
- [x] 3.2 All permission check functions: object (create/delete/edit/move/copy/transfer/take/return), script (run/compile), terrain, parcel, estate (652d801: 13 check functions in checks.rs)
- [x] 3.3 Permission check unit tests: truth table covering owner/group/estate-mgr/god/stranger x bypass on/off (652d801: 40 truth-table tests)

## 4. Estate management

- [x] 4.1 EstateManagementModule: estate settings, region flags, covenant, terrain textures, access/ban/manager lists, group access (c6a57bf: EstateSettings, access/ban/manager lists)
- [x] 4.2 Estate actions (restart, kick, teleport home, send message), TelehubManager (spawn points), sun position and time dilation (c6a57bf: TelehubManager, estate actions, sun position)

## 5. Inventory transfers and access

- [x] 5.1 InventoryTransferModule: give item/folder between avatars, accept/decline (c6a57bf)
- [x] 5.2 InventoryAccessModule: rez from inventory (single and coalesced), derez (take, take copy, return, delete) (c6a57bf: RezResult, DerezAction)
- [x] 5.3 AssetTransactionModule: upload transactions for textures, sounds, animations, clothing; XferModule (file transfer) (c6a57bf: UploadTransaction, AssetType, XferModule)

## 6. Attachments

- [x] 6.1 AttachmentsModule: wear/detach, attachment point validation, persistence on detach/logout/login, cross-region transfer through entity transfer (c6a57bf: 41 AttachmentPoint variants, 100 total tests)

## 7. Chat and instant messages

- [ ] 7.1 ChatModule: say/shout/whisper with range-based delivery, ChatFromViewer packet to scene event pipeline
- [ ] 7.2 MessageTransferModule: IM routing on the grid; OfflineMessageModule: store/retrieve offline IMs
- [ ] 7.3 MuteListModule (filter by mute list) and PresenceModule (online/offline tracking for IM delivery)

## 8. World map

- [ ] 8.1 WorldMapModule: map block requests, agent location updates, map item queries
- [ ] 8.2 MapImageModule: tiny-skia 2D renderer producing 256x256 JPEG tiles (replaces LegacyMap and Warp3DMap), periodic upload to map image service

## 9. Environment and miscellaneous world modules

- [ ] 9.1 LightShareModule (Windlight presets, EnvironmentSettings cap), WindModule (SimpleRandomWind, ConfigurableWind)
- [ ] 9.2 SoundModule (triggered/looped sounds), MediaModule (MOAP, ObjectMedia cap), RegionReadyModule, RestartModule, AccessModule, VegetationModule, SceneSerialiser

## 10. Avatar modules

- [ ] 10.1 AvatarFactoryModule (appearance baking, wearable validation), BakedTexturesModule (baked texture cache)
- [ ] 10.2 FriendsModule: friend list, online/offline notifications, map/modify rights
- [ ] 10.3 LureModule, GodsModule, DialogModule, CombatModule, GesturesModule, AvatarCommands

## 11. Entity transfer

- [ ] 11.1 EntityTransferModule: local teleport, grid teleport (seed cap handoff, agent close), region crossing (border cross, child agent promotion)
- [ ] 11.2 EntityTransferStateMachine (double-teleport prevention) and child agent management (inform neighbours, close stale children)

## 12. Framework and service connectors

- [ ] 12.1 UserManagementModule (user name cache, UUID-to-name), MonitorModule (region stats, Prometheus), LibraryModule, SearchModule
- [ ] 12.2 FlotsamAssetCache: memory tier (moka) + disk tier, expiration, deep scan, statistics
- [ ] 12.3 TextureSenderModule (J2K priority queue), IPBanModule
- [ ] 12.4 Service connectors out: Land, Neighbour, MuteList, AgentPreferences, MapImage; service connectors in: Land, MapImage, Neighbour

## 13. Security review and integration tests

- [ ] 13.1 SECURITY REVIEW: adversarial review of permissions module -- every check function, god mode, estate escalation, bypass paths (HIGH RISK, Opus reviewer)
- [ ] 13.2 Integration tests: rez/permission/take round-trip, cross-region teleport with attachments, offline IM storage, parcel buy/sell
- [ ] 13.3 Protocol replay tests (land, estate, map from C#) and Aurora Viewer acceptance (chat, IM, teleport, attachments, land panel, map, estate tools)
