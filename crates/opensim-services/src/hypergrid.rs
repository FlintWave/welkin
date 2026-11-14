// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid protocol services for cross-grid teleportation, identity, and
//! resource sharing.
//!
//! # Protocol overview
//!
//! Hypergrid (HG) is OpenSimulator's federation protocol.  It lets an avatar
//! whose *home grid* is A travel to, interact with, and transfer assets and
//! inventory items on a *foreign grid* B -- without either grid having to
//! share a common user database.
//!
//! The protocol revolves around four service endpoints that each grid exposes
//! over HTTP (XML-RPC for the link/get handshake, REST-style form POST for
//! everything else):
//!
//! ## 1. Gatekeeper service (`IGatekeeperService`)
//!
//! The **entry point** into a grid.  When a viewer on grid A teleports to a
//! region on grid B, the request first hits B's gatekeeper, which:
//!
//!   1. Resolves the destination region (or redirects to a default gateway
//!      region if teleports to arbitrary regions are disabled).
//!   2. Authenticates the visiting agent by calling back to the agent's
//!      **home** `UserAgentService` with `VerifyAgent(session, token)`.
//!   3. Checks for UUID impersonation -- if a local account shares the
//!      visitor's UUID, the gatekeeper verifies the agent is genuinely
//!      "coming home" via `IsAgentComingHome`.
//!   4. Enforces access control: allowed/denied viewer strings, MAC/ID0
//!      bans, foreign-agent allow/deny lists with domain exceptions.
//!   5. Logs a presence record and launches the agent on the destination
//!      region via the simulation service (`CreateAgent`).
//!
//! The gatekeeper also handles the initial **link_region** / **get_region**
//! XML-RPC handshake that establishes a hyperlink between two grids.
//!
//! ## 2. User-agent service (`IUserAgentService`)
//!
//! Runs on the agent's **home** grid and acts as the authoritative source of
//! truth for a traveling agent.  Key responsibilities:
//!
//!   - **LoginAgentToGrid**: initiates a cross-grid teleport on behalf of a
//!     local user.  It creates a `TravelingAgentInfo` record (session, grid
//!     name, service token, client IP) persisted in the `hg_traveling_data`
//!     table, then calls the foreign gatekeeper's `CreateAgent`.
//!   - **VerifyAgent / VerifyClient**: called back by the foreign gatekeeper
//!     to confirm that the session token and client IP are genuine.
//!   - **IsAgentComingHome**: lets a gatekeeper check whether a UUID that
//!     matches a local account really belongs to a returning user.
//!   - **GetHomeRegion**: returns the user's home region for "teleport home".
//!   - **LocateUser / GetUUI / GetUUID**: cross-grid user-location and
//!     identity lookups.
//!   - **StatusNotification / GetOnlineFriends**: relays online-status
//!     information for cross-grid friend lists.
//!   - **GetServerURLs / GetUserInfo**: exposes a user's service endpoint
//!     map and profile details to foreign grids.
//!
//! ## 3. Hypergrid asset service (`HGAssetService`)
//!
//! A filtered wrapper around the local asset service that:
//!
//!   - **Exports** assets only when the asset type is on the allowed-export
//!     list (configured via `AssetPermissions`).
//!   - **Imports** assets only when the type is on the allowed-import list.
//!   - Rewrites **creator identifiers** in object assets to the Universal
//!     User Identifier (UUI) format `uuid;home_uri;first last` so that
//!     creator attribution survives cross-grid transfers.
//!   - Sanitises serialised-object XML before import/export to fix known
//!     data-corruption patterns.
//!   - Refuses `Delete` and `UpdateContent` -- foreign grids may never
//!     mutate local assets.
//!
//! ## 4. Hypergrid inventory service (`HGInventoryService` / `HGSuitcaseInventoryService`)
//!
//! Provides a restricted view of a traveling user's inventory:
//!
//!   - The "root folder" returned to a foreign grid is the **Suitcase**
//!     folder, not the real "My Inventory" root.  This limits what a foreign
//!     grid can see or manipulate.
//!   - Folder creation, deletion, and purge are **refused** -- the foreign
//!     grid may only read, and to a limited extent add items (for derez).
//!   - `HGSuitcaseInventoryService` is the stricter variant: it validates
//!     that every folder/item operation stays within the suitcase tree and
//!     cross-checks against the avatar's current appearance to prevent
//!     accidental loss of worn items.
//!   - Item creator data is rewritten to UUI format on read, the same way
//!     the asset service does it.
//!
//! ## 5. Hypergrid friends service (`IHGFriendsService`)
//!
//! Manages friend relationships that span grid boundaries:
//!
//!   - `NewFriendship` / `DeleteFriendship`: create or remove a cross-grid
//!     friendship record, keyed by a shared secret.
//!   - `FriendshipOffered` / `ValidateFriendshipOffered`: relay and verify
//!     friend requests between grids.
//!   - `GetFriendPerms` / `StatusNotification`: query permissions and relay
//!     online/offline status across grids.
//!
//! ## 6. Hypergrid instant-message service (`HGInstantMessageService`)
//!
//! Routes instant messages between grids.  When a message targets a foreign
//! user, the service looks up the user's current grid via the user-agent
//! service and forwards the IM there.
//!
//! ## Wire format
//!
//! Most service calls use `application/x-www-form-urlencoded` POST bodies
//! with an XML `<ServerResponse>` reply (the same form-post protocol used by
//! the non-HG Robust services).  The gatekeeper's initial handshake
//! (`link_region`, `get_region`) uses XML-RPC instead.
//!
//! ## Security model
//!
//! - A **service session token** (`ServiceSessionID`) of the form
//!   `gatekeeper_url;random_uuid` is generated for each traveling-agent
//!   record.  The foreign gatekeeper verifies this token by calling back to
//!   the home grid's `UserAgentService.VerifyAgent`.
//! - The gatekeeper additionally verifies that the token's embedded grid URL
//!   matches its own address (or a configured alias), preventing a forged
//!   token from being accepted.
//! - Client-IP verification (`VerifyClient`) guards against session hijack
//!   after the initial login, though it can be bypassed by configuration.
//! - Asset and inventory services enforce per-type allow/deny lists so that
//!   grids can prevent export of sensitive asset types (scripts, etc.).

