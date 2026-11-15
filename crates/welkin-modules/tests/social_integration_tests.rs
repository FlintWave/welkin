// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for social modules: Groups, GroupsMessaging, OfflineIM,
//! Profiles, and NPC.  Exercises cross-module interactions and lifecycle
//! scenarios using in-memory state.

use std::collections::HashMap;

use uuid::Uuid;

use welkin_modules::groups::messaging::{
    ChatSessionState, GroupsMessagingModule, NoticeAttachment,
};
use welkin_modules::groups::{GroupsModule, IGroupsModule};
use welkin_modules::messaging::{GridInstantMessage, IMDialog};
use welkin_modules::module::{ModuleConfig, RegionModule};
use welkin_modules::npc::{INPCModule, NPCModule};
use welkin_modules::offline_im::OfflineIMModule;
use welkin_modules::profiles::{
    AvatarProperties, BasicProfileModule, ClassifiedSummary, PickSummary,
};
use welkin_modules::registry::SceneRef;

use welkin_data::groups::GroupPowers;

// ===========================================================================
// Test config helper
// ===========================================================================

struct TestConfig {
    strings: HashMap<(String, String), String>,
    ints: HashMap<(String, String), i64>,
    bools: HashMap<(String, String), bool>,
}

impl TestConfig {
    fn new() -> Self {
        Self {
            strings: HashMap::new(),
            ints: HashMap::new(),
            bools: HashMap::new(),
        }
    }

    fn set_string(&mut self, section: &str, key: &str, value: &str) {
        self.strings
            .insert((section.to_string(), key.to_string()), value.to_string());
    }

    fn set_int(&mut self, section: &str, key: &str, value: i64) {
        self.ints
            .insert((section.to_string(), key.to_string()), value);
    }

    fn set_bool(&mut self, section: &str, key: &str, value: bool) {
        self.bools
            .insert((section.to_string(), key.to_string()), value);
    }
}

impl ModuleConfig for TestConfig {
    fn get_string(&self, section: &str, key: &str, default: &str) -> String {
        self.strings
            .get(&(section.to_string(), key.to_string()))
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }
    fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
        self.ints
            .get(&(section.to_string(), key.to_string()))
            .copied()
            .unwrap_or(default)
    }
    fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        self.bools
            .get(&(section.to_string(), key.to_string()))
            .copied()
            .unwrap_or(default)
    }
    fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
        default
    }
}

fn scene_ref() -> SceneRef {
    SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
}

fn groups_enabled_config() -> TestConfig {
    let mut c = TestConfig::new();
    c.set_bool("Groups", "Enabled", true);
    c.set_string("Groups", "ServiceURL", "http://localhost:8003");
    c
}

fn messaging_enabled_config() -> TestConfig {
    let mut c = TestConfig::new();
    c.set_bool("Groups", "Enabled", true);
    c.set_string("Groups", "MessagingModule", "GroupsMessagingModule");
    c
}

fn offline_im_config(max_ims: i64) -> TestConfig {
    let mut c = TestConfig::new();
    c.set_bool("Messaging", "OfflineIMEnabled", true);
    c.set_bool("Messaging", "ForwardOfflineGroupMessages", true);
    c.set_int("Messaging", "MaxOfflineIMs", max_ims);
    c
}

fn sample_im(from: Uuid, to: Uuid, dialog: IMDialog, message: &str) -> GridInstantMessage {
    GridInstantMessage {
        from_agent_id: from,
        from_agent_name: "Sender Name".to_string(),
        to_agent_id: to,
        message: message.to_string(),
        im_session_id: Uuid::new_v4(),
        dialog,
        from_group: false,
        offline: 0,
        parent_estate_id: 1,
        position: [128.0, 128.0, 25.0],
        region_id: Uuid::new_v4(),
        timestamp: 1700000000,
        binary_bucket: Vec::new(),
    }
}

// ===========================================================================
// 1. Group lifecycle: create -> enabled/disabled validation
// ===========================================================================

#[test]
fn group_lifecycle_create_via_enabled_module() {
    let mut module = GroupsModule::new();
    let config = groups_enabled_config();
    let scene = scene_ref();
    module.initialise(&config).unwrap();
    module.add_region(&scene).unwrap();

    // Create group through the module.
    let agent = Uuid::new_v4();
    let result = module.handle_create_group_request(
        agent,
        "Integration Test Group",
        "Our charter",
        true,
        Uuid::nil(),
        0,
        true,
        true,
        false,
    );

    assert!(result.is_ok());
    let record = result.unwrap();
    assert_eq!(record.group_name, "Integration Test Group");
    assert_eq!(record.charter, "Our charter");
    assert!(record.open_enrollment);
    assert!(!record.group_id.is_nil());
    assert!(!record.owner_role_id.is_nil());
    assert_eq!(record.member_count, 1); // Founder is auto-member.
    assert_eq!(record.role_count, 3); // Everyone, Officers, Owners.

    // Cleanup.
    module.remove_region(&scene).unwrap();
    module.close().unwrap();
}

