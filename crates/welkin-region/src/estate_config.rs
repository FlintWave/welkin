// SPDX-License-Identifier: LGPL-3.0-or-later

//! Builds the region's permissions module from configuration.
//!
//! ```toml
//! [Estate]
//! EstateOwner    = "00000000-0000-0000-0000-000000000000"   # avatar UUID
//! EstateManagers = "uuid, uuid"                              # optional
//!
//! [Permissions]   # OpenSim names
//! serverside_object_permissions = true     # false switches all checks off
//! region_owner_is_god = true
//! ```
//!
//! Without an estate owner nobody has estate rights (terraforming and other
//! estate-only actions are refused); owners can still edit their own objects.

use uuid::Uuid;
use welkin_config::Config;
use welkin_modules::module::{ModuleConfig, RegionModule};
use welkin_modules::permissions::DefaultPermissionsModule;

struct ConfigAdapter<'a>(&'a Config);

impl ModuleConfig for ConfigAdapter<'_> {
    fn get_string(&self, section: &str, key: &str, default: &str) -> String {
        self.0.get_string_or(section, key, default)
    }
    fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
        self.0.get_i64_or(section, key, default)
    }
    fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        self.0.get_bool_or(section, key, default)
    }
    fn get_float(&self, section: &str, key: &str, default: f64) -> f64 {
        self.0.get_f64_or(section, key, default)
    }
}

fn parse_uuid_list(text: &str) -> Vec<Uuid> {
    text.split([',', ';', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| match Uuid::parse_str(s) {
            Ok(id) => Some(id),
            Err(_) => {
                tracing::error!(value = s, "[Estate] ignoring an entry that is not a UUID");
                None
            }
        })
        .collect()
}

/// Permissions module with the configured estate owner and managers.
pub fn permissions_from_config(config: &Config) -> DefaultPermissionsModule {
    let mut perms = DefaultPermissionsModule::new();
    if let Err(e) = perms.initialise(&ConfigAdapter(config)) {
        tracing::error!(error = %e, "permissions configuration rejected; using enforcing defaults");
    }
    let owner = config.get_string_or("Estate", "EstateOwner", "");
    if !owner.trim().is_empty() {
        match Uuid::parse_str(owner.trim()) {
            Ok(id) => perms.estate_owner = id,
            Err(_) => tracing::error!("[Estate] EstateOwner is not a UUID; no estate owner set"),
        }
    }
    let managers = config.get_string_or("Estate", "EstateManagers", "");
    perms.estate_managers = parse_uuid_list(&managers).into_iter().collect();

    if perms.config.bypass_permissions {
        tracing::warn!("permission checks are switched OFF (serverside_object_permissions = false)");
    } else if perms.estate_owner.is_nil() {
        tracing::warn!(
            "no [Estate] EstateOwner configured: nobody can terraform or use estate tools; \
             set EstateOwner to an avatar UUID"
        );
    }
    perms
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::welkin_types::PermissionMask;

    fn config(toml: &str) -> Config {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "welkin-estate-cfg-{}-{}.toml",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, toml).unwrap();
        let cfg = Config::from_toml_file(&path).expect("config");
        let _ = std::fs::remove_file(&path);
        cfg
    }

    #[test]
    fn enforcement_is_on_by_default_and_nobody_may_terraform_without_an_owner() {
        let perms = permissions_from_config(&config(""));
        assert!(!perms.config.bypass_permissions);
        assert!(!perms.can_edit_terrain(Uuid::new_v4()));
    }

    #[test]
    fn owner_and_managers_come_from_the_estate_section() {
        let owner = Uuid::new_v4();
        let manager = Uuid::new_v4();
        let stranger = Uuid::new_v4();
        let perms = permissions_from_config(&config(&format!(
            "[Estate]\nEstateOwner = \"{owner}\"\nEstateManagers = \"{manager}, not-a-uuid\"\n"
        )));
        assert_eq!(perms.estate_owner, owner);
        assert!(perms.can_edit_terrain(owner));
        assert!(perms.can_edit_terrain(manager));
        assert!(!perms.can_edit_terrain(stranger));
        assert!(perms.is_estate_manager(manager));
        assert!(!perms.is_estate_manager(stranger));
    }

    #[test]
    fn serverside_object_permissions_false_switches_checks_off() {
        let perms = permissions_from_config(&config(
            "[Permissions]\nserverside_object_permissions = false\n",
        ));
        assert!(perms.config.bypass_permissions);
        assert!(perms.can_edit_terrain(Uuid::new_v4()));
    }

    #[test]
    fn owners_edit_their_own_objects_and_strangers_do_not() {
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let perms = permissions_from_config(&config(""));
        // Object owned by `owner`, owner mask all, group/everyone none.
        assert!(perms.can_edit_object(owner, owner, Uuid::nil(), PermissionMask::all()));
        assert!(!perms.can_edit_object(other, owner, Uuid::nil(), PermissionMask::all()));
    }
}