use std::collections::HashMap;
use std::fmt;

use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors specific to Hypergrid operations.
#[derive(Debug, Error)]
pub enum HypergridError {
    /// The foreign gatekeeper could not be reached or returned an error.
    #[error("gatekeeper unreachable at {url}: {detail}")]
    GatekeeperUnreachable { url: String, detail: String },

    /// Identity verification failed (VerifyAgent / VerifyClient).
    #[error("identity verification failed for session {session_id}: {reason}")]
    VerificationFailed { session_id: Uuid, reason: String },

    /// The agent's home URI could not be parsed or resolved.
    #[error("invalid home URI: {0}")]
    InvalidHomeUri(String),

    /// The requested region does not exist or is not accessible.
    #[error("region not found: {0}")]
    RegionNotFound(String),

    /// The agent is banned or otherwise denied entry.
    #[error("access denied: {0}")]
    AccessDenied(String),

    /// UUID impersonation detected -- a foreign agent shares a UUID with a
    /// local account but is not genuinely coming home.
    #[error("UUID impersonation: agent {agent_id} matches local account but is not coming home")]
    UuidImpersonation { agent_id: Uuid },

    /// The requested asset type is not permitted for export or import.
    #[error("asset transfer denied for type {asset_type}: {reason}")]
    AssetTransferDenied { asset_type: i8, reason: String },

    /// An inventory operation was refused (e.g. delete on a foreign grid).
    #[error("inventory operation refused: {0}")]
    InventoryOperationRefused(String),

    /// A cross-grid friend operation failed.
    #[error("friends operation failed: {0}")]
    FriendsError(String),

    /// An HTTP transport error occurred.
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    /// Catch-all for internal / unexpected failures.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result alias for Hypergrid operations.
pub type HgResult<T> = Result<T, HypergridError>;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A three-component floating-point vector used for positions and look-at
/// directions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Vector3 {
    fn default() -> Self {
        Self {
            x: 128.0,
            y: 128.0,
            z: 0.0,
        }
    }
}

impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}, {}, {}>", self.x, self.y, self.z)
    }
}

