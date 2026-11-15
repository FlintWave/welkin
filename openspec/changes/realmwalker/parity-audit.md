# RealmWalker Parity Audit: LibreMetaverse C# Source

Comprehensive comparison of LibreMetaverse's public API surface against the
RealmWalker design and task list. Source: `/var/home/erik/Projects/libremetaverse/`.

## Manager-by-Manager Comparison

| Manager | Status | What's Covered | What's Missing |
|---------|--------|---------------|----------------|
| **AgentManager (core)** | PARTIAL | Sit/stand, movement, teleport, animations | Grab/GrabUpdate/DeGrab, Touch, AutoPilot (global coords variant), MuteList (add/remove/request), SetHome, SetHeightWidth, ScriptQuestionReply, GroupInviteRespond, SetDisplayName, SetAgentAccess, SetHoverHeight, AgentPreferences, SendPostcard, NavMesh requests, ViewerStats, ProductInfo |
| **AgentManager.Camera** | MISSING | (nothing) | Full camera control: position, orientation, Roll/Pitch/Yaw, LookAt, LookDirection, FOV, SetPositionOrientation |
| **AgentManager.Chat** | PARTIAL | Say/whisper/shout, IM send/receive, group chat | RetrieveInstantMessages (offline IMs), ReplyToScriptDialog, ChatterBoxAcceptInvite, StartIMConference, split-large-messages handling |
| **AgentManager.Effects** | MISSING | (nothing) | PointAtEffect, LookAtEffect, BeamEffect, SphereEffect (viewer visual effects) |
| **AgentManager.Experience** | MISSING | (nothing) | Full Experience API: GetAgentExperiences, GetExperienceInfo, FindExperienceByName, GetAdminExperiences, GetCreatorExperiences, ExperiencePreferences get/set, GroupExperiences, UpdateExperience, IsExperienceAdmin/Contributor, RegionExperiences, QueryExperiencesOnParcel |
| **AgentManager.Gestures** | MISSING | (nothing) | PlayGesture, ActivateGesture, DeactivateGesture |
| **AgentManager.Money** | PARTIAL | MoneyBalance event in Event enum | RequestBalance, GiveAvatarMoney, GiveObjectMoney, GiveGroupMoney, PayUploadFee, GiveMoney (generic) |
| **AgentManager.Movement** | COVERED | AgentUpdate, control flags, position/rotation | Camera sub-struct is separate (see Camera row). SendManualUpdate, SetFOVVerticalAngle are minor gaps. |
| **AgentManager.MultiSim** | MISSING | (nothing) | Multi-simulator presence tracking: TrackObjectInSimulator, UntrackObjectInSimulator, GetSimulatorsForObject, IsNearBorder, region crossing prediction events |
| **AgentManager.Profile** | MISSING | (nothing) | UpdateProfile, UpdateProfileNotes, UpdateInterests, PickInfoUpdate, PickDelete, UpdateClassifiedInfo, DeleteClassified |
| **AgentManager.RegionCrossing** | MISSING | (nothing) | Region crossing state machine (CrossingState enum, retry logic, old/new simulator tracking, failure recovery) |
| **AgentManager.Teleporting** | PARTIAL | Teleport request/response in design | TeleportLure send/respond, TeleportLureRequest, landmark teleport, async TeleportAsync variants |
| **AnimeshManager** | MISSING | (nothing) | Animated mesh player tracking: GetPlayer, AllPlayers, Update, RemovePlayer. Low priority for bots. |
| **AppearanceManager** | PARTIAL | Bake, wear/detach, visual params, rebake | AddToOutfit/RemoveFromOutfit (multi-item), ReplaceOutfit, WearOutfit, AddAttachments, RequestAgentWorn, RequestCachedBakes, CurrentOutfitFolder (COF), GetFolderWearables, ApplyArchetype, RandomizeAppearance, SendOutfitToCurrentSimulator, GetAttachmentsByAttachmentPoint |
| **AssetManager** | PARTIAL | Fetch by UUID, upload via caps | RequestAssetXfer, RequestInventoryAsset, RequestEstateAsset, RequestUploadLargeTexture, RequestUploadBakedTexture, RequestMesh, RequestImage (texture pipeline with priority/discard), RequestImageCancel, RequestServerBakedImage, asset cache management |
| **AvatarManager** | MISSING | (nothing) | RequestAvatarName/Names, GetDisplayNames, RequestAvatarProperties, RequestAvatarNameSearch, RequestAvatarNotes, RequestAvatarPicks, RequestAvatarClassified, RequestPickInfo, RequestClassifiedInfo, RequestTrackAgent, RequestOwnAvatarTextures, AgentProfileAsync. Also viewer effects: PointAt, LookAt, ViewerEffect events. |
| **DirectoryManager** | MISSING | (nothing) | SearchPeople, SearchGroups, SearchDirPlaces, SearchClassifieds, SearchEvents, SearchLand, SearchPlaces, GetEventInfo. Full grid directory/search API. |
| **DownloadManager** | SKIP | (internal) | Internal HTTP download queue. Not a public bot API; used internally by AssetManager. |
| **EnvironmentManager** | MISSING | (nothing) | GetRegionEnvironment, GetParcelEnvironment, SetRegionEnvironment, SetParcelEnvironment, ResetParcelEnvironment, ResetRegionEnvironment, legacy Windlight get/set |
| **FriendsManager** | MISSING | (nothing) | Full friends API: FriendList, AcceptFriendship, DeclineFriendship, OfferFriendship, TerminateFriendship, GrantRights, MapFriend, TrackFriend, RequestOnlineNotification, GiveCallingCard, AcceptCallingCard, DeclineCallingCard. Events: FriendOnline/Offline, FriendshipOffered/Response/Terminated, FriendRightsUpdate. |
| **GridManager** | PARTIAL | Region lookup, map blocks, neighbor info | RequestMapLayer, RequestMapItems, RequestMainlandSims, GetGridRegionAsync (by name and handle) are mostly there. Minor gap: region-by-UUID lookup. |
| **GroupManager** | MISSING | (nothing) | Full group API: RequestCurrentGroups, RequestGroupProfile, RequestGroupMembers, RequestGroupRoles, RequestGroupRolesMembers, RequestGroupTitles, GroupAccountSummary, Invite, ActivateGroup, ActivateTitle, SetGroupContribution, RequestJoinGroup, RequestCreateGroup, UpdateGroup, EjectUser, CreateRole/UpdateRole/DeleteRole, AddToRole/RemoveFromRole, GroupNotices (list/request/send), StartProposal, LeaveGroup, BannedAgents. |
| **InterestListManager** | MISSING | (nothing) | SetMode, Reset (controls what objects the server sends updates for). Useful for bandwidth optimization. |
| **InventoryManager** | PARTIAL | Folder fetch, item CRUD, give/receive, trash | FetchItemHttp, AIS client (modern API), CopyItem, CreateLink/CreateLinks, MeshUpload, MaterialCreate, RemoveDescendants, EmptyLostAndFound, EmptyTrash, FindFolderForType, RequestFindObjectByPath, UpdateFolderProperties. Task inventory: RequestTaskInventory, MoveTaskInventory, RemoveTaskInventory, RequestGetScriptRunning, RequestSetScriptRunning. |
| **MarketplaceManager** | MISSING | (nothing) | FetchListings, CreateListing, DeleteListing, Activate/DeactivateListing. Marketplace integration. Low priority for bots. |
| **NetworkManager** | COVERED | Circuit lifecycle, packet dispatch, ack handling | RegisterEventCallback/UnregisterEventCallback are internal. Connect to additional simulators, FindSimulator, DisconnectSim are multi-sim. Minor gap: BeginLogout vs Logout vs LogoutAsync granularity. |
| **ObjectManager** | PARTIAL | Rez, derez, edit, select, link/unlink, properties | BuyObject, RequestPayPrice, ClickObject, SetFlags (physical/phantom/temp), SetSaleInfo, AddTree, AddGrass, SetTextures, SetLight, SetFlexible, SetSculpt, SetExtraParamOff, SetPosition/Scale/Rotation (with childOnly), UpdateObject, DeedObject, SetPermissions, SetObjectsGroup, ObjectMedia (navigate/update/request), RequestMaterials, GLTFMaterialUpdates, SetShape, SetMaterial, AttachObject, DropObject, DetachObjects |
| **ParcelManager** | PARTIAL | Parcel info request, parcel properties (read-only) | Buy, Reclaim, DeedToGroup, RequestObjectOwners, ReturnObjects, Subdivide, Join, Terraform, SelectObjects, EjectUser, FreezeUser, ReleaseParcel, StartAuction, RequestRemoteParcelID, GetParcelResources, RequestAllSimParcels, RequestDwell, CurrentParcel tracking, ParcelAccessList |
| **ProtocolManager** | SKIP | (internal) | Message template definitions. Internal networking layer. |
| **SoundManager** | MISSING | (nothing) | PlaySound, SendSoundTrigger (with position, gain, sim). Events: AttachedSound, AttachedSoundGainChange, SoundTrigger, PreloadSound. |
| **TerrainManager** | MISSING | (nothing) | LandPatchReceived event (heightmap data), GetTerrainMaterialOverrides, SetTerrainMaterialOverrides. |

