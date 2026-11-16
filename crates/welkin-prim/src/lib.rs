// SPDX-License-Identifier: LGPL-3.0-or-later
//! Server-side prim API.
//!
//! Thin wrapper over `aurora-prim`: volume parameters, texture entries, extra
//! parameters and procedural geometry. All upstream items used by the server
//! are re-exported from here so that a change in the Aurora API touches only
//! this crate.

pub use aurora_prim::params::{
    CUT_QUANTA, HOLLOW_QUANTA, LL_PCODE_HOLE_CIRCLE, LL_PCODE_HOLE_MASK, LL_PCODE_HOLE_SAME,
    LL_PCODE_HOLE_SQUARE, LL_PCODE_HOLE_TRIANGLE, LL_PCODE_LEGACY_AVATAR, LL_PCODE_LEGACY_GRASS,
    LL_PCODE_LEGACY_PART_SYS, LL_PCODE_LEGACY_TREE, LL_PCODE_PATH_CIRCLE, LL_PCODE_PATH_CIRCLE_33,
    LL_PCODE_PATH_CIRCLE2, LL_PCODE_PATH_FLEXIBLE, LL_PCODE_PATH_LINE, LL_PCODE_PATH_TEST,
    LL_PCODE_PROFILE_CIRCLE, LL_PCODE_PROFILE_CIRCLE_HALF, LL_PCODE_PROFILE_EQUALTRI,
    LL_PCODE_PROFILE_ISOTRI, LL_PCODE_PROFILE_MASK, LL_PCODE_PROFILE_RIGHTTRI,
    LL_PCODE_PROFILE_SQUARE, LL_PCODE_TREE_NEW, LL_PCODE_VOLUME, LL_SCULPT_FLAG_INVERT,
    LL_SCULPT_FLAG_MIRROR, LL_SCULPT_TYPE_CYLINDER, LL_SCULPT_TYPE_MASK, LL_SCULPT_TYPE_MESH,
    LL_SCULPT_TYPE_NONE, LL_SCULPT_TYPE_PLANE, LL_SCULPT_TYPE_SPHERE, LL_SCULPT_TYPE_TORUS,
    PathParams, ProfileParams, REV_QUANTA, RawShape, SCALE_QUANTA, SHEAR_QUANTA, SculptParams,
    TAPER_QUANTA, VolumeParams,
};

pub use aurora_prim::te::{
    BLANK_TEXTURE, DEFAULT_TEXTURE, MAX_TES, TRANSPARENT_TEXTURE, TextureEntry, TextureFace,
    WATER_EXCLUSION_TEXTURE, pack_texture_entry, parse_texture_entry,
};

pub use aurora_prim::extra::{
    EXTENDED_MESH_ANIMATED, ExtraParams, FlexibleParams, LightImageParams, LightParams,
    PARAMS_EXTENDED_MESH, PARAMS_FLEXIBLE, PARAMS_LIGHT, PARAMS_LIGHT_IMAGE, PARAMS_MESH,
    PARAMS_REFLECTION_PROBE, PARAMS_RENDER_MATERIAL, PARAMS_SCULPT, ReflectionProbeParams,
    pack_extended_mesh, pack_flexible, pack_light, pack_light_image, pack_reflection_probe,
    pack_sculpt, parse_extra_params,
};

pub mod mesh_asset;
pub mod meshing;

pub use aurora_prim::sculpt::{generate_sculpt, sculpt_placeholder};
pub use aurora_prim::volume::{
    VolumeFace, VolumeMesh, generate_volume, lod_triangle_counts, num_faces,
};