/// Minimal representation of a grid region, sufficient for Hypergrid
/// link-region and teleport operations.
#[derive(Debug, Clone)]
pub struct GridRegionInfo {
    pub region_id: Uuid,
    pub region_name: String,
    pub region_handle: u64,
    pub server_uri: String,
    pub external_host_name: String,
    pub internal_port: u16,
    pub region_loc_x: u32,
    pub region_loc_y: u32,
    pub region_size_x: u32,
    pub region_size_y: u32,
}

/// Information about a traveling agent -- persisted in the home grid's
/// `hg_traveling_data` table while the agent is visiting a foreign grid.
#[derive(Debug, Clone)]
pub struct TravelingAgentInfo {
    pub session_id: Uuid,
    pub user_id: Uuid,
    /// The `ServerURI` of the grid the agent is currently visiting.
    pub grid_external_name: String,
    /// The service-session token (`gatekeeper_url;random_uuid`).
    pub service_token: String,
    /// The agent's client IP as seen by the home grid at login time.
    pub client_ip_address: String,
}

/// Lightweight agent-circuit descriptor carried across grids during a
/// Hypergrid teleport.
#[derive(Debug, Clone)]
pub struct AgentCircuitInfo {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub secure_session_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub start_pos: Vector3,
    pub service_session_id: String,
    pub service_urls: HashMap<String, String>,
    pub ip_address: String,
    pub viewer: String,
    pub channel: String,
    pub mac: String,
    pub id0: String,
    pub teleport_flags: u32,
}

// ---------------------------------------------------------------------------
// Trait: GatekeeperService
// ---------------------------------------------------------------------------

/// The Gatekeeper is the foreign-facing entry point of a grid.
///
/// It handles the initial hyperlink handshake (`link_region` / `get_region`
/// in XML-RPC) and the agent-login flow for incoming cross-grid teleports.
pub trait GatekeeperService: Send + Sync {
    /// Establish a hyperlink to a local region.
    ///
    /// Called via the `link_region` XML-RPC method.  Returns region metadata
    /// that the calling grid stores as a "linked region" placeholder.
    fn link_local_region(
        &self,
        region_name: &str,
    ) -> HgResult<LinkedRegionInfo>;

    /// Resolve a hyperlink-region UUID to the actual `GridRegionInfo` that a
    /// visiting agent should enter.
    ///
    /// The grid may redirect to a default gateway region when arbitrary
    /// teleports are disabled.
    fn get_hyperlink_region(
        &self,
        region_id: Uuid,
        agent_id: Uuid,
        agent_home_uri: Option<&str>,
    ) -> HgResult<(GridRegionInfo, Option<String>)>;

    /// Authenticate and launch a foreign agent on this grid.
    ///
    /// This is the core of an incoming HG teleport.  The gatekeeper:
    /// 1. Validates the viewer and MAC against allow/deny lists.
    /// 2. Calls the agent's home `UserAgentService.VerifyAgent` to confirm
    ///    the session token.
    /// 3. Checks for UUID impersonation against local accounts.
    /// 4. Enforces foreign-agent allow/deny rules.
    /// 5. Creates a presence record and launches the agent on the
    ///    destination region via the simulation service.
    fn login_agent(
        &self,
        source: Option<&GridRegionInfo>,
        circuit: &AgentCircuitInfo,
        destination: &GridRegionInfo,
    ) -> HgResult<()>;
}

/// Metadata returned by [`GatekeeperService::link_local_region`].
#[derive(Debug, Clone)]
pub struct LinkedRegionInfo {
    pub region_id: Uuid,
    pub region_handle: u64,
    pub external_name: String,
    pub image_url: String,
    pub size_x: u32,
    pub size_y: u32,
}

// ---------------------------------------------------------------------------
// Trait: UserAgentService
// ---------------------------------------------------------------------------

/// The User-Agent service runs on the agent's **home** grid and manages
/// cross-grid identity, travel records, and presence information.
pub trait UserAgentService: Send + Sync {
    /// Initiate a cross-grid teleport for a local user.
    ///
    /// Creates a `TravelingAgentInfo` record, generates a service-session
    /// token, and calls the foreign gatekeeper's `CreateAgent`.
    fn login_agent_to_grid(
        &self,
        source: Option<&GridRegionInfo>,
        circuit: &AgentCircuitInfo,
        gatekeeper: &GridRegionInfo,
        final_destination: &GridRegionInfo,
        from_login: bool,
    ) -> HgResult<()>;