## Supporting Libraries

| Library | Status | Notes |
|---------|--------|-------|
| **LibreMetaverse.Archiving** | MISSING | OAR/IAR file reading and writing. Useful for backup/restore bots. |
| **LibreMetaverse.Assets** | PARTIAL | Asset type definitions shared with AssetManager. Our welkin-types covers basic types. |
| **LibreMetaverse.RLV** | SKIP | RestrainedLove viewer restrictions. Viewer-specific, not relevant for headless bots. |
| **LibreMetaverse.StructuredData** | COVERED | LLSD/OSD parsing. We use serde + quick-xml. |
| **LibreMetaverse.Transport** | COVERED | HTTP transport layer. We use reqwest. |

## Summary of Gaps

### Entirely missing managers (new modules needed)

1. **FriendsManager** (friends.rs): friend list, offer/accept/decline, rights, tracking, calling cards
2. **GroupManager** (groups.rs): group CRUD, roles, members, titles, notices, proposals, bans
3. **AvatarManager** (avatars.rs): name resolution, profile queries, picks, classifieds, display names, tracking
4. **DirectoryManager** (directory.rs): people/group/place/land/event/classified search
5. **SoundManager** (sound.rs): trigger sounds, handle attached sound events
6. **TerrainManager** (terrain.rs): receive heightmap patches, terrain material overrides
7. **EnvironmentManager** (environment.rs): Windlight/EEP get/set for region and parcel