// ===========================================================================
// 2. Group module join/leave flow
// ===========================================================================

#[test]
fn group_join_and_leave_flow() {
    let mut module = GroupsModule::new();
    let config = groups_enabled_config();
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let agent = Uuid::new_v4();
    let group_id = Uuid::new_v4();

    // Join group (stub connector accepts any join).
    let join_result = module.handle_join_group_request(agent, group_id);
    assert!(join_result.is_ok());

    // Leave group (stub connector accepts any leave).
    let leave_result = module.handle_leave_group_request(agent, group_id);
    assert!(leave_result.is_ok());
}

// ===========================================================================
// 3. Group invite flow
// ===========================================================================

#[test]
fn group_invite_flow() {
    let mut module = GroupsModule::new();
    let config = groups_enabled_config();
    module.initialise(&config).unwrap();

    let inviter = Uuid::new_v4();
    let invitee = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let role_id = Uuid::nil();
    let invite_id = Uuid::new_v4();

    let ok = module.handle_invite_group_request(inviter, invite_id, group_id, role_id, invitee);
    assert!(ok, "Stub connector should accept invite");
}

// ===========================================================================
// 4. Group permissions: agent_has_power
// ===========================================================================

#[test]
fn group_permissions_agent_has_power() {
    // Disabled module should always return false.
    let module = GroupsModule::new();
    assert!(!module.agent_has_power(Uuid::new_v4(), Uuid::new_v4(), GroupPowers::INVITE));

    // Enabled module with stub connector: no roles stored, so false.
    let mut enabled = GroupsModule::new();
    let config = groups_enabled_config();
    enabled.initialise(&config).unwrap();
    assert!(!enabled.agent_has_power(Uuid::new_v4(), Uuid::new_v4(), GroupPowers::INVITE));
}

// ===========================================================================
// 5. Group disabled operations return errors
// ===========================================================================

#[test]
fn group_disabled_operations_return_error() {
    let module = GroupsModule::new();
    // Not initialised means disabled.

    assert!(
        module
            .handle_create_group_request(
                Uuid::new_v4(),
                "Test",
                "",
                true,
                Uuid::nil(),
                0,
                true,
                true,
                false,
            )
            .is_err()
    );
    assert!(
        module
            .handle_join_group_request(Uuid::new_v4(), Uuid::new_v4())
            .is_err()
    );
    assert!(
        module
            .handle_leave_group_request(Uuid::new_v4(), Uuid::new_v4())
            .is_err()
    );
    assert!(!module.handle_invite_group_request(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::nil(),
        Uuid::new_v4()
    ));
    assert!(module.get_groups_for_agent(Uuid::new_v4()).is_empty());
    assert!(module.get_group_record(Uuid::new_v4()).is_none());
    assert!(module.get_group_roles(Uuid::new_v4()).is_empty());
    assert!(module.get_group_members(Uuid::new_v4()).is_empty());
}

// ===========================================================================
// 6. Group IM round-trip: session start -> send -> fan-out
// ===========================================================================

#[test]
fn group_im_session_start_send_fanout() {
    let mut msg_module = GroupsMessagingModule::new();
    let config = messaging_enabled_config();
    msg_module.initialise(&config).unwrap();
    assert!(msg_module.is_enabled());

    let group_id = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();
    let carol = Uuid::new_v4();

    // All three join the session.
    msg_module.handle_session_group_start(alice, group_id);
    msg_module.handle_session_group_start(bob, group_id);
    msg_module.handle_session_group_start(carol, group_id);

    // Alice sends a message; Bob and Carol should receive it.
    let recipients = msg_module.handle_session_send(alice, group_id, "Hello group!");
    assert_eq!(recipients.len(), 2);
    assert!(recipients.contains(&bob));
    assert!(recipients.contains(&carol));
    assert!(!recipients.contains(&alice));
}

// ===========================================================================
// 7. Group messaging session drop/re-invite
// ===========================================================================

#[test]
fn group_messaging_session_drop_and_reinvite() {
    let mut msg_module = GroupsMessagingModule::new();
    let config = messaging_enabled_config();
    msg_module.initialise(&config).unwrap();

    let group_id = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();

    msg_module.handle_session_group_start(alice, group_id);
    msg_module.handle_session_group_start(bob, group_id);

    // Bob drops out.
    msg_module.handle_session_drop(bob, group_id);
    assert!(msg_module.sessions().has_agent_dropped(group_id, bob));

    // Alice sends; only active agents (none besides Alice) receive.
    let recipients = msg_module.handle_session_send(alice, group_id, "Anyone there?");
    assert!(recipients.is_empty());

    // Bob re-joins.
    msg_module.handle_session_group_start(bob, group_id);
    assert!(msg_module.sessions().is_agent_active(group_id, bob));
    assert!(!msg_module.sessions().has_agent_dropped(group_id, bob));

    let recipients = msg_module.handle_session_send(alice, group_id, "Welcome back!");
    assert_eq!(recipients.len(), 1);
    assert!(recipients.contains(&bob));
}

