// SPDX-License-Identifier: LGPL-3.0-or-later

/// Permission bits (OpenSim `PermissionMask` from `Util.cs`).
///
/// Stored as `u32` in inventory records and object properties. The folded
/// permissions pack the four main bits into the low nibble for transmission
/// in the `OwnerID` field of object updates.
use std::fmt;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PermissionMask: u32 {
        const NONE = 0;

        // Folded permissions (low nibble).
        const FOLDED_TRANSFER = 1;
        const FOLDED_MODIFY   = 1 << 1;
        const FOLDED_COPY     = 1 << 2;
        const FOLDED_EXPORT   = 1 << 3;

        const FOLDED_MASK = 0x0F;

        // Legacy slam bit — do not set in new code.
        const SLAM = 1 << 4;

        // Standard permissions.
        const TRANSFER = 1 << 13;
        const MODIFY   = 1 << 14;
        const COPY     = 1 << 15;
        const EXPORT   = 1 << 16;
        const MOVE     = 1 << 19;
        const DAMAGE   = 1 << 20;

        const UNFOLDED_MASK = 0x1E000;

        /// Transfer + Modify + Copy + Move (no Export).
        const ALL = 0x8E000;
        const ALL_AND_EXPORT = 0x9E000;
        const ALL_EFFECTIVE  = 0x9E000;
    }
}

impl PermissionMask {
    /// Number of bit positions between a normal permission and its folded
    /// counterpart. `(perm >> FOLDING_SHIFT) & FOLDED_MASK` gives the folded
    /// form.
    pub const FOLDING_SHIFT: u32 = 13;

    pub fn fold(self) -> Self {
        Self::from_bits_truncate((self.bits() >> Self::FOLDING_SHIFT) & Self::FOLDED_MASK.bits())
    }

    pub fn unfold(self) -> Self {
        Self::from_bits_truncate((self.bits() & Self::FOLDED_MASK.bits()) << Self::FOLDING_SHIFT)
    }
}

impl fmt::Display for PermissionMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.contains(Self::COPY) {
            parts.push("C");
        }
        if self.contains(Self::MODIFY) {
            parts.push("M");
        }
        if self.contains(Self::TRANSFER) {
            parts.push("T");
        }
        if self.contains(Self::EXPORT) {
            parts.push("E");
        }
        if self.contains(Self::MOVE) {
            parts.push("V");
        }
        if parts.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "{}", parts.join(""))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_sharp_values() {
        assert_eq!(PermissionMask::TRANSFER.bits(), 0x2000);
        assert_eq!(PermissionMask::MODIFY.bits(), 0x4000);
        assert_eq!(PermissionMask::COPY.bits(), 0x8000);
        assert_eq!(PermissionMask::EXPORT.bits(), 0x10000);
        assert_eq!(PermissionMask::MOVE.bits(), 0x80000);
        assert_eq!(PermissionMask::ALL.bits(), 0x8E000);
        assert_eq!(PermissionMask::ALL_AND_EXPORT.bits(), 0x9E000);
    }

    #[test]
    fn fold_unfold_roundtrip() {
        let full = PermissionMask::COPY | PermissionMask::MODIFY | PermissionMask::TRANSFER;
        let folded = full.fold();
        assert_eq!(
            folded,
            PermissionMask::FOLDED_COPY
                | PermissionMask::FOLDED_MODIFY
                | PermissionMask::FOLDED_TRANSFER
        );
        assert_eq!(folded.unfold(), full);
    }

    #[test]
    fn fold_export() {
        let with_export = PermissionMask::ALL_AND_EXPORT;
        let folded = with_export.fold();
        assert!(folded.contains(PermissionMask::FOLDED_EXPORT));
        assert!(folded.contains(PermissionMask::FOLDED_COPY));
        assert!(folded.contains(PermissionMask::FOLDED_MODIFY));
        assert!(folded.contains(PermissionMask::FOLDED_TRANSFER));
    }

    #[test]
    fn display() {
        let p = PermissionMask::COPY | PermissionMask::MODIFY | PermissionMask::TRANSFER;
        assert_eq!(p.to_string(), "CMT");
        assert_eq!(PermissionMask::NONE.to_string(), "none");
        assert_eq!(PermissionMask::ALL.to_string(), "CMTV");
        assert_eq!(PermissionMask::ALL_AND_EXPORT.to_string(), "CMTEV");
    }

    #[test]
    fn from_raw_u32() {
        let raw = 0x8E000u32;
        let p = PermissionMask::from_bits_truncate(raw);
        assert_eq!(p, PermissionMask::ALL);
    }
}