### Partially covered managers (additions to existing modules)

8. **AgentManager gaps** (agent.rs additions): grab/touch, mute list, camera control, viewer effects (beam/point/look), gestures, profile updates, money transactions, experience API, region crossing, multi-sim tracking, display name, hover height, script dialog replies
9. **AppearanceManager gaps** (appearance.rs additions): outfit management (add/remove/replace), COF folder, cached bakes, attachments by point, archetypes
10. **AssetManager gaps** (assets.rs additions): xfer transfers, inventory asset requests, texture pipeline with priority/discard, mesh requests, baked texture upload, large texture upload, asset cache
11. **InventoryManager gaps** (inventory.rs additions): task inventory (script running, move/remove from object), AIS client, copy items, links, mesh upload, material create, folder utilities
12. **ObjectManager gaps** (objects.rs additions): buy, pay, click, flags, sale info, trees/grass, textures/light/flexible/sculpt, media, materials (GLTF + legacy), attach/detach, deed, permissions, shape, position/scale with child-only
13. **ParcelManager gaps** (parcels.rs additions): buy, deed, terraform, subdivide/join, eject/freeze, return objects, auctions, access lists, dwell
14. **ChatManager gaps** (chat.rs additions): offline IM retrieval, script dialog reply, conference IM, chatterbox accept

