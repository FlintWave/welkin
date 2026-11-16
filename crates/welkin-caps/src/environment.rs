// SPDX-License-Identifier: LGPL-3.0-or-later

//! Helpers for the legacy `Cap_EnvironmentSettings` capability.
//!
//! Aurora and Firestorm fetch legacy Windlight data from this capability as an
//! LLSD array: `[ {messageID, regionID}, dayCycleTrack, skyFrames, waterFrame ]`.
//! When no custom environment is set, OpenSim returns just the region entry, and
//! viewers fall back to the default day cycle asset.

use uuid::Uuid;
use welkin_llsd::{Llsd, Map};

/// Default sky settings asset (LLSettingsSky::DEFAULT_ASSET_ID).
pub const DEFAULT_SKY_ASSET_UUID: Uuid =
    uuid::uuid!("651510b8-5f4d-8991-1592-e7eeab2a5a06");
/// Default water settings asset (LLSettingsWater::DEFAULT_ASSET_ID).
pub const DEFAULT_WATER_ASSET_UUID: Uuid =
    uuid::uuid!("59d1a851-47e7-0e5f-1ed7-6b715154f41a");
/// Default day-cycle settings asset (LLSettingsDay::DEFAULT_ASSET_ID).
pub const DEFAULT_DAY_CYCLE_ASSET_UUID: Uuid =
    uuid::uuid!("5646d39e-d3d7-6aff-ed71-30fc87d64a91");

/// The default sky/water/day-cycle asset UUIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultEnvironmentAssets {
    pub sky: Uuid,
    pub water: Uuid,
    pub day_cycle: Uuid,
}

impl Default for DefaultEnvironmentAssets {
    fn default() -> Self {
        Self {
            sky: DEFAULT_SKY_ASSET_UUID,
            water: DEFAULT_WATER_ASSET_UUID,
            day_cycle: DEFAULT_DAY_CYCLE_ASSET_UUID,
        }
    }
}

/// Returns the default sky/water/day-cycle asset UUIDs.
pub fn default_environment_assets() -> DefaultEnvironmentAssets {
    DefaultEnvironmentAssets::default()
}

/// Returns the LLSD structure served by `Cap_EnvironmentSettings` for a region
/// that has no custom environment configured.
pub fn default_environment_settings() -> Llsd {
    default_environment_settings_for_region(Uuid::nil())
}

/// Returns the LLSD structure served by `Cap_EnvironmentSettings` for `region_id`.
pub fn default_environment_settings_for_region(region_id: Uuid) -> Llsd {
    let mut region = Map::new();
    region.insert("messageID".into(), Llsd::Uuid(Uuid::nil()));
    region.insert("regionID".into(), Llsd::Uuid(region_id));
    Llsd::Array(vec![Llsd::Map(region)])
}

// Aliases used elsewhere in the codebase / viewers.
pub fn environment_settings_defaults() -> Llsd {
    default_environment_settings()
}

pub fn cap_environment_settings_default() -> Llsd {
    default_environment_settings()
}

// Extended environment capability aliases (ExtEnvironment).
pub fn default_ext_environment_settings() -> Llsd {
    default_environment_settings()
}

pub fn default_ext_environment_settings_for_region(region_id: Uuid) -> Llsd {
    default_environment_settings_for_region(region_id)
}

pub fn ext_environment_settings_defaults() -> Llsd {
    default_environment_settings()
}

pub fn cap_ext_environment_settings_default() -> Llsd {
    default_environment_settings()
}

pub fn ext_environment_defaults() -> Llsd {
    default_environment_settings()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_asset_uuids_match_firestorm() {
        assert_eq!(
            DEFAULT_SKY_ASSET_UUID.to_string(),
            "651510b8-5f4d-8991-1592-e7eeab2a5a06"
        );
        assert_eq!(
            DEFAULT_WATER_ASSET_UUID.to_string(),
            "59d1a851-47e7-0e5f-1ed7-6b715154f41a"
        );
        assert_eq!(
            DEFAULT_DAY_CYCLE_ASSET_UUID.to_string(),
            "5646d39e-d3d7-6aff-ed71-30fc87d64a91"
        );
    }

    #[test]
    fn default_assets_struct() {
        let a = default_environment_assets();
        assert_eq!(a.sky, DEFAULT_SKY_ASSET_UUID);
        assert_eq!(a.water, DEFAULT_WATER_ASSET_UUID);
        assert_eq!(a.day_cycle, DEFAULT_DAY_CYCLE_ASSET_UUID);
        assert_eq!(a, DefaultEnvironmentAssets::default());
    }

    #[test]
    fn default_structure_is_array_with_region_entry() {
        let llsd = default_environment_settings();
        match llsd {
            Llsd::Array(entries) => {
                assert_eq!(entries.len(), 1);
                match &entries[0] {
                    Llsd::Map(m) => {
                        assert_eq!(m.get("messageID"), Some(&Llsd::Uuid(Uuid::nil())));
                        assert_eq!(m.get("regionID"), Some(&Llsd::Uuid(Uuid::nil())));
                    }
                    other => panic!("expected map, got {other:?}"),
                }
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn for_region_sets_region_id() {
        let rid = Uuid::new_v4();
        let llsd = default_environment_settings_for_region(rid);
        if let Llsd::Array(entries) = llsd {
            if let Llsd::Map(m) = &entries[0] {
                assert_eq!(m.get("regionID"), Some(&Llsd::Uuid(rid)));
            } else {
                panic!("expected map");
            }
        } else {
            panic!("expected array");
        }
    }
}
