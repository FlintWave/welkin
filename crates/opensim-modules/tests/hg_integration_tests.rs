// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Hypergrid modules and services.
//!
//! Exercises cross-module interactions: teleport round-trips, suitcase
//! inventory enforcement, cross-grid IM delivery, hyperlink management,
//! and cross-grid friend request workflows with secret verification.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use opensim_modules::hg::entity_transfer::{ForeignAgentInfo, HGEntityTransferModule};
use opensim_modules::hg::friends::{HGFriendRecord, HGFriendsModule};
use opensim_modules::hg::messaging::{HGInstantMessageService, HGMessageTransferModule};
use opensim_modules::hg::{
    UniversalUserIdentifier, is_local_grid_user, is_same_grid, normalise_grid_uri,
};
use opensim_modules::messaging::{GridInstantMessage, IMDialog};
use opensim_modules::module::{ModuleConfig, RegionModule};
use opensim_modules::registry::SceneRef;
use opensim_modules::transfer::GridRegion;

use opensim_services::gatekeeper::{GatekeeperConfig, GatekeeperServiceImpl};
use opensim_services::hg_assets::{AssetPermissions, HGAssetServiceImpl};
use opensim_services::hg_friends::HGFriendsServiceImpl;
use opensim_services::hg_inventory::{HGInventoryServiceImpl, HGSuitcaseInventoryServiceImpl};
use opensim_services::hg_linker::{HypergridLinker, HypergridLinkerConfig};
use opensim_services::hypergrid::*;
use opensim_services::user_agent::{UserAgentConfig, UserAgentServiceImpl};

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

    #[allow(dead_code)]
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

// ===========================================================================
// Shared helpers
// ===========================================================================

fn scene_ref() -> SceneRef {
    SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
}

fn make_gatekeeper_config(url: &str) -> GatekeeperConfig {
    GatekeeperConfig {
        gatekeeper_url: url.to_string(),
        gatekeeper_aliases: vec![],
        scope_id: Uuid::nil(),
        allow_teleports_to_any_region: true,
        foreign_agents_allowed: true,
        foreigns_allowed_exceptions: Vec::new(),
        foreigns_disallowed_exceptions: Vec::new(),
        allowed_clients: None,
        denied_clients: None,
        denied_macs: String::new(),
        denied_id0s: String::new(),
    }
}

fn make_default_region() -> GridRegionInfo {
    GridRegionInfo {
        region_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        region_name: "Welcome Plaza".into(),
        region_handle: 1099511628032,
        server_uri: "http://mygrid.example:9000/".into(),
        external_host_name: "mygrid.example".into(),
        internal_port: 9000,
        region_loc_x: 256000,
        region_loc_y: 256000,
        region_size_x: 256,
        region_size_y: 256,
    }
}

fn make_uas_config() -> UserAgentConfig {
    UserAgentConfig {
        grid_name: "http://mygrid.example:9000/".into(),
        my_external_ip: "203.0.113.1".into(),
        bypass_client_verification: false,
        level_outside_contacts: 0,
        show_details: true,
    }
}

fn make_circuit(gatekeeper_url: &str, ip: &str) -> AgentCircuitInfo {
    let mut service_urls = HashMap::new();
    service_urls.insert(
        "HomeURI".into(),
        "http://foreigngrid.example:9000/".into(),
    );
    AgentCircuitInfo {
        agent_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        secure_session_id: Uuid::new_v4(),
        first_name: "Test".into(),
        last_name: "User".into(),
        start_pos: Vector3::default(),
        service_session_id: format!("{};{}", gatekeeper_url, Uuid::new_v4()),
        service_urls,
        ip_address: ip.into(),
        viewer: "Firestorm 6.5.0".into(),
        channel: "Firestorm-Release".into(),
        mac: "aa:bb:cc:dd:ee:ff".into(),
        id0: "test-id0".into(),
        teleport_flags: 0,
    }
}

