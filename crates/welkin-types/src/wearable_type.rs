// SPDX-License-Identifier: LGPL-3.0-or-later

/// Wearable types (OpenMetaverse `WearableType`).
///
/// Stored in the lower 8 bits of an inventory item's `flags` field when the
/// item is clothing or a body part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WearableType {
    Shape = 0,
    Skin = 1,
    Hair = 2,
    Eyes = 3,
    Shirt = 4,
    Pants = 5,
    Shoes = 6,
    Socks = 7,
    Jacket = 8,
    Gloves = 9,
    Undershirt = 10,
    Underpants = 11,
    Skirt = 12,
    Alpha = 13,
    Tattoo = 14,
    Physics = 15,
    Universal = 16,
}

impl WearableType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Shape),
            1 => Some(Self::Skin),
            2 => Some(Self::Hair),
            3 => Some(Self::Eyes),
            4 => Some(Self::Shirt),
            5 => Some(Self::Pants),
            6 => Some(Self::Shoes),
            7 => Some(Self::Socks),
            8 => Some(Self::Jacket),
            9 => Some(Self::Gloves),
            10 => Some(Self::Undershirt),
            11 => Some(Self::Underpants),
            12 => Some(Self::Skirt),
            13 => Some(Self::Alpha),
            14 => Some(Self::Tattoo),
            15 => Some(Self::Physics),
            16 => Some(Self::Universal),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn is_body_part(self) -> bool {
        matches!(self, Self::Shape | Self::Skin | Self::Hair | Self::Eyes)
    }
}

impl From<WearableType> for u8 {
    fn from(v: WearableType) -> Self {
        v.to_u8()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        for i in 0..=16u8 {
            let w = WearableType::from_u8(i).unwrap();
            assert_eq!(w.to_u8(), i);
        }
    }

    #[test]
    fn out_of_range() {
        assert!(WearableType::from_u8(17).is_none());
        assert!(WearableType::from_u8(255).is_none());
    }

    #[test]
    fn body_parts() {
        assert!(WearableType::Shape.is_body_part());
        assert!(WearableType::Skin.is_body_part());
        assert!(WearableType::Hair.is_body_part());
        assert!(WearableType::Eyes.is_body_part());
        assert!(!WearableType::Shirt.is_body_part());
        assert!(!WearableType::Physics.is_body_part());
    }
}
