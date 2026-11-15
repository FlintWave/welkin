// SPDX-License-Identifier: GPL-3.0-or-later

//! Region simulation data storage (`prims`, `primshapes`, `terrain`, `regionsettings` tables).

use crate::DataError;

// ---------------------------------------------------------------------------
// Row structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PrimRow {
    #[sqlx(rename = "UUID")]
    pub uuid: String,
    #[sqlx(rename = "RegionUUID")]
    pub region_uuid: String,
    #[sqlx(rename = "SceneGroupID")]
    pub scene_group_id: String,
    #[sqlx(rename = "CreationDate")]
    pub creation_date: i32,
    #[sqlx(rename = "Name")]
    pub name: String,
    #[sqlx(rename = "Text")]
    pub text: String,
    #[sqlx(rename = "Description")]
    pub description: String,
    #[sqlx(rename = "SitName")]
    pub sit_name: String,
    #[sqlx(rename = "TouchName")]
    pub touch_name: String,
    #[sqlx(rename = "ObjectFlags")]
    pub object_flags: i32,
    #[sqlx(rename = "CreatorID")]
    pub creator_id: String,
    #[sqlx(rename = "OwnerID")]
    pub owner_id: String,
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "LastOwnerID")]
    pub last_owner_id: String,
    #[sqlx(rename = "RezzerID")]
    pub rezzer_id: String,
    #[sqlx(rename = "OwnerMask")]
    pub owner_mask: i32,
    #[sqlx(rename = "NextOwnerMask")]
    pub next_owner_mask: i32,
    #[sqlx(rename = "GroupMask")]
    pub group_mask: i32,
    #[sqlx(rename = "EveryoneMask")]
    pub everyone_mask: i32,
    #[sqlx(rename = "BaseMask")]
    pub base_mask: i32,
    #[sqlx(rename = "PositionX")]
    pub position_x: f64,
    #[sqlx(rename = "PositionY")]
    pub position_y: f64,
    #[sqlx(rename = "PositionZ")]
    pub position_z: f64,
    #[sqlx(rename = "GroupPositionX")]
    pub group_position_x: f64,
    #[sqlx(rename = "GroupPositionY")]
    pub group_position_y: f64,
    #[sqlx(rename = "GroupPositionZ")]
    pub group_position_z: f64,
    #[sqlx(rename = "VelocityX")]
    pub velocity_x: f64,
    #[sqlx(rename = "VelocityY")]
    pub velocity_y: f64,
    #[sqlx(rename = "VelocityZ")]
    pub velocity_z: f64,
    #[sqlx(rename = "AngularVelocityX")]
    pub angular_velocity_x: f64,
    #[sqlx(rename = "AngularVelocityY")]
    pub angular_velocity_y: f64,
    #[sqlx(rename = "AngularVelocityZ")]
    pub angular_velocity_z: f64,
    #[sqlx(rename = "AccelerationX")]
    pub acceleration_x: f64,
    #[sqlx(rename = "AccelerationY")]
    pub acceleration_y: f64,
    #[sqlx(rename = "AccelerationZ")]
    pub acceleration_z: f64,
    #[sqlx(rename = "RotationX")]
    pub rotation_x: f64,
    #[sqlx(rename = "RotationY")]
    pub rotation_y: f64,
    #[sqlx(rename = "RotationZ")]
    pub rotation_z: f64,
    #[sqlx(rename = "RotationW")]
    pub rotation_w: f64,
    #[sqlx(rename = "SitTargetOffsetX")]
    pub sit_target_offset_x: f64,
    #[sqlx(rename = "SitTargetOffsetY")]
    pub sit_target_offset_y: f64,
    #[sqlx(rename = "SitTargetOffsetZ")]
    pub sit_target_offset_z: f64,
    #[sqlx(rename = "SitTargetOrientW")]
    pub sit_target_orient_w: f64,
    #[sqlx(rename = "SitTargetOrientX")]
    pub sit_target_orient_x: f64,
    #[sqlx(rename = "SitTargetOrientY")]
    pub sit_target_orient_y: f64,
    #[sqlx(rename = "SitTargetOrientZ")]
    pub sit_target_orient_z: f64,
    #[sqlx(rename = "PayPrice")]
    pub pay_price: i32,
    #[sqlx(rename = "PayButton1")]
    pub pay_button1: i32,
    #[sqlx(rename = "PayButton2")]
    pub pay_button2: i32,
    #[sqlx(rename = "PayButton3")]
    pub pay_button3: i32,
    #[sqlx(rename = "PayButton4")]
    pub pay_button4: i32,
    #[sqlx(rename = "LoopedSound")]
    pub looped_sound: String,
    #[sqlx(rename = "LoopedSoundGain")]
    pub looped_sound_gain: f64,
    #[sqlx(rename = "TextureAnimation")]
    pub texture_animation: Vec<u8>,
    #[sqlx(rename = "CameraEyeOffsetX")]
    pub camera_eye_offset_x: f64,
    #[sqlx(rename = "CameraEyeOffsetY")]
    pub camera_eye_offset_y: f64,
    #[sqlx(rename = "CameraEyeOffsetZ")]
    pub camera_eye_offset_z: f64,
    #[sqlx(rename = "CameraAtOffsetX")]
    pub camera_at_offset_x: f64,
    #[sqlx(rename = "CameraAtOffsetY")]
    pub camera_at_offset_y: f64,
    #[sqlx(rename = "CameraAtOffsetZ")]
    pub camera_at_offset_z: f64,
    #[sqlx(rename = "ForceMouselook")]
    pub force_mouselook: i32,
    #[sqlx(rename = "ScriptAccessPin")]
    pub script_access_pin: i32,
    #[sqlx(rename = "AllowedDrop")]
    pub allowed_drop: i32,
    #[sqlx(rename = "DieAtEdge")]
    pub die_at_edge: i32,
    #[sqlx(rename = "SalePrice")]
    pub sale_price: i32,
    #[sqlx(rename = "SaleType")]
    pub sale_type: i32,
    #[sqlx(rename = "ColorR")]
    pub color_r: i32,
    #[sqlx(rename = "ColorG")]
    pub color_g: i32,
    #[sqlx(rename = "ColorB")]
    pub color_b: i32,
    #[sqlx(rename = "ColorA")]
    pub color_a: i32,
    #[sqlx(rename = "ParticleSystem")]
    pub particle_system: Vec<u8>,
    #[sqlx(rename = "ClickAction")]
    pub click_action: i32,
    #[sqlx(rename = "Material")]
    pub material: i32,
    #[sqlx(rename = "CollisionSound")]
    pub collision_sound: String,
    #[sqlx(rename = "CollisionSoundVolume")]
    pub collision_sound_volume: f64,
    #[sqlx(rename = "PassTouches")]
    pub pass_touches: i32,
    #[sqlx(rename = "PassCollisions")]
    pub pass_collisions: i32,
    #[sqlx(rename = "LinkNumber")]
    pub link_number: i32,
    #[sqlx(rename = "MediaURL")]
    pub media_url: String,
    #[sqlx(rename = "PhysicsShapeType")]
    pub physics_shape_type: i32,
    #[sqlx(rename = "Density")]
    pub density: f64,
    #[sqlx(rename = "GravityModifier")]
    pub gravity_modifier: f64,
    #[sqlx(rename = "Friction")]
    pub friction: f64,
    #[sqlx(rename = "Restitution")]
    pub restitution: f64,
    #[sqlx(rename = "DynAttrs")]
    pub dyn_attrs: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PrimShapeRow {
    #[sqlx(rename = "UUID")]
    pub uuid: String,
    #[sqlx(rename = "Shape")]
    pub shape: i32,
    #[sqlx(rename = "ScaleX")]
    pub scale_x: f64,
    #[sqlx(rename = "ScaleY")]
    pub scale_y: f64,
    #[sqlx(rename = "ScaleZ")]
    pub scale_z: f64,
    #[sqlx(rename = "PCode")]
    pub pcode: i32,
    #[sqlx(rename = "PathBegin")]
    pub path_begin: i32,
    #[sqlx(rename = "PathEnd")]
    pub path_end: i32,
    #[sqlx(rename = "PathScaleX")]
    pub path_scale_x: i32,
    #[sqlx(rename = "PathScaleY")]
    pub path_scale_y: i32,
    #[sqlx(rename = "PathShearX")]
    pub path_shear_x: i32,
    #[sqlx(rename = "PathShearY")]
    pub path_shear_y: i32,
    #[sqlx(rename = "PathSkew")]
    pub path_skew: i32,
    #[sqlx(rename = "PathCurve")]
    pub path_curve: i32,
    #[sqlx(rename = "PathRadiusOffset")]
    pub path_radius_offset: i32,
    #[sqlx(rename = "PathRevolutions")]
    pub path_revolutions: i32,
    #[sqlx(rename = "PathTaperX")]
    pub path_taper_x: i32,
    #[sqlx(rename = "PathTaperY")]
    pub path_taper_y: i32,
    #[sqlx(rename = "PathTwist")]
    pub path_twist: i32,
    #[sqlx(rename = "PathTwistBegin")]
    pub path_twist_begin: i32,
    #[sqlx(rename = "ProfileBegin")]
    pub profile_begin: i32,
    #[sqlx(rename = "ProfileEnd")]
    pub profile_end: i32,
    #[sqlx(rename = "ProfileCurve")]
    pub profile_curve: i32,
    #[sqlx(rename = "ProfileHollow")]
    pub profile_hollow: i32,
    #[sqlx(rename = "Texture")]
    pub texture: Vec<u8>,
    #[sqlx(rename = "ExtraParams")]
    pub extra_params: Vec<u8>,
    #[sqlx(rename = "State")]
    pub state: i32,
    #[sqlx(rename = "LastAttachPoint")]
    pub last_attach_point: i32,
    #[sqlx(rename = "Media")]
    pub media: String,
    #[sqlx(rename = "MatOvrd")]
    pub mat_ovrd: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TerrainRow {
    #[sqlx(rename = "RegionUUID")]
    pub region_uuid: String,
    #[sqlx(rename = "Revision")]
    pub revision: i32,
    #[sqlx(rename = "Heightfield")]
    pub heightfield: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RegionSettingsRow {
    #[sqlx(rename = "regionUUID")]
    pub region_uuid: String,
    #[sqlx(rename = "block_terraform")]
    pub block_terraform: i32,
    #[sqlx(rename = "block_fly")]
    pub block_fly: i32,
    #[sqlx(rename = "allow_damage")]
    pub allow_damage: i32,
    #[sqlx(rename = "restrict_pushing")]
    pub restrict_pushing: i32,
    #[sqlx(rename = "allow_land_resell")]
    pub allow_land_resell: i32,
    #[sqlx(rename = "allow_land_join_divide")]
    pub allow_land_join_divide: i32,
    #[sqlx(rename = "block_show_in_search")]
    pub block_show_in_search: i32,
    #[sqlx(rename = "agent_limit")]
    pub agent_limit: i32,
    #[sqlx(rename = "object_bonus")]
    pub object_bonus: f64,
    #[sqlx(rename = "maturity")]
    pub maturity: i32,
    #[sqlx(rename = "disable_scripts")]
    pub disable_scripts: i32,
    #[sqlx(rename = "disable_collisions")]
    pub disable_collisions: i32,
    #[sqlx(rename = "disable_physics")]
    pub disable_physics: i32,
    #[sqlx(rename = "terrain_texture_1")]
    pub terrain_texture_1: String,
    #[sqlx(rename = "terrain_texture_2")]
    pub terrain_texture_2: String,
    #[sqlx(rename = "terrain_texture_3")]
    pub terrain_texture_3: String,
    #[sqlx(rename = "terrain_texture_4")]
    pub terrain_texture_4: String,
    #[sqlx(rename = "elevation_1_nw")]
    pub elevation_1_nw: f64,
    #[sqlx(rename = "elevation_1_ne")]
    pub elevation_1_ne: f64,
    #[sqlx(rename = "elevation_1_se")]
    pub elevation_1_se: f64,
    #[sqlx(rename = "elevation_1_sw")]
    pub elevation_1_sw: f64,
    #[sqlx(rename = "elevation_2_nw")]
    pub elevation_2_nw: f64,
    #[sqlx(rename = "elevation_2_ne")]
    pub elevation_2_ne: f64,
    #[sqlx(rename = "elevation_2_se")]
    pub elevation_2_se: f64,
    #[sqlx(rename = "elevation_2_sw")]
    pub elevation_2_sw: f64,
    #[sqlx(rename = "water_height")]
    pub water_height: f64,
    #[sqlx(rename = "terrain_raise_limit")]
    pub terrain_raise_limit: f64,
    #[sqlx(rename = "terrain_lower_limit")]
    pub terrain_lower_limit: f64,
    #[sqlx(rename = "use_estate_sun")]
    pub use_estate_sun: i32,
    #[sqlx(rename = "fixed_sun")]
    pub fixed_sun: i32,
    #[sqlx(rename = "sun_position")]
    pub sun_position: f64,
    #[sqlx(rename = "covenant")]
    pub covenant: String,
    #[sqlx(rename = "covenant_datetime")]
    pub covenant_datetime: i32,
    #[sqlx(rename = "Sandbox")]
    pub sandbox: i32,
    #[sqlx(rename = "map_tile_id")]
    pub map_tile_id: String,
    #[sqlx(rename = "telehub_object")]
    pub telehub_object: String,
    #[sqlx(rename = "parcel_tile_id")]
    pub parcel_tile_id: String,
    #[sqlx(rename = "cacheID")]
    pub cache_id: String,
    #[sqlx(rename = "block_search")]
    pub block_search: i32,
}

// ---------------------------------------------------------------------------
// Column lists (COALESCE wraps nullable string/blob columns)
// ---------------------------------------------------------------------------

const PRIM_COLS: &str = "\
    UUID, RegionUUID, COALESCE(SceneGroupID,'') AS SceneGroupID, CreationDate, \
    COALESCE(Name,'') AS Name, COALESCE(Text,'') AS Text, \
    COALESCE(Description,'') AS Description, COALESCE(SitName,'') AS SitName, \
    COALESCE(TouchName,'') AS TouchName, ObjectFlags, \
    COALESCE(CreatorID,'') AS CreatorID, COALESCE(OwnerID,'') AS OwnerID, \
    COALESCE(GroupID,'') AS GroupID, COALESCE(LastOwnerID,'') AS LastOwnerID, \
    COALESCE(RezzerID,'') AS RezzerID, \
    OwnerMask, NextOwnerMask, GroupMask, EveryoneMask, BaseMask, \
    PositionX, PositionY, PositionZ, GroupPositionX, GroupPositionY, GroupPositionZ, \
    VelocityX, VelocityY, VelocityZ, \
    AngularVelocityX, AngularVelocityY, AngularVelocityZ, \
    AccelerationX, AccelerationY, AccelerationZ, \
    RotationX, RotationY, RotationZ, RotationW, \
    SitTargetOffsetX, SitTargetOffsetY, SitTargetOffsetZ, \
    SitTargetOrientW, SitTargetOrientX, SitTargetOrientY, SitTargetOrientZ, \
    PayPrice, PayButton1, PayButton2, PayButton3, PayButton4, \
    COALESCE(LoopedSound,'') AS LoopedSound, LoopedSoundGain, \
    COALESCE(TextureAnimation,X'') AS TextureAnimation, \
    CameraEyeOffsetX, CameraEyeOffsetY, CameraEyeOffsetZ, \
    CameraAtOffsetX, CameraAtOffsetY, CameraAtOffsetZ, \
    ForceMouselook, ScriptAccessPin, AllowedDrop, DieAtEdge, SalePrice, SaleType, \
    ColorR, ColorG, ColorB, ColorA, \
    COALESCE(ParticleSystem,X'') AS ParticleSystem, ClickAction, Material, \
    COALESCE(CollisionSound,'') AS CollisionSound, CollisionSoundVolume, \
    PassTouches, PassCollisions, LinkNumber, COALESCE(MediaURL,'') AS MediaURL, \
    PhysicsShapeType, Density, GravityModifier, Friction, Restitution, \
    COALESCE(DynAttrs,'') AS DynAttrs";

const SHAPE_COLS: &str = "\
    UUID, Shape, ScaleX, ScaleY, ScaleZ, PCode, \
    PathBegin, PathEnd, PathScaleX, PathScaleY, PathShearX, PathShearY, \
    PathSkew, PathCurve, PathRadiusOffset, PathRevolutions, \
    PathTaperX, PathTaperY, PathTwist, PathTwistBegin, \
    ProfileBegin, ProfileEnd, ProfileCurve, ProfileHollow, \
    COALESCE(Texture,X'') AS Texture, COALESCE(ExtraParams,X'') AS ExtraParams, \
    State, LastAttachPoint, COALESCE(Media,'') AS Media, \
    COALESCE(MatOvrd,X'') AS MatOvrd";

const REGION_SETTINGS_COLS: &str = "\
    regionUUID, block_terraform, block_fly, allow_damage, restrict_pushing, \
    allow_land_resell, allow_land_join_divide, block_show_in_search, \
    agent_limit, object_bonus, maturity, disable_scripts, disable_collisions, disable_physics, \
    COALESCE(terrain_texture_1,'') AS terrain_texture_1, \
    COALESCE(terrain_texture_2,'') AS terrain_texture_2, \
    COALESCE(terrain_texture_3,'') AS terrain_texture_3, \
    COALESCE(terrain_texture_4,'') AS terrain_texture_4, \
    elevation_1_nw, elevation_1_ne, elevation_1_se, elevation_1_sw, \
    elevation_2_nw, elevation_2_ne, elevation_2_se, elevation_2_sw, \
    water_height, terrain_raise_limit, terrain_lower_limit, \
    use_estate_sun, fixed_sun, sun_position, \
    COALESCE(covenant,'') AS covenant, covenant_datetime, Sandbox, \
    COALESCE(map_tile_id,'') AS map_tile_id, \
    COALESCE(telehub_object,'') AS telehub_object, \
    COALESCE(parcel_tile_id,'') AS parcel_tile_id, \
    COALESCE(cacheID,'') AS cacheID, block_search";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

pub struct SimulationDataStore {
    pool: sqlx::AnyPool,
}

impl SimulationDataStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    // -- objects -------------------------------------------------------------

    /// Store a prim and its shape in a single transaction (DELETE+INSERT upsert).
    pub async fn store_object(
        &self,
        region_id: &str,
        group_id: &str,
        prim: &PrimRow,
        shape: &PrimShapeRow,
    ) -> Result<(), DataError> {
        let _ = (region_id, group_id); // row carries both; params kept for API symmetry
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM primshapes WHERE UUID = $1")
            .bind(&prim.uuid)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM prims WHERE UUID = $1")
            .bind(&prim.uuid)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO prims (\
                UUID,RegionUUID,SceneGroupID,CreationDate,Name,Text,Description,\
                SitName,TouchName,ObjectFlags,CreatorID,OwnerID,GroupID,LastOwnerID,\
                RezzerID,OwnerMask,NextOwnerMask,GroupMask,EveryoneMask,BaseMask,\
                PositionX,PositionY,PositionZ,\
                GroupPositionX,GroupPositionY,GroupPositionZ,\
                VelocityX,VelocityY,VelocityZ,\
                AngularVelocityX,AngularVelocityY,AngularVelocityZ,\
                AccelerationX,AccelerationY,AccelerationZ,\
                RotationX,RotationY,RotationZ,RotationW,\
                SitTargetOffsetX,SitTargetOffsetY,SitTargetOffsetZ,\
                SitTargetOrientW,SitTargetOrientX,SitTargetOrientY,SitTargetOrientZ,\
                PayPrice,PayButton1,PayButton2,PayButton3,PayButton4,\
                LoopedSound,LoopedSoundGain,TextureAnimation,\
                CameraEyeOffsetX,CameraEyeOffsetY,CameraEyeOffsetZ,\
                CameraAtOffsetX,CameraAtOffsetY,CameraAtOffsetZ,\
                ForceMouselook,ScriptAccessPin,AllowedDrop,DieAtEdge,SalePrice,SaleType,\
                ColorR,ColorG,ColorB,ColorA,\
                ParticleSystem,ClickAction,Material,\
                CollisionSound,CollisionSoundVolume,PassTouches,PassCollisions,\
                LinkNumber,MediaURL,PhysicsShapeType,Density,GravityModifier,\
                Friction,Restitution,DynAttrs\
            ) VALUES (\
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,\
                $21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37,$38,\
                $39,$40,$41,$42,$43,$44,$45,$46,$47,$48,$49,$50,$51,$52,$53,$54,$55,$56,\
                $57,$58,$59,$60,$61,$62,$63,$64,$65,$66,$67,$68,$69,$70,$71,$72,$73,$74,\
                $75,$76,$77,$78,$79,$80,$81,$82,$83,$84,$85\
            )",
        )
        .bind(&prim.uuid)
        .bind(&prim.region_uuid)
        .bind(&prim.scene_group_id)
        .bind(prim.creation_date)
        .bind(&prim.name)
        .bind(&prim.text)
        .bind(&prim.description)
        .bind(&prim.sit_name)
        .bind(&prim.touch_name)
        .bind(prim.object_flags)
        .bind(&prim.creator_id)
        .bind(&prim.owner_id)
        .bind(&prim.group_id)
        .bind(&prim.last_owner_id)
        .bind(&prim.rezzer_id)
        .bind(prim.owner_mask)
        .bind(prim.next_owner_mask)
        .bind(prim.group_mask)
        .bind(prim.everyone_mask)
        .bind(prim.base_mask)
        .bind(prim.position_x)
        .bind(prim.position_y)
        .bind(prim.position_z)
        .bind(prim.group_position_x)
        .bind(prim.group_position_y)
        .bind(prim.group_position_z)
        .bind(prim.velocity_x)
        .bind(prim.velocity_y)
        .bind(prim.velocity_z)
        .bind(prim.angular_velocity_x)
        .bind(prim.angular_velocity_y)
        .bind(prim.angular_velocity_z)
        .bind(prim.acceleration_x)
        .bind(prim.acceleration_y)
        .bind(prim.acceleration_z)
        .bind(prim.rotation_x)
        .bind(prim.rotation_y)
        .bind(prim.rotation_z)
        .bind(prim.rotation_w)
        .bind(prim.sit_target_offset_x)
        .bind(prim.sit_target_offset_y)
        .bind(prim.sit_target_offset_z)
        .bind(prim.sit_target_orient_w)
        .bind(prim.sit_target_orient_x)
        .bind(prim.sit_target_orient_y)
        .bind(prim.sit_target_orient_z)
        .bind(prim.pay_price)
        .bind(prim.pay_button1)
        .bind(prim.pay_button2)
        .bind(prim.pay_button3)
        .bind(prim.pay_button4)
        .bind(&prim.looped_sound)
        .bind(prim.looped_sound_gain)
        .bind(&prim.texture_animation)
        .bind(prim.camera_eye_offset_x)
        .bind(prim.camera_eye_offset_y)
        .bind(prim.camera_eye_offset_z)
        .bind(prim.camera_at_offset_x)
        .bind(prim.camera_at_offset_y)
        .bind(prim.camera_at_offset_z)
        .bind(prim.force_mouselook)
        .bind(prim.script_access_pin)
        .bind(prim.allowed_drop)
        .bind(prim.die_at_edge)
        .bind(prim.sale_price)
        .bind(prim.sale_type)
        .bind(prim.color_r)
        .bind(prim.color_g)
        .bind(prim.color_b)
        .bind(prim.color_a)
        .bind(&prim.particle_system)
        .bind(prim.click_action)
        .bind(prim.material)
        .bind(&prim.collision_sound)
        .bind(prim.collision_sound_volume)
        .bind(prim.pass_touches)
        .bind(prim.pass_collisions)
        .bind(prim.link_number)
        .bind(&prim.media_url)
        .bind(prim.physics_shape_type)
        .bind(prim.density)
        .bind(prim.gravity_modifier)
        .bind(prim.friction)
        .bind(prim.restitution)
        .bind(&prim.dyn_attrs)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO primshapes (\
                UUID,Shape,ScaleX,ScaleY,ScaleZ,PCode,\
                PathBegin,PathEnd,PathScaleX,PathScaleY,PathShearX,PathShearY,\
                PathSkew,PathCurve,PathRadiusOffset,PathRevolutions,\
                PathTaperX,PathTaperY,PathTwist,PathTwistBegin,\
                ProfileBegin,ProfileEnd,ProfileCurve,ProfileHollow,\
                Texture,ExtraParams,State,LastAttachPoint,Media,MatOvrd\
            ) VALUES (\
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,\
                $16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30\
            )",
        )
        .bind(&shape.uuid)
        .bind(shape.shape)
        .bind(shape.scale_x)
        .bind(shape.scale_y)
        .bind(shape.scale_z)
        .bind(shape.pcode)
        .bind(shape.path_begin)
        .bind(shape.path_end)
        .bind(shape.path_scale_x)
        .bind(shape.path_scale_y)
        .bind(shape.path_shear_x)
        .bind(shape.path_shear_y)
        .bind(shape.path_skew)
        .bind(shape.path_curve)
        .bind(shape.path_radius_offset)
        .bind(shape.path_revolutions)
        .bind(shape.path_taper_x)
        .bind(shape.path_taper_y)
        .bind(shape.path_twist)
        .bind(shape.path_twist_begin)
        .bind(shape.profile_begin)
        .bind(shape.profile_end)
        .bind(shape.profile_curve)
        .bind(shape.profile_hollow)
        .bind(&shape.texture)
        .bind(&shape.extra_params)
        .bind(shape.state)
        .bind(shape.last_attach_point)
        .bind(&shape.media)
        .bind(&shape.mat_ovrd)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Remove a prim and its shape from both tables.
    pub async fn remove_object(&self, uuid: &str) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM primshapes WHERE UUID = $1")
            .bind(uuid)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM prims WHERE UUID = $1")
            .bind(uuid)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Load all prims for a region, paired with their shapes.
    pub async fn load_objects(
        &self,
        region_id: &str,
    ) -> Result<Vec<(PrimRow, Option<PrimShapeRow>)>, DataError> {
        let prim_sql = format!(
            "SELECT {PRIM_COLS} FROM prims WHERE RegionUUID = $1 ORDER BY SceneGroupID,LinkNumber"
        );
        let prims: Vec<PrimRow> = sqlx::query_as::<_, PrimRow>(&prim_sql)
            .bind(region_id)
            .fetch_all(&self.pool)
            .await?;

        let shape_sql = format!(
            "SELECT {SHAPE_COLS} FROM primshapes \
             WHERE UUID IN (SELECT UUID FROM prims WHERE RegionUUID = $1)"
        );
        let shapes: Vec<PrimShapeRow> = sqlx::query_as::<_, PrimShapeRow>(&shape_sql)
            .bind(region_id)
            .fetch_all(&self.pool)
            .await?;

        let mut map: std::collections::HashMap<String, PrimShapeRow> =
            shapes.into_iter().map(|s| (s.uuid.clone(), s)).collect();
        Ok(prims
            .into_iter()
            .map(|p| {
                let s = map.remove(&p.uuid);
                (p, s)
            })
            .collect())
    }

    /// Return just the UUIDs of all prims in a region.
    pub async fn get_object_ids(&self, region_id: &str) -> Result<Vec<String>, DataError> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT UUID FROM prims WHERE RegionUUID = $1")
            .bind(region_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    // -- terrain -------------------------------------------------------------

    /// Store a terrain heightfield (DELETE old + INSERT new).
    pub async fn store_terrain(
        &self,
        region_id: &str,
        revision: i32,
        heightfield: &[u8],
    ) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM terrain WHERE RegionUUID = $1")
            .bind(region_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO terrain (RegionUUID,Revision,Heightfield) VALUES ($1,$2,$3)")
            .bind(region_id)
            .bind(revision)
            .bind(heightfield)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Load the latest terrain revision for a region.
    pub async fn load_terrain(&self, region_id: &str) -> Result<Option<TerrainRow>, DataError> {
        Ok(sqlx::query_as::<_, TerrainRow>(
            "SELECT RegionUUID,Revision,Heightfield FROM terrain \
             WHERE RegionUUID = $1 ORDER BY Revision DESC LIMIT 1",
        )
        .bind(region_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    // -- region settings -----------------------------------------------------

    /// Store region settings (DELETE+INSERT upsert).
    pub async fn store_region_settings(&self, s: &RegionSettingsRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM regionsettings WHERE regionUUID = $1")
            .bind(&s.region_uuid)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO regionsettings (\
                regionUUID,block_terraform,block_fly,allow_damage,restrict_pushing,\
                allow_land_resell,allow_land_join_divide,block_show_in_search,\
                agent_limit,object_bonus,maturity,disable_scripts,disable_collisions,\
                disable_physics,terrain_texture_1,terrain_texture_2,terrain_texture_3,\
                terrain_texture_4,elevation_1_nw,elevation_1_ne,elevation_1_se,elevation_1_sw,\
                elevation_2_nw,elevation_2_ne,elevation_2_se,elevation_2_sw,\
                water_height,terrain_raise_limit,terrain_lower_limit,\
                use_estate_sun,fixed_sun,sun_position,\
                covenant,covenant_datetime,Sandbox,\
                map_tile_id,telehub_object,parcel_tile_id,cacheID,block_search\
            ) VALUES (\
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,\
                $21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37,$38,$39,$40\
            )",
        )
        .bind(&s.region_uuid)
        .bind(s.block_terraform)
        .bind(s.block_fly)
        .bind(s.allow_damage)
        .bind(s.restrict_pushing)
        .bind(s.allow_land_resell)
        .bind(s.allow_land_join_divide)
        .bind(s.block_show_in_search)
        .bind(s.agent_limit)
        .bind(s.object_bonus)
        .bind(s.maturity)
        .bind(s.disable_scripts)
        .bind(s.disable_collisions)
        .bind(s.disable_physics)
        .bind(&s.terrain_texture_1)
        .bind(&s.terrain_texture_2)
        .bind(&s.terrain_texture_3)
        .bind(&s.terrain_texture_4)
        .bind(s.elevation_1_nw)
        .bind(s.elevation_1_ne)
        .bind(s.elevation_1_se)
        .bind(s.elevation_1_sw)
        .bind(s.elevation_2_nw)
        .bind(s.elevation_2_ne)
        .bind(s.elevation_2_se)
        .bind(s.elevation_2_sw)
        .bind(s.water_height)
        .bind(s.terrain_raise_limit)
        .bind(s.terrain_lower_limit)
        .bind(s.use_estate_sun)
        .bind(s.fixed_sun)
        .bind(s.sun_position)
        .bind(&s.covenant)
        .bind(s.covenant_datetime)
        .bind(s.sandbox)
        .bind(&s.map_tile_id)
        .bind(&s.telehub_object)
        .bind(&s.parcel_tile_id)
        .bind(&s.cache_id)
        .bind(s.block_search)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Load region settings by region UUID.
    pub async fn load_region_settings(
        &self,
        region_id: &str,
    ) -> Result<Option<RegionSettingsRow>, DataError> {
        let sql =
            format!("SELECT {REGION_SETTINGS_COLS} FROM regionsettings WHERE regionUUID = $1");
        Ok(sqlx::query_as::<_, RegionSettingsRow>(&sql)
            .bind(region_id)
            .fetch_optional(&self.pool)
            .await?)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const Z: &str = "00000000-0000-0000-0000-000000000000";

    fn prim() -> PrimRow {
        PrimRow {
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            region_uuid: "00000000-0000-0000-0000-000000000099".into(),
            scene_group_id: "00000000-0000-0000-0000-000000000001".into(),
            creation_date: 1_700_000_000,
            name: "Box".into(),
            text: String::new(),
            description: String::new(),
            sit_name: String::new(),
            touch_name: String::new(),
            object_flags: 0,
            creator_id: Z.into(),
            owner_id: Z.into(),
            group_id: Z.into(),
            last_owner_id: Z.into(),
            rezzer_id: Z.into(),
            owner_mask: 0x7FFF_FFFF,
            next_owner_mask: 0x7FFF_FFFF,
            group_mask: 0,
            everyone_mask: 0,
            base_mask: 0x7FFF_FFFF,
            position_x: 128.0,
            position_y: 128.0,
            position_z: 25.0,
            group_position_x: 128.0,
            group_position_y: 128.0,
            group_position_z: 25.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            angular_velocity_x: 0.0,
            angular_velocity_y: 0.0,
            angular_velocity_z: 0.0,
            acceleration_x: 0.0,
            acceleration_y: 0.0,
            acceleration_z: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            rotation_w: 1.0,
            sit_target_offset_x: 0.0,
            sit_target_offset_y: 0.0,
            sit_target_offset_z: 0.0,
            sit_target_orient_w: 1.0,
            sit_target_orient_x: 0.0,
            sit_target_orient_y: 0.0,
            sit_target_orient_z: 0.0,
            pay_price: -2,
            pay_button1: -2,
            pay_button2: -2,
            pay_button3: -2,
            pay_button4: -2,
            looped_sound: Z.into(),
            looped_sound_gain: 0.0,
            texture_animation: Vec::new(),
            camera_eye_offset_x: 0.0,
            camera_eye_offset_y: 0.0,
            camera_eye_offset_z: 0.0,
            camera_at_offset_x: 0.0,
            camera_at_offset_y: 0.0,
            camera_at_offset_z: 0.0,
            force_mouselook: 0,
            script_access_pin: 0,
            allowed_drop: 0,
            die_at_edge: 0,
            sale_price: 10,
            sale_type: 0,
            color_r: 0,
            color_g: 0,
            color_b: 0,
            color_a: 0,
            particle_system: Vec::new(),
            click_action: 0,
            material: 3,
            collision_sound: Z.into(),
            collision_sound_volume: 0.0,
            pass_touches: 0,
            pass_collisions: 0,
            link_number: 1,
            media_url: String::new(),
            physics_shape_type: 0,
            density: 1000.0,
            gravity_modifier: 1.0,
            friction: 0.6,
            restitution: 0.5,
            dyn_attrs: String::new(),
        }
    }

    fn shape() -> PrimShapeRow {
        PrimShapeRow {
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            shape: 0,
            scale_x: 0.5,
            scale_y: 0.5,
            scale_z: 0.5,
            pcode: 9,
            path_begin: 0,
            path_end: 0,
            path_scale_x: 100,
            path_scale_y: 100,
            path_shear_x: 0,
            path_shear_y: 0,
            path_skew: 0,
            path_curve: 16,
            path_radius_offset: 0,
            path_revolutions: 0,
            path_taper_x: 0,
            path_taper_y: 0,
            path_twist: 0,
            path_twist_begin: 0,
            profile_begin: 0,
            profile_end: 0,
            profile_curve: 1,
            profile_hollow: 0,
            texture: Vec::new(),
            extra_params: Vec::new(),
            state: 0,
            last_attach_point: 0,
            media: String::new(),
            mat_ovrd: Vec::new(),
        }
    }

    fn settings() -> RegionSettingsRow {
        RegionSettingsRow {
            region_uuid: "00000000-0000-0000-0000-000000000099".into(),
            block_terraform: 0,
            block_fly: 0,
            allow_damage: 0,
            restrict_pushing: 0,
            allow_land_resell: 1,
            allow_land_join_divide: 1,
            block_show_in_search: 0,
            agent_limit: 40,
            object_bonus: 1.0,
            maturity: 1,
            disable_scripts: 0,
            disable_collisions: 0,
            disable_physics: 0,
            terrain_texture_1: Z.into(),
            terrain_texture_2: Z.into(),
            terrain_texture_3: Z.into(),
            terrain_texture_4: Z.into(),
            elevation_1_nw: 10.0,
            elevation_1_ne: 10.0,
            elevation_1_se: 10.0,
            elevation_1_sw: 10.0,
            elevation_2_nw: 60.0,
            elevation_2_ne: 60.0,
            elevation_2_se: 60.0,
            elevation_2_sw: 60.0,
            water_height: 20.0,
            terrain_raise_limit: 100.0,
            terrain_lower_limit: -100.0,
            use_estate_sun: 1,
            fixed_sun: 0,
            sun_position: 0.0,
            covenant: Z.into(),
            covenant_datetime: 0,
            sandbox: 0,
            map_tile_id: Z.into(),
            telehub_object: Z.into(),
            parcel_tile_id: Z.into(),
            cache_id: Z.into(),
            block_search: 0,
        }
    }

    #[test]
    fn prim_row_defaults() {
        let p = prim();
        assert_eq!(p.name, "Box");
        assert_eq!(p.position_x, 128.0);
        assert_eq!(p.rotation_w, 1.0);
        assert_eq!(p.link_number, 1);
    }

    #[test]
    fn shape_row_defaults() {
        let s = shape();
        assert_eq!(s.pcode, 9);
        assert_eq!(s.scale_x, 0.5);
        assert_eq!(s.path_curve, 16);
        assert_eq!(s.profile_curve, 1);
    }

    #[test]
    fn region_settings_defaults() {
        let rs = settings();
        assert_eq!(rs.agent_limit, 40);
        assert_eq!(rs.water_height, 20.0);
        assert_eq!(rs.allow_land_resell, 1);
    }

    #[test]
    fn terrain_row_construct() {
        let t = TerrainRow {
            region_uuid: "00000000-0000-0000-0000-000000000099".into(),
            revision: 1,
            heightfield: vec![0u8; 256 * 256 * 4],
        };
        assert_eq!(t.revision, 1);
        assert_eq!(t.heightfield.len(), 256 * 256 * 4);
    }

    #[test]
    fn store_is_send() {
        fn _assert_send<T: Send>() {}
        _assert_send::<SimulationDataStore>();
    }

    #[test]
    fn clone_independence() {
        let p = prim();
        let mut p2 = p.clone();
        p2.name = "Modified".into();
        assert_ne!(p.name, p2.name);
    }
}