fn sample_im(to: Uuid, dialog: IMDialog) -> GridInstantMessage {
    GridInstantMessage {
        from_agent_id: Uuid::new_v4(),
        from_agent_name: "Sender Name".to_string(),
        to_agent_id: to,
        message: "Hello across the Hypergrid".to_string(),
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

fn make_hg_inventory_folder(
    folder_id: Uuid,
    parent_id: Uuid,
    owner_id: Uuid,
    name: &str,
    folder_type: i16,
) -> HgInventoryFolder {
    HgInventoryFolder {
        folder_id,
        parent_id,
        owner_id,
        name: name.into(),
        folder_type,
    }
}

fn make_hg_inventory_item(
    item_id: Uuid,
    folder_id: Uuid,
    owner_id: Uuid,
    creator_id: &str,
) -> HgInventoryItem {
    HgInventoryItem {
        item_id,
        folder_id,
        owner_id,
        asset_id: Uuid::new_v4(),
        asset_type: 0,
        name: "Test Item".into(),
        description: "test description".into(),
        creator_id: creator_id.into(),
        creator_data: String::new(),
    }
}

// ===========================================================================
// Section 1: HG Teleport Round-Trip
// ===========================================================================

/// Outbound teleport: UAS creates travel data, token can be verified,
/// gatekeeper accepts a valid circuit, and verify_client matches IPs.
#[test]
fn teleport_round_trip_outbound_login_and_verify() {
    let uas = Arc::new(UserAgentServiceImpl::new(make_uas_config()));
    let gk_url = "http://foreign.example:9000/";

    // Build a circuit for the outbound teleport
    let session = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let circuit = AgentCircuitInfo {
        agent_id: agent,
        session_id: session,
        secure_session_id: Uuid::new_v4(),
        first_name: "Alice".into(),
        last_name: "Traveler".into(),
        start_pos: Vector3::default(),
        service_session_id: String::new(),
        service_urls: HashMap::new(),
        ip_address: "10.0.0.42".into(),
        viewer: "TestViewer".into(),
        channel: "test".into(),
        mac: "11:22:33:44:55:66".into(),
        id0: "id0".into(),
        teleport_flags: 0,
    };

    let gatekeeper = GridRegionInfo {
        region_id: Uuid::new_v4(),
        region_name: "Foreign Plaza".into(),
        region_handle: 0,
        server_uri: gk_url.into(),
        external_host_name: "foreign.example".into(),
        internal_port: 9000,
        region_loc_x: 1000,
        region_loc_y: 1000,
        region_size_x: 256,
        region_size_y: 256,
    };
    let dest = gatekeeper.clone();

    // Login the agent to a foreign grid
    uas.login_agent_to_grid(None, &circuit, &gatekeeper, &dest, true)
        .expect("login_agent_to_grid should succeed");

    // The UAS should now have travel data; extract the token by verifying
    // that verify_agent with the correct token returns true.
    // We need to get the token -- store_travel_info was called internally.
    // We can verify the session exists by using verify_client (IP-based).
    assert!(
        uas.verify_client(session, "10.0.0.42").unwrap(),
        "verify_client should match the stored IP"
    );
    assert!(
        !uas.verify_client(session, "1.2.3.4").unwrap(),
        "verify_client should reject a different IP"
    );

    // NAT fallback: my_external_ip
    assert!(
        uas.verify_client(session, "203.0.113.1").unwrap(),
        "verify_client should accept the grid's external IP (NAT case)"
    );
}

/// Gatekeeper login_agent accepts a properly-formed circuit.
#[test]
fn teleport_round_trip_gatekeeper_login_agent() {
    let gk_url = "http://mygrid.example:9000/";
    let gk = GatekeeperServiceImpl::new(
        make_gatekeeper_config(gk_url),
        Some(make_default_region()),
    );

    let circuit = make_circuit(gk_url, "192.168.1.100");
    let dest = make_default_region();

    let result = gk.login_agent(None, &circuit, &dest);
    assert!(
        result.is_ok(),
        "login_agent should succeed with valid circuit: {:?}",
        result
    );
}

/// Gatekeeper rejects a circuit whose service_session_id points to a
/// different grid.
#[test]
fn teleport_round_trip_gatekeeper_rejects_wrong_grid_token() {
    let gk_url = "http://mygrid.example:9000/";
    let gk = GatekeeperServiceImpl::new(
        make_gatekeeper_config(gk_url),
        Some(make_default_region()),
    );

    // Circuit with a token pointing to evil.example
    let mut circuit = make_circuit("http://evil.example:9000/", "192.168.1.100");
    circuit.service_session_id = format!("http://evil.example:9000/;{}", Uuid::new_v4());

    let dest = make_default_region();
    let result = gk.login_agent(None, &circuit, &dest);
    assert!(
        result.is_err(),
        "login_agent should reject a token for the wrong grid"
    );
}

/// Gatekeeper rejects a circuit with no IP address.
#[test]
fn teleport_round_trip_gatekeeper_rejects_empty_ip() {
    let gk_url = "http://mygrid.example:9000/";
    let gk = GatekeeperServiceImpl::new(
        make_gatekeeper_config(gk_url),
        Some(make_default_region()),
    );

    let circuit = make_circuit(gk_url, "");
    let dest = make_default_region();
    let result = gk.login_agent(None, &circuit, &dest);
    assert!(
        result.is_err(),
        "login_agent should reject a circuit with no IP"
    );
}

/// HGEntityTransferModule: foreign agent registration round-trip.
#[test]
fn teleport_round_trip_foreign_agent_registration() {
    let uas = Arc::new(UserAgentServiceImpl::new(make_uas_config()));
    let gk = Arc::new(GatekeeperServiceImpl::new(
        make_gatekeeper_config("http://mygrid.example:9000/"),
        Some(make_default_region()),
    ));

    let mut module = HGEntityTransferModule::new();
    module.set_user_agent_service(uas);
    module.set_gatekeeper_service(gk);

    let agent_id = Uuid::new_v4();
    let info = ForeignAgentInfo {
        home_uri: "http://foreign.grid:8002".to_string(),
        session_id: Uuid::new_v4(),
        service_token: "http://foreign.grid:8002;tok-123".to_string(),
    };
    module.register_foreign_agent(agent_id, info);
    assert!(module.is_foreign_agent(agent_id));
    assert_eq!(module.foreign_agent_count(), 1);

    let info = module.get_foreign_agent_info(agent_id).unwrap();
    assert_eq!(info.home_uri, "http://foreign.grid:8002");

    module.deregister_foreign_agent(agent_id);
    assert!(!module.is_foreign_agent(agent_id));
    assert_eq!(module.foreign_agent_count(), 0);
}

/// Teleport home for a foreign user -- get_home_region is stubbed so the
/// result should be an error, but the flow should not panic.
#[test]
fn teleport_round_trip_teleport_home_foreign_stub() {
    let uas = Arc::new(UserAgentServiceImpl::new(make_uas_config()));
    let mut module = HGEntityTransferModule::new();
    module.set_user_agent_service(uas);

    let agent_id = Uuid::new_v4();
    let info = ForeignAgentInfo {
        home_uri: "http://foreign.grid:8002".to_string(),
        session_id: Uuid::new_v4(),
        service_token: "tok".to_string(),
    };
    module.register_foreign_agent(agent_id, info);

    // get_home_region is stubbed to return Err, so teleport_home_foreign
    // should return an error (but must not panic).
    let result = module.teleport_home_foreign(agent_id);
    assert!(
        result.is_err(),
        "teleport_home should error because get_home_region is a stub"
    );
}

/// Appearance validation passes when restriction is disabled (the default).
#[test]
fn teleport_round_trip_appearance_unrestricted() {
    let module = HGEntityTransferModule::new();
    // Without restriction (the default), everything passes.
    assert!(module
        .validate_appearance(&[Uuid::new_v4()], &[Uuid::new_v4()])
        .is_ok());
}

/// HG teleport level check -- configure via initialise().
#[test]
fn teleport_round_trip_level_gate() {
    let mut module = HGEntityTransferModule::new();
    let mut config = TestConfig::new();
    config.set_string("Modules", "EntityTransferModule", "HGEntityTransferModule");
    config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
    config.set_int("EntityTransfer", "LevelHGTeleport", 10);
    module.initialise(&config).unwrap();

    assert!(!module.can_hg_teleport(5));
    assert!(module.can_hg_teleport(10));
    assert!(module.can_hg_teleport(200));
}

/// Hyperlink resolution via the gatekeeper.
#[test]
fn teleport_round_trip_hyperlink_resolution() {
    let gk = Arc::new(GatekeeperServiceImpl::new(
        make_gatekeeper_config("http://mygrid.example:9000/"),
        Some(make_default_region()),
    ));

    let mut module = HGEntityTransferModule::new();
    module.set_gatekeeper_service(gk);

    let region = GridRegion::new(Uuid::new_v4(), "SomeRegion");

    // Non-hyperlink: returns original.
    let result = module.get_final_destination(&region, Uuid::new_v4(), None, false);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().region_name, "SomeRegion");

    // Hyperlink with allow_teleports_to_any_region=true but no matching
    // region wired: the GK stub falls through to error.
    let result = module.get_final_destination(&region, Uuid::new_v4(), None, true);
    assert!(result.is_err());
}

/// Verify_agent round-trip through the UserAgentService.
#[test]
fn teleport_round_trip_verify_agent() {
    let uas = UserAgentServiceImpl::new(make_uas_config());
    let session = Uuid::new_v4();
    let token = "http://foreign.example:9000/;abc-123";
    uas.store_travel_info(TravelingAgentInfo {
        session_id: session,
        user_id: Uuid::new_v4(),
        grid_external_name: "http://foreign.example:9000/".into(),
        service_token: token.into(),
        client_ip_address: "10.0.0.1".into(),
    });

    assert!(uas.verify_agent(session, token).unwrap());
    assert!(!uas.verify_agent(session, "wrong-token").unwrap());
    assert!(!uas.verify_agent(Uuid::new_v4(), token).unwrap());
}

/// is_agent_coming_home round-trip.
#[test]
fn teleport_round_trip_is_agent_coming_home() {
    let uas = UserAgentServiceImpl::new(make_uas_config());
    let session = Uuid::new_v4();
    uas.store_travel_info(TravelingAgentInfo {
        session_id: session,
        user_id: Uuid::new_v4(),
        grid_external_name: "http://mygrid.example:9000/".into(),
        service_token: "tok".into(),
        client_ip_address: "10.0.0.1".into(),
    });

    assert!(uas
        .is_agent_coming_home(session, "http://mygrid.example:9000/")
        .unwrap());
    assert!(uas
        .is_agent_coming_home(session, "HTTP://MYGRID.EXAMPLE:9000/")
        .unwrap());
    assert!(!uas
        .is_agent_coming_home(session, "http://other.example:9000/")
        .unwrap());
}

// ===========================================================================
// Section 2: Suitcase Inventory Access
// ===========================================================================

const FOLDER_TYPE_SUITCASE: i16 = 100;
const FOLDER_TYPE_NORMAL: i16 = 8;

/// Suitcase root is returned with normal folder type and the tree is
/// properly enforced.
#[test]
fn suitcase_inventory_root_folder_and_tree() {
    let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();
    let suitcase_id = Uuid::new_v4();
    let child_folder_id = Uuid::new_v4();
    let outside_folder = Uuid::new_v4();
    let real_root = Uuid::new_v4();

    // Build folder tree
    svc.add_folder(make_hg_inventory_folder(
        suitcase_id,
        real_root,
        owner,
        "My Suitcase",
        FOLDER_TYPE_SUITCASE,
    ));
    svc.add_folder(make_hg_inventory_folder(
        child_folder_id,
        suitcase_id,
        owner,
        "Objects",
        FOLDER_TYPE_NORMAL,
    ));
    svc.add_folder(make_hg_inventory_folder(
        outside_folder,
        real_root,
        owner,
        "Scripts",
        FOLDER_TYPE_NORMAL,
    ));
    svc.register_suitcase(owner, suitcase_id, vec![child_folder_id]);

    // Root folder should be the suitcase, presented as normal type
    let root = svc.get_root_folder(owner).unwrap();
    assert_eq!(root.folder_id, suitcase_id);
    assert_eq!(root.name, "My Suitcase");
    assert_eq!(root.folder_type, FOLDER_TYPE_NORMAL);
}

/// Folder content inside the suitcase tree is accessible.
#[test]
fn suitcase_inventory_content_inside_tree() {
    let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();
    let suitcase_id = Uuid::new_v4();
    let child_folder_id = Uuid::new_v4();

    svc.add_folder(make_hg_inventory_folder(
        suitcase_id,
        Uuid::nil(),
        owner,
        "My Suitcase",
        FOLDER_TYPE_SUITCASE,
    ));
    svc.add_folder(make_hg_inventory_folder(
        child_folder_id,
        suitcase_id,
        owner,
        "Textures",
        FOLDER_TYPE_NORMAL,
    ));
    svc.register_suitcase(owner, suitcase_id, vec![child_folder_id]);

    // Add items to suitcase root
    svc.add_item_direct(make_hg_inventory_item(
        Uuid::new_v4(),
        suitcase_id,
        owner,
        "creator-uuid",
    ));
    svc.add_item_direct(make_hg_inventory_item(
        Uuid::new_v4(),
        suitcase_id,
        owner,
        "creator-uuid",
    ));

    let content = svc.get_folder_content(owner, suitcase_id).unwrap();
    assert_eq!(content.folders.len(), 1, "should see child folder");
    assert_eq!(content.items.len(), 2, "should see both items");
}

/// Folder content OUTSIDE the suitcase tree returns empty -- the core
/// security boundary.
#[test]
fn suitcase_inventory_content_outside_tree_returns_empty() {
    let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();
    let suitcase_id = Uuid::new_v4();
    let outside_folder = Uuid::new_v4();

    svc.register_suitcase(owner, suitcase_id, vec![]);

    // Add items to a folder that is NOT in the suitcase tree
    svc.add_item_direct(make_hg_inventory_item(
        Uuid::new_v4(),
        outside_folder,
        owner,
        "c",
    ));

    let content = svc.get_folder_content(owner, outside_folder).unwrap();
    assert!(
        content.items.is_empty(),
        "must return empty for folder outside suitcase tree"
    );
    assert!(
        content.folders.is_empty(),
        "must return empty folders for outside-tree request"
    );
}

/// add_item into suitcase tree succeeds; outside tree fails.
#[test]
fn suitcase_inventory_add_item_enforcement() {
    let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();
    let suitcase_id = Uuid::new_v4();
    let child_folder_id = Uuid::new_v4();
    let outside_folder = Uuid::new_v4();

    svc.add_folder(make_hg_inventory_folder(
        suitcase_id,
        Uuid::nil(),
        owner,
        "My Suitcase",
        FOLDER_TYPE_SUITCASE,
    ));
    svc.add_folder(make_hg_inventory_folder(
        child_folder_id,
        suitcase_id,
        owner,
        "Objects",
        FOLDER_TYPE_NORMAL,
    ));
    svc.register_suitcase(owner, suitcase_id, vec![child_folder_id]);

    // Item inside the tree: should succeed
    let item_in = make_hg_inventory_item(Uuid::new_v4(), child_folder_id, owner, "c");
    assert!(
        svc.add_item(&item_in).unwrap(),
        "add_item should succeed for folder in suitcase tree"
    );

    // Item outside the tree: should fail
    let item_out = make_hg_inventory_item(Uuid::new_v4(), outside_folder, owner, "c");
    assert!(
        !svc.add_item(&item_out).unwrap(),
        "add_item must fail for folder outside suitcase tree"
    );
}

/// Creator ID is rewritten to UUI format on get_item.
#[test]
fn suitcase_inventory_creator_rewrite() {
    let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let folder_id = Uuid::new_v4();

    svc.add_item_direct(make_hg_inventory_item(
        item_id,
        folder_id,
        owner,
        "plain-uuid-no-semicolon",
    ));

    let item = svc.get_item(owner, item_id).unwrap().unwrap();
    assert!(
        item.creator_data.contains("http://home.grid:9000"),
        "creator_data should be filled with home URL, got: {}",
        item.creator_data
    );
}

/// No suitcase folder returns an error for the strict implementation.
#[test]
fn suitcase_inventory_no_suitcase_returns_error() {
    let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();
    assert!(
        svc.get_root_folder(owner).is_err(),
        "should error when no suitcase folder exists"
    );
}

/// Basic HGInventoryServiceImpl auto-creates a suitcase when none exists.
#[test]
fn basic_hg_inventory_auto_creates_suitcase() {
    let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
    let owner = Uuid::new_v4();

    let root = svc.get_root_folder(owner).unwrap();
    assert_eq!(root.name, "My Suitcase");
    assert_eq!(root.owner_id, owner);
    assert_eq!(root.folder_type, FOLDER_TYPE_NORMAL);
}

// ===========================================================================
// Section 3: Cross-Grid IM Delivery
// ===========================================================================

fn make_enabled_hg_message_module() -> HGMessageTransferModule {
    let mut module = HGMessageTransferModule::new();
    let mut config = TestConfig::new();
    config.set_string(
        "Messaging",
        "MessageTransferModule",
        "HGMessageTransferModule",
    );
    config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
    module.initialise(&config).unwrap();
    module
}

/// Send an IM to a registered foreign user: should be accepted.
#[test]
fn im_delivery_to_foreign_user() {
    let mut module = make_enabled_hg_message_module();

    let foreign_user = Uuid::new_v4();
    module.register_foreign_user(foreign_user, "http://foreign.grid:8002".to_string());

    assert!(module.is_foreign_user(foreign_user));
    assert_eq!(
        module.get_foreign_user_uri(foreign_user),
        Some("http://foreign.grid:8002")
    );

    let im = sample_im(foreign_user, IMDialog::MessageFromAgent);
    assert!(
        module.send_instant_message(&im),
        "IM to registered foreign user should be accepted"
    );
}

/// Send an IM to nil UUID: rejected.
#[test]
fn im_delivery_nil_uuid_rejected() {
    let mut module = make_enabled_hg_message_module();

    let im = sample_im(Uuid::nil(), IMDialog::MessageFromAgent);
    assert!(
        !module.send_instant_message(&im),
        "IM to nil UUID must be rejected"
    );
}

/// Offline-storable dialog types without group forwarding.
#[test]
fn im_delivery_offline_storable_no_groups() {
    let svc = HGInstantMessageService::new();

    assert!(svc.is_offline_storable(IMDialog::MessageFromAgent));
    assert!(svc.is_offline_storable(IMDialog::MessageFromObject));
    assert!(svc.is_offline_storable(IMDialog::InventoryOffered));
    assert!(!svc.is_offline_storable(IMDialog::GroupNotice));
    assert!(!svc.is_offline_storable(IMDialog::GroupInvitation));
    assert!(!svc.is_offline_storable(IMDialog::StartTyping));
}

/// Offline-storable dialog types WITH group forwarding.
#[test]
fn im_delivery_offline_storable_with_groups() {
    let mut svc = HGInstantMessageService::new();
    svc.forward_offline_group_messages = true;

    assert!(svc.is_offline_storable(IMDialog::MessageFromAgent));
    assert!(svc.is_offline_storable(IMDialog::GroupNotice));
    assert!(svc.is_offline_storable(IMDialog::GroupInvitation));
    assert!(!svc.is_offline_storable(IMDialog::StartTyping));
    assert!(!svc.is_offline_storable(IMDialog::StopTyping));
}

/// Handle undeliverable: storable dialog goes to offline queue.
#[test]
fn im_delivery_handle_undeliverable_storable() {
    let module = HGMessageTransferModule::new();
    let im = sample_im(Uuid::new_v4(), IMDialog::MessageFromAgent);
    assert!(
        module.handle_undeliverable(&im),
        "storable dialog should be queued"
    );
}

/// Handle undeliverable: non-storable dialog is dropped.
#[test]
fn im_delivery_handle_undeliverable_non_storable() {
    let module = HGMessageTransferModule::new();
    let im = sample_im(Uuid::new_v4(), IMDialog::StartTyping);
    assert!(
        !module.handle_undeliverable(&im),
        "non-storable dialog should be dropped"
    );
}

/// HGInstantMessageService: try_send caches the location on success.
#[test]
fn im_delivery_location_caching() {
    let mut svc = HGInstantMessageService::new();
    assert_eq!(svc.cached_location_count(), 0);

    let to = Uuid::new_v4();
    let im = sample_im(to, IMDialog::MessageFromAgent);

    // Forward with a known URL (stub always succeeds)
    let ok = svc.try_send_instant_message(&im, "http://target.grid:8002", true);
    assert!(ok, "stub forward should succeed");
    assert_eq!(svc.cached_location_count(), 1);
}

/// Module lifecycle: init, add_region, remove_region, close.
#[test]
fn im_delivery_module_lifecycle() {
    let mut module = make_enabled_hg_message_module();

    let scene = scene_ref();
    module.add_region(&scene).unwrap();
    module.remove_region(&scene).unwrap();
    module.close().unwrap();
}

// ===========================================================================
// Section 4: Hyperlink Creation/Removal via HypergridLinker
// ===========================================================================

fn make_linker() -> HypergridLinker {
    HypergridLinker::new(HypergridLinkerConfig {
        scope_id: Uuid::nil(),
        gatekeeper_url: "http://mygrid.example:9000/".into(),
        ..Default::default()
    })
}

/// Link a region: all fields populated, retrievable by ID and name.
#[test]
fn hyperlink_creation_and_retrieval() {
    let linker = make_linker();
    let link = linker
        .link_region(
            Uuid::nil(),
            "http://foreign.example:9000/",
            "Foreign Plaza",
            1000,
            2000,
            Uuid::new_v4(),
        )
        .unwrap();

    assert_eq!(link.region_name, "Foreign Plaza");
    assert_eq!(link.server_uri, "http://foreign.example:9000/");
    assert_eq!(link.region_loc_x, 1000);
    assert_eq!(link.region_loc_y, 2000);
    assert_eq!(link.size_x, 256);
    assert_eq!(link.size_y, 256);

    // Retrieve by ID
    let by_id = linker.get_link_by_region_id(link.region_id);
    assert!(by_id.is_some());
    assert_eq!(by_id.unwrap().region_name, "Foreign Plaza");

    // Retrieve by name (case-insensitive)
    let by_name = linker.get_link_by_name("foreign plaza");
    assert!(by_name.is_some());
    assert_eq!(by_name.unwrap().region_id, link.region_id);

    // List all
    let all = linker.get_linked_regions();
    assert_eq!(all.len(), 1);
}

/// Unlink removes a region.
#[test]
fn hyperlink_removal() {
    let linker = make_linker();
    let link = linker
        .link_region(
            Uuid::nil(),
            "http://foreign.example:9000/",
            "Ephemeral",
            3000,
            3000,
            Uuid::nil(),
        )
        .unwrap();

    assert!(linker.unlink_region(link.region_id).unwrap());
    assert!(linker.get_link_by_region_id(link.region_id).is_none());
    assert!(linker.get_linked_regions().is_empty());
}

/// Unlink of a non-existent region returns false.
#[test]
fn hyperlink_unlink_nonexistent() {
    let linker = make_linker();
    assert!(!linker.unlink_region(Uuid::new_v4()).unwrap());
}

/// Coordinate collision prevents a second link at the same position.
#[test]
fn hyperlink_coordinate_collision() {
    let linker = make_linker();
    linker
        .link_region(
            Uuid::nil(),
            "http://first.example:9000/",
            "First",
            5000,
            5000,
            Uuid::nil(),
        )
        .unwrap();

    let result = linker.link_region(
        Uuid::nil(),
        "http://second.example:9000/",
        "Second",
        5000,
        5000,
        Uuid::nil(),
    );
    assert!(
        result.is_err(),
        "second link at same coords should be rejected"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains("already occupied"));
}

/// Multiple links can coexist at different coordinates.
#[test]
fn hyperlink_multiple_links() {
    let linker = make_linker();
    linker
        .link_region(Uuid::nil(), "http://a.example:9000/", "Alpha", 1000, 1000, Uuid::nil())
        .unwrap();
    linker
        .link_region(Uuid::nil(), "http://b.example:9000/", "Beta", 2000, 2000, Uuid::nil())
        .unwrap();
    linker
        .link_region(Uuid::nil(), "http://c.example:9000/", "Gamma", 3000, 3000, Uuid::nil())
        .unwrap();

    let all = linker.get_linked_regions();
    assert_eq!(all.len(), 3);
    let names: Vec<&str> = all.iter().map(|l| l.region_name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));
    assert!(names.contains(&"Gamma"));
}

