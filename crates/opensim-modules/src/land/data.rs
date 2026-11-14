// SPDX-License-Identifier: GPL-3.0-or-later

//! Land data structures mirroring the C# `LandData`, `LandAccessEntry`, and
//! related enums from OpenSimulator 0.9.3.

use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Parcel category for classified listings.
///
/// Mirrors `OpenMetaverse.ParcelCategory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i8)]
pub enum ParcelCategory {
    #[default]
    None = 0,
    Linden = 1,
    Adult = 2,
    Arts = 3,
    Business = 4,
    Educational = 5,
    Gaming = 6,
    Hangout = 7,
    Newcomer = 8,
    Park = 9,
    Residential = 10,
    Shopping = 11,
    Stage = 12,
    Other = 13,
    Any = -1,
}

/// Parcel ownership status.
///
/// Mirrors `OpenMetaverse.ParcelStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i8)]
pub enum ParcelStatus {
    #[default]
    None = -1,
    Leased = 0,
    Abandoned = 1,
}

/// Access-list entry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessList {
    Access = 1,
    Ban = 2,
}

// ---------------------------------------------------------------------------
// ParcelFlags (bitflags)
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    /// Parcel permission / feature flags.
    ///
    /// Values from `OpenMetaverse.ParcelFlags`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ParcelFlags: u32 {
        const ALLOW_FLY               = 1 << 0;
        const ALLOW_OTHER_SCRIPTS     = 1 << 1;
        const FOR_SALE                = 1 << 2;
        const ALLOW_LANDMARK          = 1 << 3;
        const ALLOW_TERRAFORM         = 1 << 4;
        const ALLOW_DAMAGE            = 1 << 5;
        const CREATE_OBJECTS          = 1 << 6;
        const FOR_SALE_OBJECTS        = 1 << 7;
        const USE_ACCESS_GROUP        = 1 << 8;
        const USE_ACCESS_LIST         = 1 << 9;
        const USE_BAN_LIST            = 1 << 10;
        const USE_LAND_PASS_LIST      = 1 << 11;
        const SHOW_DIRECTORY          = 1 << 12;
        const ALLOW_DEED_TO_GROUP     = 1 << 13;
        const CONTRIBUTE_WITH_DEED    = 1 << 14;
        const SOUND_LOCAL             = 1 << 15;
        const SELL_PARCEL_OBJECTS     = 1 << 16;
        const ALLOW_PUBLISH           = 1 << 17;
        const MATURE_PUBLISH          = 1 << 18;
        const URL_WEB_PAGE            = 1 << 19;
        const URL_RAW_HTML            = 1 << 20;
        const RESTRICT_PUSHOBJECT     = 1 << 21;
        const DENY_ANONYMOUS          = 1 << 22;
        // 1 << 23 unused
        const ALLOW_GROUP_SCRIPTS     = 1 << 24;
        const CREATE_GROUP_OBJECTS    = 1 << 25;
        const ALLOW_ALL_OBJECT_ENTRY  = 1 << 26;
        const ALLOW_GROUP_OBJECT_ENTRY = 1 << 27;
        const ALLOW_VOICE_CHAT        = 1 << 28;
        const USE_ESTATE_VOICE_CHAN   = 1 << 29;
        const DENY_AGE_UNVERIFIED    = 1 << 30;
        const ALLOW_ENVIRONMENT_OVERRIDE = 1 << 31;

        // Convenience aliases matching C# default flags.
        const ALLOW_A_PRIMITIVE_ENTRY = Self::ALLOW_ALL_OBJECT_ENTRY.bits();
    }
}

impl Default for ParcelFlags {
    /// Default flags matching the C# `LandData._flags` initialiser.
    fn default() -> Self {
        Self::ALLOW_FLY
            | Self::ALLOW_LANDMARK
            | Self::ALLOW_A_PRIMITIVE_ENTRY
            | Self::ALLOW_DEED_TO_GROUP
            | Self::CREATE_OBJECTS
            | Self::ALLOW_OTHER_SCRIPTS
            | Self::ALLOW_VOICE_CHAT
    }
}

// ---------------------------------------------------------------------------
// LandAccessEntry
// ---------------------------------------------------------------------------

/// A single access-list row (allow or ban) for an agent on a parcel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandAccessEntry {
    pub agent_id: Uuid,
    /// Unix timestamp; 0 = permanent.
    pub expires: i64,
    pub flags: AccessList,
}

// ---------------------------------------------------------------------------
// LandDataUpdate — viewer property change payload
// ---------------------------------------------------------------------------

/// Subset of [`LandData`] fields that the viewer can change via
/// `ParcelPropertiesUpdate`.
#[derive(Debug, Clone, Default)]
pub struct LandDataUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub flags: Option<u32>,
    pub sale_price: Option<i32>,
    pub category: Option<ParcelCategory>,
    pub landing_type: Option<u8>,
    pub user_location: Option<[f32; 3]>,
    pub user_look_at: Option<[f32; 3]>,
    pub snapshot_id: Option<Uuid>,
    pub music_url: Option<String>,
    pub media_url: Option<String>,
    pub media_id: Option<Uuid>,
    pub media_auto_scale: Option<u8>,
    pub media_type: Option<String>,
    pub media_description: Option<String>,
    pub media_width: Option<i32>,
    pub media_height: Option<i32>,
    pub media_loop: Option<bool>,
    pub pass_hours: Option<f32>,
    pub pass_price: Option<i32>,
    pub group_id: Option<Uuid>,
    pub other_clean_time: Option<i32>,
    pub see_avs: Option<bool>,
    pub any_av_sounds: Option<bool>,
    pub group_av_sounds: Option<bool>,
}

