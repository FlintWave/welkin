// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use clap::Parser;
use tracing::{error, info, warn};

use opensim_config::Config;
use opensim_data::DataPool;
use opensim_scene::scene::{RegionInfo, Scene, SceneConfig};

/// OpenSimulator Region Simulator
#[derive(Parser, Debug)]
#[command(name = "opensim-region", about = "OpenSimulator region simulator")]
struct Cli {
    /// Path to the main configuration file
    #[arg(long, default_value = "opensim.toml")]
    config: PathBuf,

    /// Path to an optional region-specific configuration file
    #[arg(long)]
    region_config: Option<PathBuf>,
}

/// Bridges between the raw Config and the typed RegionInfo / SceneConfig used by
/// the scene graph.
struct RegionConfig {
    region_name: String,
    region_loc_x: u32,
    region_loc_y: u32,
    external_host_name: String,
    internal_port: u16,
    database_url: String,
    grid_service_url: Option<String>,
    scene_config: SceneConfig,
}

impl RegionConfig {
    fn from_config(config: &Config) -> Self {
        let region_name = config
            .get_string_or("Region", "RegionName", "My Region");
        let region_loc_x = config.get_i64_or("Region", "RegionLocX", 1000) as u32;
        let region_loc_y = config.get_i64_or("Region", "RegionLocY", 1000) as u32;
        let external_host_name = config
            .get_string_or("Network", "ExternalHostName", "127.0.0.1");
        let internal_port = config.get_i64_or("Network", "InternalPort", 9000) as u16;

        let database_url = config
            .get_string_or("DatabaseService", "ConnectionString", "sqlite:region.db");

        let grid_service_url = config
            .get_string("GridService", "GridServerURI")
            .map(|s| s.to_string());

        let mut scene_config = SceneConfig::default();
        scene_config.physics_enabled =
            config.get_bool_or("Scene", "PhysicsEnabled", scene_config.physics_enabled);
        scene_config.scripts_enabled =
            config.get_bool_or("Scene", "ScriptsEnabled", scene_config.scripts_enabled);
        scene_config.physical_prims =
            config.get_bool_or("Scene", "PhysicalPrims", scene_config.physical_prims);
        scene_config.collidable_prims =
            config.get_bool_or("Scene", "CollidablePrims", scene_config.collidable_prims);
        scene_config.max_nonphys_size =
            config.get_f64_or("Scene", "MaxNonPhysSize", scene_config.max_nonphys_size as f64) as f32;
        scene_config.max_phys_size =
            config.get_f64_or("Scene", "MaxPhysSize", scene_config.max_phys_size as f64) as f32;
        scene_config.max_prims_per_frame =
            config.get_i64_or("Scene", "MaxPrimsPerFrame", scene_config.max_prims_per_frame as i64) as i32;
        scene_config.default_draw_distance =
            config.get_f64_or("Scene", "DefaultDrawDistance", scene_config.default_draw_distance as f64) as f32;
        scene_config.max_draw_distance =
            config.get_f64_or("Scene", "MaxDrawDistance", scene_config.max_draw_distance as f64) as f32;
        scene_config.frame_time =
            config.get_f64_or("Scene", "FrameTime", scene_config.frame_time as f64) as f32;

        Self {
            region_name,
            region_loc_x,
            region_loc_y,
            external_host_name,
            internal_port,
            database_url,
            grid_service_url,
            scene_config,
        }
    }

