# Viewer Messaging and Interaction

## Summary

Implement the ImprovedInstantMessage packet handler (20+ dialog types), ViewerEffect
relay, UUID name/group lookups, avatar sitting, profile queries, and script dialog
replies. These are the communication and interaction primitives every viewer session
depends on.

## Motivation

Without ImprovedInstantMessage, users cannot send personal IMs, participate in group
chat, accept inventory offers, manage friendships, or respond to teleport lures.
Without ViewerEffect relay, typing indicators and look-at/point-at are invisible to
other users. Without UUIDNameRequest, the viewer shows raw UUIDs instead of names in
chat, IMs, and object inspectors. Without sitting, users cannot use furniture or
vehicles. These are all P0 or P1 for any usable grid.

## Scope

### In scope

- **ImprovedInstantMessage** handler with full dialog-type dispatch:
  - Dialog 0 (MessageFromAgent): personal IM relay, offline storage
  - Dialog 3 (GroupInvitation): forward to groups service
  - Dialog 4 (InventoryOffered): pending offer, relay to target
  - Dialog 38 (FriendshipOffered): forward to friends service
  - Dialog 39 (FriendshipAccepted): update friends service, grant online visibility
  - Dialog 40 (FriendshipDeclined): update friends service
  - Dialog 41/42 (StartTyping/StopTyping): relay typing indicator
  - Dialog 50 (TeleportLure): forward to entity transfer module
  - Dialog 17 (SessionGroupStart): group IM session creation
  - Dialog 20 (SessionSend): group IM message relay
  - Dialog 22/23/24 (GroupNotice*): group notice distribution
  - Dialog 32 (BusyAutoResponse): auto-reply relay
- **ViewerEffect** relay between all agents in the same region
- **UUIDNameRequest** — batch name lookup, send UUIDNameReply
- **UUIDGroupNameRequest** — group name lookup, send UUIDGroupNameReply
- **AvatarPropertiesRequest** — query profiles service, send AvatarPropertiesReply
- **AgentRequestSit / AgentSit** — sit target resolution, position calculation, state transition
- **ScriptDialogReply** — forward user's dialog button choice to the script engine

### Out of scope

- Group creation and management (covered by `social-modules`)
- Offline IM storage service (already implemented in ROBUST)
- Friends list display on login (already in login response)
- Group chat moderation (session admin commands)

## Phase

Cross-cutting, required before any multi-user grid testing. Depends on
`protocol-core` (LLUDP framing) and `grid-services` (ROBUST friends, groups,
profiles, presence). No dependency on script engine for core IM functionality.

## Risk

Medium. ImprovedInstantMessage is the most complex single packet in the protocol —
its Dialog field selects from 20+ code paths, each with different forwarding rules,
service interactions, and viewer expectations. The reference implementation in
OpenSim's `LLClientView.cs` and `MessageTransferModule.cs` must be followed closely
to avoid subtle routing bugs. Individual handlers are straightforward once the
dispatch is correct.
