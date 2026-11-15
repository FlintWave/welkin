// SPDX-License-Identifier: GPL-3.0-or-later

/// System folder types (OpenMetaverse `FolderType` / `AssetType` in the
/// `type_default` field of an inventory folder).
///
/// These overlap with [`super::AssetType`] values because the C# server
/// stores the preferred asset type of a system folder in the same i32 field.
/// Variants that don't correspond to an asset type use values >= 46.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum FolderType {
    None = -1,
    Texture = 0,
    Sound = 1,
    CallingCard = 2,
    Landmark = 3,
    Clothing = 5,
    Object = 6,
    Notecard = 7,
    Root = 8,
    LSLText = 10,
    BodyPart = 13,
    Trash = 14,
    Snapshot = 15,
    LostAndFound = 16,
    Animation = 20,
    Gesture = 21,
    Favorites = 23,
    CurrentOutfit = 46,
    Outfit = 47,
    MyOutfits = 48,
    Mesh = 49,
    Inbox = 51,
    Outbox = 52,
    Settings = 56,
    Material = 57,
    Suitcase = 100,
}

impl FolderType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Texture,
            1 => Self::Sound,
            2 => Self::CallingCard,
            3 => Self::Landmark,
            5 => Self::Clothing,
            6 => Self::Object,
            7 => Self::Notecard,
            8 => Self::Root,
            10 => Self::LSLText,
            13 => Self::BodyPart,
            14 => Self::Trash,
            15 => Self::Snapshot,
            16 => Self::LostAndFound,
            20 => Self::Animation,
            21 => Self::Gesture,
            23 => Self::Favorites,
            46 => Self::CurrentOutfit,
            47 => Self::Outfit,
            48 => Self::MyOutfits,
            49 => Self::Mesh,
            51 => Self::Inbox,
            52 => Self::Outbox,
            56 => Self::Settings,
            57 => Self::Material,
            100 => Self::Suitcase,
            _ => Self::None,
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Suitcase => 100,
            other => other as i8 as i32,
        }
    }
}

impl From<i32> for FolderType {
    fn from(v: i32) -> Self {
        Self::from_i32(v)
    }
}

impl From<FolderType> for i32 {
    fn from(v: FolderType) -> Self {
        v.to_i32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        let variants = [
            (FolderType::None, -1),
            (FolderType::Texture, 0),
            (FolderType::Sound, 1),
            (FolderType::CallingCard, 2),
            (FolderType::Landmark, 3),
            (FolderType::Clothing, 5),
            (FolderType::Object, 6),
            (FolderType::Notecard, 7),
            (FolderType::Root, 8),
            (FolderType::LSLText, 10),
            (FolderType::BodyPart, 13),
            (FolderType::Trash, 14),
            (FolderType::Snapshot, 15),
            (FolderType::LostAndFound, 16),
            (FolderType::Animation, 20),
            (FolderType::Gesture, 21),
            (FolderType::Favorites, 23),
            (FolderType::CurrentOutfit, 46),
            (FolderType::Outfit, 47),
            (FolderType::MyOutfits, 48),
            (FolderType::Mesh, 49),
            (FolderType::Inbox, 51),
            (FolderType::Outbox, 52),
            (FolderType::Settings, 56),
            (FolderType::Material, 57),
            (FolderType::Suitcase, 100),
        ];
        for (variant, wire) in variants {
            assert_eq!(variant.to_i32(), wire, "{variant:?}");
            assert_eq!(FolderType::from_i32(wire), variant);
        }
    }

    #[test]
    fn unrecognised_maps_to_none() {
        assert_eq!(FolderType::from_i32(999), FolderType::None);
        assert_eq!(FolderType::from_i32(4), FolderType::None);
    }

    /// System folders share wire values with their asset types.
    #[test]
    fn shared_asset_type_values() {
        assert_eq!(FolderType::Texture.to_i32(), 0);
        assert_eq!(FolderType::Sound.to_i32(), 1);
        assert_eq!(FolderType::Clothing.to_i32(), 5);
        assert_eq!(FolderType::Mesh.to_i32(), 49);
        assert_eq!(FolderType::Material.to_i32(), 57);
    }
}