    /// Record that a user has logged out and clean up travel records.
    fn logout_agent(&self, user_id: Uuid, session_id: Uuid) -> HgResult<()>;

    /// Return the user's home region, position and look-at direction.
    fn get_home_region(
        &self,
        user_id: Uuid,
    ) -> HgResult<(GridRegionInfo, Vector3, Vector3)>;

    /// Return the user's service URL map (HomeURI, InventoryServerURI, etc.).
    fn get_server_urls(&self, user_id: Uuid) -> HgResult<HashMap<String, String>>;

    /// Return profile information about a user (name, flags, created date,
    /// title).
    fn get_user_info(&self, user_id: Uuid) -> HgResult<HashMap<String, String>>;

    /// Locate a traveling user -- returns the `ServerURI` of the foreign grid
    /// they are currently visiting, or an empty string if they are home.
    fn locate_user(&self, user_id: Uuid) -> HgResult<String>;

    /// Return the Universal User Identifier for `target_user_id` on behalf
    /// of `requester_id`.  Format: `uuid;home_uri;FirstName LastName`.
    fn get_uui(&self, requester_id: Uuid, target_user_id: Uuid) -> HgResult<String>;

    /// Look up a user by first and last name.  Returns their UUID, or `None`
    /// if no such user exists or their level is below the configured
    /// threshold.
    fn get_uuid(&self, first: &str, last: &str) -> HgResult<Option<Uuid>>;

    /// Verify that a session token matches the traveling-agent record.
    ///
    /// Called by a foreign gatekeeper during the `LoginAgent` flow.
    fn verify_agent(&self, session_id: Uuid, token: &str) -> HgResult<bool>;

    /// Verify that the reported client IP matches the one stored at login.
    ///
    /// Returns `true` when verification is bypassed by configuration.
    fn verify_client(&self, session_id: Uuid, reported_ip: &str) -> HgResult<bool>;

    /// Check whether a session belongs to a local user who is genuinely
    /// returning to this grid (vs. a foreign user with a colliding UUID).
    fn is_agent_coming_home(
        &self,
        session_id: Uuid,
        this_grid_external_name: &str,
    ) -> HgResult<bool>;
}

// ---------------------------------------------------------------------------
// Trait: HypergridAssetService
// ---------------------------------------------------------------------------

/// A filtered asset service for cross-grid asset transfers.
///
/// Wraps the local asset store with permission checks and creator-identity
/// rewriting.
pub trait HypergridAssetService: Send + Sync {
    /// Retrieve an asset by ID, applying export-permission filtering and
    /// UUI creator rewriting.
    ///
    /// Returns `None` if the asset does not exist or its type is not
    /// permitted for export.
    fn get_asset(&self, asset_id: Uuid) -> HgResult<Option<HgAsset>>;

    /// Retrieve only the metadata for an asset, with UUI creator rewriting.
    fn get_metadata(&self, asset_id: Uuid) -> HgResult<Option<HgAssetMetadata>>;

    /// Store a foreign asset on the local grid, subject to import-permission
    /// checks.
    ///
    /// Returns the stored asset's ID (which may differ from the input if the
    /// grid re-maps it), or `None` if the asset type is not permitted.
    fn store_asset(&self, asset: HgAsset) -> HgResult<Option<Uuid>>;

    /// Check whether a set of assets exist locally.
    fn assets_exist(&self, asset_ids: &[Uuid]) -> HgResult<Vec<bool>>;
}

/// An asset transferred via Hypergrid, with metadata and data payload.
#[derive(Debug, Clone)]
pub struct HgAsset {
    pub id: Uuid,
    pub asset_type: i8,
    pub name: String,
    pub description: String,
    pub creator_id: String,
    pub data: Vec<u8>,
    pub local: bool,
    pub temporary: bool,
}

/// Asset metadata as returned by the Hypergrid asset service.
#[derive(Debug, Clone)]
pub struct HgAssetMetadata {
    pub id: Uuid,
    pub asset_type: i8,
    pub name: String,
    pub description: String,
    pub creator_id: String,
}

// ---------------------------------------------------------------------------
// Trait: HypergridInventoryService
// ---------------------------------------------------------------------------

