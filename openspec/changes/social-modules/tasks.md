## 1. Groups data model and service

- [ ] 1.1 Groups data model: GroupRecord, GroupMembershipData, GroupRolesData, GroupRoleMembersData, GroupNoticeData, GroupInviteData; sqlx migrations matching C# schema
- [ ] 1.2 GroupsService: CRUD for groups, roles, membership, invitations, notices, proposals, voting; database queries via sqlx
- [ ] 1.3 GroupsService HTTP handlers: REST endpoints matching C# Robust format for mixed-grid compatibility

## 2. Groups region module

- [ ] 2.1 GroupsModule (region-side): IGroupsModule trait, handle group packets (CreateGroupRequest, UpdateGroupInfo, JoinGroupRequest, LeaveGroupRequest, GroupRoleDataRequest, etc.), caps for GroupActiveProposals/GroupVoteHistory/GroupAccountSummary
- [ ] 2.2 GroupsMessagingModule: group IM routing via MessageTransferModule, group notices with attachments, session-based group chat
- [ ] 2.3 HGGroupsServiceConnector: Hypergrid group federation, ForeignImporter for cross-grid membership, RemoteConnectorCacheWrapper for caching

## 3. OfflineIM

- [ ] 3.1 OfflineIMService: store/retrieve GridInstantMessage rows, sqlx backend, configurable retention period, remote connector for Robust
- [ ] 3.2 OfflineIMRegionModule: hook MessageTransferModule undelivered callback, store offline, deliver on user login event

## 4. User Profiles

- [ ] 4.1 UserProfilesData model and service: profile text, partner, interests, picks, classifieds, notes, user preferences; sqlx backend matching C# schema
- [ ] 4.2 BasicProfileModule (region-side): AvatarPropertiesRequest/Update handlers, AvatarInterestsUpdate, picks/classifieds caps, user notes
- [ ] 4.3 UserProfilesService HTTP handlers: REST endpoints matching C# Robust format

## 5. NPCs

- [ ] 5.1 NPCAvatar: implement IClientAPI trait as fake viewer — only methods actually called by NPCModule get real bodies, rest are no-ops
- [ ] 5.2 NPCModule: create/move/say/sit/stand/remove/rotate/touch NPCs, appearance cloning, persistence option, wire to OSSL osNpc* function stubs
- [ ] 5.3 NPC tests: create-move-say round trip, appearance cloning, osNpc* OSSL integration, cleanup on region shutdown

## 6. Ancillary modules

- [ ] 6.1 ConciergeModule: lobby/welcome announcements based on parcel entry, HTTP broker notifications for external systems
- [ ] 6.2 IRCBridgeModule: IRC-to-in-world chat bridge using `irc` crate, channel mapping, user nick formatting, reconnect handling

## 7. Testing and security

- [ ] 7.1 Integration tests: group create/join/leave/IM round trip, offline IM store/deliver, profile update/query, NPC lifecycle
- [ ] 7.2 Protocol replay tests: group packets, profile caps, offline IM from C# recordings
- [ ] 7.3 SECURITY REVIEW: group role/power permission checks (HIGH RISK, Opus reviewer), NPC command validation, profile text sanitization
