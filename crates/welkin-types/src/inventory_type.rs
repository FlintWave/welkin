// SPDX-License-Identifier: LGPL-3.0-or-later

/// Second Life / OpenSimulator inventory types (OpenMetaverse `InventoryType`).
///
/// Each inventory item carries an inventory type alongside its asset type.
/// The inventory type determines the UI panel (texture preview, notecard
/// editor, wearable picker, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum InventoryType {
    Unknown = -1,
    Texture = 0,
    Sound = 1,
    CallingCard = 2,
    Landmark = 3,
    Object = 6,
    Notecard = 7,
    Folder = 8,
    RootCategory = 9,
    LSL = 10,
    Snapshot = 15,
    Attachment = 17,
    Wearable = 18,
    Animation = 19,
    Gesture = 20,
    Mesh = 22,
    Settings = 25,
    Material = 26,
}

impl InventoryType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Texture,
            1 => Self::Sound,
            2 => Self::CallingCard,
            3 => Self::Landmark,
            6 => Self::Object,
            7 => Self::Notecard,
            8 => Self::Folder,
            9 => Self::RootCategory,
            10 => Self::LSL,
            15 => Self::Snapshot,
            17 => Self::Attachment,
            18 => Self::Wearable,
            19 => Self::Animation,
            20 => Self::Gesture,
            22 => Self::Mesh,
            25 => Self::Settings,
            26 => Self::Material,
            _ => Self::Unknown,
        }
    }

    pub fn to_i32(self) -> i32 {
        self as i8 as i32
    }

    pub fn as_name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Texture => "texture",
            Self::Sound => "sound",
            Self::CallingCard => "callcard",
            Self::Landmark => "landmark",
            Self::Object => "object",
            Self::Notecard => "notecard",
            Self::Folder => "category",
            Self::RootCategory => "root",
            Self::LSL => "script",
            Self::Snapshot => "snapshot",
            Self::Attachment => "attach",
            Self::Wearable => "wearable",
            Self::Animation => "animation",
            Self::Gesture => "gesture",
            Self::Mesh => "mesh",
            Self::Settings => "settings",
            Self::Material => "material",
        }
    }

    pub fn from_name(s: &str) -> Self {
        match s {
            "texture" => Self::Texture,
            "sound" => Self::Sound,
            "callcard" => Self::CallingCard,
            "landmark" => Self::Landmark,
            "object" => Self::Object,
            "notecard" => Self::Notecard,
            "category" => Self::Folder,
            "root" => Self::RootCategory,
            "script" => Self::LSL,
            "snapshot" => Self::Snapshot,
            "attach" => Self::Attachment,
            "wearable" => Self::Wearable,
            "animation" => Self::Animation,
            "gesture" => Self::Gesture,
            "mesh" => Self::Mesh,
            "settings" => Self::Settings,
            "material" => Self::Material,
            _ => Self::Unknown,
        }
    }
}

impl From<i32> for InventoryType {
    fn from(v: i32) -> Self {
        Self::from_i32(v)
    }
}

impl From<InventoryType> for i32 {
    fn from(v: InventoryType) -> Self {
        v.to_i32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        let variants = [
            (InventoryType::Unknown, -1),
            (InventoryType::Texture, 0),
            (InventoryType::Sound, 1),
            (InventoryType::CallingCard, 2),
            (InventoryType::Landmark, 3),
            (InventoryType::Object, 6),
            (InventoryType::Notecard, 7),
            (InventoryType::Folder, 8),
            (InventoryType::RootCategory, 9),
            (InventoryType::LSL, 10),
            (InventoryType::Snapshot, 15),
            (InventoryType::Attachment, 17),
            (InventoryType::Wearable, 18),
            (InventoryType::Animation, 19),
            (InventoryType::Gesture, 20),
            (InventoryType::Mesh, 22),
            (InventoryType::Settings, 25),
            (InventoryType::Material, 26),
        ];
        for (variant, wire) in variants {
            assert_eq!(variant.to_i32(), wire, "{variant:?}");
            assert_eq!(InventoryType::from_i32(wire), variant);
        }
    }

    #[test]
    fn matches_aurora_wire_values() {
        assert_eq!(InventoryType::Texture.to_i32(), 0);
        assert_eq!(InventoryType::Sound.to_i32(), 1);
        assert_eq!(InventoryType::CallingCard.to_i32(), 2);
        assert_eq!(InventoryType::Landmark.to_i32(), 3);
        assert_eq!(InventoryType::Object.to_i32(), 6);
        assert_eq!(InventoryType::Notecard.to_i32(), 7);
        assert_eq!(InventoryType::LSL.to_i32(), 10);
        assert_eq!(InventoryType::Snapshot.to_i32(), 15);
        assert_eq!(InventoryType::Attachment.to_i32(), 17);
        assert_eq!(InventoryType::Wearable.to_i32(), 18);
        assert_eq!(InventoryType::Animation.to_i32(), 19);
        assert_eq!(InventoryType::Gesture.to_i32(), 20);
        assert_eq!(InventoryType::Mesh.to_i32(), 22);
        assert_eq!(InventoryType::Settings.to_i32(), 25);
        assert_eq!(InventoryType::Material.to_i32(), 26);
    }

    #[test]
    fn name_roundtrip() {
        for v in [
            InventoryType::Texture,
            InventoryType::LSL,
            InventoryType::Wearable,
            InventoryType::Mesh,
        ] {
            assert_eq!(InventoryType::from_name(v.as_name()), v);
        }
    }
}