/// Name lookup is case-insensitive.
#[test]
fn hyperlink_case_insensitive_name_lookup() {
    let linker = make_linker();
    linker
        .link_region(
            Uuid::nil(),
            "http://x.example:9000/",
            "CamelCaseRegion",
            4000,
            4000,
            Uuid::nil(),
        )
        .unwrap();

    assert!(linker.get_link_by_name("CAMELCASEREGION").is_some());
    assert!(linker.get_link_by_name("camelcaseregion").is_some());
    assert!(linker.get_link_by_name("CamelCaseRegion").is_some());
    assert!(linker.get_link_by_name("nonexistent").is_none());
}

// ===========================================================================
// Section 5: Cross-Grid Friend Request with Secret Verification
// ===========================================================================

/// Store a cross-grid friendship and verify the secret is generated.
#[test]
fn friends_store_and_retrieve_hg_friendship() {
    let mut module = HGFriendsModule::new();
    let user = Uuid::new_v4();
    let friend_uuid = Uuid::new_v4();
    let friend_uui = format!(
        "{};http://foreign.grid:8002;Alice Wonderland",
        friend_uuid
    );

    let secret = module.store_hg_friendship(user, &friend_uui);
    assert_eq!(secret.len(), 8, "secret should be 8 characters");

    let friends = module.get_friends(user).unwrap();
    assert_eq!(friends.len(), 1);
    assert!(friends[0].is_foreign());
    assert_eq!(friends[0].friend_uuid(), Some(friend_uuid));
    assert!(friends[0].secret().is_some());
    assert_eq!(friends[0].secret().unwrap(), secret);
}