// ===========================================================================
// 8. Group notice IM construction
// ===========================================================================

#[test]
fn group_notice_im_construction() {
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let attachment = NoticeAttachment {
        item_id: Uuid::new_v4(),
        owner_id: from,
        asset_type: 10, // LSLText
        name: "test_script.lsl".to_string(),
    };

    let im = GroupsMessagingModule::build_group_notice(
        from,
        "Test User",
        to,
        group_id,
        "Meeting Tonight",
        "Details about the meeting",
        Some(&attachment),
    );

    assert_eq!(im.from_agent_id, from);
    assert_eq!(im.to_agent_id, to);
    assert_eq!(im.im_session_id, group_id);
    assert_eq!(im.dialog, IMDialog::GroupNotice);
    assert!(im.from_group);
    assert_eq!(im.message, "Meeting Tonight|Details about the meeting");

    // Binary bucket should contain attachment metadata.
    assert!(!im.binary_bucket.is_empty());
    assert_eq!(im.binary_bucket[0], 1); // has_attachment = true.
    assert_eq!(im.binary_bucket[1], 10); // asset_type.
}

// ===========================================================================
// 9. Group notice without attachment
// ===========================================================================

#[test]
fn group_notice_im_no_attachment() {
    let im = GroupsMessagingModule::build_group_notice(
        Uuid::new_v4(),
        "Sender",
        Uuid::new_v4(),
        Uuid::new_v4(),
        "Subject",
        "Body text",
        None,
    );

    assert_eq!(im.binary_bucket.len(), 1);
    assert_eq!(im.binary_bucket[0], 0); // no attachment.
}

// ===========================================================================
// 10. Process group IM by dialog type
// ===========================================================================

#[test]
fn process_group_im_dispatches_by_dialog() {
    let mut msg_module = GroupsMessagingModule::new();
    let config = messaging_enabled_config();
    msg_module.initialise(&config).unwrap();

    let group_id = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();

    // Session911Start should add the agent to the session.
    let start_im = GridInstantMessage {
        from_agent_id: alice,
        from_agent_name: "Alice".to_string(),
        to_agent_id: Uuid::nil(),
        message: String::new(),
        im_session_id: group_id,
        dialog: IMDialog::Session911Start,
        from_group: true,
        offline: 0,
        parent_estate_id: 0,
        position: [0.0, 0.0, 0.0],
        region_id: Uuid::nil(),
        timestamp: 0,
        binary_bucket: Vec::new(),
    };
    let result = msg_module.process_group_im(&start_im);
    assert!(result.is_empty()); // Session start returns no recipients.
    assert!(msg_module.sessions().is_agent_active(group_id, alice));

    // Add Bob too.
    msg_module.handle_session_group_start(bob, group_id);

    // SessionAdd (send) should return active agents minus the sender.
    let send_im = GridInstantMessage {
        from_agent_id: alice,
        from_agent_name: "Alice".to_string(),
        to_agent_id: Uuid::nil(),
        message: "Hello".to_string(),
        im_session_id: group_id,
        dialog: IMDialog::SessionAdd,
        from_group: true,
        offline: 0,
        parent_estate_id: 0,
        position: [0.0, 0.0, 0.0],
        region_id: Uuid::nil(),
        timestamp: 0,
        binary_bucket: Vec::new(),
    };
    let recipients = msg_module.process_group_im(&send_im);
    assert_eq!(recipients.len(), 1);
    assert!(recipients.contains(&bob));

    // SessionDrop should remove the agent.
    let drop_im = GridInstantMessage {
        from_agent_id: bob,
        from_agent_name: "Bob".to_string(),
        to_agent_id: Uuid::nil(),
        message: String::new(),
        im_session_id: group_id,
        dialog: IMDialog::SessionDrop,
        from_group: true,
        offline: 0,
        parent_estate_id: 0,
        position: [0.0, 0.0, 0.0],
        region_id: Uuid::nil(),
        timestamp: 0,
        binary_bucket: Vec::new(),
    };
    let result = msg_module.process_group_im(&drop_im);
    assert!(result.is_empty()); // Drop returns no recipients.
    assert!(msg_module.sessions().has_agent_dropped(group_id, bob));
}

// ===========================================================================
// 11. Chat session state: standalone tests
// ===========================================================================

#[test]
fn chat_session_state_comprehensive() {
    let mut state = ChatSessionState::new();
    let group_a = Uuid::new_v4();
    let group_b = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();

    // Initially no sessions.
    assert_eq!(state.session_count(), 0);
    assert!(state.active_agents(group_a).is_empty());

    // Invite to two different groups.
    state.invite_agent(group_a, alice);
    state.invite_agent(group_a, bob);
    state.invite_agent(group_b, alice);

    assert_eq!(state.session_count(), 2);
    assert!(state.is_agent_active(group_a, alice));
    assert!(state.is_agent_active(group_a, bob));
    assert!(state.is_agent_active(group_b, alice));
    assert!(!state.is_agent_active(group_b, bob));

    let active = state.active_agents(group_a);
    assert_eq!(active.len(), 2);

    // Drop Bob from group A.
    state.drop_agent(group_a, bob);
    assert!(!state.is_agent_active(group_a, bob));
    assert!(state.has_agent_dropped(group_a, bob));
    let active = state.active_agents(group_a);
    assert_eq!(active.len(), 1);

    // Clear session for group A only.
    state.clear_session(group_a);
    assert!(!state.is_agent_active(group_a, alice));
    assert!(state.is_agent_active(group_b, alice)); // Group B unaffected.

    // Clear all.
    state.clear_all();
    assert_eq!(state.session_count(), 0);
}