// ---------------------------------------------------------------------------
// LandData
// ---------------------------------------------------------------------------

/// All persistent data for a single land parcel.
///
/// Mirrors the C# `OpenSim.Framework.LandData` class.
#[derive(Debug, Clone)]
pub struct LandData {
    pub global_id: Uuid,
    pub local_id: i32,
    pub name: String,
    pub description: String,

    pub owner_id: Uuid,
    pub group_id: Uuid,
    pub is_group_owned: bool,

    pub area: i32,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],

    pub claim_date: i64,
    pub claim_price: i32,
    pub sale_price: i32,

    pub auction_id: u32,
    pub auth_buyer_id: Uuid,

    pub category: ParcelCategory,
    pub status: ParcelStatus,
    pub flags: u32,

    pub landing_type: u8,
    pub user_location: [f32; 3],
    pub user_look_at: [f32; 3],

    pub snapshot_id: Uuid,

    // Media
    pub media_id: Uuid,
    pub media_url: String,
    pub media_type: String,
    pub media_description: String,
    pub media_width: i32,
    pub media_height: i32,
    pub media_loop: bool,
    pub media_auto_scale: u8,

    pub music_url: String,

    pub pass_hours: f32,
    pub pass_price: i32,

    pub other_clean_time: i32,

    pub dwell: f32,

    pub see_avs: bool,
    pub any_av_sounds: bool,
    pub group_av_sounds: bool,

    /// Serialised parcel bitmap (one bit per 4 m x 4 m cell, row-major).
    pub bitmap: Vec<u8>,

    /// Per-parcel access / ban list.
    pub access_list: Vec<LandAccessEntry>,
}

impl Default for LandData {
    fn default() -> Self {
        Self {
            global_id: Uuid::new_v4(),
            local_id: 0,
            name: "Your Parcel".to_string(),
            description: String::new(),

            owner_id: Uuid::nil(),
            group_id: Uuid::nil(),
            is_group_owned: false,

            area: 0,
            aabb_min: [0.0; 3],
            aabb_max: [0.0; 3],

            claim_date: 0,
            claim_price: 0,
            sale_price: 0,

            auction_id: 0,
            auth_buyer_id: Uuid::nil(),

            category: ParcelCategory::None,
            status: ParcelStatus::Leased,
            flags: ParcelFlags::default().bits(),

            landing_type: 1, // Direct
            user_location: [0.0; 3],
            user_look_at: [0.0; 3],

            snapshot_id: Uuid::nil(),

            media_id: Uuid::nil(),
            media_url: String::new(),
            media_type: "none/none".to_string(),
            media_description: String::new(),
            media_width: 0,
            media_height: 0,
            media_loop: false,
            media_auto_scale: 0,

            music_url: String::new(),

            pass_hours: 0.0,
            pass_price: 0,

            other_clean_time: 0,

            dwell: 0.0,

            see_avs: true,
            any_av_sounds: true,
            group_av_sounds: true,

            bitmap: vec![0u8; 512],
            access_list: Vec::new(),
        }
    }
}

impl LandData {
    /// Apply a viewer-originated property update, touching only the fields
    /// that are `Some`.
    pub fn apply_update(&mut self, update: &LandDataUpdate) {
        if let Some(ref name) = update.name {
            self.name = name.clone();
        }
        if let Some(ref desc) = update.description {
            self.description = desc.clone();
        }
        if let Some(flags) = update.flags {
            self.flags = flags;
        }
        if let Some(price) = update.sale_price {
            self.sale_price = price;
        }
        if let Some(cat) = update.category {
            self.category = cat;
        }
        if let Some(lt) = update.landing_type {
            self.landing_type = lt;
        }
        if let Some(loc) = update.user_location {
            self.user_location = loc;
        }
        if let Some(look) = update.user_look_at {
            self.user_look_at = look;
        }
        if let Some(snap) = update.snapshot_id {
            self.snapshot_id = snap;
        }
        if let Some(ref url) = update.music_url {
            self.music_url = url.clone();
        }
        if let Some(ref url) = update.media_url {
            self.media_url = url.clone();
        }
        if let Some(mid) = update.media_id {
            self.media_id = mid;
        }
        if let Some(mas) = update.media_auto_scale {
            self.media_auto_scale = mas;
        }
        if let Some(ref mt) = update.media_type {
            self.media_type = mt.clone();
        }
        if let Some(ref md) = update.media_description {
            self.media_description = md.clone();
        }
        if let Some(mw) = update.media_width {
            self.media_width = mw;
        }
        if let Some(mh) = update.media_height {
            self.media_height = mh;
        }
        if let Some(ml) = update.media_loop {
            self.media_loop = ml;
        }
        if let Some(ph) = update.pass_hours {
            self.pass_hours = ph;
        }
        if let Some(pp) = update.pass_price {
            self.pass_price = pp;
        }
        if let Some(gid) = update.group_id {
            self.group_id = gid;
        }
        if let Some(oct) = update.other_clean_time {
            self.other_clean_time = oct;
        }
        if let Some(sa) = update.see_avs {
            self.see_avs = sa;
        }
        if let Some(aas) = update.any_av_sounds {
            self.any_av_sounds = aas;
        }
        if let Some(gas) = update.group_av_sounds {
            self.group_av_sounds = gas;
        }
    }
}