    fn build_region_info(&self) -> RegionInfo {
        let mut info = RegionInfo::new(&self.region_name, self.region_loc_x, self.region_loc_y);
        info.external_host_name = self.external_host_name.clone();
        info.internal_port = self.internal_port;
        info
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = run().await {
        error!("Region simulator failed: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // --- Config loading ---
    info!(path = %cli.config.display(), "Loading main configuration");
    let mut config = Config::from_toml_file(&cli.config)?;

    if let Some(ref region_path) = cli.region_config {
        info!(path = %region_path.display(), "Loading region configuration");
        let region_cfg = Config::from_toml_file(region_path)?;
        config.merge(&region_cfg);
    }

    config.apply_env_overrides();
    config.resolve_substitutions()?;

    // --- Extract typed config ---
    let region_config = RegionConfig::from_config(&config);

    info!(
        region = %region_config.region_name,
        loc_x = region_config.region_loc_x,
        loc_y = region_config.region_loc_y,
        port = region_config.internal_port,
        host = %region_config.external_host_name,
        "Region configuration loaded"
    );

    // --- Database init ---
    info!(url = %redact_connection_string(&region_config.database_url), "Connecting to database");
    let _pool = DataPool::connect(&region_config.database_url).await?;
    info!("Database connection established");

    // --- Scene creation ---
    let region_info = region_config.build_region_info();
    info!(
        region_id = %region_info.region_id,
        handle = region_info.region_handle,
        "Creating scene"
    );
    let scene = Scene::new(region_info, region_config.scene_config);
    scene.start();

    // --- Grid service registration (stub) ---
    match region_config.grid_service_url {
        Some(ref url) => {
            info!(
                url = %url,
                region = %scene.region_info.region_name,
                "Grid service registration would happen here (not yet implemented)"
            );
        }
        None => {
            warn!("No GridService URL configured; skipping grid registration");
        }
    }

    info!(
        region = %scene.region_info.region_name,
        "Region simulator started successfully"
    );

    // The actual event loop will be wired in a later task.
    // For now, we just demonstrate that the startup pipeline works.

    scene.shutdown();
    info!("Region simulator shut down");

    Ok(())
}

/// Redacts credentials from a connection string for safe logging.
fn redact_connection_string(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            return format!("{}://***@{}", &url[..scheme_end], &url[at + 1..]);
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_scene_config() {
        let sc = SceneConfig::default();
        assert!(sc.physics_enabled);
        assert!(sc.scripts_enabled);
        assert!(sc.physical_prims);
        assert!(sc.collidable_prims);
        assert!(sc.max_nonphys_size > 0.0);
        assert!(sc.max_phys_size > 0.0);
        assert!(sc.max_nonphys_size > sc.max_phys_size);
        assert!(sc.default_draw_distance > 0.0);
        assert!(sc.frame_time > 0.0);
        assert!(sc.persist_min_seconds > 0);
        assert!(sc.persist_max_seconds > sc.persist_min_seconds);
    }

    #[test]
    fn test_region_info_from_config() {
        let info = RegionInfo::new("Test Region", 1000, 2000);
        assert_eq!(info.region_name, "Test Region");
        assert_eq!(info.region_loc_x, 1000);
        assert_eq!(info.region_loc_y, 2000);
        // Handle = (loc_x << 32) | loc_y
        let expected_handle = (1000u64 << 32) | 2000u64;
        assert_eq!(info.region_handle, expected_handle);
        assert_eq!(info.region_size_x, 256);
        assert_eq!(info.region_size_y, 256);
    }

    #[test]
    fn test_config_database_default() {
        let config = Config::default();
        let db_url = config
            .get_string_or("DatabaseService", "ConnectionString", "sqlite:region.db");
        assert_eq!(db_url, "sqlite:region.db");
    }

    #[test]
    fn test_region_config_defaults() {
        let config = Config::default();
        let rc = RegionConfig::from_config(&config);
        assert_eq!(rc.region_name, "My Region");
        assert_eq!(rc.region_loc_x, 1000);
        assert_eq!(rc.region_loc_y, 1000);
        assert_eq!(rc.external_host_name, "127.0.0.1");
        assert_eq!(rc.internal_port, 9000);
        assert_eq!(rc.database_url, "sqlite:region.db");
        assert!(rc.grid_service_url.is_none());
    }

    #[test]
    fn test_redact_connection_string() {
        assert_eq!(
            redact_connection_string("mysql://user:pass@localhost/db"),
            "mysql://***@localhost/db"
        );
        assert_eq!(
            redact_connection_string("sqlite:region.db"),
            "sqlite:region.db"
        );
    }

    #[test]
    fn test_build_region_info() {
        let config = Config::default();
        let rc = RegionConfig::from_config(&config);
        let info = rc.build_region_info();
        assert_eq!(info.region_name, "My Region");
        assert_eq!(info.region_loc_x, 1000);
        assert_eq!(info.region_loc_y, 1000);
        assert_eq!(info.external_host_name, "127.0.0.1");
        assert_eq!(info.internal_port, 9000);
        assert!(!info.region_id.is_nil());
    }
}