// ===========================================================================
// 12. Offline IM: store and retrieve
// ===========================================================================

#[test]
fn offline_im_store_and_retrieve() {
    let mut module = OfflineIMModule::new();
    module.initialise(&offline_im_config(25)).unwrap();

    let recipient = Uuid::new_v4();
    let sender = Uuid::new_v4();

    let im = sample_im(
        sender,
        recipient,
        IMDialog::MessageFromAgent,
        "Hello offline!",
    );
    let stored = module.handle_undelivered(&im).unwrap();
    assert!(stored);
    assert_eq!(module.pending_count(recipient), 1);

    // Simulate login: retrieve and drain.
    let msgs = module.retrieve_messages(recipient);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].message, "Hello offline!");
    assert_eq!(msgs[0].from_agent_id, sender);

    // After drain, count should be zero.
    assert_eq!(module.pending_count(recipient), 0);
}

// ===========================================================================
// 13. Offline IM: max limit enforcement
// ===========================================================================

#[test]
fn offline_im_max_limit_enforcement() {
    let mut module = OfflineIMModule::new();
    module.initialise(&offline_im_config(3)).unwrap();

    let recipient = Uuid::new_v4();

    // Fill to capacity.
    for i in 0..3 {
        let im = sample_im(
            Uuid::new_v4(),
            recipient,
            IMDialog::MessageFromAgent,
            &format!("Message {i}"),
        );
        assert!(module.handle_undelivered(&im).unwrap());
    }
    assert_eq!(module.pending_count(recipient), 3);

    // Next message should be rejected.
    let overflow = sample_im(
        Uuid::new_v4(),
        recipient,
        IMDialog::MessageFromAgent,
        "Overflow message",
    );
    let result = module.handle_undelivered(&overflow);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("maxed out"));

    // Original 3 messages still available.
    assert_eq!(module.pending_count(recipient), 3);
}

// ===========================================================================
// 14. Offline IM: dialog type filtering
// ===========================================================================

#[test]
fn offline_im_dialog_type_filtering() {
    let mut module = OfflineIMModule::new();
    module.initialise(&offline_im_config(25)).unwrap();

    let recipient = Uuid::new_v4();

    // Storable types.
    assert!(
        module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::MessageFromAgent,
                "test"
            ))
            .unwrap()
    );
    assert!(
        module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::MessageFromObject,
                "test"
            ))
            .unwrap()
    );
    assert!(
        module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::InventoryOffered,
                "test"
            ))
            .unwrap()
    );

    // Non-storable types should return Ok(false).
    assert!(
        !module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::StartTyping,
                "test"
            ))
            .unwrap()
    );
    assert!(
        !module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::StopTyping,
                "test"
            ))
            .unwrap()
    );
    assert!(
        !module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::SessionSend,
                "test"
            ))
            .unwrap()
    );

    // Only the 3 storable messages should be pending.
    assert_eq!(module.pending_count(recipient), 3);
}

// ===========================================================================
// 15. Offline IM: group forwarding flag
// ===========================================================================

#[test]
fn offline_im_group_forwarding_disabled() {
    let mut module = OfflineIMModule::new();
    let mut config = TestConfig::new();
    config.set_bool("Messaging", "OfflineIMEnabled", true);
    config.set_bool("Messaging", "ForwardOfflineGroupMessages", false);
    module.initialise(&config).unwrap();

    let recipient = Uuid::new_v4();

    // Group dialogs should be silently rejected.
    assert!(
        !module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::GroupNotice,
                "notice"
            ))
            .unwrap()
    );
    assert!(
        !module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::GroupInvitation,
                "invite"
            ))
            .unwrap()
    );

    // Regular IM should still work.
    assert!(
        module
            .handle_undelivered(&sample_im(
                Uuid::new_v4(),
                recipient,
                IMDialog::MessageFromAgent,
                "hello"
            ))
            .unwrap()
    );

    assert_eq!(module.pending_count(recipient), 1);
}

// ===========================================================================
// 16. Offline IM: delete removes both directions
// ===========================================================================