### Skipped (not relevant for headless bots)

- DownloadManager (internal to AssetManager)
- ProtocolManager (internal networking)
- RLV (viewer-specific restrictions)
- AnimeshManager (animated mesh playback; very low priority)
- MarketplaceManager (marketplace listings; very low priority, SL-specific)

## Proposed Additional Tasks

These would extend the existing 37-task plan (6 phases) to reach full parity.

### Phase 7: Friends + Groups + Avatars

- 7.1 Implement `friends.rs`: friend list tracking, OfferFriendship, AcceptFriendship, DeclineFriendship, TerminateFriendship, GrantRights, MapFriend, TrackFriend, OnlineNotification, CallingCards. Events: FriendOnline, FriendOffline, FriendshipOffered, FriendshipResponse, FriendshipTerminated
- 7.2 Unit tests for friends: verify friendship offer/accept packet construction, friend list parsing
- 7.3 Implement `groups.rs`: RequestCurrentGroups, RequestGroupProfile, RequestGroupMembers, Invite, ActivateGroup, ActivateTitle, JoinGroup, LeaveGroup, CreateGroup, roles (create/update/delete, add/remove member), notices (list/request/send), group proposals, ban actions
- 7.4 Unit tests for groups: verify group profile request packet, role assignment packets
- 7.5 Implement `avatars.rs`: RequestAvatarName/Names, GetDisplayNames, RequestAvatarProperties, RequestAvatarNameSearch, AvatarNotes, AvatarPicks, AvatarClassified, PickInfo, ClassifiedInfo, TrackAgent, AgentProfile
- 7.6 Unit tests for avatars: verify name request packet, display name cap request
- 7.7 Add FriendOnline, FriendOffline, FriendshipOffered, FriendshipTerminated, GroupInvitation, AvatarPropertiesReply, DisplayNameUpdate to Event enum
- 7.8 Integration test: two bots, bot A offers friendship to bot B, bot B accepts, verify both friend lists update

### Phase 8: Directory + Sound + Terrain + Environment

- 8.1 Implement `directory.rs`: SearchPeople, SearchGroups, SearchDirPlaces, SearchClassifieds, SearchEvents, SearchLand, SearchPlaces, GetEventInfo. Async streaming results.
- 8.2 Unit tests for directory: verify search request packets, parse search result packets
- 8.3 Implement `sound.rs`: PlaySound, SendSoundTrigger (with position, gain). Handle AttachedSound, SoundTrigger, PreloadSound inbound packets. Events: SoundTriggered, AttachedSoundChanged
- 8.4 Implement `terrain.rs`: handle LandPatch packets, store heightmap per region, GetTerrainMaterialOverrides, SetTerrainMaterialOverrides
- 8.5 Implement `environment.rs`: GetRegionEnvironment, SetRegionEnvironment, GetParcelEnvironment, SetParcelEnvironment, ResetParcelEnvironment, ResetRegionEnvironment. Handle legacy Windlight too.
- 8.6 Unit tests for sound, terrain, environment: verify packet construction, heightmap parsing
- 8.7 Add SoundTriggered, LandPatchReceived, EnvironmentChanged to Event enum

### Phase 9: Agent Subsystem Completions

