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
//! The estate is also stored in the region database (`estate_settings`, `estate_map`,
//! `estate_managers`): the configured owner and managers are written there on first
//! start, and later starts read the stored estate (a changed `EstateOwner` in the
//! config replaces the stored owner).
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
    }
    perms
}

/// The region-wide parcel used until the region has real land data: owned by the
/// estate owner with OpenSim's default parcel flags (everyone may rez objects).
pub fn default_parcel(perms: &DefaultPermissionsModule) -> welkin_modules::land::LandData {
    welkin_modules::land::LandData {
        name: "Your Parcel".to_string(),
        owner_id: perms.estate_owner,
        ..Default::default()
    }
}

fn default_estate(owner: Uuid) -> welkin_data::estate::EstateSettingsRow {
    // OpenSim's defaults for a new estate.
    welkin_data::estate::EstateSettingsRow {
        estate_id: 0,
        estate_name: Some("My Estate".to_string()),
        abuse_email_to_estate_owner: 0,
        deny_anonymous: 0,
        reset_home_on_teleport: 0,
        fixed_sun: 0,
        deny_transacted: 0,
        block_dwell: 0,
        deny_identified: 0,
        allow_voice: 1,
        use_global_time: 1,
        price_per_meter: 1,
        tax_free: 0,
        allow_direct_teleport: 1,
        redirect_grid_x: 0,
        redirect_grid_y: 0,
        parent_estate_id: 1,
        sun_position: 0.0,
        estate_skip_scripts: 0,
        billable_factor: 0.0,
        public_access: 1,
        abuse_email: String::new(),
        estate_owner: owner.to_string(),
        deny_minors: 0,
        allow_landmark: 1,
        allow_parcel_changes: 1,
        allow_set_home: 1,
        allow_environment_override: 0,
    }
}