#[test]
fn offline_im_delete_bidirectional() {
    let mut module = OfflineIMModule::new();
    module.initialise(&offline_im_config(25)).unwrap();

    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();
    let charlie = Uuid::new_v4();

    // Alice -> Bob.
    let mut im = sample_im(alice, bob, IMDialog::MessageFromAgent, "hi bob");
    module.handle_undelivered(&im).unwrap();

    // Charlie -> Alice.
    im = sample_im(charlie, alice, IMDialog::MessageFromAgent, "hi alice");
    module.handle_undelivered(&im).unwrap();

    // Charlie -> Bob.
    im = sample_im(
        charlie,
        bob,
        IMDialog::MessageFromAgent,
        "hi bob from charlie",
    );
    module.handle_undelivered(&im).unwrap();

    assert_eq!(module.total_pending(), 3);

    // Delete Alice: removes messages TO Alice and FROM Alice.
    module.delete_messages(alice);

    // Message to Alice from Charlie: gone.
    assert_eq!(module.pending_count(alice), 0);
    // Messages from Alice to Bob: gone.
    // Message from Charlie to Bob: still there.
    assert_eq!(module.pending_count(bob), 1);
    let remaining = module.retrieve_messages(bob);
    assert_eq!(remaining[0].from_agent_id, charlie);
}

// ===========================================================================
// 17. Profile: update and query round-trip
// ===========================================================================

#[test]
fn profile_update_and_query_roundtrip() {
    let mut module = BasicProfileModule::new();
    let config = TestConfig::new(); // Default enables basic profile.
    module.initialise(&config).unwrap();
    assert!(module.is_enabled());

    let user = Uuid::new_v4();
    let partner = Uuid::new_v4();

    let mut props = AvatarProperties::default();
    props.user_id = user;
    props.partner_id = partner;
    props.about_text = "I build things in virtual worlds.".to_string();
    props.first_life_text = "Software developer".to_string();
    props.web_url = "https://example.com".to_string();
    props.publish_profile = true;

    module.update_avatar_properties(props);

    let result = module.request_avatar_properties(user);
    assert_eq!(result.user_id, user);
    assert_eq!(result.partner_id, partner);
    assert_eq!(result.about_text, "I build things in virtual worlds.");
    assert_eq!(result.first_life_text, "Software developer");
    assert_eq!(result.web_url, "https://example.com");
    assert!(result.publish_profile);
}

// ===========================================================================
// 18. Profile: interests update
// ===========================================================================

#[test]
fn profile_interests_update() {
    let mut module = BasicProfileModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();

    let user = Uuid::new_v4();
    module.update_avatar_interests(
        user,
        0xFF,
        "Building, Scripting",
        0xF0,
        "Architecture, LSL",
        "English, French",
    );

    let props = module.request_avatar_properties(user);
    assert_eq!(props.want_to_mask, 0xFF);
    assert_eq!(props.want_to_text, "Building, Scripting");
    assert_eq!(props.skills_mask, 0xF0);
    assert_eq!(props.skills_text, "Architecture, LSL");
    assert_eq!(props.languages, "English, French");
}

// ===========================================================================
// 19. Profile: picks lifecycle (create -> update -> delete)
// ===========================================================================

#[test]
fn profile_picks_lifecycle() {
    let mut module = BasicProfileModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();

    let user = Uuid::new_v4();
    let pick_id = Uuid::new_v4();

    // No picks initially.
    assert!(module.get_picks(user).is_empty());

    // Create a pick.
    module.update_pick(
        user,
        PickSummary {
            pick_id,
            name: "My Favourite Spot".to_string(),
        },
    );
    let picks = module.get_picks(user);
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].name, "My Favourite Spot");
    assert_eq!(picks[0].pick_id, pick_id);

    // Update the pick.
    module.update_pick(
        user,
        PickSummary {
            pick_id,
            name: "My Updated Spot".to_string(),
        },
    );
    let picks = module.get_picks(user);
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].name, "My Updated Spot");

    // Add a second pick.
    let pick_id_2 = Uuid::new_v4();
    module.update_pick(
        user,
        PickSummary {
            pick_id: pick_id_2,
            name: "Second Spot".to_string(),
        },
    );
    assert_eq!(module.get_picks(user).len(), 2);

    // Delete the first pick.
    assert!(module.delete_pick(user, pick_id));
    let picks = module.get_picks(user);
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].pick_id, pick_id_2);

    // Delete non-existent returns false.
    assert!(!module.delete_pick(user, Uuid::new_v4()));
}

// ===========================================================================
// 20. Profile: classifieds lifecycle
// ===========================================================================

#[test]
fn profile_classifieds_lifecycle() {
    let mut module = BasicProfileModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();

    let user = Uuid::new_v4();
    let cid = Uuid::new_v4();

    // Create.
    module.update_classified(
        user,
        ClassifiedSummary {
            classified_id: cid,
            name: "Land for Sale".to_string(),
        },
    );
    assert_eq!(module.get_classifieds(user).len(), 1);

    // Update.
    module.update_classified(
        user,
        ClassifiedSummary {
            classified_id: cid,
            name: "Premium Land for Sale".to_string(),
        },
    );
    let classifieds = module.get_classifieds(user);
    assert_eq!(classifieds.len(), 1);
    assert_eq!(classifieds[0].name, "Premium Land for Sale");

    // Delete.
    assert!(module.delete_classified(user, cid));
    assert!(module.get_classifieds(user).is_empty());

    // Delete non-existent.
    assert!(!module.delete_classified(user, Uuid::new_v4()));
}

