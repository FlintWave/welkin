# Social modules summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Decisions](decisions.md#sm1) · Previous: [Archives](archives-summary.md) · Next: [Hypergrid](hypergrid-summary.md)

Status: accepted (19 of 19 tasks, 7 sections, 444 tests). Security review
verdict: CONDITIONAL PASS. Change: `openspec/changes/social-modules/`.

## What was built

The social subsystems from `OpenSim/Addons/Groups/` (~9,194 lines),
`OpenSim/Region/OptionalModules/Avatar/` (~5,600 lines) and supporting
services (~6,800 lines), totalling ~21,600 lines of C# across ~35 files.
Ported as ~12,500 lines of Rust with 444 tests.

| Section | Area | Tasks |
|---|---|---|
| 1 | Groups V2 data model, service, HTTP handlers | 1.1-1.3 |
| 2 | Groups region module, messaging, HG connector | 2.1-2.3 |
| 3 | OfflineIM service and region module | 3.1-3.2 |
| 4 | User profiles data, service, region module | 4.1-4.3 |
| 5 | NPCs: NPCAvatar, NPCModule, tests | 5.1-5.3 |
| 6 | Concierge and IRC bridge | 6.1-6.2 |
| 7 | Integration tests, protocol replay, security review | 7.1-7.3 |

## Architecture

### Groups V2

```text
GroupsModule (region) ←→ GroupsServiceRemoteConnector ←→ GroupsService (Robust)
                                                              ↓
                                                         GroupsData (sqlx)
GroupsMessagingModule: group IMs via MessageTransferModule
HGGroupsServiceConnector: Hypergrid group federation
```

Only the V2 data model is ported. The service exposes the same HTTP endpoints
as C# Robust for mixed-grid compatibility during cutover. The legacy XmlRpc
connector (3,991 lines) is dropped.

### NPCs

`NPCAvatar` implements the `IClientAPI` trait as a no-op fake viewer. Only
methods actually called by `NPCModule` have real bodies; the rest are no-ops.
OSSL `osNpc*` functions call into `NPCModule`.

### Module breakdown

| Module | Lines | Tests | Responsibility |
|---|---|---|---|
| `groups.rs` (service) | 2,201 | 14 | Data model, CRUD, HTTP handlers |
| `groups/mod.rs` (module) | 1,497 | 20 | Region-side GroupsModule, packet handling, caps |
| `groups/messaging.rs` | 756 | 16 | Group IM routing, notices with attachments, session chat |
| `groups/hg_connector.rs` | 1,262 | 20 | HG federation, ForeignImporter, cache wrapper |
| `offline_im.rs` | 929 | 17 | Service + module, store/retrieve, login delivery |
| `profiles.rs` | 951 | 11 | Data model, service, BasicProfileModule, picks, classifieds |
| `npc_avatar.rs` | 241 | - | IClientAPI fake viewer |
| `npc/mod.rs` | 742 | 12 | Create/move/say/sit/stand/remove, appearance cloning |
| `concierge.rs` | 536 | 10 | Lobby announcements, HTTP broker notifications |
| `irc_bridge.rs` | 761 | 13 | IRC-to-in-world chat, channel mapping, reconnect |
| Integration tests | 1,408 | 36 | Group round-trip, offline IM, profiles, NPC lifecycle |
| Protocol replay | 961 | 17 | Group packets, profile caps, offline IM from C# recordings |

## Key decisions

- **SM1:** Port Groups V2 only. The V2 implementation is cleaner, supports HG,
  and is the default in 0.9.3.0. The XmlRpc connector is legacy (3,991 lines
  dropped).
- **SM2:** NPC avatar stubs: `NPCAvatar` has ~1,400 lines of `IClientAPI`
  stubs in C#. Only methods actually called by NPCModule are implemented; the
  rest are no-ops (241 lines vs 1,400).
- **SM3:** IRC bridge uses the `irc` crate (MPL-2.0, GPLv3-compatible) instead
  of porting the C# IRC client code.
- **SM4:** OfflineIM uses the same `sqlx` backend as other services, no
  separate storage engine.

## Tests

444 tests total (391 unit/lib + 36 integration + 17 protocol replay). Integration
tests cover group create/join/leave/IM round trip, offline IM store/deliver,
profile update/query, and NPC lifecycle. Protocol replay tests verify wire
compatibility against C# recordings.

## Security review

> [!WARNING]
> **CONDITIONAL PASS**: 0 critical, 3 high, 10 medium tracked for remediation.
> See [security review](security-review-social-modules.md) for details.

Full report: [security-review-social-modules.md](security-review-social-modules.md).

| ID | Severity | Finding |
|---|---|---|
| H-1 | HIGH | IRC protocol injection via avatar names and chat messages |
| H-2 | HIGH | Group creation missing role power check at service layer |
| H-3 | HIGH | Group notice attachment size unbounded |

All 3 HIGH findings require remediation before production. The IRC bridge must
not go live until H-1 is fixed. 10 MEDIUM findings relate to missing size limits
and input sanitization.

## Commits

| Hash | Description |
|---|---|
| `e3dc382` | Sections 1, 3-6: Groups V2 service, OfflineIM, Profiles, NPCs, Concierge, IRC bridge |
| `4061c14` | Section 2: Groups V2 region modules: GroupsModule, messaging, HG connector |
| `e5997a1` | Section 7: integration tests, protocol replay, security review |
