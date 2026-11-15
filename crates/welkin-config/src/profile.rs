// SPDX-License-Identifier: GPL-3.0-or-later

//! Grid profile support.
//!
//! A *grid profile* captures the service URLs and mode flags that distinguish
//! standalone, own-grid, and external-grid (e.g. OSgrid) deployments.  The
//! profile is applied as the lowest-priority layer: explicit config, then env
//! vars, then profile defaults — so operators can always override.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{Config, ConfigError};

// ---------------------------------------------------------------------------
// GridProfile enum
// ---------------------------------------------------------------------------

/// The three deployment modes the simulator supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridProfile {
    /// All services run inside the simulator process on localhost.
    Standalone,
    /// Operator runs their own grid; service URLs are configurable.
    OwnGrid,
    /// Connecting to an external grid whose settings come from a named profile
    /// file (e.g. `profiles/osgrid.toml`).
    Custom(String),
}

impl GridProfile {
    /// Parse a profile name string into a [`GridProfile`].
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "standalone" => Self::Standalone,
            "own-grid" | "owngrid" | "own_grid" => Self::OwnGrid,
            other => Self::Custom(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// ServiceUrls
// ---------------------------------------------------------------------------

/// The full set of grid service URLs.  Each field corresponds to a well-known
/// service endpoint from the C# `GridCommon.ini` / `StandaloneCommon.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceUrls {
    pub asset_server_uri: String,
    pub inventory_server_uri: String,
    pub grid_server_uri: String,
    pub avatar_server_uri: String,
    pub presence_server_uri: String,
    pub user_account_server_uri: String,
    pub grid_user_server_uri: String,
    pub authentication_server_uri: String,
    pub friends_server_uri: String,
    pub map_image_server_uri: String,
    pub mute_list_server_uri: String,
    pub estate_server_uri: String,
    pub agent_prefs_server_uri: String,
}

impl ServiceUrls {
    /// All URLs pointing to `http://localhost:{port}`.
    pub fn localhost(port: u16) -> Self {
        let base = format!("http://localhost:{port}");
        Self::all_same(&base)
    }