// ===========================================================================
// 21. Profile: user notes CRUD
// ===========================================================================

#[test]
fn profile_notes_crud() {
    let mut module = BasicProfileModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();

    let writer = Uuid::new_v4();
    let target = Uuid::new_v4();

    assert!(module.get_note(writer, target).is_none());

    module.update_note(writer, target, "Great builder, very helpful");
    assert_eq!(
        module.get_note(writer, target),
        Some("Great builder, very helpful")
    );

    // Overwrite.
    module.update_note(writer, target, "Updated note text");
    assert_eq!(module.get_note(writer, target), Some("Updated note text"));

    // Different writer's note is independent.
    let other_writer = Uuid::new_v4();
    assert!(module.get_note(other_writer, target).is_none());
    module.update_note(other_writer, target, "Also nice");
    assert_eq!(module.get_note(other_writer, target), Some("Also nice"));
    // Original writer's note unchanged.
    assert_eq!(module.get_note(writer, target), Some("Updated note text"));
}

// ===========================================================================
// 22. NPC lifecycle: create -> move -> say -> delete
// ===========================================================================

#[test]
fn npc_lifecycle_create_move_say_delete() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let owner = Uuid::new_v4();
    let npc_id = module
        .create_npc("Test", "Bot", [128.0, 128.0, 25.0], owner, false, None)
        .unwrap();

    // Verify creation.
    assert!(module.is_npc(npc_id));
    assert_eq!(module.get_owner(npc_id), Some(owner));
    assert_eq!(module.npc_count(), 1);
    assert_eq!(module.get_npc_position(npc_id), Some([128.0, 128.0, 25.0]));

    // Move.
    module.move_to(npc_id, [64.0, 64.0, 30.0]).unwrap();
    assert_eq!(module.get_npc_position(npc_id), Some([64.0, 64.0, 30.0]));

    // Say (broadcasts chat event).
    let mut rx = scene.events.on_chat_message.subscribe();
    module.say(npc_id, 0, "Hello from NPC!").unwrap();
    let msg = rx.try_recv().unwrap();
    assert_eq!(msg.message, "Hello from NPC!");
    assert_eq!(msg.sender_id, npc_id);
    assert_eq!(msg.channel, 0);

    // Delete.
    module.delete_npc(npc_id, owner).unwrap();
    assert!(!module.is_npc(npc_id));
    assert_eq!(module.npc_count(), 0);
}

// ===========================================================================
// 23. NPC owner validation: non-owner rejection
// ===========================================================================

#[test]
fn npc_owner_validation_rejects_non_owner() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let owner = Uuid::new_v4();
    let intruder = Uuid::new_v4();

    let npc_id = module
        .create_npc("Owned", "Bot", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();

    // Non-owner should be rejected.
    let result = module.delete_npc(npc_id, intruder);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("not the owner"),
        "Expected 'not the owner' in error: {err_msg}"
    );

    // NPC should still exist.
    assert!(module.is_npc(npc_id));

    // Owner can delete.
    assert!(module.delete_npc(npc_id, owner).is_ok());
}

// ===========================================================================
// 24. NPC: max NPCs per scene enforcement
// ===========================================================================

