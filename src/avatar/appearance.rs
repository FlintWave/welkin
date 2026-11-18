use crate::pbr_material::{Material, SystemClothingMaterial};

/// Represents the visual appearance of an avatar.
pub struct AvatarAppearance {
    /// The material applied to the avatar's body mesh.
    pub body_material: Material,
    // ... other fields ...
}

impl AvatarAppearance {
    /// Creates a new `AvatarAppearance` from the given parameters.
    ///
    /// If a specific material for the body mesh is not provided, the system
    /// clothing material is used so that the default clothing appears correctly.
    pub fn new(body_material: Option<Material> /*, other params */) -> Self {
        let body_material = match body_material {
            Some(mat) => mat,
            None => SystemClothingMaterial::default().into(),
        };

        AvatarAppearance {
            body_material,
            // ... initialise other fields ...
        }
    }

    /// Updates the avatar's appearance based on the latest inventory data.
    ///
    /// This method now ensures that when the body mesh lacks an explicit
    /// material definition, the system clothing material is applied. This
    /// change fixes the issue where system clothing on the mesh body did
    /// not display correctly in Aurora.
    pub fn update_from_inventory(&mut self, inventory: &Inventory) {
        // Existing logic for updating other parts...

        // Ensure body material falls back to system clothing if missing.
        if self.body_material.is_placeholder() {
            self.body_material = SystemClothingMaterial::default().into();
        }
    }
}

// Helper trait to detect placeholder materials.
trait MaterialExt {
    fn is_placeholder(&self) -> bool;
}

impl MaterialExt for Material {
    fn is_placeholder(&self) -> bool {
        matches!(self, Material::Placeholder(_))
    }
}