/// A restricted inventory service for cross-grid inventory access.
///
/// Foreign grids see only the Suitcase folder (not the real root), and most
/// mutating operations are refused.
pub trait HypergridInventoryService: Send + Sync {
    /// Return the suitcase root folder for a traveling agent.
    ///
    /// This is the "My Suitcase" folder, not the real "My Inventory" root.
    fn get_root_folder(&self, principal_id: Uuid) -> HgResult<HgInventoryFolder>;

    /// Return a folder's contents (child folders + items), with creator data
    /// rewritten to UUI format.
    fn get_folder_content(
        &self,
        principal_id: Uuid,
        folder_id: Uuid,
    ) -> HgResult<HgInventoryCollection>;

    /// Retrieve a single inventory item with UUI creator rewriting.
    fn get_item(&self, principal_id: Uuid, item_id: Uuid) -> HgResult<Option<HgInventoryItem>>;

    /// Add an item to the suitcase tree (used by derez on foreign grids).
    ///
    /// Implementations should verify the target folder is within the
    /// suitcase tree.
    fn add_item(&self, item: &HgInventoryItem) -> HgResult<bool>;
}

/// A folder within the Hypergrid suitcase inventory.
#[derive(Debug, Clone)]
pub struct HgInventoryFolder {
    pub folder_id: Uuid,
    pub parent_id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub folder_type: i16,
}

/// An inventory item with HG-adjusted creator data.
#[derive(Debug, Clone)]
pub struct HgInventoryItem {
    pub item_id: Uuid,
    pub folder_id: Uuid,
    pub owner_id: Uuid,
    pub asset_id: Uuid,
    pub asset_type: i32,
    pub name: String,
    pub description: String,
    pub creator_id: String,
    pub creator_data: String,
}

/// A collection of folders and items returned by a folder-content query.
#[derive(Debug, Clone)]
pub struct HgInventoryCollection {
    pub folders: Vec<HgInventoryFolder>,
    pub items: Vec<HgInventoryItem>,
}

// ---------------------------------------------------------------------------
// Trait: HypergridFriendsService
// ---------------------------------------------------------------------------

/// Manages friend relationships that span grid boundaries.
pub trait HypergridFriendsService: Send + Sync {
    /// Query the permission flags for a cross-grid friendship.
    fn get_friend_perms(&self, user_id: Uuid, friend_id: Uuid) -> HgResult<i32>;

    /// Create a new cross-grid friendship record.
    fn new_friendship(&self, friend: &HgFriendInfo, verified: bool) -> HgResult<bool>;

    /// Delete a cross-grid friendship record, validated by the shared secret.
    fn delete_friendship(&self, friend: &HgFriendInfo, secret: &str) -> HgResult<bool>;

    /// Relay a friendship offer to the target user on this grid.
    fn friendship_offered(
        &self,
        from_id: Uuid,
        from_name: &str,
        to_id: Uuid,
        message: &str,
    ) -> HgResult<bool>;

    /// Validate that a pending friendship offer exists between two users.
    fn validate_friendship_offered(
        &self,
        from_id: Uuid,
        to_id: Uuid,
    ) -> HgResult<bool>;

    /// Relay online/offline status notifications for a list of cross-grid
    /// friends.  Returns the subset of friends that are online locally.
    fn status_notification(
        &self,
        friends: &[String],
        user_id: Uuid,
        online: bool,
    ) -> HgResult<Vec<Uuid>>;
}

/// A cross-grid friendship record.
#[derive(Debug, Clone)]
pub struct HgFriendInfo {
    pub principal_id: Uuid,
    pub friend: String,
    pub my_flags: i32,
    pub their_flags: i32,
}

// ---------------------------------------------------------------------------
// Trait: HypergridInstantMessageService
// ---------------------------------------------------------------------------

/// Routes instant messages between grids.
pub trait HypergridInstantMessageService: Send + Sync {
    /// Handle an IM arriving from a foreign grid, destined for a local user.
    fn incoming_instant_message(&self, im: &HgInstantMessage) -> HgResult<bool>;

    /// Forward an IM from a local user to a foreign recipient.
    fn outgoing_instant_message(
        &self,
        im: &HgInstantMessage,
        target_url: &str,
        foreigner: bool,
    ) -> HgResult<bool>;
}

