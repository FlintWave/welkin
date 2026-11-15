# Viewer Messaging and Interaction — Design

## Architecture

```
Viewer A                     Region Server                      Viewer B
   |                              |                                |
   |--ImprovedInstantMessage----->|                                |
   |  (Dialog=0, ToAgentID=B)    |                                |
   |                              |--[presence check]------------->|
   |                              |  online? relay IM              |
   |                              |  offline? store via ROBUST     |
   |                              |                                |
   |--ViewerEffect--------------->|--ViewerEffect (broadcast)----->|
   |  (typing, look-at)          |                                |
   |                              |                                |
   |--UUIDNameRequest------------>|                                |
   |  (batch of UUIDs)           |--[user accounts lookup]        |
   |<-UUIDNameReply--------------|                                |
   |                              |                                |
   |--AgentRequestSit------------>|                                |
   |  (TargetID, Offset)         |--[find object, calc position]  |
   |<-AvatarSitResponse----------|                                |
   |--AgentSit------------------->|                                |
   |                              |--ObjectUpdate (sitting)------->|
```

## ImprovedInstantMessage dispatch

The packet carries a `Dialog` field (u8) that determines how the message is
processed. OpenSim's `InstantMessageDialog` enum defines the values. The
handler decodes the packet once and dispatches on Dialog:

| Dialog | Name | Action |
|--------|------|--------|
| 0 | MessageFromAgent | Check target presence. Online: relay IM packet. Offline: POST to ROBUST offline IM service. |
| 3 | GroupInvitation | Forward to GroupsModule.InviteGroup(). |
| 4 | InventoryOffered | Create InventoryOfferedIM record, relay to target. Target's accept/decline comes back as Dialog 4 response. |
| 5 | InventoryAccepted | Relay acceptance notification to offerer. |
| 6 | InventoryDeclined | Relay decline notification, clean up pending offer. |
| 17 | SessionGroupStart | Create or join a group IM session. Track session membership in memory. |
| 20 | SessionSend | Relay message to all session participants (group members online). |
| 22 | GroupNotice | Send group notice via GroupsModule, relay to online members. |
| 23 | GroupNoticeRequested | Return stored group notice to requester. |
| 24 | GroupNoticeInventoryAccepted | Attach notice inventory item to accepting user. |
| 32 | BusyAutoResponse | Relay auto-reply text to sender. |
| 38 | FriendshipOffered | Call FriendsModule.OfferFriendship(), relay to target. |
| 39 | FriendshipAccepted | Call FriendsModule.ApproveFriendship(), notify both parties. |
| 40 | FriendshipDeclined | Call FriendsModule.DenyFriendship(), notify offerer. |
| 41 | StartTyping | Relay typing indicator to target agent. |
| 42 | StopTyping | Relay stop-typing indicator to target agent. |
| 50 | RequestTeleport | Forward to EntityTransferModule as teleport lure. |
| 253 | GodLikeRequestTeleport | God-mode teleport (admin only). |

Unknown dialog types are logged and dropped.

### Cross-region IM routing

When the target agent is on a different region (same grid), the IM is forwarded
via the ROBUST messaging service (HTTP POST to the target region's IM endpoint).
The target region then delivers locally. This uses the same `InstantMessage` XML
structure that offline IM uses, just delivered synchronously.

## ViewerEffect

The ViewerEffect packet carries typed visual effects:

| Type | Name | Handling |
|------|------|----------|
| 0 | Text | Relay to all agents in region |
| 1 | Icon | Relay |
| 2 | Connector | Relay (beam between two points) |
| 3 | FlexibleObject | Relay |
| 4 | AnimalControls | Relay |
| 9 | PointAt | Relay — viewer shows point-at indicator |
| 10 | LookAt | Relay — viewer shows look-at indicator |
| 11 | Beam | Relay — colored beam between agent and target |

All ViewerEffect types are relayed to every other agent in the region without
server-side processing. The server does not interpret or validate the effect
data beyond framing.

## UUIDNameRequest / UUIDNameReply

The viewer sends batches of UUIDs it needs names for. The server queries the
user accounts service for each UUID and returns `UUIDNameReply` with
`(UUID, FirstName, LastName)` tuples. For agents not found, return
`("Unknown", "User")`. Cache results in a region-local LRU cache (TTL 5 min)
to avoid hammering ROBUST on repeated lookups.

Reference: `LLClientView.HandleUUIDNameRequest`, `UserManagementModule.HandleUUIDNameRequest`.

## UUIDGroupNameRequest / UUIDGroupNameReply

Same pattern as UUID name lookup but for group UUIDs. Query the groups service,
return `(GroupID, GroupName)` tuples.

## AvatarPropertiesRequest / AvatarPropertiesReply

Viewer requests another user's profile. Query ROBUST profiles service for:
- AboutText, FLAboutText (first life)
- ImageID, FLImageID
- PartnerID
- BornOn date
- CharterMember flag
- Flags (online status visibility, etc.)

Return AvatarPropertiesReply packet with all fields populated.

Reference: `AvatarPropertiesRequestHandler` in OpenSim profiles module.

## Sitting

### AgentRequestSit

1. Look up target object by `TargetID` in the scene graph.
2. If the object has explicit sit targets (set by `llSitTarget`), use that
   position and rotation.
3. If no sit target, calculate a default sit position on the object's top
   surface (bounding box top center, offset by avatar hip height).
4. Check permissions (object must allow sit, not full, etc.).
5. Send `AvatarSitResponse` with `SitObject = TargetID`,
   `SitPosition = calculated`, `SitRotation`, `AutoPilot = true/false`.

### AgentSit

Received after the viewer has completed the autopilot/animation transition.
1. Set the avatar's parent to the sit target object.
2. Set avatar state to sitting (SitGround or SitObject).
3. Broadcast ObjectUpdate with the avatar's new parent linkage so other
   viewers see the seated avatar.
4. Fire `changed(CHANGED_LINK)` event on the sit target's scripts.

### Stand

When the viewer sends AgentUpdate with the stand flag, or the user clicks
Stand:
1. Clear avatar's parent linkage.
2. Set avatar state to standing.
3. Position avatar at the stand-up position (beside the object).
4. Broadcast ObjectUpdate with updated position.
5. Fire `changed(CHANGED_LINK)` on the previously-sat-upon object.

## ScriptDialogReply

When a user clicks a button on a script dialog (from `llDialog`), the viewer
sends ScriptDialogReply with:
- `ObjectID`: the object that sent the dialog
- `ChatChannel`: the channel the script is listening on
- `ButtonLabel`: the text of the clicked button

The handler fires a `chat` event on the specified channel with the button
label as the message, source type `AGENT`, and the clicking user as sender.
This is how `llDialog` + `listen()` works in LSL.

## Decisions

1. **IM dispatch is a single match statement**, not a chain of modules. Each
   arm calls into the appropriate service. This matches OpenSim's approach
   and keeps the routing visible in one place.
2. **ViewerEffect is relay-only.** The server never interprets effect data.
3. **Name cache is region-local, not global.** Each region server caches
   independently. The cache is small (a few thousand entries) and the TTL
   is short enough that name changes propagate within minutes.
4. **Sit position calculation copies OpenSim's logic** from
   `ScenePresence.HandleAgentRequestSit` and `SOPObject.GetSitTarget`.
