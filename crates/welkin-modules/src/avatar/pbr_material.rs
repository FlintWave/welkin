use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum PbrAlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl Default for PbrAlphaMode {
    fn default() -> Self {
        Self::Opaque
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PbrMaterial {
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub normal_map: Option<Uuid>,
    pub emissive: [f32; 3],
    pub alpha_mode: PbrAlphaMode,
    pub double_sided: bool,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 1.0,
            normal_map: None,
            emissive: [0.0, 0.0, 0.0],
            alpha_mode: PbrAlphaMode::default(),
            double_sided: false,
        }
    }
}

impl PbrMaterial {
    pub fn roughness_clamped(&self) -> f32 {
        self.roughness.clamp(0.0, 1.0)
    }

    pub fn to_llsd(&self) -> String {
        let normal_map = match self.normal_map {
            Some(uuid) => format!("<key>normal_map</key><uuid>{uuid}</uuid>"),
            None => String::from("<key>normal_map</key><undef/>"),
        };
        let alpha_mode = match self.alpha_mode {
            PbrAlphaMode::Opaque => "Opaque",
            PbrAlphaMode::Mask => "Mask",
            PbrAlphaMode::Blend => "Blend",
        };

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<llsd>
<map>
<key>base_color</key>
<array><real>{}</real><real>{}</real><real>{}</real></array>
<key>metallic</key><real>{}</real>
<key>roughness</key><real>{}</real>
{}
<key>emissive</key>
<array><real>{}</real><real>{}</real><real>{}</real></array>
<key>alpha_mode</key><string>{}</string>
<key>double_sided</key><boolean>{}</boolean>
</map>
</llsd>"#,
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            self.metallic,
            self.roughness,
            normal_map,
            self.emissive[0],
            self.emissive[1],
            self.emissive[2],
            alpha_mode,
            if self.double_sided { 1 } else { 0 }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let material = PbrMaterial::default();
        assert_eq!(material.base_color, [1.0, 1.0, 1.0]);
        assert_eq!(material.metallic, 0.0);
        assert_eq!(material.roughness, 1.0);
        assert_eq!(material.normal_map, None);
        assert_eq!(material.emissive, [0.0, 0.0, 0.0]);
        assert_eq!(material.alpha_mode, PbrAlphaMode::Opaque);
        assert!(!material.double_sided);
    }

    #[test]
    fn roughness_is_clamped() {
        let mut material = PbrMaterial::default();
        material.roughness = 1.5;
        assert_eq!(material.roughness_clamped(), 1.0);
        material.roughness = -0.25;
        assert_eq!(material.roughness_clamped(), 0.0);
    }

    #[test]
    fn to_llsd_contains_all_fields() {
        let material = PbrMaterial::default();
        let llsd = material.to_llsd();
        for field in [
            "base_color",
            "metallic",
            "roughness",
            "normal_map",
            "emissive",
            "alpha_mode",
            "double_sided",
        ] {
            assert!(llsd.contains(field), "missing field: {field}");
        }
    }
}