/// A grid instant message, as relayed between Hypergrid services.
#[derive(Debug, Clone)]
pub struct HgInstantMessage {
    pub from_agent_id: Uuid,
    pub from_agent_name: String,
    pub to_agent_id: Uuid,
    pub dialog: u8,
    pub message: String,
    pub im_session_id: Uuid,
    pub offline: u8,
    pub position: Vector3,
    pub region_id: Uuid,
    pub timestamp: u32,
    pub from_group: bool,
    pub parent_estate_id: u32,
    pub binary_bucket: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Stub implementations
// ---------------------------------------------------------------------------

/// A placeholder gatekeeper that refuses all operations.
///
/// Replace with a real implementation that wires up grid, presence,
/// user-account, simulation, and user-agent backing services.
pub struct StubGatekeeperService;

impl GatekeeperService for StubGatekeeperService {
    fn link_local_region(&self, _region_name: &str) -> HgResult<LinkedRegionInfo> {
        todo!("GatekeeperService::link_local_region")
    }

    fn get_hyperlink_region(
        &self,
        _region_id: Uuid,
        _agent_id: Uuid,
        _agent_home_uri: Option<&str>,
    ) -> HgResult<(GridRegionInfo, Option<String>)> {
        todo!("GatekeeperService::get_hyperlink_region")
    }

    fn login_agent(
        &self,
        _source: Option<&GridRegionInfo>,
        _circuit: &AgentCircuitInfo,
        _destination: &GridRegionInfo,
    ) -> HgResult<()> {
        todo!("GatekeeperService::login_agent")
    }
}

/// A placeholder user-agent service.
pub struct StubUserAgentService;

impl UserAgentService for StubUserAgentService {
    fn login_agent_to_grid(
        &self,
        _source: Option<&GridRegionInfo>,
        _circuit: &AgentCircuitInfo,
        _gatekeeper: &GridRegionInfo,
        _final_destination: &GridRegionInfo,
        _from_login: bool,
    ) -> HgResult<()> {
        todo!("UserAgentService::login_agent_to_grid")
    }

    fn logout_agent(&self, _user_id: Uuid, _session_id: Uuid) -> HgResult<()> {
        todo!("UserAgentService::logout_agent")
    }

    fn get_home_region(&self, _user_id: Uuid) -> HgResult<(GridRegionInfo, Vector3, Vector3)> {
        todo!("UserAgentService::get_home_region")
    }

    fn get_server_urls(&self, _user_id: Uuid) -> HgResult<HashMap<String, String>> {
        todo!("UserAgentService::get_server_urls")
    }

    fn get_user_info(&self, _user_id: Uuid) -> HgResult<HashMap<String, String>> {
        todo!("UserAgentService::get_user_info")
    }

    fn locate_user(&self, _user_id: Uuid) -> HgResult<String> {
        todo!("UserAgentService::locate_user")
    }

    fn get_uui(&self, _requester_id: Uuid, _target_user_id: Uuid) -> HgResult<String> {
        todo!("UserAgentService::get_uui")
    }

    fn get_uuid(&self, _first: &str, _last: &str) -> HgResult<Option<Uuid>> {
        todo!("UserAgentService::get_uuid")
    }

    fn verify_agent(&self, _session_id: Uuid, _token: &str) -> HgResult<bool> {
        todo!("UserAgentService::verify_agent")
    }

    fn verify_client(&self, _session_id: Uuid, _reported_ip: &str) -> HgResult<bool> {
        todo!("UserAgentService::verify_client")
    }

    fn is_agent_coming_home(
        &self,
        _session_id: Uuid,
        _this_grid_external_name: &str,
    ) -> HgResult<bool> {
        todo!("UserAgentService::is_agent_coming_home")
    }
}

/// A placeholder Hypergrid asset service.
pub struct StubHypergridAssetService;

impl HypergridAssetService for StubHypergridAssetService {
    fn get_asset(&self, _asset_id: Uuid) -> HgResult<Option<HgAsset>> {
        todo!("HypergridAssetService::get_asset")
    }

    fn get_metadata(&self, _asset_id: Uuid) -> HgResult<Option<HgAssetMetadata>> {
        todo!("HypergridAssetService::get_metadata")
    }

    fn store_asset(&self, _asset: HgAsset) -> HgResult<Option<Uuid>> {
        todo!("HypergridAssetService::store_asset")
    }