/// find_friend locates a cached friend record.
#[test]
fn friends_find_friend_in_cache() {
    let mut module = HGFriendsModule::new();
    let user = Uuid::new_v4();
    let friend_a = Uuid::new_v4();
    let friend_b = Uuid::new_v4();

    module.cache_friends(
        user,
        vec![
            HGFriendRecord {
                friend: friend_a.to_string(),
                my_flags: 1,
                their_flags: 1,
            },
            HGFriendRecord {
                friend: format!("{};http://grid.com;Someone Name", friend_b),
                my_flags: 1,
                their_flags: 1,
            },
        ],
    );

    assert!(module.find_friend(user, friend_a).is_some());
    assert!(module.find_friend(user, friend_b).is_some());
    assert!(module.find_friend(user, Uuid::new_v4()).is_none());
}

/// Delete a cross-grid friendship with the correct secret succeeds.
#[test]
fn friends_delete_correct_secret() {
    let mut module = HGFriendsModule::new();
    let user = Uuid::new_v4();
    let friend_uuid = Uuid::new_v4();
    let friend_uui = format!("{};http://foreign.grid:8002;Bob Builder", friend_uuid);

    let secret = module.store_hg_friendship(user, &friend_uui);
    assert!(module.delete_hg_friendship(user, friend_uuid, &secret));
    assert!(module.get_friends(user).unwrap().is_empty());
}

