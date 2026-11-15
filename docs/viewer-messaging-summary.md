<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Viewer messaging and interaction

[← Back to docs](README.md) · [Architecture](architecture.md) · [Viewer compatibility](viewer-compatibility.md)

Status: complete (23 of 23 tasks, 3 phases). Change:
`openspec/changes/viewer-messaging/`.

## What was built

The viewer communication and interaction layer: ImprovedInstantMessage
dispatch (20+ dialog types), ViewerEffect relay, UUID name and group
lookups, avatar profiles, sitting, and script dialog replies. These are
the primitives every multi-user viewer session depends on.

Source reference: `OpenSim/Region/ClientStack/Linden/UDP/LLClientView.cs`
and `OpenSim/Region/CoreModules/Avatar/InstantMessage/MessageTransferModule.cs`.

| Phase | Area | Tasks |
|---|---|---|
| 1 | Core IM: personal relay, typing, friendship, inventory offers, teleport lure | 1-9 |
| 2 | Group messaging: session create/join, group IM relay, notices, invitations | 10-15 |
| 3 | Identity and interaction: name lookups, profiles, ViewerEffect, sitting, ScriptDialogReply | 16-23 |

## Architecture

All viewer messaging goes through the region server. Incoming packets are
decoded once and dispatched by type.

ImprovedInstantMessage carries a `Dialog` field (u8) that selects from
20+ code paths. The handler is a single match statement, each arm calling
into the appropriate service (friends, groups, entity transfer, presence).
When the target agent is on a different region, the IM is forwarded via
the ROBUST messaging service over HTTP.

ViewerEffect packets (typing indicators, look-at, point-at, beams) are
relayed to every other agent in the region without server-side
interpretation.

| Component | Responsibility |
|---|---|
| IM dispatch | Match on Dialog field, route to service (friends, groups, presence, transfer) |
| Offline storage | POST to ROBUST offline IM service when target is not online |
| Group sessions | Track session membership in memory, relay messages to online participants |
| ViewerEffect relay | Broadcast to all agents in region, no interpretation |
| UUIDNameRequest | Batch name lookup against user accounts service, region-local LRU cache (5 min TTL) |
| UUIDGroupNameRequest | Group name lookup against groups service |
| AvatarPropertiesRequest | Query ROBUST profiles service, return full profile |
| Sitting | Sit target resolution, position calculation, parent linkage, ObjectUpdate broadcast |
| ScriptDialogReply | Fire chat event on the script's listen channel with the clicked button label |

### IM dialog dispatch

| Dialog | Name | Action |
|---|---|---|
| 0 | MessageFromAgent | Online: relay. Offline: store via ROBUST |
| 3 | GroupInvitation | Forward to groups service |
| 4/5/6 | InventoryOffered/Accepted/Declined | Create pending offer, relay, handle response |
| 17 | SessionGroupStart | Create or join group IM session |
| 20 | SessionSend | Relay to all online session participants |
| 22/23/24 | GroupNotice variants | Send, retrieve, or accept notice inventory |
| 32 | BusyAutoResponse | Relay auto-reply text |
| 38/39/40 | Friendship offered/accepted/declined | Forward to friends service, notify both parties |
| 41/42 | StartTyping/StopTyping | Relay typing indicator |
| 50 | RequestTeleport | Forward to entity transfer module |

Unknown dialog types are logged and dropped.

## Key decisions

- D1: IM dispatch is a single match statement, not a chain of modules.
  Each arm calls into the appropriate service. This keeps routing visible
  in one place and matches OpenSim's approach.
- D2: ViewerEffect is relay-only. The server never interprets effect data.
- D3: Name cache is region-local, not global. Each region caches
  independently with a short TTL so name changes propagate within minutes.
- D4: Sit position calculation copies OpenSim's logic from
  `ScenePresence.HandleAgentRequestSit` and `SOPObject.GetSitTarget`.

## Commits

| Hash | Description |
|---|---|
| `c6834aa` | Phase 1: ImprovedInstantMessage handler, dialog dispatch, personal IM, friendship, inventory offers |
| `f1b98fa` | Phase 2: group messaging, session management, group notices, invitations |
| `afdecf9` | Phase 3: UUIDNameRequest, ViewerEffect relay, profiles, sitting, ScriptDialogReply |