    fn assets_exist(&self, _asset_ids: &[Uuid]) -> HgResult<Vec<bool>> {
        todo!("HypergridAssetService::assets_exist")
    }
}

/// A placeholder Hypergrid inventory service.
pub struct StubHypergridInventoryService;

impl HypergridInventoryService for StubHypergridInventoryService {
    fn get_root_folder(&self, _principal_id: Uuid) -> HgResult<HgInventoryFolder> {
        todo!("HypergridInventoryService::get_root_folder")
    }

    fn get_folder_content(
        &self,
        _principal_id: Uuid,
        _folder_id: Uuid,
    ) -> HgResult<HgInventoryCollection> {
        todo!("HypergridInventoryService::get_folder_content")
    }

    fn get_item(
        &self,
        _principal_id: Uuid,
        _item_id: Uuid,
    ) -> HgResult<Option<HgInventoryItem>> {
        todo!("HypergridInventoryService::get_item")
    }

    fn add_item(&self, _item: &HgInventoryItem) -> HgResult<bool> {
        todo!("HypergridInventoryService::add_item")
    }
}

/// A placeholder Hypergrid friends service.
pub struct StubHypergridFriendsService;

impl HypergridFriendsService for StubHypergridFriendsService {
    fn get_friend_perms(&self, _user_id: Uuid, _friend_id: Uuid) -> HgResult<i32> {
        todo!("HypergridFriendsService::get_friend_perms")
    }

    fn new_friendship(&self, _friend: &HgFriendInfo, _verified: bool) -> HgResult<bool> {
        todo!("HypergridFriendsService::new_friendship")
    }

    fn delete_friendship(&self, _friend: &HgFriendInfo, _secret: &str) -> HgResult<bool> {
        todo!("HypergridFriendsService::delete_friendship")
    }

    fn friendship_offered(
        &self,
        _from_id: Uuid,
        _from_name: &str,
        _to_id: Uuid,
        _message: &str,
    ) -> HgResult<bool> {
        todo!("HypergridFriendsService::friendship_offered")
    }

    fn validate_friendship_offered(&self, _from_id: Uuid, _to_id: Uuid) -> HgResult<bool> {
        todo!("HypergridFriendsService::validate_friendship_offered")
    }

    fn status_notification(
        &self,
        _friends: &[String],
        _user_id: Uuid,
        _online: bool,
    ) -> HgResult<Vec<Uuid>> {
        todo!("HypergridFriendsService::status_notification")
    }
}

/// A placeholder Hypergrid instant-message service.
pub struct StubHypergridInstantMessageService;

impl HypergridInstantMessageService for StubHypergridInstantMessageService {
    fn incoming_instant_message(&self, _im: &HgInstantMessage) -> HgResult<bool> {
        todo!("HypergridInstantMessageService::incoming_instant_message")
    }

    fn outgoing_instant_message(
        &self,
        _im: &HgInstantMessage,
        _target_url: &str,
        _foreigner: bool,
    ) -> HgResult<bool> {
        todo!("HypergridInstantMessageService::outgoing_instant_message")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = HypergridError::AccessDenied("foreigners not allowed".into());
        assert_eq!(err.to_string(), "access denied: foreigners not allowed");
    }

    #[test]
    fn vector3_default() {
        let v = Vector3::default();
        assert!((v.x - 128.0).abs() < f32::EPSILON);
        assert!((v.y - 128.0).abs() < f32::EPSILON);
        assert!((v.z - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn linked_region_info_debug() {
        let info = LinkedRegionInfo {
            region_id: Uuid::nil(),
            region_handle: 0,
            external_name: "http://example.com:9000/ Test Region".into(),
            image_url: String::new(),
            size_x: 256,
            size_y: 256,
        };
        // Ensure Debug impl works (compile-time + runtime check).
        let dbg = format!("{:?}", info);
        assert!(dbg.contains("Test Region"));
    }

    #[test]
    fn traveling_agent_info_fields() {
        let tai = TravelingAgentInfo {
            session_id: Uuid::nil(),
            user_id: Uuid::nil(),
            grid_external_name: "http://other-grid.example:9000/".into(),
            service_token: "http://other-grid.example:9000/;some-random-uuid".into(),
            client_ip_address: "192.168.1.100".into(),
        };
        assert!(tai.service_token.contains(';'));
        assert_eq!(tai.client_ip_address, "192.168.1.100");
    }
}