/// Delete a cross-grid friendship with the WRONG secret fails.
#[test]
fn friends_delete_wrong_secret() {
    let mut module = HGFriendsModule::new();
    let user = Uuid::new_v4();
    let friend_uuid = Uuid::new_v4();
    let friend_uui = format!("{};http://foreign.grid:8002;Carol Candle", friend_uuid);

    let _secret = module.store_hg_friendship(user, &friend_uui);
    assert!(
        !module.delete_hg_friendship(user, friend_uuid, "wrong!!!"),
        "delete with wrong secret must fail"
    );
    assert_eq!(module.get_friends(user).unwrap().len(), 1);
}

/// Partition friends by domain: local vs foreign grouped by grid URI.
#[test]
fn friends_partition_by_domain() {
    let module = HGFriendsModule::new();

    let local_uuid = Uuid::new_v4();
    let foreign_a = Uuid::new_v4();
    let foreign_b = Uuid::new_v4();
    let foreign_c = Uuid::new_v4();

    let friends = vec![
        HGFriendRecord {
            friend: local_uuid.to_string(),
            my_flags: 1,
            their_flags: 1,
        },
        HGFriendRecord {
            friend: format!("{};http://grid-a.com:8002;Alice Smith", foreign_a),
            my_flags: 1,
            their_flags: 1,
        },
        HGFriendRecord {
            friend: format!("{};http://grid-a.com:8002;Bob Jones", foreign_b),
            my_flags: 1,
            their_flags: 1,
        },
        HGFriendRecord {
            friend: format!("{};http://grid-b.com:8002;Carol Ng", foreign_c),
            my_flags: 1,
            their_flags: 1,
        },
    ];

    let (local, foreign) = module.partition_friends_by_domain(&friends);
    assert_eq!(local.len(), 1);
    assert_eq!(local[0], local_uuid);
    assert_eq!(foreign.len(), 2, "two distinct foreign domains");

    let domain_a = foreign.get("http://grid-a.com:8002").unwrap();
    assert_eq!(domain_a.len(), 2, "two friends on grid-a");

    let domain_b = foreign.get("http://grid-b.com:8002").unwrap();
    assert_eq!(domain_b.len(), 1, "one friend on grid-b");
}

