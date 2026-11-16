// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL built-in function bindings for the Luau VM.
//!
//! Each sub-module registers a family of `ll*` functions as Lua globals so
//! that in-world scripts can call them.  Pure-math and string functions are
//! fully implemented; communication, physics and object functions are stubs
//! that log via [`tracing`] and return sensible defaults.

pub mod avatar;
pub mod cap_url;
pub mod communication;
pub mod constants;
pub mod constants_extra;
pub mod email;
pub mod http;
pub mod inventory;
pub mod land;
pub mod list;
pub mod math;
pub mod object;
pub mod physics;
pub mod control;
pub mod prim_params;
pub mod sensor;
pub mod agent_data;
pub mod dialogs;
pub mod sound;
pub mod string;
pub mod timer;
pub mod vehicle;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::VmError;

// ---------------------------------------------------------------------------
// WindlightData — region environment/windlight settings for LightShare API
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindlightData {
    pub water_color: [f64; 3],
    pub water_fog_density_exponent: f64,
    pub underwater_fog_modifier: f64,
    pub reflection_wavelet_scale: [f64; 3],
    pub fresnel_scale: f64,
    pub fresnel_offset: f64,
    pub refract_scale_above: f64,
    pub refract_scale_below: f64,
    pub blur_multiplier: f64,
    pub big_wave_direction: [f64; 2],
    pub little_wave_direction: [f64; 2],
    pub normal_map_texture: String,
    pub horizon: [f64; 4],
    pub haze_density: f64,
    pub haze_horizon: f64,
    pub density_multiplier: f64,
    pub distance_multiplier: f64,
    pub max_altitude: u16,
    pub sun_moon_color: [f64; 4],
    pub sun_moon_position: f64,
    pub ambient: [f64; 4],
    pub east_angle: f64,
    pub sun_glow_focus: f64,
    pub sun_glow_size: f64,
    pub scene_gamma: f64,
    pub star_brightness: f64,
    pub cloud_color: [f64; 4],
    pub cloud_xy_density: [f64; 3],
    pub cloud_coverage: f64,
    pub cloud_scale: f64,
    pub cloud_detail_xy_density: [f64; 3],
    pub cloud_scroll_x: f64,
    pub cloud_scroll_y: f64,
    pub cloud_scroll_x_lock: bool,
    pub cloud_scroll_y_lock: bool,
    pub draw_classic_clouds: bool,
}

impl Default for WindlightData {
    fn default() -> Self {
        Self {
            water_color: [4.0, 38.0, 64.0],
            water_fog_density_exponent: 4.0,
            underwater_fog_modifier: 0.25,
            reflection_wavelet_scale: [2.0, 2.0, 2.0],
            fresnel_scale: 0.4,
            fresnel_offset: 0.5,
            refract_scale_above: 0.03,
            refract_scale_below: 0.2,
            blur_multiplier: 0.04,
            big_wave_direction: [1.05, -0.42],
            little_wave_direction: [1.11, -0.76],
            normal_map_texture: "822ded49-9a6c-f61c-cb89-6df54f42cdf4".to_string(),
            horizon: [0.25, 0.25, 0.32, 0.32],
            haze_density: 4.0,
            haze_horizon: 0.19,
            density_multiplier: 0.18,
            distance_multiplier: 0.8,
            max_altitude: 1605,
            sun_moon_color: [0.24, 0.26, 0.30, 0.30],
            sun_moon_position: 0.317,
            ambient: [0.35, 0.35, 0.35, 0.35],
            east_angle: 0.0,
            sun_glow_focus: 0.1,
            sun_glow_size: 1.75,
            scene_gamma: 1.0,
            star_brightness: 0.0,
            cloud_color: [0.41, 0.41, 0.41, 0.41],
            cloud_xy_density: [1.0, 0.53, 1.0],
            cloud_coverage: 0.27,
            cloud_scale: 0.42,
            cloud_detail_xy_density: [1.0, 0.53, 0.12],
            cloud_scroll_x: 0.2,
            cloud_scroll_y: 0.01,
            cloud_scroll_x_lock: false,
            cloud_scroll_y_lock: false,
            draw_classic_clouds: true,
        }
    }
}

// ---------------------------------------------------------------------------
// MOD API types — module invocation registry for modInvoke* functions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModCommand {
    pub module: String,
    pub command: String,
    pub key: String,
}

pub type ModuleCallback = Arc<dyn Fn(&[serde_json::Value]) -> serde_json::Value + Send + Sync>;