- 9.1 Add camera control to agent.rs: AgentCamera struct with position/orientation, Roll/Pitch/Yaw/LookAt/LookDirection methods, FOV setting
- 9.2 Add grab/touch to agent.rs: Grab, GrabUpdate, DeGrab, Touch packets
- 9.3 Add mute list to agent.rs: RequestMuteList, UpdateMuteListEntry, RemoveMuteListEntry, MuteList storage
- 9.4 Add viewer effects to agent.rs: PointAtEffect, LookAtEffect, BeamEffect, SphereEffect
- 9.5 Add gestures to agent.rs: PlayGesture, ActivateGesture, DeactivateGesture
- 9.6 Add money transactions to agent.rs: RequestBalance, GiveAvatarMoney, GiveObjectMoney, GiveGroupMoney, PayUploadFee
- 9.7 Add profile management to agent.rs: UpdateProfile, UpdateProfileNotes, UpdateInterests, pick CRUD, classified CRUD
- 9.8 Add misc agent methods: SetHome, SetDisplayName, SetHoverHeight, ScriptQuestionReply, GroupInviteRespond, SetAgentAccess, AgentPreferences
- 9.9 Add region crossing support: crossing state machine, retry logic, old/new simulator handoff
- 9.10 Unit tests for phase 9 additions

### Phase 10: Extended Object, Inventory, Parcel, Appearance

- 10.1 Extend objects.rs: BuyObject, RequestPayPrice, ClickObject, SetFlags, SetSaleInfo, AddTree, AddGrass, SetTextures, SetLight, SetFlexible, SetSculpt, SetExtraParamOff, AttachObject, DropObject, DetachObjects, DeedObject, SetPermissions, SetObjectsGroup, SetShape, SetMaterial, object media (navigate/update/request), materials (GLTF + legacy)
- 10.2 Extend inventory.rs: task inventory (RequestTaskInventory, MoveTaskInventory, RemoveTaskInventory, Get/SetScriptRunning), CopyItem, CreateLink, FindFolderForType, EmptyTrash, EmptyLostAndFound, MeshUpload, MaterialCreate
- 10.3 Extend parcels.rs: Buy, Reclaim, DeedToGroup, ReturnObjects, Subdivide, Join, Terraform, EjectUser, FreezeUser, RequestObjectOwners, RequestAllSimParcels, RequestDwell, CurrentParcel tracking, ParcelAccessList, ReleaseParcel, StartAuction
- 10.4 Extend appearance.rs: AddToOutfit/RemoveFromOutfit (multi-item), ReplaceOutfit, WearOutfit, RequestAgentWorn, RequestCachedBakes, COF folder, GetAttachmentsByAttachmentPoint, AddAttachments
- 10.5 Extend chat.rs: RetrieveInstantMessages (offline IMs), ReplyToScriptDialog, StartIMConference, ChatterBoxAcceptInvite
- 10.6 Extend assets.rs: RequestAssetXfer, RequestInventoryAsset, texture pipeline (RequestImage with priority/discard, cancel), RequestMesh, baked texture upload, large texture upload, asset cache
- 10.7 Add multi-sim support: connect to child simulators, region crossing handoff, multi-sim object tracking, InterestListManager (SetMode, Reset)
- 10.8 Unit tests for all phase 10 extensions
- 10.9 Integration tests: buy object, task inventory script start/stop, parcel terraform, friend tracking across regions

### Phase 11 (Optional, Low Priority)

- 11.1 OAR/IAR archive reading (archiving.rs): parse OAR/IAR tar streams, extract objects, assets, terrain
- 11.2 Experience API (experience.rs): full experience query/update/permission management
- 11.3 MarketplaceManager (marketplace.rs): listing CRUD, activate/deactivate

## Task Count Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| Phases 1-6 (existing) | 37 | Core bot: login, chat, movement, objects, inventory, appearance, grid, multi-bot, refactor |
| Phase 7 | 8 | Friends, groups, avatars |
| Phase 8 | 7 | Directory, sound, terrain, environment |
| Phase 9 | 10 | Agent subsystem completions (camera, grab, mute, effects, gestures, money, profile, region crossing) |
| Phase 10 | 9 | Extended object, inventory, parcel, appearance, chat, assets, multi-sim |
| Phase 11 (optional) | 3 | Archiving, experiences, marketplace |
| **Total** | **74** | **Full LibreMetaverse parity (37 new tasks beyond existing plan)** |
