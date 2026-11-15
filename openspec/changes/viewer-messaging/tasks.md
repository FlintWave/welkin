# viewer-messaging tasks

## Phase 1: Core IM (P0)

- [x] Handle ImprovedInstantMessage — decode dialog type, route by match statement
- [x] Dialog 0 (MessageFromAgent): personal IM relay (online → relay, offline → store via ROBUST)
- [x] Dialog 41/42 (StartTyping/StopTyping): relay typing indicator to target agent
- [x] Dialog 38 (FriendshipOffered): forward to friends service, relay to target
- [x] Dialog 39 (FriendshipAccepted): update friends service, notify both parties
- [x] Dialog 40 (FriendshipDeclined): update friends service, notify offerer
- [x] Dialog 4/5/6 (InventoryOffered/Accepted/Declined): create pending offer, relay, handle accept/decline
- [x] Dialog 22 (RequestTeleport): forward teleport lure to entity transfer module
- [x] Dialog 20 (BusyAutoResponse/SessionSend): relay auto-reply text to sender

## Phase 2: Group messaging

- [ ] Dialog 17 (SessionGroupStart): create/join group IM session, track membership in memory
- [ ] Dialog 20 with from_group (SessionSend): relay group IM to all online session participants
- [ ] Dialog 32 (GroupNotice): send group notice via groups service, relay to online members
- [ ] Dialog 37 (GroupNoticeRequested): return stored group notice to requester
- [ ] Dialog 33 (GroupNoticeInventoryAccepted): attach notice inventory item to accepting user
- [ ] Dialog 3 (GroupInvitation): forward to groups service invite flow

## Phase 3: Identity and interaction

- [ ] Handle UUIDNameRequest — batch name lookup against user accounts service, send UUIDNameReply
- [ ] Handle UUIDGroupNameRequest — group name lookup against groups service, send UUIDGroupNameReply
- [ ] Handle AvatarPropertiesRequest — query profiles service, send AvatarPropertiesReply
- [ ] Handle ViewerEffect — relay to all agents in region (no server-side interpretation)
- [ ] Handle AgentRequestSit — find sit target, check permissions, calculate sit position, send AvatarSitResponse
- [ ] Handle AgentSit — transition avatar to sitting state, set parent linkage, broadcast ObjectUpdate
- [ ] Implement stand logic — clear parent, reposition avatar, broadcast update
- [ ] Handle ScriptDialogReply — fire chat event on specified channel with button label