pub const INVENTORY_TEXTURE: i32 = 0;
pub const INVENTORY_SOUND: i32 = 1;
pub const INVENTORY_LANDMARK: i32 = 3;
pub const INVENTORY_CLOTHING: i32 = 5;
pub const INVENTORY_OBJECT: i32 = 6;
pub const INVENTORY_NOTECARD: i32 = 7;
pub const INVENTORY_SCRIPT: i32 = 10;
pub const INVENTORY_BODYPART: i32 = 13;
pub const INVENTORY_ANIMATION: i32 = 20;
pub const INVENTORY_GESTURE: i32 = 21;
pub const INVENTORY_ALL: i32 = -1;
pub const INVENTORY_NONE: i32 = -1;

#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub name: String,
    pub item_type: i32,
    pub asset_id: Uuid,
    /// The inventory item's own UUID (distinct from the asset UUID).
    pub item_id: Uuid,
    pub description: String,
    /// UUID of the previous owner before the current one.
    pub last_owner_id: Uuid,
    /// UUID of the item's creator.
    pub creator_id: Uuid,
    /// Permission masks: base, owner, group, everyone, next-owner.
    pub permissions_base: u32,
    pub permissions_owner: u32,
    pub permissions_group: u32,
    pub permissions_everyone: u32,
    pub permissions_next: u32,
}