#[test]
fn npc_max_limit_enforcement() {
    let mut module = NPCModule::new();
    let mut config = TestConfig::new();
    config.set_int("NPC", "MaxNumberOfNPCs", 2);
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let owner = Uuid::new_v4();

    // Fill to capacity.
    module
        .create_npc("NPC", "One", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();
    module
        .create_npc("NPC", "Two", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();
    assert_eq!(module.npc_count(), 2);

    // Third should fail.
    let result = module.create_npc("NPC", "Three", [0.0, 0.0, 0.0], owner, false, None);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Maximum NPC count"),
        "Expected max count error: {err_msg}"
    );
}

// ===========================================================================
// 25. NPC: sit and stand cycle
// ===========================================================================

#[test]
fn npc_sit_stand_cycle() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let owner = Uuid::new_v4();
    let chair = Uuid::new_v4();

    let npc_id = module
        .create_npc("Sitter", "Bot", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();

    // Initially not sitting.
    let npc = module.get_npc(npc_id).unwrap();
    assert!(!npc.is_sitting());

    // Sit on chair.
    module.sit(npc_id, chair).unwrap();
    let npc = module.get_npc(npc_id).unwrap();
    assert!(npc.is_sitting());

    // Stand up.
    module.stand(npc_id).unwrap();
    let npc = module.get_npc(npc_id).unwrap();
    assert!(!npc.is_sitting());
}

// ===========================================================================
// 26. NPC: presence events fired on create and delete
// ===========================================================================

#[test]
fn npc_presence_events_fired() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let mut added_rx = scene.events.on_presence_added.subscribe();
    let mut removed_rx = scene.events.on_presence_removed.subscribe();

    let owner = Uuid::new_v4();
    let npc_id = module
        .create_npc("Event", "Bot", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();

    // Should have received presence-added event.
    let added = added_rx.try_recv().unwrap();
    assert_eq!(added.agent_id, npc_id);
    assert_eq!(added.first_name, "Event");
    assert_eq!(added.last_name, "Bot");

    // Delete triggers presence-removed event.
    module.delete_npc(npc_id, owner).unwrap();
    let removed = removed_rx.try_recv().unwrap();
    assert_eq!(removed.agent_id, npc_id);
}

// ===========================================================================
// 27. NPC: sense_as_agent requires config flag
// ===========================================================================

#[test]
fn npc_sense_as_agent_requires_flag() {
    let mut module = NPCModule::new();
    let config = TestConfig::new(); // AllowSenseAsAvatar defaults to false.
    module.initialise(&config).unwrap();
    let scene = scene_ref();
    module.add_region(&scene).unwrap();

    let owner = Uuid::new_v4();

    // Should fail without the flag.
    let result = module.create_npc("Sensor", "Bot", [0.0, 0.0, 0.0], owner, true, None);
    assert!(result.is_err());

    // Enable the flag.
    let mut module2 = NPCModule::new();
    let mut config2 = TestConfig::new();
    config2.set_bool("NPC", "AllowSenseAsAvatar", true);
    module2.initialise(&config2).unwrap();
    module2.add_region(&scene_ref()).unwrap();

    let result = module2.create_npc("Sensor", "Bot", [0.0, 0.0, 0.0], owner, true, None);
    assert!(result.is_ok());
}

// ===========================================================================
// 28. Cross-module: NPC chat event propagation
// ===========================================================================

#[test]
fn cross_module_npc_chat_event_propagation() {
    let mut npc_module = NPCModule::new();
    let config = TestConfig::new();
    npc_module.initialise(&config).unwrap();
    let scene = scene_ref();
    npc_module.add_region(&scene).unwrap();

    // Subscribe to chat events.
    let mut chat_rx = scene.events.on_chat_message.subscribe();

    let owner = Uuid::new_v4();
    let npc_id = npc_module
        .create_npc("Chatty", "Bot", [128.0, 128.0, 25.0], owner, false, None)
        .unwrap();

    // NPC says on channel 0.
    npc_module.say(npc_id, 0, "Hello World!").unwrap();
    let evt = chat_rx.try_recv().unwrap();
    assert_eq!(evt.sender_id, npc_id);
    assert_eq!(evt.message, "Hello World!");
    assert_eq!(evt.channel, 0);
    assert_eq!(evt.region_id, scene.region_id);

    // NPC says on a different channel.
    npc_module.say(npc_id, 42, "Private channel").unwrap();
    let evt = chat_rx.try_recv().unwrap();
    assert_eq!(evt.channel, 42);
    assert_eq!(evt.message, "Private channel");
}

// ===========================================================================
// 29. Cross-module: offline IM stores group notice
// ===========================================================================

#[test]
fn cross_module_offline_im_stores_group_notice() {
    let mut offline_module = OfflineIMModule::new();
    offline_module.initialise(&offline_im_config(25)).unwrap();

    let sender = Uuid::new_v4();
    let offline_member = Uuid::new_v4();
    let group_id = Uuid::new_v4();

    // Build a group notice IM (as would come from GroupsMessagingModule).
    let notice_im = GroupsMessagingModule::build_group_notice(
        sender,
        "Group Leader",
        offline_member,
        group_id,
        "Important Notice",
        "Please read this carefully",
        None,
    );

    // Store it for the offline member.
    let stored = offline_module.handle_undelivered(&notice_im).unwrap();
    assert!(stored);
    assert_eq!(offline_module.pending_count(offline_member), 1);

    // Retrieve on login.
    let msgs = offline_module.retrieve_messages(offline_member);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].dialog, IMDialog::GroupNotice);
    assert_eq!(
        msgs[0].message,
        "Important Notice|Please read this carefully"
    );
    assert_eq!(msgs[0].im_session_id, group_id);
}

// ===========================================================================
// 30. Cross-module: messaging session with offline fallback
// ===========================================================================

#[test]
fn cross_module_messaging_with_offline_fallback() {
    // Set up messaging module.
    let mut msg_module = GroupsMessagingModule::new();
    msg_module.initialise(&messaging_enabled_config()).unwrap();

    // Set up offline IM module.
    let mut offline_module = OfflineIMModule::new();
    offline_module.initialise(&offline_im_config(25)).unwrap();

    let group_id = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();

    // Both join the session.
    msg_module.handle_session_group_start(alice, group_id);
    msg_module.handle_session_group_start(bob, group_id);

    // Bob drops from session (simulating going offline).
    msg_module.handle_session_drop(bob, group_id);

    // Alice sends: only active agents minus sender are returned.
    let recipients = msg_module.handle_session_send(alice, group_id, "Anyone?");
    assert!(recipients.is_empty()); // Nobody active besides Alice.

    // Build an IM for the offline user.  `build_group_im` uses SessionAdd
    // which is not a storable dialog type, so we construct a
    // MessageFromAgent IM instead (matching how the C# offline-IM module
    // would receive a forwarded personal message).
    let im = sample_im(alice, bob, IMDialog::MessageFromAgent, "Missed you!");
    let stored = offline_module.handle_undelivered(&im).unwrap();
    assert!(stored);

    // Bob comes back online, retrieves messages.
    let msgs = offline_module.retrieve_messages(bob);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].message, "Missed you!");
}

