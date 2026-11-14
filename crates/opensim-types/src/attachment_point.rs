// SPDX-License-Identifier: GPL-3.0-or-later

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttachmentPoint {
    Default = 0,
    Chest = 1,
    Skull = 2,
    LeftShoulder = 3,
    RightShoulder = 4,
    LeftHand = 5,
    RightHand = 6,
    LeftFoot = 7,
    RightFoot = 8,
    Spine = 9,
    Pelvis = 10,
    Mouth = 11,
    Chin = 12,
    LeftEar = 13,
    RightEar = 14,
    LeftEyeball = 15,
    RightEyeball = 16,
    Nose = 17,
    RightUpperArm = 18,
    RightForearm = 19,
    LeftUpperArm = 20,
    LeftForearm = 21,
    RightHip = 22,
    RightUpperLeg = 23,
    RightLowerLeg = 24,
    LeftHip = 25,
    LeftUpperLeg = 26,
    LeftLowerLeg = 27,
    Stomach = 28,
    LeftPec = 29,
    RightPec = 30,
    HudCenter2 = 31,
    HudTopRight = 32,
    HudTop = 33,
    HudTopLeft = 34,
    HudCenter = 35,
    HudBottomLeft = 36,
    HudBottom = 37,
    HudBottomRight = 38,
    Neck = 39,
    AvatarCenter = 40,
    LeftRingFinger = 41,
    RightRingFinger = 42,
    TailBase = 43,
    TailTip = 44,
    LeftWing = 45,
    RightWing = 46,
    Jaw = 47,
    AltLeftEar = 48,
    AltRightEar = 49,
    AltLeftEye = 50,
    AltRightEye = 51,
    Tongue = 52,
    Groin = 53,
    LeftHindFoot = 54,
    RightHindFoot = 55,
}

impl AttachmentPoint {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Default),
            1 => Some(Self::Chest),
            2 => Some(Self::Skull),
            3 => Some(Self::LeftShoulder),
            4 => Some(Self::RightShoulder),
            5 => Some(Self::LeftHand),
            6 => Some(Self::RightHand),
            7 => Some(Self::LeftFoot),
            8 => Some(Self::RightFoot),
            9 => Some(Self::Spine),
            10 => Some(Self::Pelvis),
            11 => Some(Self::Mouth),
            12 => Some(Self::Chin),
            13 => Some(Self::LeftEar),
            14 => Some(Self::RightEar),
            15 => Some(Self::LeftEyeball),
            16 => Some(Self::RightEyeball),
            17 => Some(Self::Nose),
            18 => Some(Self::RightUpperArm),
            19 => Some(Self::RightForearm),
            20 => Some(Self::LeftUpperArm),
            21 => Some(Self::LeftForearm),
            22 => Some(Self::RightHip),
            23 => Some(Self::RightUpperLeg),
            24 => Some(Self::RightLowerLeg),
            25 => Some(Self::LeftHip),
            26 => Some(Self::LeftUpperLeg),
            27 => Some(Self::LeftLowerLeg),
            28 => Some(Self::Stomach),
            29 => Some(Self::LeftPec),
            30 => Some(Self::RightPec),
            31 => Some(Self::HudCenter2),
            32 => Some(Self::HudTopRight),
            33 => Some(Self::HudTop),
            34 => Some(Self::HudTopLeft),
            35 => Some(Self::HudCenter),
            36 => Some(Self::HudBottomLeft),
            37 => Some(Self::HudBottom),
            38 => Some(Self::HudBottomRight),
            39 => Some(Self::Neck),
            40 => Some(Self::AvatarCenter),
            41 => Some(Self::LeftRingFinger),
            42 => Some(Self::RightRingFinger),
            43 => Some(Self::TailBase),
            44 => Some(Self::TailTip),
            45 => Some(Self::LeftWing),
            46 => Some(Self::RightWing),
            47 => Some(Self::Jaw),
            48 => Some(Self::AltLeftEar),
            49 => Some(Self::AltRightEar),
            50 => Some(Self::AltLeftEye),
            51 => Some(Self::AltRightEye),
            52 => Some(Self::Tongue),
            53 => Some(Self::Groin),
            54 => Some(Self::LeftHindFoot),
            55 => Some(Self::RightHindFoot),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn is_hud(self) -> bool {
        (31..=38).contains(&(self as u8))
    }
}

impl From<AttachmentPoint> for u8 {
    fn from(value: AttachmentPoint) -> Self {
        value.to_u8()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        for v in 0..=55u8 {
            let point = AttachmentPoint::from_u8(v).expect("valid attachment point");
            assert_eq!(point.to_u8(), v);
            assert_eq!(u8::from(point), v);
        }
    }

    #[test]
    fn out_of_range() {
        assert!(AttachmentPoint::from_u8(56).is_none());
        assert!(AttachmentPoint::from_u8(255).is_none());
    }

    #[test]
    fn hud_points() {
        for v in 31..=38u8 {
            assert!(AttachmentPoint::from_u8(v).unwrap().is_hud());
        }
        assert!(!AttachmentPoint::Default.is_hud());
        assert!(!AttachmentPoint::Chest.is_hud());
    }

    #[test]
    fn classic_points() {
        assert_eq!(AttachmentPoint::Default.to_u8(), 0);
        assert_eq!(AttachmentPoint::Chest.to_u8(), 1);
        assert_eq!(AttachmentPoint::Skull.to_u8(), 2);
        assert_eq!(AttachmentPoint::LeftShoulder.to_u8(), 3);
        assert_eq!(AttachmentPoint::RightShoulder.to_u8(), 4);
        assert!(!AttachmentPoint::Chest.is_hud());
        assert!(!AttachmentPoint::Pelvis.is_hud());
        assert!(!AttachmentPoint::RightFoot.is_hud());
    }
}