    /// Every URL set to the same base.
    pub fn all_same(base: &str) -> Self {
        let b = base.trim_end_matches('/').to_string();
        Self {
            asset_server_uri: b.clone(),
            inventory_server_uri: b.clone(),
            grid_server_uri: b.clone(),
            avatar_server_uri: b.clone(),
            presence_server_uri: b.clone(),
            user_account_server_uri: b.clone(),
            grid_user_server_uri: b.clone(),
            authentication_server_uri: b.clone(),
            friends_server_uri: b.clone(),
            map_image_server_uri: b.clone(),
            mute_list_server_uri: b.clone(),
            estate_server_uri: b.clone(),
            agent_prefs_server_uri: b,
        }
    }
}

// ---------------------------------------------------------------------------
// GridProfileConfig
// ---------------------------------------------------------------------------

/// A fully-resolved grid profile: the deployment mode, service URLs, locality
/// flag, and any additional config overrides.
#[derive(Debug, Clone)]
pub struct GridProfileConfig {
    /// Human-readable name, e.g. `"standalone"`, `"osgrid"`.
    pub name: String,
    /// Which deployment mode this profile represents.
    pub profile: GridProfile,
    /// Service endpoint URLs.
    pub service_urls: ServiceUrls,
    /// `true` when services run inside the simulator process.
    pub services_local: bool,
    /// Extra section/key overrides carried by this profile.
    pub overrides: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

impl GridProfileConfig {
    /// Convert the profile into a [`Config`] that can be merged as the
    /// lowest-priority layer.
    pub fn to_config(&self) -> Config {
        let mut cfg = Config::default();
        let urls = &self.service_urls;

        // [AssetService]
        cfg.set("AssetService", "AssetServerURI", &urls.asset_server_uri);
        // [InventoryService]
        cfg.set(
            "InventoryService",
            "InventoryServerURI",
            &urls.inventory_server_uri,
        );
        // [GridService]
        cfg.set("GridService", "GridServerURI", &urls.grid_server_uri);
        // [AvatarService]
        cfg.set("AvatarService", "AvatarServerURI", &urls.avatar_server_uri);
        // [PresenceService]
        cfg.set(
            "PresenceService",
            "PresenceServerURI",
            &urls.presence_server_uri,
        );
        // [UserAccountService]
        cfg.set(
            "UserAccountService",
            "UserAccountServerURI",
            &urls.user_account_server_uri,
        );
        // [GridUserService]
        cfg.set(
            "GridUserService",
            "GridUserServerURI",
            &urls.grid_user_server_uri,
        );
        // [AuthenticationService]
        cfg.set(
            "AuthenticationService",
            "AuthenticationServerURI",
            &urls.authentication_server_uri,
        );
        // [FriendsService]
        cfg.set(
            "FriendsService",
            "FriendsServerURI",
            &urls.friends_server_uri,
        );
        // [MapImageService]
        cfg.set(
            "MapImageService",
            "MapImageServerURI",
            &urls.map_image_server_uri,
        );
        // [MuteListService]
        cfg.set(
            "MuteListService",
            "MuteListServerURI",
            &urls.mute_list_server_uri,
        );
        // [EstateService]
        cfg.set(
            "EstateService",
            "EstateServerURI",
            &urls.estate_server_uri,
        );
        // [AgentPreferencesService]
        cfg.set(
            "AgentPreferencesService",
            "AgentPreferencesServerURI",
            &urls.agent_prefs_server_uri,
        );

        // General metadata
        cfg.set("GridProfile", "Name", &self.name);
        cfg.set_bool("GridProfile", "ServicesLocal", self.services_local);

        // Apply any extra overrides the profile carries.
        let overrides_cfg = Config::from_sections(self.overrides.clone());
        cfg.merge(&overrides_cfg);

        cfg
    }
}

// ---------------------------------------------------------------------------
// load_profile
// ---------------------------------------------------------------------------

/// Load a grid profile by name.
///
/// - `"standalone"` — all services on `http://localhost:9000`.
/// - `"own-grid"` — services on a configurable base URL (defaults to
///   `http://localhost:8003` matching the C# Robust private port).
/// - Anything else — treated as a custom profile; loads from
///   `{profiles_dir}/{name}.toml`.
pub fn load_profile(name: &str, profiles_dir: Option<&Path>) -> Result<GridProfileConfig, ConfigError> {
    match GridProfile::from_name(name) {
        GridProfile::Standalone => Ok(GridProfileConfig {
            name: "standalone".to_string(),
            profile: GridProfile::Standalone,
            service_urls: ServiceUrls::localhost(9000),
            services_local: true,
            overrides: BTreeMap::new(),
        }),

        GridProfile::OwnGrid => Ok(GridProfileConfig {
            name: "own-grid".to_string(),
            profile: GridProfile::OwnGrid,
            service_urls: ServiceUrls::localhost(8003),
            services_local: false,
            overrides: BTreeMap::new(),
        }),

        GridProfile::Custom(ref profile_name) => {
            let dir = profiles_dir.unwrap_or_else(|| Path::new("profiles"));
            let path = dir.join(format!("{profile_name}.toml"));
            load_custom_profile(&path, profile_name, GridProfile::Custom(profile_name.clone()))
        }
    }
}

/// Load a custom profile from a TOML file.
///
/// Expected format:
/// ```toml
/// [GridProfile]
/// Name = "osgrid"
/// ServicesLocal = false
///
/// [AssetService]
/// AssetServerURI = "http://hg.osgrid.org:8003"
/// # ...other service sections...
/// ```
fn load_custom_profile(
    path: &Path,
    name: &str,
    variant: GridProfile,
) -> Result<GridProfileConfig, ConfigError> {
    let cfg = Config::from_toml_file(path)?;

    let base_url = cfg
        .get_string("GridProfile", "BaseURL")
        .unwrap_or("http://localhost:8003");

    let defaults = ServiceUrls::all_same(base_url);

    let services_local = cfg.get_bool_or("GridProfile", "ServicesLocal", false);

    let service_urls = ServiceUrls {
        asset_server_uri: cfg
            .get_string("AssetService", "AssetServerURI")
            .unwrap_or(&defaults.asset_server_uri)
            .to_string(),
        inventory_server_uri: cfg
            .get_string("InventoryService", "InventoryServerURI")
            .unwrap_or(&defaults.inventory_server_uri)
            .to_string(),
        grid_server_uri: cfg
            .get_string("GridService", "GridServerURI")
            .unwrap_or(&defaults.grid_server_uri)
            .to_string(),
        avatar_server_uri: cfg
            .get_string("AvatarService", "AvatarServerURI")
            .unwrap_or(&defaults.avatar_server_uri)
            .to_string(),
        presence_server_uri: cfg
            .get_string("PresenceService", "PresenceServerURI")
            .unwrap_or(&defaults.presence_server_uri)
            .to_string(),
        user_account_server_uri: cfg
            .get_string("UserAccountService", "UserAccountServerURI")
            .unwrap_or(&defaults.user_account_server_uri)
            .to_string(),
        grid_user_server_uri: cfg
            .get_string("GridUserService", "GridUserServerURI")
            .unwrap_or(&defaults.grid_user_server_uri)
            .to_string(),
        authentication_server_uri: cfg
            .get_string("AuthenticationService", "AuthenticationServerURI")
            .unwrap_or(&defaults.authentication_server_uri)
            .to_string(),
        friends_server_uri: cfg
            .get_string("FriendsService", "FriendsServerURI")
            .unwrap_or(&defaults.friends_server_uri)
            .to_string(),
        map_image_server_uri: cfg
            .get_string("MapImageService", "MapImageServerURI")
            .unwrap_or(&defaults.map_image_server_uri)
            .to_string(),
        mute_list_server_uri: cfg
            .get_string("MuteListService", "MuteListServerURI")
            .unwrap_or(&defaults.mute_list_server_uri)
            .to_string(),
        estate_server_uri: cfg
            .get_string("EstateService", "EstateServerURI")
            .unwrap_or(&defaults.estate_server_uri)
            .to_string(),
        agent_prefs_server_uri: cfg
            .get_string("AgentPreferencesService", "AgentPreferencesServerURI")
            .unwrap_or(&defaults.agent_prefs_server_uri)
            .to_string(),
    };

    // Collect remaining sections as overrides (skip service sections we
    // already consumed).
    let consumed: &[&str] = &[
        "GridProfile",
        "AssetService",
        "InventoryService",
        "GridService",
        "AvatarService",
        "PresenceService",
        "UserAccountService",
        "GridUserService",
        "AuthenticationService",
        "FriendsService",
        "MapImageService",
        "MuteListService",
        "EstateService",
        "AgentPreferencesService",
    ];

    let mut overrides = BTreeMap::new();
    for section in cfg.sections() {
        if !consumed.contains(&section) {
            if let Some(keys) = cfg.keys(section) {
                let mut sec_map = BTreeMap::new();
                for key in keys {
                    if let Some(val) = cfg.get_value(section, key) {
                        sec_map.insert(key.to_string(), val.clone());
                    }
                }
                if !sec_map.is_empty() {
                    overrides.insert(section.to_string(), sec_map);
                }
            }
        }
    }

    Ok(GridProfileConfig {
        name: cfg
            .get_string("GridProfile", "Name")
            .unwrap_or(name)
            .to_string(),
        profile: variant,
        service_urls,
        services_local,
        overrides,
    })
}

// ---------------------------------------------------------------------------
// INI importer (Task 1.2 — OSgrid / GridCommon.ini import)
// ---------------------------------------------------------------------------

/// Import grid settings from C#-style INI files (`GridCommon.ini` and
/// optionally `OpenSim.ini`) and produce a [`GridProfileConfig`] for the
/// `"osgrid"` custom profile (or whichever external grid they describe).
///
/// The function extracts the well-known service URL keys from their expected
/// INI sections.  Keys that are absent fall back to a sensible default.
pub fn import_grid_ini(
    grid_common_path: &Path,
    opensim_ini_path: Option<&Path>,
) -> Result<GridProfileConfig, ConfigError> {
    let mut ini_cfg = crate::import_ini(grid_common_path)?;

    if let Some(extra) = opensim_ini_path {
        let extra_cfg = crate::import_ini(extra)?;
        ini_cfg.merge(&extra_cfg);
    }

    // configparser lowercases section names and keys, so look up with
    // lowercase names.  It also preserves surrounding double-quotes in
    // values, which C# INI files commonly use — strip them.
    let get = |section: &str, key: &str| -> Option<String> {
        ini_cfg
            .get_string(&section.to_ascii_lowercase(), &key.to_ascii_lowercase())
            .map(|s| strip_ini_quotes(s).to_string())
    };

    let default_base = "http://localhost:8003".to_string();

    let service_urls = ServiceUrls {
        asset_server_uri: get("assetservice", "assetserveruri")
            .unwrap_or_else(|| default_base.clone()),
        inventory_server_uri: get("inventoryservice", "inventoryserveruri")
            .unwrap_or_else(|| default_base.clone()),
        grid_server_uri: get("gridservice", "gridserveruri")
            .unwrap_or_else(|| default_base.clone()),
        avatar_server_uri: get("avatarservice", "avatarserveruri")
            .unwrap_or_else(|| default_base.clone()),
        presence_server_uri: get("presenceservice", "presenceserveruri")
            .unwrap_or_else(|| default_base.clone()),
        user_account_server_uri: get("useraccountservice", "useraccountserveruri")
            .unwrap_or_else(|| default_base.clone()),
        grid_user_server_uri: get("griduserservice", "griduserserveruri")
            .unwrap_or_else(|| default_base.clone()),
        authentication_server_uri: get("authenticationservice", "authenticationserveruri")
            .unwrap_or_else(|| default_base.clone()),
        friends_server_uri: get("friendsservice", "friendsserveruri")
            .unwrap_or_else(|| default_base.clone()),
        map_image_server_uri: get("mapimageservice", "mapimageserveruri")
            .unwrap_or_else(|| default_base.clone()),
        mute_list_server_uri: get("mutelistservice", "mutelistserveruri")
            .unwrap_or_else(|| default_base.clone()),
        estate_server_uri: get("estateservice", "estateserveruri")
            .unwrap_or_else(|| default_base.clone()),
        agent_prefs_server_uri: get("agentpreferencesservice", "agentpreferencesserveruri")
            .unwrap_or_else(|| default_base.clone()),
    };

    let grid_name = get("gridinfoservice", "gridname")
        .or_else(|| get("gridinfo", "gridname"))
        .unwrap_or_else(|| "imported-grid".to_string());

    Ok(GridProfileConfig {
        name: grid_name,
        profile: GridProfile::Custom("imported".to_string()),
        service_urls,
        services_local: false,
        overrides: BTreeMap::new(),
    })
}

/// Strip surrounding double-quotes that `configparser` preserves from INI
/// values like `GridServerURI = "http://example.com:8003"`.
fn strip_ini_quotes(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Config extensions for profile support (Task 1.3)
// ---------------------------------------------------------------------------

impl Config {
    /// Create a new [`Config`] pre-populated from a grid profile.
    ///
    /// The profile supplies the lowest-priority defaults; callers should then
    /// merge explicit config files, apply env overrides, and resolve
    /// substitutions on top.
    pub fn with_profile(profile: &GridProfileConfig) -> Self {
        profile.to_config()
    }

    /// Set a string value in the config.
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        self.sections
            .entry(section.to_string())
            .or_default()
            .insert(key.to_string(), toml::Value::String(value.to_string()));
    }

    /// Set a boolean value in the config.
    pub fn set_bool(&mut self, section: &str, key: &str, value: bool) {
        self.sections
            .entry(section.to_string())
            .or_default()
            .insert(key.to_string(), toml::Value::Boolean(value));
    }

    /// Build a [`Config`] from pre-built sections.
    pub fn from_sections(sections: BTreeMap<String, BTreeMap<String, toml::Value>>) -> Self {
        Config { sections }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // -- GridProfile::from_name --

    #[test]
    fn parse_standalone_name() {
        assert_eq!(GridProfile::from_name("standalone"), GridProfile::Standalone);
        assert_eq!(GridProfile::from_name("Standalone"), GridProfile::Standalone);
        assert_eq!(GridProfile::from_name("STANDALONE"), GridProfile::Standalone);
    }

    #[test]
    fn parse_own_grid_variants() {
        assert_eq!(GridProfile::from_name("own-grid"), GridProfile::OwnGrid);
        assert_eq!(GridProfile::from_name("owngrid"), GridProfile::OwnGrid);
        assert_eq!(GridProfile::from_name("own_grid"), GridProfile::OwnGrid);
        assert_eq!(GridProfile::from_name("OwnGrid"), GridProfile::OwnGrid);
    }

    #[test]
    fn parse_custom_profile() {
        assert_eq!(
            GridProfile::from_name("osgrid"),
            GridProfile::Custom("osgrid".to_string())
        );
    }

    // -- load_profile: built-in profiles --

    #[test]
    fn load_standalone_profile() {
        let p = load_profile("standalone", None).unwrap();
        assert_eq!(p.name, "standalone");
        assert_eq!(p.profile, GridProfile::Standalone);
        assert!(p.services_local);
        assert_eq!(p.service_urls.grid_server_uri, "http://localhost:9000");
        assert_eq!(p.service_urls.asset_server_uri, "http://localhost:9000");
    }

    #[test]
    fn load_own_grid_profile() {
        let p = load_profile("own-grid", None).unwrap();
        assert_eq!(p.name, "own-grid");
        assert_eq!(p.profile, GridProfile::OwnGrid);
        assert!(!p.services_local);
        assert_eq!(p.service_urls.grid_server_uri, "http://localhost:8003");
    }

    // -- load_profile: custom TOML --

    #[test]
    fn load_custom_toml_profile() {
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join("testgrid.toml");
        let mut f = std::fs::File::create(&toml_path).unwrap();
        writeln!(
            f,
            r#"
[GridProfile]
Name = "TestGrid"
ServicesLocal = false
BaseURL = "http://grid.example.com:8003"

[GridService]
GridServerURI = "http://grid.example.com:9000"

[LoginService]
WelcomeMessage = "Hello!"
"#
        )
        .unwrap();

        let p = load_profile("testgrid", Some(dir.path())).unwrap();
        assert_eq!(p.name, "TestGrid");
        assert!(!p.services_local);
        // GridServerURI overridden explicitly
        assert_eq!(
            p.service_urls.grid_server_uri,
            "http://grid.example.com:9000"
        );
        // Others fall back to BaseURL
        assert_eq!(
            p.service_urls.asset_server_uri,
            "http://grid.example.com:8003"
        );
        // Extra sections captured as overrides
        assert!(p.overrides.contains_key("LoginService"));
    }

    // -- to_config round-trip --

    #[test]
    fn profile_to_config_sets_service_urls() {
        let p = load_profile("standalone", None).unwrap();
        let cfg = p.to_config();
        assert_eq!(
            cfg.get_string("GridService", "GridServerURI"),
            Some("http://localhost:9000")
        );
        assert_eq!(
            cfg.get_string("AssetService", "AssetServerURI"),
            Some("http://localhost:9000")
        );
        assert_eq!(cfg.get_bool("GridProfile", "ServicesLocal"), Some(true));
    }

    // -- Config::with_profile --

    #[test]
    fn with_profile_creates_prepopulated_config() {
        let p = load_profile("standalone", None).unwrap();
        let cfg = Config::with_profile(&p);
        assert_eq!(
            cfg.get_string("GridService", "GridServerURI"),
            Some("http://localhost:9000")
        );
    }

    // -- Override chain: profile < config file < env --

    #[test]
    fn override_chain_profile_then_file_then_env() {
        // 1. Start with standalone profile
        let profile = load_profile("standalone", None).unwrap();
        let mut cfg = Config::with_profile(&profile);

        // Profile default
        assert_eq!(
            cfg.get_string("GridService", "GridServerURI"),
            Some("http://localhost:9000")
        );

        // 2. Config file layer overrides the profile
        let mut file_layer = Config::default();
        file_layer.set(
            "GridService",
            "GridServerURI",
            "http://mygrid.example.com:8003",
        );
        cfg.merge(&file_layer);
        assert_eq!(
            cfg.get_string("GridService", "GridServerURI"),
            Some("http://mygrid.example.com:8003")
        );

        // 3. Env var layer overrides both
        // We'll simulate by directly setting — the real mechanism uses
        // apply_env_overrides() which reads OPENSIM__section__key.
        cfg.set(
            "GridService",
            "GridServerURI",
            "http://env-override.example.com:8003",
        );
        assert_eq!(
            cfg.get_string("GridService", "GridServerURI"),
            Some("http://env-override.example.com:8003")
        );
    }

    #[test]
    fn env_overrides_beat_profile() {
        // Use the real env-var mechanism
        let profile = load_profile("standalone", None).unwrap();
        let mut cfg = Config::with_profile(&profile);

        // Inject a fake env var
        unsafe {
            std::env::set_var(
                "OPENSIM__GridService__GridServerURI",
                "http://from-env.example.com:9999",
            );
        }
        cfg.apply_env_overrides();
        assert_eq!(
            cfg.get_string("GridService", "GridServerURI"),
            Some("http://from-env.example.com:9999")
        );

        // Clean up
        unsafe {
            std::env::remove_var("OPENSIM__GridService__GridServerURI");
        }
    }

    // -- INI import --

    #[test]
    fn import_grid_common_ini() {
        let dir = tempfile::tempdir().unwrap();
        let ini_path = dir.path().join("GridCommon.ini");
        let mut f = std::fs::File::create(&ini_path).unwrap();
        writeln!(
            f,
            r#"
[GridService]
    GridServerURI = "http://hg.osgrid.org:8003"

[AssetService]
    AssetServerURI = "http://hg.osgrid.org:8003"

[InventoryService]
    InventoryServerURI = "http://hg.osgrid.org:8003"

[GridInfoService]
    gridname = "OSgrid"
"#
        )
        .unwrap();

        let p = import_grid_ini(&ini_path, None).unwrap();
        assert_eq!(p.name, "OSgrid");
        assert!(!p.services_local);
        assert_eq!(
            p.service_urls.grid_server_uri,
            "http://hg.osgrid.org:8003"
        );
        assert_eq!(
            p.service_urls.asset_server_uri,
            "http://hg.osgrid.org:8003"
        );
        assert_eq!(
            p.service_urls.inventory_server_uri,
            "http://hg.osgrid.org:8003"
        );
        // Unspecified URLs fall back to default
        assert_eq!(
            p.service_urls.friends_server_uri,
            "http://localhost:8003"
        );
    }

    #[test]
    fn import_with_opensim_ini_overlay() {
        let dir = tempfile::tempdir().unwrap();

        let grid_path = dir.path().join("GridCommon.ini");
        let mut f = std::fs::File::create(&grid_path).unwrap();
        writeln!(
            f,
            r#"
[GridService]
    GridServerURI = "http://grid.example.com:8003"
"#
        )
        .unwrap();

        let osim_path = dir.path().join("OpenSim.ini");
        let mut f2 = std::fs::File::create(&osim_path).unwrap();
        writeln!(
            f2,
            r#"
[GridService]
    GridServerURI = "http://override.example.com:9999"
"#
        )
        .unwrap();

        let p = import_grid_ini(&grid_path, Some(&osim_path)).unwrap();
        // The OpenSim.ini values merge on top
        assert_eq!(
            p.service_urls.grid_server_uri,
            "http://override.example.com:9999"
        );
    }

    // -- ServiceUrls helpers --

    #[test]
    fn service_urls_localhost() {
        let urls = ServiceUrls::localhost(9000);
        assert_eq!(urls.asset_server_uri, "http://localhost:9000");
        assert_eq!(urls.grid_server_uri, "http://localhost:9000");
    }

    #[test]
    fn service_urls_all_same_trims_trailing_slash() {
        let urls = ServiceUrls::all_same("http://example.com:8003/");
        assert_eq!(urls.asset_server_uri, "http://example.com:8003");
        assert_eq!(urls.friends_server_uri, "http://example.com:8003");
    }

    // -- Config::set / set_bool --

    #[test]
    fn config_set_and_set_bool() {
        let mut cfg = Config::default();
        cfg.set("Test", "Key", "value");
        cfg.set_bool("Test", "Flag", true);
        assert_eq!(cfg.get_string("Test", "Key"), Some("value"));
        assert_eq!(cfg.get_bool("Test", "Flag"), Some(true));
    }

    // -- Config::from_sections --

    #[test]
    fn config_from_sections_round_trip() {
        let mut sections = BTreeMap::new();
        let mut kv = BTreeMap::new();
        kv.insert("hello".to_string(), toml::Value::String("world".to_string()));
        sections.insert("General".to_string(), kv);

        let cfg = Config::from_sections(sections);
        assert_eq!(cfg.get_string("General", "hello"), Some("world"));
    }
}