// ===========================================================================
// 31. Profile module: close clears all state
// ===========================================================================

#[test]
fn profile_module_close_clears_state() {
    let mut module = BasicProfileModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    module.add_region(&scene_ref()).unwrap();

    let user = Uuid::new_v4();
    let mut props = AvatarProperties::default();
    props.user_id = user;
    props.about_text = "Will be cleared".to_string();
    module.update_avatar_properties(props);
    module.update_note(user, Uuid::new_v4(), "Will be cleared");
    module.update_pick(
        user,
        PickSummary {
            pick_id: Uuid::new_v4(),
            name: "Will be cleared".to_string(),
        },
    );

    module.close().unwrap();

    // Profile returns default after close.
    let result = module.request_avatar_properties(user);
    assert!(result.about_text.is_empty());
}

// ===========================================================================
// 32. NPC: rotation
// ===========================================================================

#[test]
fn npc_rotation() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    module.add_region(&scene_ref()).unwrap();

    let owner = Uuid::new_v4();
    let npc_id = module
        .create_npc("Rotator", "Bot", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();

    // Default rotation is identity quaternion.
    let npc = module.get_npc(npc_id).unwrap();
    assert_eq!(npc.rotation(), [0.0, 0.0, 0.0, 1.0]);

    // Set rotation.
    module.rotate(npc_id, [0.0, 0.707, 0.0, 0.707]).unwrap();
    let npc = module.get_npc(npc_id).unwrap();
    assert_eq!(npc.rotation(), [0.0, 0.707, 0.0, 0.707]);
}

// ===========================================================================
// 33. NPC: touch stub succeeds
// ===========================================================================

#[test]
fn npc_touch_stub() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    module.add_region(&scene_ref()).unwrap();

    let owner = Uuid::new_v4();
    let target_prim = Uuid::new_v4();
    let npc_id = module
        .create_npc("Toucher", "Bot", [0.0, 0.0, 0.0], owner, false, None)
        .unwrap();

    // Touch is a stub but should succeed.
    assert!(module.touch(npc_id, target_prim).is_ok());
}

// ===========================================================================
// 34. NPC: operations on nonexistent NPC
// ===========================================================================

#[test]
fn npc_operations_on_nonexistent() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    module.add_region(&scene_ref()).unwrap();

    let fake_id = Uuid::new_v4();
    let owner = Uuid::new_v4();

    assert!(!module.is_npc(fake_id));
    assert!(module.get_owner(fake_id).is_none());
    assert!(module.get_npc_position(fake_id).is_none());
    assert!(module.get_npc(fake_id).is_none());
    assert!(module.move_to(fake_id, [0.0, 0.0, 0.0]).is_err());
    assert!(module.say(fake_id, 0, "hello").is_err());
    assert!(module.sit(fake_id, Uuid::new_v4()).is_err());
    assert!(module.stand(fake_id).is_err());
    assert!(module.rotate(fake_id, [0.0, 0.0, 0.0, 1.0]).is_err());
    assert!(module.delete_npc(fake_id, owner).is_err());
}

// ===========================================================================
// 35. Groups module: eject member
// ===========================================================================

#[test]
fn groups_module_eject_member() {
    let mut module = GroupsModule::new();
    let config = groups_enabled_config();
    module.initialise(&config).unwrap();

    let admin = Uuid::new_v4();
    let target = Uuid::new_v4();
    let group_id = Uuid::new_v4();

    // Stub connector accepts any eject.
    let result = module.handle_eject_group_member_request(admin, target, group_id);
    assert!(result.is_ok());
}

// ===========================================================================
// 36. NPC avatar data accessors
// ===========================================================================

#[test]
fn npc_avatar_accessors() {
    let mut module = NPCModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    module.add_region(&scene_ref()).unwrap();

    let owner = Uuid::new_v4();
    let npc_id = module
        .create_npc("Jane", "Doe", [10.0, 20.0, 30.0], owner, false, None)
        .unwrap();

    let npc = module.get_npc(npc_id).unwrap();
    assert_eq!(npc.agent_id(), npc_id);
    assert_eq!(npc.first_name(), "Jane");
    assert_eq!(npc.last_name(), "Doe");
    assert_eq!(npc.name(), "Jane Doe");
    assert_eq!(npc.owner_id(), owner);
    assert_eq!(npc.start_pos(), [10.0, 20.0, 30.0]);
    assert_eq!(npc.position(), [10.0, 20.0, 30.0]);
    assert!(!npc.sense_as_agent());
    assert!(!npc.is_disconnected());
}