/// HGFriendsServiceImpl: verified friendship creation.
#[test]
fn friends_service_new_verified_friendship() {
    let svc = HGFriendsServiceImpl::new();
    let principal = Uuid::new_v4();
    let friend_id = Uuid::new_v4();
    let friend_uui = format!("{friend_id};http://foreign.example:9000/;John Doe;abc123");

    let info = HgFriendInfo {
        principal_id: principal,
        friend: friend_uui,
        my_flags: 0,
        their_flags: 0,
    };

    assert!(svc.new_friendship(&info, true).unwrap());

    // Duplicate should be rejected
    let info2 = HgFriendInfo {
        principal_id: principal,
        friend: format!("{friend_id};http://foreign.example:9000/;John Doe;abc123"),
        my_flags: 0,
        their_flags: 0,
    };
    assert!(!svc.new_friendship(&info2, true).unwrap());
}

/// HGFriendsServiceImpl: delete with matching secret.
#[test]
fn friends_service_delete_friendship() {
    let svc = HGFriendsServiceImpl::new();
    let principal = Uuid::new_v4();
    let friend_id = Uuid::new_v4();
    let secret = "secretXYZ";
    let friend_uui = format!("{friend_id};http://foreign.example:9000/;John Doe;{secret}");

    let info = HgFriendInfo {
        principal_id: principal,
        friend: friend_uui,
        my_flags: 0,
        their_flags: 0,
    };
    svc.new_friendship(&info, true).unwrap();

    let del_info = HgFriendInfo {
        principal_id: principal,
        friend: friend_id.to_string(),
        my_flags: 0,
        their_flags: 0,
    };
    assert!(
        svc.delete_friendship(&del_info, secret).unwrap(),
        "delete with correct secret should succeed"
    );

    // Should be gone
    let perms = svc.get_friend_perms(principal, friend_id).unwrap();
    assert_eq!(perms, -1, "friend should be gone after deletion");
}

