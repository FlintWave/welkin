// SPDX-License-Identifier: LGPL-3.0-or-later

/// Second Life / OpenSimulator asset types (OpenMetaverse `AssetType`).
///
/// Wire values match the integers the C# server stores in inventory records
/// and sends in LLSD / UDP messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum AssetType {
    Unknown = -1,
    Texture = 0,
    Sound = 1,
    CallingCard = 2,
    Landmark = 3,
    Clothing = 5,
    Object = 6,
    Notecard = 7,
    Folder = 8,
    LSLText = 10,
    LSLBytecode = 11,
    TextureTGA = 12,
    Bodypart = 13,
    SoundWAV = 17,
    ImageTGA = 18,
    ImageJPEG = 19,
    Animation = 20,
    Gesture = 21,
    Simstate = 22,
    Link = 24,
    LinkFolder = 25,
    Mesh = 49,
    Settings = 56,
    Material = 57,
}

impl AssetType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Texture,
            1 => Self::Sound,
            2 => Self::CallingCard,
            3 => Self::Landmark,
            5 => Self::Clothing,
            6 => Self::Object,
            7 => Self::Notecard,
            8 => Self::Folder,
            10 => Self::LSLText,
            11 => Self::LSLBytecode,
            12 => Self::TextureTGA,
            13 => Self::Bodypart,
            17 => Self::SoundWAV,
            18 => Self::ImageTGA,
            19 => Self::ImageJPEG,
            20 => Self::Animation,
            21 => Self::Gesture,
            22 => Self::Simstate,
            24 => Self::Link,
            25 => Self::LinkFolder,
            49 => Self::Mesh,
            56 => Self::Settings,
            57 => Self::Material,
            _ => Self::Unknown,
        }
    }

    pub fn to_i32(self) -> i32 {
        self as i8 as i32
    }

    /// The short string name used in LLSD inventory records (`llassettype.cpp`).
    pub fn as_name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Texture => "texture",
            Self::Sound => "sound",
            Self::CallingCard => "callcard",
            Self::Landmark => "landmark",
            Self::Clothing => "clothing",
            Self::Object => "object",
            Self::Notecard => "notecard",
            Self::Folder => "category",
            Self::LSLText => "lsltext",
            Self::LSLBytecode => "lslbyte",
            Self::TextureTGA => "txtr_tga",
            Self::Bodypart => "bodypart",
            Self::SoundWAV => "snd_wav",
            Self::ImageTGA => "img_tga",
            Self::ImageJPEG => "jpeg",
            Self::Animation => "animatn",
            Self::Gesture => "gesture",
            Self::Simstate => "simstate",
            Self::Link => "link",
            Self::LinkFolder => "link_f",
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
            "clothing" => Self::Clothing,
            "object" => Self::Object,
            "notecard" => Self::Notecard,
            "category" => Self::Folder,
            "lsltext" => Self::LSLText,
            "lslbyte" => Self::LSLBytecode,
            "txtr_tga" => Self::TextureTGA,
            "bodypart" => Self::Bodypart,
            "snd_wav" => Self::SoundWAV,
            "img_tga" => Self::ImageTGA,
            "jpeg" => Self::ImageJPEG,
            "animatn" => Self::Animation,
            "gesture" => Self::Gesture,
            "simstate" => Self::Simstate,
            "link" => Self::Link,
            "link_f" => Self::LinkFolder,
            "mesh" => Self::Mesh,
            "settings" => Self::Settings,
            "material" => Self::Material,
            _ => Self::Unknown,
        }
    }
}

impl From<i32> for AssetType {
    fn from(v: i32) -> Self {
        Self::from_i32(v)
    }
}

impl From<AssetType> for i32 {
    fn from(v: AssetType) -> Self {
        v.to_i32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        let variants = [
            (AssetType::Unknown, -1),
            (AssetType::Texture, 0),
            (AssetType::Sound, 1),
            (AssetType::CallingCard, 2),
            (AssetType::Landmark, 3),
            (AssetType::Clothing, 5),
            (AssetType::Object, 6),
            (AssetType::Notecard, 7),
            (AssetType::Folder, 8),
            (AssetType::LSLText, 10),
            (AssetType::LSLBytecode, 11),
            (AssetType::TextureTGA, 12),
            (AssetType::Bodypart, 13),
            (AssetType::SoundWAV, 17),
            (AssetType::ImageTGA, 18),
            (AssetType::ImageJPEG, 19),
            (AssetType::Animation, 20),
            (AssetType::Gesture, 21),
            (AssetType::Simstate, 22),
            (AssetType::Link, 24),
            (AssetType::LinkFolder, 25),
            (AssetType::Mesh, 49),
            (AssetType::Settings, 56),
            (AssetType::Material, 57),
        ];
        for (variant, wire) in variants {
            assert_eq!(variant.to_i32(), wire, "{variant:?}");
            assert_eq!(AssetType::from_i32(wire), variant);
        }
    }

    #[test]
    fn unknown_for_unrecognised() {
        assert_eq!(AssetType::from_i32(999), AssetType::Unknown);
        assert_eq!(AssetType::from_i32(4), AssetType::Unknown);
    }

    #[test]
    fn name_roundtrip() {
        for v in [
            AssetType::Texture,
            AssetType::Sound,
            AssetType::Mesh,
            AssetType::Material,
            AssetType::Folder,
        ] {
            assert_eq!(AssetType::from_name(v.as_name()), v);
        }
    }

    /// Values match aurora-net's `asset_type_from_name`.
    #[test]
    fn matches_aurora_wire_values() {
        assert_eq!(AssetType::Texture.to_i32(), 0);
        assert_eq!(AssetType::Sound.to_i32(), 1);
        assert_eq!(AssetType::CallingCard.to_i32(), 2);
        assert_eq!(AssetType::Landmark.to_i32(), 3);
        assert_eq!(AssetType::Clothing.to_i32(), 5);
        assert_eq!(AssetType::Object.to_i32(), 6);
        assert_eq!(AssetType::Notecard.to_i32(), 7);
        assert_eq!(AssetType::Folder.to_i32(), 8);
        assert_eq!(AssetType::LSLText.to_i32(), 10);
        assert_eq!(AssetType::LSLBytecode.to_i32(), 11);
        assert_eq!(AssetType::TextureTGA.to_i32(), 12);
        assert_eq!(AssetType::Bodypart.to_i32(), 13);
        assert_eq!(AssetType::ImageJPEG.to_i32(), 19);
        assert_eq!(AssetType::Animation.to_i32(), 20);
        assert_eq!(AssetType::Gesture.to_i32(), 21);
        assert_eq!(AssetType::Simstate.to_i32(), 22);
        assert_eq!(AssetType::Link.to_i32(), 24);
        assert_eq!(AssetType::LinkFolder.to_i32(), 25);
        assert_eq!(AssetType::Mesh.to_i32(), 49);
        assert_eq!(AssetType::Settings.to_i32(), 56);
        assert_eq!(AssetType::Material.to_i32(), 57);
    }
}
