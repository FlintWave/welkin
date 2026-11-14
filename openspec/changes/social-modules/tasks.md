## 1. Groups data model and service

- [x] 1.1 Groups data model: GroupRecord, GroupMembershipData, GroupRolesData, GroupRoleMembersData, GroupNoticeData, GroupInviteData; sqlx migrations matching C# schema (e3dc382: groups.rs, 904 lines, 6 tests)
- [x] 1.2 GroupsService: CRUD for groups, roles, membership, invitations, notices, proposals, voting; database queries via sqlx (e3dc382: groups.rs service, 1,297 lines, 8 tests)
- [x] 1.3 GroupsService HTTP handlers: REST endpoints matching C# Robust format for mixed-grid compatibility (e3dc382: 18 HTTP handler methods)

## 2. Groups region module

- [x] 2.1 GroupsModule (region-side): IGroupsModule trait, handle group packets (CreateGroupRequest, UpdateGroupInfo, JoinGroupRequest, LeaveGroupRequest, GroupRoleDataRequest, etc.), caps for GroupActiveProposals/GroupVoteHistory/GroupAccountSummary (4061c14: groups/mod.rs 1,497 lines, 20 tests)
- [x] 2.2 GroupsMessagingModule: group IM routing via MessageTransferModule, group notices with attachments, session-based group chat (4061c14: groups/messaging.rs 756 lines, 16 tests)
- [x] 2.3 HGGroupsServiceConnector: Hypergrid group federation, ForeignImporter for cross-grid membership, RemoteConnectorCacheWrapper for caching (4061c14: groups/hg_connector.rs 1,262 lines, 20 tests)

## 3. OfflineIM

- [x] 3.1 OfflineIMService: store/retrieve GridInstantMessage rows, sqlx backend, configurable retention period, remote connector for Robust (e3dc382: offline_im.rs data 176 lines + service 262 lines, 6 tests)
- [x] 3.2 OfflineIMRegionModule: hook MessageTransferModule undelivered callback, store offline, deliver on user login event (e3dc382: offline_im.rs module 491 lines, 11 tests)

## 4. User Profiles

- [x] 4.1 UserProfilesData model and service: profile text, partner, interests, picks, classifieds, notes, user preferences; sqlx backend matching C# schema (e3dc382: profiles.rs extended with UserAppData, 434 lines)
- [x] 4.2 BasicProfileModule (region-side): AvatarPropertiesRequest/Update handlers, AvatarInterestsUpdate, picks/classifieds caps, user notes (e3dc382: profiles.rs module 517 lines, 11 tests)
- [x] 4.3 UserProfilesService HTTP handlers: REST endpoints matching C# Robust format (pre-existing: all JSON-RPC methods confirmed complete)

## 5. NPCs

- [x] 5.1 NPCAvatar: implement IClientAPI trait as fake viewer — only methods actually called by NPCModule get real bodies, rest are no-ops (e3dc382: npc_avatar.rs 241 lines)
- [x] 5.2 NPCModule: create/move/say/sit/stand/remove/rotate/touch NPCs, appearance cloning, persistence option, wire to OSSL osNpc* function stubs (e3dc382: npc/mod.rs 742 lines)
- [x] 5.3 NPC tests: create-move-say round trip, appearance cloning, osNpc* OSSL integration, cleanup on region shutdown (e3dc382: 12 tests)

## 6. Ancillary modules

- [x] 6.1 ConciergeModule: lobby/welcome announcements based on parcel entry, HTTP broker notifications for external systems (e3dc382: concierge.rs 536 lines, 10 tests)
- [x] 6.2 IRCBridgeModule: IRC-to-in-world chat bridge using `irc` crate, channel mapping, user nick formatting, reconnect handling (e3dc382: irc_bridge.rs 761 lines, 13 tests)

## 7. Testing and security

- [ ] 7.1 Integration tests: group create/join/leave/IM round trip, offline IM store/deliver, profile update/query, NPC lifecycle
- [ ] 7.2 Protocol replay tests: group packets, profile caps, offline IM from C# recordings
- [ ] 7.3 SECURITY REVIEW: group role/power permission checks (HIGH RISK, Opus reviewer), NPC command validation, profile text sanitization