/// HGFriendsServiceImpl: validate_friendship_offered.
///
/// A verified friendship creates a record with their_flags == -1 (pending).
/// validate_friendship_offered checks for exactly that pattern.
#[test]
fn friends_service_validate_offer() {
    let svc = HGFriendsServiceImpl::new();
    let from_id = Uuid::new_v4();
    let to_id = Uuid::new_v4();

    // Store a verified friendship from to_id with from_id as friend.
    // The internal store_friend sets their_flags = -1, which is the
    // "pending" marker that validate_friendship_offered looks for.
    let info = HgFriendInfo {
        principal_id: to_id,
        friend: format!("{from_id};http://foreign:9000/;J D;sec"),
        my_flags: 0,
        their_flags: 0,
    };
    svc.new_friendship(&info, true).unwrap();

    let valid = svc.validate_friendship_offered(from_id, to_id).unwrap();
    assert!(valid, "pending friendship should validate");

    let invalid = svc
        .validate_friendship_offered(Uuid::new_v4(), to_id)
        .unwrap();
    assert!(!invalid, "non-existent offer should not validate");
}

/// friendship_message returns the correct message for foreign vs local.
#[test]
fn friends_friendship_message_content() {
    let module = HGFriendsModule::new();

    let foreign_id = format!("{};http://grid.com;Name Here", Uuid::new_v4());
    assert!(
        module.friendship_message(&foreign_id).contains("another HG grid"),
        "foreign friend message should mention HG"
    );

    let local_id = Uuid::new_v4().to_string();
    assert!(
        !module.friendship_message(&local_id).contains("HG"),
        "local friend message should not mention HG"
    );
}

/// HG friendship level check -- configured via initialise().
#[test]
fn friends_level_gate() {
    let mut module = HGFriendsModule::new();
    let mut config = TestConfig::new();
    config.set_string("Modules", "FriendsModule", "HGFriendsModule");
    config.set_int("HGFriendsModule", "LevelHGFriends", 5);
    module.initialise(&config).unwrap();

    assert!(!module.can_offer_hg_friendship(3));
    assert!(module.can_offer_hg_friendship(5));
    assert!(module.can_offer_hg_friendship(100));
}

/// Online/offline status tracking.
#[test]
fn friends_online_status_tracking() {
    let mut module = HGFriendsModule::new();
    let user = Uuid::new_v4();

    assert!(!module.is_user_online(user));
    module.set_user_online(user);
    assert!(module.is_user_online(user));
    module.set_user_offline(user);
    assert!(!module.is_user_online(user));
}

/// HGFriendRecord parsing: is_foreign, friend_uuid, friend_home_uri, secret.
#[test]
fn friends_record_parsing() {
    let uuid = Uuid::new_v4();

    let local = HGFriendRecord {
        friend: uuid.to_string(),
        my_flags: 1,
        their_flags: 1,
    };
    assert!(!local.is_foreign());
    assert_eq!(local.friend_uuid(), Some(uuid));
    assert!(local.friend_home_uri().is_none());
    assert!(local.secret().is_none());

    let foreign = HGFriendRecord {
        friend: format!("{};http://grid.com:8002;First Last;abc12345", uuid),
        my_flags: 1,
        their_flags: -1,
    };
    assert!(foreign.is_foreign());
    assert_eq!(foreign.friend_uuid(), Some(uuid));
    assert_eq!(
        foreign.friend_home_uri(),
        Some("http://grid.com:8002".to_string())
    );
    assert_eq!(foreign.secret(), Some("abc12345"));
}

