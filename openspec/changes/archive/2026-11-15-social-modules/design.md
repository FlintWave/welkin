# social-modules: Groups, OfflineIM, profiles, NPCs

## Summary

Port the social subsystems from OpenSimulator 0.9.3.0: Groups (V2 only),
OfflineIM, user profiles, NPCs, and ancillary modules (Concierge, IRC bridge).
~21,600 lines of C# across ~35 files. Groups is the largest at ~13,450 lines;
only the Addons/Groups V2 implementation is ported (the legacy XmlRpc
connector is dropped).

## Scope

### In scope

- **Groups V2** (Addons/Groups, 9,194 lines): `GroupsModule` (region-side),
  `GroupsMessagingModule` (group IM and notices), `GroupsService` (backend with
  database), group roles, powers, membership, proposals, voting, invitations.
  Remote and HG connectors.
- **OfflineIM** (831 lines): `OfflineIMRegionModule` stores IMs for offline
  users, `OfflineIMService` with database backend, remote connector for Robust.
- **User Profiles** (2,417 lines): `BasicProfileModule` (region stub),
  `UserProfilesService` (full service), profile text, partner, interests,
  picks, classifieds, user notes, user preferences.
- **NPCs** (2,375 lines): `NPCModule` (create/move/say/sit/stand/remove),
  `NPCAvatar` (IClientAPI fake viewer implementation), integration with OSSL
  `osNpc*` function stubs.
- **Concierge** (621 lines): lobby announcements, HTTP broker notifications.
- **IRC bridge** (2,206 lines): IRC-to-in-world chat bridge using the `irc`
  crate (MPL-2.0).

### Out of scope

- Legacy XmlRpc groups connector (3,991 lines, replaced by V2).
- Matrix/Discord chat bridges (future enhancement).
- Group land and group deeding (partially in region-modules permissions).

## Dependencies

- IM routing: `MessageTransferModule` (region-modules section 7)
- UserManagement: (region-modules section 12)
- AvatarFactory and Attachments: (region-modules sections 6, 10)
- OSSL `osNpc*` stubs: (ossl-api)
- Service connectors: (region-modules section 12)
- Capabilities system: (region-simulator-core)

## Crates

- `irc` (crates.io, MPL-2.0): IRC client for the bridge module.
- Reuse existing `welkin-services`, `welkin-region`, `sqlx`.

## Design

### Groups V2 architecture

```
GroupsModule (region) ←→ GroupsServiceRemoteConnector ←→ GroupsService (Robust)
                                                              ↓
                                                         GroupsData (sqlx)
GroupsMessagingModule: group IMs via MessageTransferModule
HGGroupsService: Hypergrid group federation
```

Only the V2 data model is ported. The service exposes the same HTTP endpoints
as C# Robust for compatibility with mixed grids during cutover.

### NPC architecture

`NPCAvatar` implements the `IClientAPI` trait as a no-op fake viewer. The
`NPCModule` creates NPCAvatar instances and drives them through the scene
graph like real avatars. OSSL `osNpc*` functions call into NPCModule.

### Key decisions

- S1: Port Groups V2 only. The V2 implementation is cleaner, supports HG, and
  is the default in 0.9.3.0. The XmlRpc connector is legacy.
- S2: NPC avatar stubs — `NPCAvatar` has ~1,400 lines of `IClientAPI` stubs.
  Implement only methods actually called by NPCModule; the rest are no-ops.
- S3: IRC bridge uses the `irc` crate (MPL-2.0, GPLv3-compatible) instead of
  porting the C# IRC client code.
- S4: OfflineIM uses the same `sqlx` backend as other services, no separate
  storage engine.

## Security considerations

- **Group permissions** (HIGH): group roles and powers control land access,
  object manipulation, and estate privileges. The permission checks must match
  C# behavior exactly.
- **NPC command injection**: OSSL `osNpc*` functions must validate NPC names
  and prevent NPCs from executing privileged operations.
- **Profile injection**: user-submitted profile text, picks, and classifieds
  must be sanitized for storage (no SQL injection via sqlx parameterization).