impl Default for InventoryItem {
    fn default() -> Self {
        Self {
            name: String::new(),
            item_type: 0,
            asset_id: Uuid::nil(),
            item_id: Uuid::nil(),
            description: String::new(),
            last_owner_id: Uuid::nil(),
            creator_id: Uuid::nil(),
            permissions_base: 0,
            permissions_owner: 0,
            permissions_group: 0,
            permissions_everyone: 0,
            permissions_next: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Permission-mask category indices (OpenSim ScriptBaseClass)
// ---------------------------------------------------------------------------

pub const MASK_BASE: i32 = 0;
pub const MASK_OWNER: i32 = 1;
pub const MASK_GROUP: i32 = 2;
pub const MASK_EVERYONE: i32 = 3;
pub const MASK_NEXT: i32 = 4;

/// OpenSim `PermissionMask` flag bits.
pub const PERM_TRANSFER: u32 = 0x2000;
pub const PERM_MODIFY: u32 = 0x4000;
pub const PERM_COPY: u32 = 0x8000;
pub const PERM_MOVE: u32 = 0x0008_0000;

/// Attachment-message option constants (OpenSim ScriptBaseClass).
pub const OS_ATTACH_MSG_ALL: i32 = -1;
pub const OS_ATTACH_MSG_INVERT_POINTS: i32 = 0x01;

/// Content-type constants for `osSetContentType` (OpenSim).
pub const OS_CONTENT_TYPE_TEXT: i32 = 0;
pub const OS_CONTENT_TYPE_HTML: i32 = 1;
pub const OS_CONTENT_TYPE_XML: i32 = 2;
pub const OS_CONTENT_TYPE_XHTML: i32 = 3;
pub const OS_CONTENT_TYPE_ATOM: i32 = 4;
pub const OS_CONTENT_TYPE_JSON: i32 = 5;
pub const OS_CONTENT_TYPE_RSS: i32 = 6;

#[derive(Debug, Clone)]
pub struct ListenFilter {
    pub handle: i32,
    pub channel: i32,
    pub name: String,
    pub id: String,
    pub msg: String,
    pub active: bool,
    /// Bitmask: OS_LISTEN_REGEX_NAME (0x1) and/or OS_LISTEN_REGEX_MESSAGE (0x2).
    pub regex_bitfield: i32,
}

#[derive(Debug, Clone)]
pub struct RezRequest {
    pub source_script_id: Uuid,
    pub source_prim_id: Uuid,
    pub item_name: String,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub rotation: [f64; 4],
    pub start_param: i32,
    pub at_root: bool,
}

#[derive(Debug, Clone)]
pub struct AttachRequest {
    pub script_id: Uuid,
    pub prim_id: Uuid,
    pub owner_id: Uuid,
    pub attach_point: i32,
    pub is_temp: bool,
}

#[derive(Debug, Clone)]
pub enum AttachAction {
    Attach(AttachRequest),
    Detach { prim_id: Uuid },
}

#[derive(Debug, Clone)]
pub struct PositionTarget {
    pub handle: i32,
    pub position: [f64; 3],
    pub range: f64,
}

#[derive(Debug, Clone)]
pub struct RotationTarget {
    pub handle: i32,
    pub rotation: [f64; 4],
    pub range: f64,
}

/// NPC registry entry tracking a spawned NPC's state.
#[derive(Debug, Clone)]
pub struct NpcInfo {
    pub owner_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub position: [f64; 3],
    pub rotation: [f64; 4],
    pub profile_about: String,
    pub profile_image: Uuid,
}

/// Commands sent from os* NPC functions to the region NPC subsystem.
#[derive(Debug, Clone)]
pub enum NpcCommand {
    Create {
        npc_id: Uuid,
        owner: Uuid,
        first_name: String,
        last_name: String,
        position: [f64; 3],
        options: i32,
        appearance_notecard: String,
    },
    Remove { npc_id: Uuid },
    MoveTo { npc_id: Uuid, target: [f64; 3], options: i32 },
    StopMove { npc_id: Uuid },
    Say { npc_id: Uuid, channel: i32, message: String, chat_type: i32 },
    Sit { npc_id: Uuid, target: Uuid, options: i32 },
    Stand { npc_id: Uuid },
    Touch { npc_id: Uuid, target: Uuid, link_num: i32 },
    SetRot { npc_id: Uuid, rotation: [f64; 4] },
    PlayAnimation { npc_id: Uuid, animation: String },
    StopAnimation { npc_id: Uuid, animation: String },
    SaveAppearance { npc_id: Uuid, notecard_name: String, notecard_id: Uuid },
    LoadAppearance { npc_id: Uuid, notecard_name: String },
    SetProfileAbout { npc_id: Uuid, about: String },
    SetProfileImage { npc_id: Uuid, image_id: Uuid },
    LookAt { npc_id: Uuid, look_type: i32, target_id: Uuid, offset: [f64; 3] },
}

/// Commands sent from os* dynamic texture functions.
#[derive(Debug, Clone)]
pub enum DynamicTextureCommand {
    FromUrl {
        prim_id: Uuid,
        content_type: String,
        url: String,
        extra_params: String,
        timer: i32,
        alpha: i32,
        texture_id: Uuid,
    },
    FromData {
        prim_id: Uuid,
        content_type: String,
        data: String,
        extra_params: String,
        timer: i32,
        alpha: i32,
        texture_id: Uuid,
    },
}

/// Command sent from osRegionRestart to the region subsystem.
#[derive(Debug, Clone)]
pub struct RegionRestartCommand {
    pub delay_seconds: f64,
    pub message: String,
    pub requester_id: Uuid,
}

/// Teleport request sent from os* teleport functions.
#[derive(Debug, Clone)]
pub struct TeleportRequest {
    pub agent_id: Uuid,
    pub region_name: String,
    pub position: [f64; 3],
    pub look_at: [f64; 3],
}

/// Information about a presence (avatar or NPC) in the scene, provided to
/// scripts by the hosting scene system.
#[derive(Debug, Clone)]
pub struct PresenceInfo {
    pub uuid: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub position: [f64; 3],
    pub is_npc: bool,
    /// The agent's IP address (for osGetAgentIP).
    pub ip_address: String,
    /// Visual appearance parameters (bake-layer byte array).
    pub visual_params: Vec<u8>,
}

/// Commands sent from OSSL agent functions to the region agent subsystem.
#[derive(Debug, Clone)]
pub enum AgentCommand {
    Kick {
        agent_id: Uuid,
        message: String,
    },
    ForceSit {
        agent_id: Uuid,
        target_id: Uuid,
    },
    LocalTeleport {
        agent_id: Uuid,
        position: [f64; 3],
        velocity: [f64; 3],
        look_at: [f64; 3],
        flags: i32,
    },
    TeleportObject {
        object_id: Uuid,
        target_pos: [f64; 3],
        rotation: [f64; 4],
        flags: i32,
    },
}

/// Commands sent from OSSL animation functions to the region animation subsystem.
#[derive(Debug, Clone)]
pub enum AnimationCommand {
    Play { agent_id: Uuid, animation: String },
    Stop { agent_id: Uuid, animation: String },
}

/// Commands sent from OSSL inventory functions to the region inventory subsystem.
#[derive(Debug, Clone)]
pub enum InventoryCommand {
    Give {
        destination: Uuid,
        item_name: String,
        item_id: Uuid,
        asset_id: Uuid,
        item_type: i32,
    },
    GiveList {
        destination: Uuid,
        category: String,
        items: Vec<(String, Uuid, Uuid)>,
    },
    TakeObject {
        object_id: Uuid,
        owner_id: Uuid,
    },
}

/// Commands sent from OSSL sound functions to the region sound subsystem.
#[derive(Debug, Clone)]
pub enum SoundCommand {
    PlaySound { prim_id: Uuid, sound_id: Uuid, volume: f64 },
    LoopSound { prim_id: Uuid, sound_id: Uuid, volume: f64, master: bool, slave: bool },
    StopSound { prim_id: Uuid },
    PreloadSound { prim_id: Uuid, sound_id: Uuid },
    AdjustVolume { prim_id: Uuid, volume: f64 },
    TriggerSound { prim_id: Uuid, sound_id: Uuid, volume: f64 },
    TriggerSoundLimited {
        prim_id: Uuid,
        sound_id: Uuid,
        volume: f64,
        top_ne: [f64; 3],
        bottom_sw: [f64; 3],
    },
    SetRadius { prim_id: Uuid, radius: f64 },
    CollisionSound { prim_id: Uuid, sound_id: Uuid, volume: f64 },
    SetQueueing { prim_id: Uuid, enabled: bool },
}

/// Commands sent from OSSL link functions to the region link subsystem.
#[derive(Debug, Clone)]
pub enum LinkCommand {
    CreateLink { target_id: Uuid, parent: bool },
    BreakLink { link_num: i32 },
    BreakAllLinks,
}

/// Commands sent from OSSL object functions to the region object subsystem.
#[derive(Debug, Clone)]
pub enum ObjectCommand {
    Die { object_id: Uuid },
    MessageObject { object_id: Uuid, source_id: Uuid, message: String },
    SetContentType { request_id: Uuid, content_type: String },
    VolumeDetect { prim_id: Uuid, enable: bool },
    SetProjectionParams {
        prim_id: Uuid,
        enabled: bool,
        texture: Uuid,
        fov: f64,
        focus: f64,
        ambience: f64,
    },
}

/// Info about a prim in a linkset, for link-number lookups.
#[derive(Debug, Clone)]
pub struct LinkPartInfo {
    pub link_num: i32,
    pub name: String,
    pub prim_id: Uuid,
}

/// Custom inertia override for a prim/linkset.
#[derive(Debug, Clone)]
pub struct InertiaData {
    pub mass: f64,
    pub center_of_mass: [f64; 3],
    /// Principal moments of inertia divided by mass: Ixx/m, Iyy/m, Izz/m.
    pub principal_inertia: [f64; 3],
    /// Off-diagonal inertia terms divided by mass: Ixy/m, Ixz/m, Iyz/m.
    pub aux: [f64; 3],
    /// Rotation of the inertia frame relative to the local axis.
    pub rotation: [f64; 4],
}

/// Commands sent from misc OSSL functions to the region subsystem.
#[derive(Debug, Clone)]
pub enum MiscCommand {
    SaveAppearance {
        agent_id: Uuid,
        notecard_name: String,
        notecard_id: Uuid,
    },
    SetInertia {
        prim_id: Uuid,
        data: Option<InertiaData>,
    },
    SitOnLink {
        agent_id: Uuid,
        link_num: i32,
        offset: [f64; 3],
        rotation: [f64; 4],
    },
    ClearObjectAnimations {
        prim_id: Uuid,
    },
    ResetAllScripts {
        prim_id: Uuid,
        caller_script_id: Uuid,
    },
    SetParticleSystem {
        prim_id: Uuid,
        link_num: i32,
        rules: Vec<String>,
    },
    InviteToGroup {
        group_id: Uuid,
        agent_id: Uuid,
        inviter_id: Uuid,
    },
    EjectFromGroup {
        group_id: Uuid,
        agent_id: Uuid,
        ejector_id: Uuid,
    },
    ConsoleCommand {
        command: String,
        owner_id: Uuid,
    },
    SetEntityRotation {
        target_id: Uuid,
        rotation: [f64; 4],
    },
}

/// Shared context available to every ll* function closure.
///
/// Wrapped in `Arc<RwLock<..>>` and cloned into each registered closure so
/// that builtins can read prim identity, position, etc.
pub struct ScriptContext {
    pub script_id: Uuid,
    pub prim_id: Uuid,
    pub prim_name: String,
    pub prim_desc: String,
    pub owner_id: Uuid,
    pub position: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
    pub link_num: i32,
    pub color: [f64; 3],
    pub alpha: f64,
    pub creator_id: Uuid,
    pub texture: Uuid,
    pub permissions_granted: u32,
    pub sit_target_pos: [f64; 3],
    pub sit_target_rot: [f64; 4],
    pub sitting_avatar: Option<Uuid>,
    pub inventory: Vec<InventoryItem>,
    pub allow_inventory_drop: bool,
    pub parcel_name: String,
    pub parcel_description: String,
    pub parcel_owner: Uuid,
    pub parcel_area: i32,
    pub parcel_flags: u32,
    pub event_sender: Option<std::sync::mpsc::Sender<(Uuid, crate::ScriptEvent)>>,
    pub chat_sender: Option<std::sync::mpsc::Sender<crate::ChatMessage>>,
    pub listen_handles: Vec<ListenFilter>,
    pub next_listen_handle: i32,
    pub physics_sender: Option<std::sync::mpsc::Sender<(Uuid, crate::PhysicsCommand)>>,
    pub velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    pub acceleration: [f64; 3],
    pub position_targets: Vec<PositionTarget>,
    pub rotation_targets: Vec<RotationTarget>,
    pub next_target_handle: i32,
    pub notecard_data: HashMap<Uuid, String>,
    pub vehicle: vehicle::VehicleParams,
    pub hover_text: String,
    pub hover_text_color: [f64; 3],
    pub prim_material: i32,
    pub is_phantom: bool,
    pub is_physics: bool,
    pub is_temp_on_rez: bool,
    pub fullbright: bool,
    pub glow: f64,
    pub physics_shape_type: i32,
    pub rez_sender: Option<std::sync::mpsc::Sender<RezRequest>>,
    pub attach_sender: Option<std::sync::mpsc::Sender<AttachAction>>,
    pub email_queue: VecDeque<email::EmailMessage>,
    pub email_sender: Option<std::sync::mpsc::Sender<email::EmailOutbound>>,
    pub last_email_time: Option<std::time::Instant>,
    pub cap_url_sender: Option<std::sync::mpsc::Sender<cap_url::CapUrlRequest>>,
    pub active_urls: Vec<String>,
    pub http_sender: Option<std::sync::mpsc::Sender<http::HttpOutboundRequest>>,
    pub http_response_sender: Option<std::sync::mpsc::Sender<http::HttpResponse>>,
    pub http_pending_count: i32,
    pub http_content_type: String,
    pub region_size: [i32; 2],
    pub water_height: f64,
    pub sun_hour: f64,
    pub sun_fixed: bool,
    pub day_length: f64,
    pub use_estate_sun: bool,
    pub speed_multiplier: f64,
    pub terrain_heights: Option<Vec<f32>>,
    pub terrain_textures: [Uuid; 4],
    pub terrain_texture_heights: [[f64; 2]; 4],
    pub terrain_dirty: bool,
    pub last_terrain_flush: f64,
    pub wind_params: HashMap<String, f64>,
    pub float_on_water: bool,
    pub parcel_media_url: String,
    pub parcel_music_url: String,
    pub parcel_sip_address: String,
    pub parcel_dwell: f64,
    pub parcel_prim_count: i32,
    pub parcel_max_prims: i32,
    pub notecard_names: HashMap<String, Uuid>,
    pub npc_registry: HashMap<Uuid, NpcInfo>,
    pub npc_sender: Option<std::sync::mpsc::Sender<NpcCommand>>,
    pub teleport_sender: Option<std::sync::mpsc::Sender<TeleportRequest>>,
    pub dynamic_texture_sender: Option<std::sync::mpsc::Sender<DynamicTextureCommand>>,
    pub timer_sender: Option<std::sync::mpsc::Sender<(uuid::Uuid, f64)>>,
    pub start_time: std::time::Instant,
    pub sensor_sender: Option<std::sync::mpsc::Sender<sensor::SensorRequest>>,
    pub control_sender: Option<std::sync::mpsc::Sender<control::ControlRequest>>,
    /// Presences currently in the scene (populated by the hosting system).
    pub presence_list: Vec<PresenceInfo>,
    /// Per-agent health values (default 100.0 for a living agent).
    pub agent_health: HashMap<Uuid, f64>,
    /// Per-agent heal rates (hp/sec, default 0.0).
    pub agent_heal_rate: HashMap<Uuid, f64>,
    /// Sender for agent commands (kick, force-sit, teleport, etc.).
    pub agent_sender: Option<std::sync::mpsc::Sender<AgentCommand>>,
    /// Sender for animation commands (play/stop on a specific avatar).
    pub animation_sender: Option<std::sync::mpsc::Sender<AnimationCommand>>,
    /// Sender for miscellaneous OSSL commands (appearance, inertia, etc.).
    pub misc_sender: Option<std::sync::mpsc::Sender<MiscCommand>>,
    /// Region OAR creation date string (from RegionSettings.LoadedCreationDate).
    pub region_loaded_creation_date: String,
    /// Region OAR creation time string (from RegionSettings.LoadedCreationTime).
    pub region_loaded_creation_time: String,
    /// Region OAR creation ID (UUID string from RegionSettings.LoadedCreationID).
    pub region_loaded_creation_id: String,
    /// The map texture UUID for the current region.
    pub region_map_texture: Uuid,
    /// The group UUID that the host prim is assigned to.
    pub group_id: Uuid,
    /// Custom inertia override (None = automatic physics calculation).
    pub inertia: Option<InertiaData>,
    /// Active object-level animations on the host prim.
    pub object_animations: Vec<Uuid>,
    /// The grid's home URI (returned by osGetAvatarHomeURI when no user-specific URI).
    pub grid_home_uri: String,
    /// Short nickname of the grid (e.g. "welkin").
    pub grid_nick: String,
    /// Human-readable name of the grid.
    pub grid_name: String,
    /// The grid's login URI.
    pub grid_login_uri: String,
    /// The grid's gatekeeper URI (for hypergrid).
    pub grid_gatekeeper_uri: String,
    /// Arbitrary grid-custom key/value pairs.
    pub grid_custom: HashMap<String, String>,
    /// Sender for region restart commands from osRegionRestart.
    pub restart_sender: Option<std::sync::mpsc::Sender<RegionRestartCommand>>,
    /// Current simulator frames per second.
    pub sim_fps: f64,
    /// Current physics frames per second.
    pub physics_fps: f64,
    /// Time dilation (1.0 = normal, <1.0 = lagging).
    pub time_dilation: f64,
    /// Total prim count in the region.
    pub total_prims: i32,
    /// Number of active scripts in the region.
    pub active_scripts: i32,
    /// Script lines per second.
    pub script_lps: f64,
    /// Per-link-number color (link_num -> color), populated by the hosting system.
    pub link_colors: HashMap<i32, [f64; 3]>,
    /// UUID of the object that rezzed this object (for `osGetRezzingObject`).
    pub rezzing_object_id: Uuid,
    /// Key from the last `changed` event (for `osGetLastChangedEventKey`).
    pub last_changed_event_key: Option<Uuid>,
    /// Sender for inventory commands (give, take).
    pub inventory_command_sender: Option<std::sync::mpsc::Sender<InventoryCommand>>,
    /// Sender for sound commands.
    pub sound_sender: Option<std::sync::mpsc::Sender<SoundCommand>>,
    /// Dialog, URL and text-box requests for the region to show to viewers.
    pub ui_sender: Option<std::sync::mpsc::Sender<dialogs::UiCommand>>,
    /// Account lookups for llRequestAgentData about agents outside the region.
    pub agent_data_sender: Option<std::sync::mpsc::Sender<agent_data::AgentDataRequest>>,
    /// Sender for link commands (create link, break link).
    pub link_command_sender: Option<std::sync::mpsc::Sender<LinkCommand>>,
    /// Sender for object commands (die, message, etc.).
    pub object_command_sender: Option<std::sync::mpsc::Sender<ObjectCommand>>,
    /// Whether this prim is currently worn as an attachment.
    pub is_attachment: bool,
    /// UUIDs of avatars currently sitting on this linkset.
    pub sitting_avatars: Vec<Uuid>,
    /// Link-number and name info for every prim in this linkset.
    pub link_parts: Vec<LinkPartInfo>,
    /// Total prim count for the host object's linkset.
    pub prim_count: i32,
    /// Current windlight/environment settings for LightShare API.
    pub windlight: WindlightData,
    /// Channel for pushing updated windlight data to the region subsystem.
    pub windlight_sender: Option<std::sync::mpsc::Sender<WindlightData>>,
    /// Registry of module callbacks for the modInvoke* API.
    pub mod_invoke_registry: HashMap<String, ModuleCallback>,
    /// Channel for modSendCommand messages to the region module subsystem.
    pub mod_command_sender: Option<std::sync::mpsc::Sender<ModCommand>>,
    /// In-memory JSON document stores for the JsonStore API.
    pub json_stores: HashMap<Uuid, serde_json::Value>,
}

impl Default for ScriptContext {
    fn default() -> Self {
        Self {
            script_id: Uuid::nil(),
            prim_id: Uuid::nil(),
            prim_name: String::new(),
            prim_desc: String::new(),
            owner_id: Uuid::nil(),
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            link_num: 0,
            color: [1.0, 1.0, 1.0],
            alpha: 1.0,
            creator_id: Uuid::nil(),
            texture: Uuid::nil(),
            permissions_granted: 0,
            sit_target_pos: [0.0; 3],
            sit_target_rot: [0.0, 0.0, 0.0, 1.0],
            sitting_avatar: None,
            inventory: Vec::new(),
            allow_inventory_drop: false,
            parcel_name: String::new(),
            parcel_description: String::new(),
            parcel_owner: Uuid::nil(),
            parcel_area: 256,
            parcel_flags: 0,
            event_sender: None,
            chat_sender: None,
            listen_handles: Vec::new(),
            next_listen_handle: 1,
            physics_sender: None,
            velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            acceleration: [0.0; 3],
            position_targets: Vec::new(),
            rotation_targets: Vec::new(),
            next_target_handle: 1,
            notecard_data: HashMap::new(),
            vehicle: vehicle::VehicleParams::default(),
            hover_text: String::new(),
            hover_text_color: [1.0, 1.0, 1.0],
            prim_material: 0,
            is_phantom: false,
            is_physics: false,
            is_temp_on_rez: false,
            fullbright: false,
            glow: 0.0,
            physics_shape_type: 0,
            rez_sender: None,
            attach_sender: None,
            email_queue: VecDeque::new(),
            email_sender: None,
            last_email_time: None,
            cap_url_sender: None,
            active_urls: Vec::new(),
            http_sender: None,
            http_response_sender: None,
            http_pending_count: 0,
            http_content_type: "text/plain".into(),
            region_size: [256, 256],
            water_height: 20.0,
            sun_hour: 6.0,
            sun_fixed: false,
            day_length: 14400.0,
            use_estate_sun: true,
            speed_multiplier: 1.0,
            terrain_heights: None,
            terrain_textures: [Uuid::nil(); 4],
            terrain_texture_heights: [[0.0, 10.0], [10.0, 20.0], [20.0, 30.0], [30.0, 60.0]],
            terrain_dirty: false,
            last_terrain_flush: 0.0,
            wind_params: HashMap::new(),
            float_on_water: false,
            parcel_media_url: String::new(),
            parcel_music_url: String::new(),
            parcel_sip_address: String::new(),
            parcel_dwell: 0.0,
            parcel_prim_count: 0,
            parcel_max_prims: 15000,
            notecard_names: HashMap::new(),
            npc_registry: HashMap::new(),
            npc_sender: None,
            teleport_sender: None,
            dynamic_texture_sender: None,
            timer_sender: None,
            start_time: std::time::Instant::now(),
            sensor_sender: None,
            control_sender: None,
            presence_list: Vec::new(),
            agent_health: HashMap::new(),
            agent_heal_rate: HashMap::new(),
            agent_sender: None,
            animation_sender: None,
            misc_sender: None,
            region_loaded_creation_date: String::new(),
            region_loaded_creation_time: String::new(),
            region_loaded_creation_id: String::new(),
            region_map_texture: Uuid::nil(),
            group_id: Uuid::nil(),
            inertia: None,
            object_animations: Vec::new(),
            grid_home_uri: String::new(),
            grid_nick: String::new(),
            grid_name: String::new(),
            grid_login_uri: String::new(),
            grid_gatekeeper_uri: String::new(),
            grid_custom: HashMap::new(),
            restart_sender: None,
            sim_fps: 0.0,
            physics_fps: 0.0,
            time_dilation: 1.0,
            total_prims: 0,
            active_scripts: 0,
            script_lps: 0.0,
            link_colors: HashMap::new(),
            rezzing_object_id: Uuid::nil(),
            last_changed_event_key: None,
            inventory_command_sender: None,
            sound_sender: None,
            ui_sender: None,
            agent_data_sender: None,
            link_command_sender: None,
            object_command_sender: None,
            is_attachment: false,
            sitting_avatars: Vec::new(),
            link_parts: Vec::new(),
            prim_count: 1,
            windlight: WindlightData::default(),
            windlight_sender: None,
            mod_invoke_registry: HashMap::new(),
            mod_command_sender: None,
            json_stores: HashMap::new(),
        }
    }
}

/// Register **all** built-in `ll*` functions on `lua`.
///
/// Math and string functions are pure and need no context.
/// Communication, physics and object stubs use `ctx` for identity / defaults.
pub fn register_all(lua: &mlua::Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), VmError> {
    // Base constants first so module-specific registrations can refine them.
    constants::register_constants(lua)?;
    math::register_math(lua)?;
    string::register_string(lua)?;
    list::register_list(lua)?;
    communication::register_communication(lua, ctx)?;
    sound::register_sound(lua, ctx)?;
    dialogs::register_dialogs(lua, ctx)?;
    physics::register_physics(lua, ctx)?;
    object::register_object(lua, ctx)?;
    avatar::register_avatar(lua, ctx)?;
    inventory::register_inventory(lua, ctx)?;
    land::register_land(lua, ctx)?;
    vehicle::register_vehicle(lua, ctx)?;
    prim_params::register_prim_params(lua, ctx)?;
    email::register_email(lua, ctx)?;
    cap_url::register_cap_url(lua, ctx)?;
    http::register_http(lua, ctx)?;
    timer::register_timer(lua, ctx)?;
    sensor::register_sensor(lua, ctx)?;
    control::register_control(lua, ctx)?;
    crate::lightshare::register_lightshare(lua, ctx)?;
    crate::mod_api::register_mod_api(lua, ctx)?;
    crate::jsonstore::register_jsonstore(lua, ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a non-sandboxed VM with all builtins registered.
    fn vm_with_builtins() -> (mlua::Lua, Arc<RwLock<ScriptContext>>) {
        let lua = mlua::Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            prim_name: "TestPrim".into(),
            prim_desc: "A test prim".into(),
            owner_id: Uuid::new_v4(),
            position: [128.0, 128.0, 25.0],
            ..Default::default()
        }));
        register_all(&lua, &ctx).expect("register_all should succeed");
        (lua, ctx)
    }

    #[test]
    fn register_all_succeeds() {
        let _pair = vm_with_builtins();
    }

    #[test]
    fn end_to_end_math_and_string() {
        let (lua, _ctx) = vm_with_builtins();

        let result: f64 = lua
            .load("return llAbs(-42)")
            .eval()
            .expect("llAbs should work");
        assert!((result - 42.0).abs() < f64::EPSILON);

        let result: String = lua
            .load(r#"return llToUpper("hello")"#)
            .eval()
            .expect("llToUpper should work");
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn end_to_end_communication_stub() {
        let (lua, _ctx) = vm_with_builtins();

        lua.load(r#"llSay(0, "Hello from test!")"#)
            .exec()
            .expect("llSay should not error");

        lua.load(r#"llWhisper(0, "psst")"#)
            .exec()
            .expect("llWhisper should not error");
    }

    #[test]
    fn end_to_end_object_read() {
        let (lua, _ctx) = vm_with_builtins();

        let name: String = lua
            .load("return llGetObjectName()")
            .eval()
            .expect("llGetObjectName should work");
        assert_eq!(name, "TestPrim");

        let desc: String = lua
            .load("return llGetObjectDesc()")
            .eval()
            .expect("llGetObjectDesc should work");
        assert_eq!(desc, "A test prim");
    }

    #[test]
    fn end_to_end_multi_function_script() {
        let (lua, _ctx) = vm_with_builtins();

        let script = r#"
            local name = llGetObjectName()
            local upper = llToUpper(name)
            local len = llStringLength(upper)
            llSay(0, upper)
            return len
        "#;

        let len: i64 = lua
            .load(script)
            .eval()
            .expect("multi-function script should work");
        assert_eq!(len, 8); // "TESTPRIM"
    }

    #[test]
    fn end_to_end_list_operations() {
        let (lua, _ctx) = vm_with_builtins();

        let script = r#"
            local list = {10, 20, 30}
            local len = llGetListLength(list)
            local val = llList2Integer(list, 1)
            return len + val
        "#;

        let result: i64 = lua.load(script).eval().expect("list ops should work");
        assert_eq!(result, 23); // 3 + 20
    }
}