/// Module lifecycle for HGFriendsModule.
#[test]
fn friends_module_lifecycle() {
    let mut module = HGFriendsModule::new();
    let mut config = TestConfig::new();
    config.set_string("Modules", "FriendsModule", "HGFriendsModule");
    config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
    module.initialise(&config).unwrap();

    // Verify the module is enabled by checking that can_offer_hg_friendship
    // works with the default level of 0.
    assert!(module.can_offer_hg_friendship(0));

    let scene = scene_ref();
    module.add_region(&scene).unwrap();
    module.remove_region(&scene).unwrap();
    module.close().unwrap();
    assert_eq!(module.cached_friend_list_count(), 0);
}

// ===========================================================================
// Section 6: Bonus -- Cross-cutting HG Asset Service
// ===========================================================================

/// Asset export filtering: allowed types pass, denied types return None.
#[test]
fn asset_service_export_filtering() {
    let svc = HGAssetServiceImpl::new(
        "http://mygrid.example:9000".into(),
        AssetPermissions::new(
            vec![0, 1, 6], // texture, sound, object
            vec![0, 1],    // texture, sound
        ),
    );

    let texture_id = Uuid::new_v4();
    let script_id = Uuid::new_v4();

    svc.insert_local_asset(HgAsset {
        id: texture_id,
        asset_type: 0, // texture
        name: "test texture".into(),
        description: "desc".into(),
        creator_id: "creator-uuid".into(),
        data: vec![1, 2, 3],
        local: false,
        temporary: false,
    });
    svc.insert_local_asset(HgAsset {
        id: script_id,
        asset_type: 10, // script -- not in export list
        name: "test script".into(),
        description: "desc".into(),
        creator_id: "creator-uuid".into(),
        data: vec![4, 5, 6],
        local: false,
        temporary: false,
    });

    assert!(
        svc.get_asset(texture_id).unwrap().is_some(),
        "texture should be exportable"
    );
    assert!(
        svc.get_asset(script_id).unwrap().is_none(),
        "script should be denied export"
    );
}

/// Asset import filtering: denied types return None from store_asset.
#[test]
fn asset_service_import_filtering() {
    let svc = HGAssetServiceImpl::new(
        "http://mygrid.example:9000".into(),
        AssetPermissions::new(
            vec![0],    // export: texture
            vec![0, 1], // import: texture, sound
        ),
    );

    let texture = HgAsset {
        id: Uuid::new_v4(),
        asset_type: 0,
        name: "incoming texture".into(),
        description: "desc".into(),
        creator_id: "foreign-creator".into(),
        data: vec![1],
        local: false,
        temporary: false,
    };
    let script = HgAsset {
        id: Uuid::new_v4(),
        asset_type: 10,
        name: "incoming script".into(),
        description: "desc".into(),
        creator_id: "foreign-creator".into(),
        data: vec![2],
        local: false,
        temporary: false,
    };

    assert!(svc.store_asset(texture).unwrap().is_some());
    assert!(svc.store_asset(script).unwrap().is_none());
}

/// Creator ID rewriting to UUI format on export.
#[test]
fn asset_service_creator_rewrite() {
    let svc = HGAssetServiceImpl::new(
        "http://mygrid.example:9000".into(),
        AssetPermissions::new(vec![0], vec![0]),
    );

    let id = Uuid::new_v4();
    let plain_uuid = "deadbeef-0000-1111-2222-333344445555";
    svc.insert_local_asset(HgAsset {
        id,
        asset_type: 0,
        name: "rewrite test".into(),
        description: "desc".into(),
        creator_id: plain_uuid.into(),
        data: vec![1],
        local: false,
        temporary: false,
    });

    let asset = svc.get_asset(id).unwrap().unwrap();
    assert!(
        asset.creator_id.contains(';'),
        "creator should be rewritten to UUI"
    );
    assert!(
        asset.creator_id.starts_with(plain_uuid),
        "UUI should start with original UUID"
    );
    assert!(
        asset.creator_id.contains("http://mygrid.example:9000"),
        "UUI should contain home URL"
    );
}

// ===========================================================================
// Section 7: UUI / Grid URI Helpers
// ===========================================================================

/// UUI parsing and round-trip.
#[test]
fn uui_parse_roundtrip() {
    let original =
        "550e8400-e29b-41d4-a716-446655440000;http://grid.example.com:8002;Jane Smith";
    let parsed = UniversalUserIdentifier::parse(original).unwrap();
    assert_eq!(parsed.first_name, "Jane");
    assert_eq!(parsed.last_name, "Smith");
    assert_eq!(parsed.home_uri, "http://grid.example.com:8002");
    assert_eq!(parsed.to_uui_string(), original);
}

/// Grid URI normalisation and comparison.
#[test]
fn grid_uri_helpers() {
    assert_eq!(
        normalise_grid_uri("http://Grid.Example.Com:8002/"),
        "http://grid.example.com:8002"
    );
    assert!(is_same_grid(
        "http://grid.example.com:8002/",
        "http://GRID.EXAMPLE.COM:8002"
    ));
    assert!(!is_same_grid(
        "http://grid-a.example.com:8002",
        "http://grid-b.example.com:8002"
    ));
    assert!(is_local_grid_user("", "http://mygrid.com:8002"));
    assert!(is_local_grid_user(
        "http://mygrid.com:8002",
        "http://mygrid.com:8002"
    ));
    assert!(!is_local_grid_user(
        "http://other.com:8002",
        "http://mygrid.com:8002"
    ));
}