/// Reconcile the configured estate owner/managers (already in `perms`) with the
/// estate stored for this region, creating the estate on first start, and leave
/// `perms` holding the result.
pub async fn apply_estate_data(
    perms: &mut DefaultPermissionsModule,
    store: &welkin_data::estate::EstateStore,
    region_id: &str,
) {
    let cfg_owner = perms.estate_owner;
    let cfg_managers: Vec<Uuid> = perms.estate_managers.iter().copied().collect();

    let estate = match store.get_estate_for_region(region_id).await {
        Ok(estate) => estate,
        Err(e) => {
            tracing::error!(error = %e, "could not read the estate; using the configured owner only");
            return;
        }
    };
    let result: Result<(), welkin_data::DataError> = async {
        let estate_id = match estate {
            Some(mut row) => {
                let stored_owner = Uuid::parse_str(row.estate_owner.trim()).unwrap_or(Uuid::nil());
                if !cfg_owner.is_nil() && cfg_owner != stored_owner {
                    tracing::info!(old = %stored_owner, new = %cfg_owner, "estate owner changed by configuration");
                    row.estate_owner = cfg_owner.to_string();
                    store.store_estate(&row).await?;
                } else {
                    perms.estate_owner = stored_owner;
                }
                row.estate_id
            }
            None if !cfg_owner.is_nil() => {
                let id = store.create_estate(&default_estate(cfg_owner)).await?;
                store.link_region(id, region_id).await?;
                tracing::info!(estate_id = id, owner = %cfg_owner, "created the region's estate");
                id
            }
            None => return Ok(()),
        };
        let mut managers: std::collections::HashSet<Uuid> = store
            .get_estate_managers(estate_id)
            .await?
            .iter()
            .filter_map(|s| Uuid::parse_str(s.trim()).ok())
            .collect();
        for m in cfg_managers {
            if managers.insert(m) {
                store.add_estate_manager(estate_id, &m.to_string()).await?;
            }
        }
        perms.estate_managers = managers;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        tracing::error!(error = %e, "estate could not be loaded or saved; using the configured owner only");
    }
    if perms.estate_owner.is_nil() && !perms.config.bypass_permissions {
        tracing::warn!(
            "the region has no estate owner (none in the database and no [Estate] EstateOwner configured): \
             nobody can terraform or use estate tools; set EstateOwner to an avatar UUID"
        );
    }
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

    async fn estate_store() -> (welkin_data::estate::EstateStore, std::path::PathBuf) {
        sqlx::any::install_default_drivers();
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "welkin-estate-test-{}-{}.db",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        let pool = welkin_data::DataPool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .expect("pool");
        let backend = welkin_data::migrations::Backend::SQLite;
        let runner = welkin_data::migrations::MigrationRunner::new(pool.inner().clone(), backend);
        runner.ensure_migrations_table().await.unwrap();
        let sql = welkin_data::migrations::migration_content("EstateStore", backend).unwrap();
        runner.run_migrations("EstateStore", sql).await.unwrap();
        (welkin_data::estate::EstateStore::new(pool.inner().clone()), path)
    }

    #[tokio::test]
    async fn estate_is_created_stored_and_read_back_on_the_next_start() {
        let (store, path) = estate_store().await;
        let region = "22222222-2222-2222-2222-222222222222";
        let owner = Uuid::new_v4();
        let manager = Uuid::new_v4();

        // First start: config only.
        let mut perms = permissions_from_config(&config(&format!(
            "[Estate]\nEstateOwner = \"{owner}\"\nEstateManagers = \"{manager}\"\n"
        )));
        apply_estate_data(&mut perms, &store, region).await;
        assert_eq!(perms.estate_owner, owner);
        let row = store.get_estate_for_region(region).await.unwrap().expect("estate created");
        assert_eq!(row.estate_owner, owner.to_string());
        assert_eq!(row.estate_id, 100);
        assert_eq!(store.get_estate_managers(row.estate_id).await.unwrap(), vec![manager.to_string()]);

        // Second start with no [Estate] config: the stored estate supplies everything.
        let mut perms = permissions_from_config(&config(""));
        assert!(perms.estate_owner.is_nil());
        apply_estate_data(&mut perms, &store, region).await;
        assert_eq!(perms.estate_owner, owner);
        assert!(perms.is_estate_manager(manager));
        assert!(perms.can_edit_terrain(owner));

        // A new owner in the config replaces the stored one; managers are not duplicated.
        let new_owner = Uuid::new_v4();
        let mut perms = permissions_from_config(&config(&format!(
            "[Estate]\nEstateOwner = \"{new_owner}\"\nEstateManagers = \"{manager}\"\n"
        )));
        apply_estate_data(&mut perms, &store, region).await;
        assert_eq!(perms.estate_owner, new_owner);
        let row = store.get_estate_for_region(region).await.unwrap().unwrap();
        assert_eq!(row.estate_owner, new_owner.to_string());
        assert_eq!(store.get_estate_managers(row.estate_id).await.unwrap().len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn without_config_or_stored_estate_nobody_has_estate_rights() {
        let (store, path) = estate_store().await;
        let mut perms = permissions_from_config(&config(""));
        apply_estate_data(&mut perms, &store, "33333333-3333-3333-3333-333333333333").await;
        assert!(perms.estate_owner.is_nil());
        assert!(store.get_estate_for_region("33333333-3333-3333-3333-333333333333").await.unwrap().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rezzing_goes_through_the_permissions_module_with_the_default_parcel() {
        let owner = Uuid::new_v4();
        let stranger = Uuid::new_v4();
        let mut perms = permissions_from_config(&config(""));
        perms.estate_owner = owner;
        let parcel = default_parcel(&perms);
        assert_eq!(parcel.owner_id, owner);
        // Enforcement on: the default parcel allows everyone to rez (OpenSim's default flags).
        assert!(perms.can_create_object(stranger, (1.0, 2.0, 3.0), Some(&parcel)));
        // Without any parcel the module refuses, so the call site must always pass one.
        assert!(!perms.can_create_object(stranger, (1.0, 2.0, 3.0), None));
        // A parcel with rezzing closed refuses strangers but not its owner.
        let mut closed = parcel.clone();
        closed.flags = 0;
        assert!(!perms.can_create_object(stranger, (1.0, 2.0, 3.0), Some(&closed)));
        assert!(perms.can_create_object(owner, (1.0, 2.0, 3.0), Some(&closed)));
    }
}
