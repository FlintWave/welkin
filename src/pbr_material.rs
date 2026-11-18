// SPDX-License-Identifier: MIT OR Apache-2.0

//! Physically Based Rendering (PBR) material definition.
//!
//! This module defines the core `PbrMaterial` struct used throughout Welkin to
//! describe the appearance of a surface.  The struct mirrors the glTF PBR
//! material model but is kept deliberately simple to avoid pulling in heavy
//! dependencies.  It is intended to be used by both the rendering pipeline
//! and the asset import code.

use std::sync::Arc;

/// The alpha mode determines how transparency is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    /// Opaque surfaces – the alpha channel is ignored.
    Opaque,
    /// Blend the surface with the background using normal alpha blending.
    Blend,
    /// Use a cutoff value; fragments with alpha below the cutoff are discarded.
    Mask(f32),
}

/// A texture reference.  In Welkin a texture is identified by a shared `Arc<str>`
/// which points to the texture's asset path or a UUID.  Keeping it an `Arc` makes
/// cloning cheap and allows the renderer to share texture handles without extra
/// allocations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureRef(pub Arc<str>);

impl From<&str> for TextureRef {
    fn from(s: &str) -> Self {
        TextureRef(Arc::from(s))
    }
}

impl From<String> for TextureRef {
    fn from(s: String) -> Self {
        TextureRef(Arc::from(s))
    }
}

/// The core PBR material structure.
///
/// All fields are optional to allow partial material definitions (for example,
/// when loading from a format that does not provide a normal map).  When a
/// field is `None` the renderer will fall back to a sensible default.
#[derive(Debug, Clone, PartialEq)]
pub struct PbrMaterial {
    /// Base colour (albedo) as linear RGBA values in the range `[0.0, 1.0]`.
    /// If `None`, the default is solid white (`[1.0, 1.0, 1.0, 1.0]`).
    pub base_color: Option<[f32; 4]>,

    /// Metallic factor in the range `[0.0, 1.0]`. `None` defaults to `0.0`.
    pub metallic: Option<f32>,

    /// Roughness factor in the range `[0.0, 1.0]`. `None` defaults to `1.0`.
    pub roughness: Option<f32>,

    /// Optional base colour texture.
    pub base_color_texture: Option<TextureRef>,

    /// Optional metallic‑roughness texture.  The metallic factor is stored in
    /// the B channel and the roughness factor in the G channel, matching the
    /// glTF specification.
    pub metallic_roughness_texture: Option<TextureRef>,

    /// Optional normal map texture.  The texture's red and green channels store
    /// the X and Y components of the normal; the blue channel is ignored.
    pub normal_texture: Option<TextureRef>,

    /// Optional emissive colour (linear RGB) and intensity.  If `None` the
    /// material does not emit light.
    pub emissive: Option<[f32; 3]>,

    /// Optional emissive texture which multiplies the emissive colour.
    pub emissive_texture: Option<TextureRef>,

    /// Determines how alpha is interpreted.
    pub alpha_mode: AlphaMode,

    /// If `true` the material is rendered double‑sided (no back‑face culling).
    pub double_sided: bool,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        PbrMaterial {
            base_color: None,
            metallic: None,
            roughness: None,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
            emissive: None,
            emissive_texture: None,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
        }
    }
}