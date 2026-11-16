// SPDX-License-Identifier: LGPL-3.0-or-later

mod logging;
mod avatar_move;
mod console;
mod console_io;
mod object_add;
mod persist;
mod physics_cmd;
mod script_persist;
mod scene_sync;
mod sim_stats;
mod sound_io;
mod terraform;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use axum::body::Bytes;
use axum::extract::State;
use axum::response::IntoResponse;

use welkin_config::Config;
use welkin_data::DataPool;
use welkin_llsd::{Llsd, Map, from_xml, to_xml_string};
use welkin_modules::groups::messaging::ChatSessionState;
use welkin_modules::{
    AccessModule, AssetTransactionModule, AttachmentsModule, AvatarCombatModule,
    AvatarFactoryModule, BakedTexturesModule, BasicProfileModule, CapsModule, ChatModule,
    DefaultPermissionsModule, DialogModule, DwellModule, DynAttrsModule, EstateManagementModule,
    EntityTransferModule, FriendsModule, GesturesModule, GodsModule, GroupsMessagingModule,
    GroupsModule, HGEntityTransferModule, HGFriendsModule, HGInventoryAccessModule,
    HGLureModule, HGMessageTransferModule, HGUserManagementModule, HGWorldMapModule,
    IPBanModule, InventoryAccessModule, InventoryTransferModule, LandManagementModule,
    LibraryModule, LightShareModule, LureModule, MapImageModule, MapSearchModule,
    MessageTransferModule, ModuleConfig, MonitorModule, MuteListModule, NPCModule,
    OfflineMessageModule, PresenceModule, PrimCountModule, RegionReadyModule, RestartModule,
    SceneRef, SearchModule, ServiceThrottleModule, SoundModule, UserManagementModule,
    WebRtcVoiceModule, WindModule, WorldMapModule, XferModule,
};
use welkin_msg::Msg;
use welkin_msg::msgs::*;
use welkin_msg::objects::{FullObjectUpdate, encode_terse};
use welkin_net::dispatch::{HandleError, PacketContext, PacketDispatcher};
use welkin_prim::{TextureEntry, pack_texture_entry};
use welkin_net::server::ClientSession;
use welkin_scene::group::SceneObjectGroup;
use welkin_scene::part::SceneObjectPart;
use welkin_scene::presence::{PresenceType, ScenePresence};
use welkin_scene::scene::{RegionInfo as SceneRegionInfo, Scene, SceneConfig};
use welkin_msg::welkin_types::PermissionMask;
use welkin_script_vm::{ScriptEvent, ScriptScheduler};

/// OpenSimulator Region Simulator
#[derive(Parser, Debug)]
#[command(name = "welkin-region", about = "OpenSimulator region simulator")]
struct Cli {
    /// Path to the main configuration file
    #[arg(long, default_value = "welkin.toml")]
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
        let region_name = config.get_string_or("Region", "RegionName", "My Region");
        let region_loc_x = config.get_i64_or("Region", "RegionLocX", 1000) as u32;
        let region_loc_y = config.get_i64_or("Region", "RegionLocY", 1000) as u32;
        let external_host_name = config.get_string_or("Network", "ExternalHostName", "127.0.0.1");
        let internal_port = config.get_i64_or("Network", "InternalPort", 9000) as u16;

        let database_url =
            config.get_string_or("DatabaseService", "RegionConnectionString",
                &config.get_string_or("DatabaseService", "ConnectionString", "sqlite:region.db"));

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
        scene_config.max_nonphys_size = config.get_f64_or(
            "Scene",
            "MaxNonPhysSize",
            scene_config.max_nonphys_size as f64,
        ) as f32;
        scene_config.max_phys_size =
            config.get_f64_or("Scene", "MaxPhysSize", scene_config.max_phys_size as f64) as f32;
        scene_config.max_prims_per_frame = config.get_i64_or(
            "Scene",
            "MaxPrimsPerFrame",
            scene_config.max_prims_per_frame as i64,
        ) as i32;
        scene_config.default_draw_distance = config.get_f64_or(
            "Scene",
            "DefaultDrawDistance",
            scene_config.default_draw_distance as f64,
        ) as f32;
        scene_config.max_draw_distance = config.get_f64_or(
            "Scene",
            "MaxDrawDistance",
            scene_config.max_draw_distance as f64,
        ) as f32;
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

    fn build_region_info(&self) -> SceneRegionInfo {
        let mut info = SceneRegionInfo::new(&self.region_name, self.region_loc_x, self.region_loc_y);
        info.external_host_name = self.external_host_name.clone();
        info.internal_port = self.internal_port;
        info
    }
}

struct ConfigBridge<'a>(&'a Config);

impl ModuleConfig for ConfigBridge<'_> {
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

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Region simulator failed: {e}");
        error!("Region simulator failed: {e}");
        std::process::exit(1);
    }
    // The console REPL blocks in a terminal read that cannot be cancelled; exit
    // explicitly so a TTY run shuts down instead of waiting on readline. Logging
    // guards were dropped when run() returned, so buffered logs are flushed.
    std::process::exit(0);
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

    // --- Logging (file rotation + console) ---
    let _log_guard = logging::init(&config);

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
    let pool = DataPool::connect(&region_config.database_url).await?;
    script_persist::create_script_states_table(pool.inner()).await?;
    let sim_store = std::sync::Arc::new(welkin_data::simulation::SimulationDataStore::new(pool.inner().clone()));
    info!("Database connection established");

    // --- Scene creation ---
    let region_info = region_config.build_region_info();
    info!(
        region_id = %region_info.region_id,
        handle = region_info.region_handle,
        "Creating scene"
    );
    let frame_time = region_config.scene_config.frame_time;
    let physics_enabled = region_config.scene_config.physics_enabled;
    let region_id_str = region_info.region_id.to_string();
    let scene = Arc::new(Scene::new(region_info, region_config.scene_config));
    scene.start();

    if physics_enabled {
        let physics =
            welkin_physics_rapier::RapierPhysicsScene::new(&scene.region_info.region_name);
        scene.set_physics(Box::new(physics));
        // Flat terrain at the default height: the same ground viewers are sent in
        // `send_movement_complete`, so physics, avatars and the viewer agree.
        let height_map = vec![welkin_scene::terrain::DEFAULT_TERRAIN_HEIGHT; 256 * 256];
        scene.set_terrain(&height_map);
        info!("Physics engine (Rapier) initialised with terrain heightfield");
    }

    // --- Module system ---
    let mut scene_ref = SceneRef::new(
        scene.region_info.region_id,
        scene.region_info.region_name.clone(),
    );
    scene_ref.region_handle = scene.region_info.region_handle;
    scene_ref.region_size_x = scene.region_info.region_size_x;
    scene_ref.region_size_y = scene.region_info.region_size_y;
    {
        let mut registry = scene_ref.registry.write();
        registry.register(ChatModule::new());
        registry.register(DefaultPermissionsModule::new());
        registry.register(AvatarFactoryModule::new());
        registry.register(AttachmentsModule::new());
        registry.register(EstateManagementModule::new());
        registry.register(LandManagementModule::new());
        registry.register(CapsModule::new());
        registry.register(DynAttrsModule::new());
        registry.register(ServiceThrottleModule::new());
        registry.register(UserManagementModule::new());
        registry.register(BakedTexturesModule::new());
        registry.register(WorldMapModule::new());
        registry.register(MapImageModule::new());
        registry.register(MapSearchModule::new());
        registry.register(FriendsModule::new());
        registry.register(DialogModule::new());
        registry.register(InventoryAccessModule::new());
        registry.register(InventoryTransferModule::new());
        registry.register(AssetTransactionModule::new());
        registry.register(XferModule::new());
        registry.register(MessageTransferModule::new());
        registry.register(OfflineMessageModule::new());
        registry.register(MuteListModule::new());
        registry.register(PresenceModule::new());
        registry.register(SoundModule::new());
        registry.register(BasicProfileModule::new());
        registry.register(LibraryModule::new());
        registry.register(SearchModule::new());
        registry.register(GesturesModule::new());
        registry.register(LureModule::new());
        registry.register(GodsModule::new());
        registry.register(GroupsModule::new());
        registry.register(GroupsMessagingModule::new());
        registry.register(NPCModule::new());
        registry.register(WebRtcVoiceModule::new());
        registry.register(WindModule::new());
        registry.register(LightShareModule::new());
        registry.register(AvatarCombatModule::new());
        registry.register(PrimCountModule::new());
        registry.register(DwellModule::new());
        registry.register(MonitorModule::new());
        registry.register(IPBanModule::new());
        registry.register(RegionReadyModule::new());
        registry.register(RestartModule::new());
        registry.register(AccessModule::new());
        registry.register(EntityTransferModule::new());
        registry.register(HGEntityTransferModule::new());
        registry.register(HGFriendsModule::new());
        registry.register(HGInventoryAccessModule::new());
        registry.register(HGLureModule::new());
        registry.register(HGMessageTransferModule::new());
        registry.register(HGUserManagementModule::new());
        registry.register(HGWorldMapModule::new());
        if let Err(e) = registry.initialise_all(&ConfigBridge(&config)) {
            warn!(error = %e, "module initialisation failed");
        }
        if let Err(e) = registry.add_region_all(&scene_ref) {
            warn!(error = %e, "module add_region failed");
        }
        info!(count = registry.module_count(), "modules initialised");
    }

    // --- Script scheduler ---
    let script_scheduler = Arc::new(parking_lot::Mutex::new(ScriptScheduler::new(10_000)));
    let scripts_enabled = scene.config.scripts_enabled;
    info!(scripts_enabled, "script scheduler initialised");

    // Restore persisted script states from DB
    if scripts_enabled {
        match script_persist::load_script_states(pool.inner()).await {
            Ok(states) if !states.is_empty() => {
                let mut sched = script_scheduler.lock();
                let mut restored = 0usize;
                for (script_id, prim_id, state) in &states {
                    match welkin_script_vm::ScriptInstance::restore_from(
                        *script_id, *prim_id, state, 10_000,
                    ) {
                        Ok(instance) => {
                            sched.add_script(instance);
                            sched.post_event(*script_id, ScriptEvent::StateEntry);
                            restored += 1;
                        }
                        Err(e) => {
                            warn!(script = %script_id, error = %e, "failed to restore script state");
                        }
                    }
                }
                info!(restored, total = states.len(), "restored script states from DB");
            }
            Ok(_) => {}
            Err(e) => warn!(error = %e, "failed to load script states from DB"),
        }
    }

    // Event subscriber tasks are spawned below, after the CancellationToken.

    // --- Grid service registration ---
    let (grid_reg, grid_connector_opt) = match region_config.grid_service_url {
        Some(ref url) => {
            let caps_port = region_config.internal_port.wrapping_add(1);
            let server_uri = format!("http://{}:{}/", region_config.external_host_name, caps_port);
            let zero_uuid = "00000000-0000-0000-0000-000000000000";
            let loc_x = (region_config.region_loc_x * 256) as i32;
            let loc_y = (region_config.region_loc_y * 256) as i32;
            let region_row = welkin_data::grid::RegionRow {
                uuid: scene.region_info.region_id.to_string(),
                region_handle: (i64::from(loc_x) << 32) | i64::from(loc_y),
                region_name: region_config.region_name.clone(),
                region_secret: String::new(),
                server_ip: region_config.external_host_name.clone(),
                server_port: Some(region_config.internal_port as i32),
                server_uri,
                loc_x,
                loc_y,
                loc_z: 0,
                region_map_texture: zero_uuid.to_string(),
                server_http_port: Some(caps_port as i32),
                owner_uuid: zero_uuid.to_string(),
                access: 1,
                scope_id: zero_uuid.to_string(),
                size_x: 256,
                size_y: 256,
                flags: 9,
                last_seen: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i32)
                    .unwrap_or(0),
                principal_id: zero_uuid.to_string(),
                token: Uuid::new_v4().to_string(),
                parcel_map_texture: zero_uuid.to_string(),
            };

            let connector = Arc::new(welkin_services::remote::grid::RemoteGridConnector::new(url));
            let grid_conn_for_dispatch = connector.clone();
            let registration = welkin_services::registration::GridRegistration::new(
                connector,
                region_row,
                reqwest::Client::new(),
            );
            match registration.startup().await {
                Ok(()) => info!(region = %scene.region_info.region_name, "Registered with grid"),
                Err(e) => warn!(error = %e, "Failed to register region with grid"),
            }
            (Some(registration), Some(grid_conn_for_dispatch))
        }
        None => {
            warn!("No GridService URL configured; skipping grid registration");
            (None, None)
        }
    };
    let grid_connector: Option<Arc<welkin_services::remote::grid::RemoteGridConnector>> = grid_connector_opt;

    // --- Cancellation token for coordinated shutdown ---
    let cancel = CancellationToken::new();

    // --- LLUDP server ---
    let udp_addr = SocketAddr::from(([0, 0, 0, 0], region_config.internal_port));
    let udp_server = welkin_net::server::LLUDPServer::bind(udp_addr).await?;

    let udp_socket = udp_server.socket().clone();
    let udp_clients = udp_server.clients().clone();

    let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel();
    let udp_cancel = cancel.clone();
    let udp_handle = tokio::spawn(async move {
        udp_server.run(msg_tx, udp_cancel).await;
    });

    info!(addr = %udp_addr, "LLUDP receive loop started");

    // --- Per-session EventQueue map ---
    let event_queues: EventQueueMap = Arc::new(RwLock::new(std::collections::HashMap::new()));

    // --- Caps base URL (used by both process_messages and the caps server) ---
    let caps_port = region_config.internal_port.wrapping_add(1);
    let caps_addr = SocketAddr::from(([0, 0, 0, 0], caps_port));
    let caps_base = format!("http://{}:{}", region_config.external_host_name, caps_port);

    // --- Message processing loop ---
    let mut dispatcher = PacketDispatcher::new();
    welkin_net::handlers::register_core_handlers(&mut dispatcher);

    let process_cancel = cancel.clone();
    let process_scene = Arc::clone(&scene);
    let process_socket = udp_socket.clone();
    let process_clients = udp_clients.clone();
    let process_eqs = event_queues.clone();
    let process_caps_base = caps_base.clone();
    let process_scheduler = Arc::clone(&script_scheduler);
    let process_grid_connector = grid_connector.clone();
    let perms = Arc::new(DefaultPermissionsModule::new());
    let process_perms = Arc::clone(&perms);
    let accounts_url = region_config
        .grid_service_url
        .as_deref()
        .unwrap_or("http://localhost:8003");
    let accounts_connector = Arc::new(
        welkin_services::remote::accounts::RemoteAccountsConnector::new(accounts_url),
    );
    let process_accounts = Arc::clone(&accounts_connector);
    let process_handle = tokio::spawn(async move {
        process_messages(
            msg_rx,
            dispatcher,
            process_socket,
            process_clients,
            process_scene,
            process_eqs,
            process_caps_base,
            process_cancel,
            process_scheduler,
            scripts_enabled,
            process_grid_connector,
            process_perms,
            process_accounts,
        )
        .await;
    });

    let cap_server = welkin_caps::server::CapabilityServer::new();

    let asset_url = region_config
        .grid_service_url
        .as_deref()
        .unwrap_or("http://localhost:8003");
    let http_client = reqwest::Client::new();
    let asset_state = welkin_caps::handlers::AssetServiceState {
        client: http_client.clone(),
        asset_url: asset_url.to_string(),
    };
    let inv_state = welkin_caps::handlers::InventoryProxyState {
        client: http_client.clone(),
        inventory_url: asset_url.to_string(),
    };
    let bake_state = welkin_caps::handlers::BakeUploadState {
        client: http_client,
        asset_url: asset_url.to_string(),
        caps_url: format!("http://localhost:{}", caps_addr.port()),
        pending: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    // --- Script load channel: cap handler → scheduler ---
    let (script_load_tx, script_load_rx) =
        std::sync::mpsc::channel::<welkin_caps::handlers::ScriptLoadRequest>();
    let script_load_state = welkin_caps::handlers::ScriptLoadState {
        sender: script_load_tx,
    };
    let script_upload_state = welkin_caps::handlers::ScriptUploadState {
        script_load: script_load_state,
        caps_url: format!("http://localhost:{}", caps_addr.port()),
        pending: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let mut caps_router = cap_server.into_router();
    for (name, method_router) in
        welkin_caps::handlers::cap_routes(asset_state, inv_state, bake_state.clone(), script_upload_state.clone())
    {
        caps_router = caps_router.route(&format!("/{name}"), method_router);
    }
    caps_router = caps_router.merge(welkin_caps::handlers::bake_upload_route(bake_state));
    caps_router = caps_router.merge(welkin_caps::handlers::script_upload_route(script_upload_state));
    caps_router = caps_router.route(
        "/EventQueueGet",
        axum::routing::post(welkin_caps::handlers::event_queue_get)
            .with_state(event_queues.clone()),
    );
    caps_router = caps_router.route(
        "/CAPS/{*path}",
        axum::routing::post(seed_caps_handler).with_state(caps_base.clone()),
    );
    caps_router = caps_router
        .route("/currency.php", axum::routing::post(currency_php_handler))
        .route("/landtool.php", axum::routing::post(landtool_php_handler));

    let caps_listener = tokio::net::TcpListener::bind(caps_addr).await?;
    info!(addr = %caps_addr, "Capability HTTP server listening");

    let caps_cancel = cancel.clone();
    let caps_handle = tokio::spawn(async move {
        axum::serve(caps_listener, caps_router)
            .with_graceful_shutdown(async move {
                caps_cancel.cancelled().await;
            })
            .await
            .ok();
    });

    // --- Script event subscribers ---
    // Money events → LSL money() handler
    {
        let sched = Arc::clone(&script_scheduler);
        let mut money_rx = scene_ref.events.on_money_received.subscribe();
        let money_cancel = cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = money_cancel.cancelled() => break,
                    Ok(evt) = money_rx.recv() => {
                        sched.lock().post_event_to_prim(
                            evt.object_id,
                            ScriptEvent::Money { agent_id: evt.agent_id, amount: evt.amount },
                        );
                    }
                }
            }
        });
    }
    // Changed events: object added → CHANGED_REGION (0x100) to scripts on that object
    {
        let sched = Arc::clone(&script_scheduler);
        let mut added_rx = scene_ref.events.on_object_added.subscribe();
        let added_cancel = cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = added_cancel.cancelled() => break,
                    Ok(evt) = added_rx.recv() => {
                        sched.lock().post_event_to_prim(
                            evt.object_id,
                            ScriptEvent::Changed { change: 0x100 },
                        );
                    }
                }
            }
        });
    }
    // Land change → CHANGED_REGION to all scripts
    {
        let sched = Arc::clone(&script_scheduler);
        let mut land_rx = scene_ref.events.on_land_change.subscribe();
        let land_cancel = cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = land_cancel.cancelled() => break,
                    Ok(_evt) = land_rx.recv() => {
                        sched.lock().broadcast_event(
                            ScriptEvent::Changed { change: 0x100 },
                        );
                    }
                }
            }
        });
    }
    // Object removal → clean up scripts
    {
        let sched = Arc::clone(&script_scheduler);
        let mut removed_rx = scene_ref.events.on_object_removed.subscribe();
        let removed_cancel = cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = removed_cancel.cancelled() => break,
                    Ok(evt) = removed_rx.recv() => {
                        let mut s = sched.lock();
                        let to_remove = s.scripts_on_prim(&evt.object_id);
                        for sid in to_remove {
                            s.remove_script(sid);
                        }
                    }
                }
            }
        });
    }

    // --- Link message channel for builtin → scheduler event routing ---
    let (link_msg_tx, link_msg_rx) =
        std::sync::mpsc::channel::<(Uuid, ScriptEvent)>();
    let heartbeat_event_tx = link_msg_tx.clone();
    let _link_msg_tx = Arc::new(link_msg_tx);

    // --- Timer command channel: scripts → scheduler set_timer ---
    let (timer_cmd_tx, timer_cmd_rx) =
        std::sync::mpsc::channel::<(Uuid, f64)>();
    let heartbeat_timer_tx = timer_cmd_tx.clone();
    let _timer_cmd_tx = Arc::new(timer_cmd_tx);

    // --- Script chat channel: scripts → clients + listen events ---
    let (script_chat_tx, script_chat_rx) =
        std::sync::mpsc::channel::<welkin_script_vm::ChatMessage>();
    let heartbeat_chat_tx = script_chat_tx.clone();
    let _script_chat_tx = Arc::new(script_chat_tx);

    // --- Sensor request channel: scripts → heartbeat sensor scan ---
    let (sensor_cmd_tx, sensor_cmd_rx) =
        std::sync::mpsc::channel::<welkin_script_vm::SensorRequest>();
    let heartbeat_sensor_tx = sensor_cmd_tx.clone();
    let _sensor_cmd_tx = Arc::new(sensor_cmd_tx);

    // --- Control request channel: scripts → control event tracking ---
    let (control_cmd_tx, control_cmd_rx) =
        std::sync::mpsc::channel::<welkin_script_vm::ControlRequest>();
    let heartbeat_control_tx = control_cmd_tx.clone();
    let _control_cmd_tx = Arc::new(control_cmd_tx);

    // --- Sound command channel: scripts → SoundTrigger/AttachedSound packets ---
    let (sound_cmd_tx, sound_cmd_rx) =
        std::sync::mpsc::channel::<welkin_script_vm::builtins::SoundCommand>();
    let heartbeat_sound_tx = sound_cmd_tx.clone();
    let _sound_cmd_tx = Arc::new(sound_cmd_tx);

    // --- Physics command channel: scripts (llApplyImpulse, llSetForce, ...) -> physics ---
    let (physics_cmd_tx, physics_cmd_rx) =
        std::sync::mpsc::channel::<(Uuid, welkin_script_vm::PhysicsCommand)>();
    let heartbeat_physics_tx = physics_cmd_tx.clone();
    let _physics_cmd_tx = Arc::new(physics_cmd_tx);

    // --- Heartbeat loop ---
    let heartbeat_cancel = cancel.clone();
    let heartbeat_scene = Arc::clone(&scene);
    let heartbeat_scheduler = Arc::clone(&script_scheduler);
    let heartbeat_socket = udp_socket.clone();
    let heartbeat_clients = udp_clients.clone();
    let heartbeat_sim_store = Arc::clone(&sim_store);
    let heartbeat_region_id = region_id_str.clone();
    let frame_metrics = Arc::new(std::sync::Mutex::new(sim_stats::FrameMetrics::new()));
    let heartbeat_metrics = Arc::clone(&frame_metrics);
    let frame_time_ms = (frame_time * 1000.0) as u64;
    let backup_interval_secs: u64 = {
        let raw = config.get_i64_or("Backup", "IntervalSeconds", 900);
        if raw <= 0 { 900 } else { raw.max(5) as u64 }
    };
    info!(backup_interval_secs, "periodic backup interval configured");
    let heartbeat_handle = tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(frame_time_ms.max(50)));
        let mut last_backup = Instant::now();
        let backup_interval = Duration::from_secs(backup_interval_secs);
        let mut prev_colliding: HashSet<u32> = HashSet::new();
        let mut collision_sound_throttle = sound_io::CollisionSoundThrottle::new();
        let mut sensor_repeats: HashMap<Uuid, (welkin_script_vm::SensorRequest, std::time::Instant, std::time::Duration)> = HashMap::new();
        let mut control_registrations: HashMap<Uuid, (Uuid, u32, bool, bool)> = HashMap::new();
        loop {
            tokio::select! {
                _ = heartbeat_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let frame_start = std::time::Instant::now();
                    let frame_stats = heartbeat_scene.tick();
                    let physics_ms = frame_stats.physics_ms;

                    // Script physics commands act on the physics bodies before the next step.
                    while let Ok((prim_id, cmd)) = physics_cmd_rx.try_recv() {
                        if let Err(reason) = physics_cmd::apply_physics_command(&heartbeat_scene, &prim_id, &cmd) {
                            warn!(prim = %prim_id, command = ?cmd, %reason, "physics command not applied");
                        }
                    }

                    // Physical prims: copy simulated motion into the scene and tell viewers.
                    for m in heartbeat_scene.sync_physics_to_objects() {
                        broadcast_terse_object_update(
                            m.local_id,
                            m.position,
                            m.velocity,
                            glam::Vec3::ZERO,
                            m.rotation,
                            m.angular_velocity,
                            heartbeat_scene.region_info.region_handle,
                            &heartbeat_socket,
                            &heartbeat_clients,
                        )
                        .await;
                    }
                    let mut script_ms = 0.0f32;
                    if scripts_enabled {
                        // Drain any link_message events from builtins
                        while let Ok((target_prim, event)) = link_msg_rx.try_recv() {
                            heartbeat_scheduler.lock().post_event_to_prim(target_prim, event);
                        }

                        // Drain script load requests from cap handler
                        while let Ok(load_req) = script_load_rx.try_recv() {
                            match welkin_script_vm::ScriptInstance::new(
                                load_req.script_id,
                                load_req.prim_id,
                                &load_req.luau_source,
                                0,
                            ) {
                                Ok(mut instance) => {
                                    let prim_pos = heartbeat_scene
                                        .with_object(&load_req.prim_id, |g| g.position())
                                        .unwrap_or(glam::Vec3::new(128.0, 128.0, 22.0));
                                    let ctx = std::sync::Arc::new(parking_lot::RwLock::new(
                                        welkin_script_vm::builtins::ScriptContext {
                                            script_id: load_req.script_id,
                                            prim_id: load_req.prim_id,
                                            position: [prim_pos.x as f64, prim_pos.y as f64, prim_pos.z as f64],
                                            chat_sender: Some(heartbeat_chat_tx.clone()),
                                            timer_sender: Some(heartbeat_timer_tx.clone()),
                                            event_sender: Some(heartbeat_event_tx.clone()),
                                            sensor_sender: Some(heartbeat_sensor_tx.clone()),
                                            control_sender: Some(heartbeat_control_tx.clone()),
                                            sound_sender: Some(heartbeat_sound_tx.clone()),
                                            physics_sender: Some(heartbeat_physics_tx.clone()),
                                            ..Default::default()
                                        }
                                    ));
                                    if let Err(e) = instance.register_builtins(ctx) {
                                        tracing::warn!(
                                            script = %load_req.script_id,
                                            error = %e,
                                            "failed to register builtins"
                                        );
                                        continue;
                                    }
                                    let mut sched = heartbeat_scheduler.lock();
                                    sched.add_script(instance);
                                    sched.post_event(load_req.script_id, ScriptEvent::StateEntry);
                                    tracing::info!(
                                        script = %load_req.script_id,
                                        prim = %load_req.prim_id,
                                        "script loaded into VM with builtins, state_entry queued"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        script = %load_req.script_id,
                                        error = %e,
                                        "failed to compile script bytecode"
                                    );
                                }
                            }
                        }

                        // Drain timer commands from scripts
                        while let Ok((script_id, interval_secs)) = timer_cmd_rx.try_recv() {
                            if interval_secs > 0.0 {
                                heartbeat_scheduler.lock().set_timer(
                                    script_id,
                                    std::time::Duration::from_secs_f64(interval_secs),
                                );
                            } else {
                                heartbeat_scheduler.lock().clear_timer(script_id);
                            }
                        }

                        // Drain script chat messages → trigger listen events + collect for client relay
                        let mut pending_chat: Vec<welkin_script_vm::ChatMessage> = Vec::new();
                        while let Ok(chat_msg) = script_chat_rx.try_recv() {
                            tracing::info!(
                                channel = chat_msg.channel,
                                message = %chat_msg.message,
                                source = %chat_msg.source_name,
                                "script chat message received"
                            );
                            // Route to script listen events
                            heartbeat_scheduler.lock().broadcast_event(ScriptEvent::Listen {
                                channel: chat_msg.channel,
                                name: chat_msg.source_name.clone(),
                                id: chat_msg.source_id,
                                message: chat_msg.message.clone(),
                            });
                            if chat_msg.channel == 0 {
                                pending_chat.push(chat_msg);
                            }
                        }
                        // Relay channel-0 script chat to connected clients
                        if !pending_chat.is_empty() {
                            let clients_map = heartbeat_clients.read().await;
                            for chat_msg in &pending_chat {
                                let chat_type_num: u8 = match chat_msg.chat_type {
                                    welkin_script_vm::ChatType::Whisper => 0,
                                    welkin_script_vm::ChatType::Say => 1,
                                    welkin_script_vm::ChatType::Shout => 2,
                                    _ => 1,
                                };
                                let range_sq: f32 = match chat_type_num {
                                    0 => 10.0 * 10.0,
                                    2 => 100.0 * 100.0,
                                    _ => 20.0 * 20.0,
                                };
                                let sender_pos = glam::Vec3::new(
                                    chat_msg.source_pos[0] as f32,
                                    chat_msg.source_pos[1] as f32,
                                    chat_msg.source_pos[2] as f32,
                                );
                                let mut name_bytes = chat_msg.source_name.clone().into_bytes();
                                name_bytes.push(0);
                                let relay = ChatFromSimulator {
                                    chat_data: chat_from_simulator::ChatData {
                                        from_name: name_bytes,
                                        source_id: chat_msg.source_id,
                                        owner_id: chat_msg.owner_id,
                                        source_type: 2, // object
                                        chat_type: chat_type_num,
                                        audible: 1,
                                        position: sender_pos,
                                        message: chat_msg.message.clone().into_bytes(),
                                    },
                                };
                                for (peer_addr, peer_client) in clients_map.iter() {
                                    let mut peer = peer_client.lock().await;
                                    if let Some(presence) = heartbeat_scene.get_presence(&peer.agent_id) {
                                        if sender_pos.distance_squared(presence.position) <= range_sq {
                                            let (pkt_bytes, _) = peer.circuit.send_msg(&relay, true);
                                            let _ = heartbeat_socket.send_to(&pkt_bytes, *peer_addr).await;
                                        }
                                    }
                                }
                            }
                        }

                        // Drain script sound commands → viewer sound packets
                        let mut pending_sounds: Vec<welkin_script_vm::builtins::SoundCommand> = Vec::new();
                        while let Ok(cmd) = sound_cmd_rx.try_recv() {
                            pending_sounds.push(cmd);
                        }
                        if !pending_sounds.is_empty() {
                            sound_io::relay_sound(
                                pending_sounds,
                                &heartbeat_scene,
                                &heartbeat_socket,
                                &heartbeat_clients,
                            )
                            .await;
                        }

                        // Drain sensor requests
                        while let Ok(req) = sensor_cmd_rx.try_recv() {
                            match req {
                                welkin_script_vm::SensorRequest::Scan { script_id, .. } => {
                                    // One-shot sensor: fire sensor event with 0 detected (simplified)
                                    heartbeat_scheduler.lock().post_event(
                                        script_id,
                                        ScriptEvent::Sensor { num_detected: 0 },
                                    );
                                }
                                welkin_script_vm::SensorRequest::Repeat { ref script_id, ref rate, .. } => {
                                    let dur = std::time::Duration::from_secs_f64(*rate);
                                    sensor_repeats.insert(*script_id, (req, std::time::Instant::now(), dur));
                                }
                                welkin_script_vm::SensorRequest::Remove { script_id } => {
                                    sensor_repeats.remove(&script_id);
                                }
                            }
                        }

                        // Fire repeating sensor events
                        let now = std::time::Instant::now();
                        for (_sid, (req, last_fire, interval_dur)) in sensor_repeats.iter_mut() {
                            if now.duration_since(*last_fire) >= *interval_dur {
                                *last_fire = now;
                                if let welkin_script_vm::SensorRequest::Repeat { script_id, .. } = req {
                                    heartbeat_scheduler.lock().post_event(
                                        *script_id,
                                        ScriptEvent::Sensor { num_detected: 0 },
                                    );
                                }
                            }
                        }

                        // Drain control registration requests
                        while let Ok(req) = control_cmd_rx.try_recv() {
                            match req {
                                welkin_script_vm::ControlRequest::Take { script_id, controls, accept, pass_on } => {
                                    control_registrations.insert(script_id, (Uuid::nil(), controls, accept, pass_on));
                                }
                                welkin_script_vm::ControlRequest::Release { script_id } => {
                                    control_registrations.remove(&script_id);
                                }
                            }
                        }

                        let script_start = std::time::Instant::now();
                        let errors = heartbeat_scheduler.lock().tick();
                        script_ms = script_start.elapsed().as_secs_f32() * 1000.0;
                        for (script_id, err) in errors {
                            tracing::warn!(script = %script_id, error = %err, "script execution error");
                        }

                        // Propagate script context changes to scene objects
                        let script_changes: Vec<(Uuid, glam::Vec3, glam::Quat, glam::Vec3, String)> = {
                            let sched = heartbeat_scheduler.lock();
                            sched.all_scripts().filter_map(|inst| {
                                let ctx_arc = inst.context()?;
                                let ctx = ctx_arc.read();
                                Some((
                                    inst.prim_id,
                                    glam::Vec3::new(ctx.position[0] as f32, ctx.position[1] as f32, ctx.position[2] as f32),
                                    glam::Quat::from_xyzw(ctx.rotation[0] as f32, ctx.rotation[1] as f32, ctx.rotation[2] as f32, ctx.rotation[3] as f32),
                                    glam::Vec3::new(ctx.scale[0] as f32, ctx.scale[1] as f32, ctx.scale[2] as f32),
                                    ctx.hover_text.clone(),
                                ))
                            }).collect()
                        };
                        for (prim_id, ctx_pos, ctx_rot, ctx_scale, ctx_text) in &script_changes {
                            let needs_update = if let Some(obj) = heartbeat_scene.get_object(prim_id) {
                                let rp = obj.root_part();
                                let pos_diff = (rp.group_position - *ctx_pos).length() > 0.001;
                                let rot_diff = (rp.rotation_offset.x - ctx_rot.x).abs() > 0.001
                                    || (rp.rotation_offset.y - ctx_rot.y).abs() > 0.001
                                    || (rp.rotation_offset.z - ctx_rot.z).abs() > 0.001
                                    || (rp.rotation_offset.w - ctx_rot.w).abs() > 0.001;
                                let scale_diff = (rp.scale - *ctx_scale).length() > 0.001;
                                let text_diff = rp.text != *ctx_text;
                                (pos_diff, rot_diff, scale_diff, text_diff)
                            } else {
                                continue;
                            };
                            let (pos_changed, rot_changed, scale_changed, text_changed) = needs_update;
                            if pos_changed || rot_changed || scale_changed || text_changed {
                                heartbeat_scene.with_object_mut(prim_id, |group| {
                                    if pos_changed {
                                        group.set_position(*ctx_pos);
                                    }
                                    if rot_changed {
                                        group.set_rotation(*ctx_rot);
                                    }
                                    if scale_changed {
                                        group.root_part_mut().scale = *ctx_scale;
                                    }
                                    if text_changed {
                                        group.root_part_mut().text = ctx_text.clone();
                                    }
                                });
                                heartbeat_scene.push_object_to_physics(prim_id);
                                if let Some(updated) = heartbeat_scene.get_object(prim_id) {
                                    if text_changed {
                                        broadcast_full_object_update(&updated, heartbeat_scene.region_info.region_handle, &heartbeat_socket, &heartbeat_clients).await;
                                    } else {
                                        let rp = updated.root_part();
                                        broadcast_terse_object_update(
                                            rp.local_id,
                                            rp.group_position,
                                            glam::Vec3::ZERO,
                                            glam::Vec3::ZERO,
                                            rp.rotation_offset,
                                            glam::Vec3::ZERO,
                                            heartbeat_scene.region_info.region_handle,
                                            &heartbeat_socket,
                                            &heartbeat_clients,
                                        ).await;
                                    }
                                }
                            }
                        }

                        // Post collision events with start/end tracking
                        let colliders = heartbeat_scene.get_top_colliders();
                        let cur_colliding: HashSet<u32> = colliders.iter().map(|(id, _)| *id).collect();
                        let mut started_collisions: Vec<u32> = Vec::new();
                        if !cur_colliding.is_empty() || !prev_colliding.is_empty() {
                            let mut sched = heartbeat_scheduler.lock();
                            for &local_id in &cur_colliding {
                                if let Some(obj_uuid) = heartbeat_scene.get_object_by_local_id(local_id) {
                                    if !prev_colliding.contains(&local_id) {
                                        started_collisions.push(local_id);
                                        sched.post_event_to_prim(
                                            obj_uuid,
                                            ScriptEvent::CollisionStart { num_detected: 1 },
                                        );
                                    }
                                    sched.post_event_to_prim(
                                        obj_uuid,
                                        ScriptEvent::Collision { num_detected: 1 },
                                    );
                                }
                            }
                            for &local_id in &prev_colliding {
                                if !cur_colliding.contains(&local_id) {
                                    if let Some(obj_uuid) = heartbeat_scene.get_object_by_local_id(local_id) {
                                        sched.post_event_to_prim(
                                            obj_uuid,
                                            ScriptEvent::CollisionEnd { num_detected: 0 },
                                        );
                                    }
                                }
                            }
                        }
                        // Script-selected collision sounds (llCollisionSound) on collision start.
                        // The scheduler lock above is released before awaiting.
                        for local_id in started_collisions {
                            sound_io::relay_collision_sound(
                                &heartbeat_scene,
                                &heartbeat_socket,
                                &heartbeat_clients,
                                local_id,
                                &mut collision_sound_throttle,
                            )
                            .await;
                        }
                        prev_colliding = cur_colliding;
                    }

                    let frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
                    heartbeat_metrics.lock().unwrap().record(frame_ms, physics_ms, script_ms);

                    if last_backup.elapsed() >= backup_interval {
                        last_backup = Instant::now();
                        let n = persist::backup_dirty_objects(
                            &heartbeat_scene,
                            &heartbeat_sim_store,
                            &heartbeat_region_id,
                        ).await;
                        if n > 0 {
                            tracing::info!(count = n, "persisted dirty objects");
                        }
                    }
                }
            }
        }
    });

    // --- SimStats reporter (1 Hz) ---
    let sim_stats_handle = tokio::spawn({
        let scene = scene.clone();
        let script_scheduler = script_scheduler.clone();
        let udp_socket = udp_socket.clone();
        let udp_clients = udp_clients.clone();
        let cancel = cancel.clone();
        let frame_metrics = frame_metrics.clone();
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {
                        let averages = frame_metrics.lock().unwrap().take();
                        let (total_prims, active_prims) = scene.prim_stats();
                        let input = sim_stats::SimStatsInput {
                            region_x: scene.region_info.region_loc_x * 256,
                            region_y: scene.region_info.region_loc_y * 256,
                            region_flags: 0,
                            object_capacity: 45000,
                            root_agents: scene.presence_count() as u32,
                            child_agents: 0,
                            total_prims,
                            active_prims,
                            active_scripts: script_scheduler.lock().script_count() as u32,
                            averages,
                        };
                        let stats_msg = sim_stats::build_sim_stats(&input);
                        let clients = udp_clients.read().await;
                        for (addr, client) in clients.iter() {
                            let mut client = client.lock().await;
                            let (bytes, _) = client.circuit.send_msg(&stats_msg, false);
                            if let Err(e) = udp_socket.send_to(&bytes, addr).await {
                                tracing::debug!("sim stats send failed: {e}");
                            }
                        }
                    }
                }
            }
        }
    });

    // --- CoarseLocationUpdate periodic loop ---
    let coarse_cancel = cancel.clone();
    let coarse_scene = Arc::clone(&scene);
    let coarse_socket = udp_socket.clone();
    let coarse_clients = udp_clients.clone();
    let coarse_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            tokio::select! {
                _ = coarse_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let mut locations = Vec::new();
                    let mut agent_ids = Vec::new();
                    coarse_scene.for_each_presence(|p| {
                        locations.push(coarse_location_update::Location {
                            x: (p.position.x.clamp(0.0, 255.0)) as u8,
                            y: (p.position.y.clamp(0.0, 255.0)) as u8,
                            z: (p.position.z / 4.0).clamp(0.0, 255.0) as u8,
                        });
                        agent_ids.push(p.uuid);
                    });
                    if locations.is_empty() { continue; }
                    let clients_map = coarse_clients.read().await;
                    for (addr, client) in clients_map.iter() {
                        let mut session = client.lock().await;
                        let my_idx = agent_ids.iter().position(|&id| id == session.agent_id)
                            .map(|i| i as i16).unwrap_or(-1);
                        let msg = CoarseLocationUpdate {
                            location: locations.clone(),
                            index: coarse_location_update::Index { you: my_idx, prey: -1 },
                            agent_data: agent_ids.iter().map(|&id| {
                                coarse_location_update::AgentData { agent_id: id }
                            }).collect(),
                        };
                        let (pkt, _) = session.circuit.send_msg(&msg, false);
                        let _ = coarse_socket.send_to(&pkt, *addr).await;
                    }
                }
            }
        }
    });

    // --- Connection timeout sweep ---
    let timeout_cancel = cancel.clone();
    let timeout_scene = Arc::clone(&scene);
    let timeout_clients = udp_clients.clone();
    let timeout_eqs = event_queues.clone();
    let timeout_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = timeout_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let mut timed_out = Vec::new();
                    {
                        let clients_map = timeout_clients.read().await;
                        for (addr, client) in clients_map.iter() {
                            let session = client.lock().await;
                            if session.circuit.is_timed_out(Duration::from_secs(60)) {
                                timed_out.push((*addr, session.agent_id));
                            }
                        }
                    }
                    for (addr, agent) in &timed_out {
                        timeout_clients.write().await.remove(addr);
                        timeout_scene.remove_presence(agent);
                        timeout_eqs.write().await.remove(agent);
                        info!(agent = %agent, addr = %addr, "client timed out, removed");
                    }
                }
            }
        }
    });

    info!(
        region = %scene.region_info.region_name,
        "Region simulator started successfully"
    );

    // --- Operator console (REPL + admin REST) ---
    let console_shutdown = Arc::new(tokio::sync::Notify::new());
    let (console_tx, console_rx) = tokio::sync::mpsc::unbounded_channel();
    let console_ctx = Arc::new(console::ConsoleContext {
        scene: scene.clone(),
        sim_store: sim_store.clone(),
        region_id: region_id_str.clone(),
        started: Instant::now(),
        actions: console_tx,
        grid_uri: config.get_string_or("GridService", "GridServerURI", ""),
        http: reqwest::Client::new(),
    });
    tokio::spawn(console_io::run_actions(
        console_rx,
        udp_socket.clone(),
        udp_clients.clone(),
        console_shutdown.clone(),
    ));
    let _repl_handle = console_io::spawn_repl(console_ctx.clone(), cancel.clone());
    let admin_token = config.get_string_or("Admin", "Token", "");
    if admin_token.is_empty() {
        info!("admin REST API disabled ([Admin] Token not set)");
    } else {
        let bind = config.get_string_or("Admin", "BindAddress", "127.0.0.1");
        let port = config.get_i64_or("Admin", "Port", 9100);
        let is_loopback = bind
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(bind == "localhost");
        let allow_remote = config.get_bool_or("Admin", "AllowRemoteBind", false);
        if !is_loopback && !allow_remote {
            error!(
                %bind,
                "admin REST API NOT started: it is plain HTTP and carries the bearer token and passwords \
                 in cleartext. Bind to 127.0.0.1 (or front it with a TLS reverse proxy) and set \
                 [Admin] AllowRemoteBind = true to accept the risk on a non-loopback address"
            );
        } else {
            if !is_loopback {
                warn!(%bind, "admin REST API bound to a non-loopback address over plain HTTP");
            }
        match tokio::net::TcpListener::bind(format!("{bind}:{port}")).await {
            Ok(listener) => {
                info!(%bind, port, "admin REST API listening");
                let router = console_io::admin_router(console_ctx.clone(), admin_token);
                let admin_cancel = cancel.clone();
                tokio::spawn(async move {
                    let _ = axum::serve(listener, router)
                        .with_graceful_shutdown(async move { admin_cancel.cancelled().await })
                        .await;
                });
            }
            Err(e) => error!(%bind, port, error = %e, "admin REST API failed to bind"),
        }
        }
    }

    // --- Wait for shutdown signal (OS signal or console `shutdown`) ---
    tokio::select! {
        _ = welkin_runtime::shutdown_signal() => {}
        _ = console_shutdown.notified() => info!("shutdown requested from console"),
    }
    info!("Initiating graceful shutdown");

    // --- Notify connected clients before tearing down ---
    {
        let snapshot: Vec<(SocketAddr, Arc<Mutex<ClientSession>>)> = {
            let guard = udp_clients.read().await;
            guard.iter().map(|(addr, c)| (*addr, c.clone())).collect()
        };
        let client_count = snapshot.len();

        let system_chat = ChatFromSimulator {
            chat_data: chat_from_simulator::ChatData {
                from_name: b"SYSTEM\0".to_vec(),
                source_id: Uuid::nil(),
                owner_id: Uuid::nil(),
                source_type: 1,
                chat_type: 1,
                audible: 1,
                position: glam::Vec3::ZERO,
                message: b"Region shutting down".to_vec(),
            },
        };
        for (addr, client) in &snapshot {
            let pkt = {
                let mut session = client.lock().await;
                session.circuit.send_msg(&system_chat, true).0
            };
            let _ = udp_socket.send_to(&pkt, *addr).await;
        }

        for (addr, client) in &snapshot {
            let (kick_pkt, disable_pkt) = {
                let mut session = client.lock().await;
                let kick = KickUser {
                    target_block: kick_user::TargetBlock {
                        target_ip: [0, 0, 0, 0],
                        target_port: 0,
                    },
                    user_info: kick_user::UserInfo {
                        agent_id: session.agent_id,
                        session_id: session.session_id,
                        reason: b"The simulator is going down.".to_vec(),
                    },
                };
                let disable = DisableSimulator {};
                (
                    session.circuit.send_msg(&kick, true).0,
                    session.circuit.send_msg(&disable, true).0,
                )
            };
            let _ = udp_socket.send_to(&kick_pkt, *addr).await;
            let _ = udp_socket.send_to(&disable_pkt, *addr).await;
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        for (addr, client) in &snapshot {
            let (logout_pkt, agent_id) = {
                let mut session = client.lock().await;
                let logout = LogoutReply {
                    agent_data: logout_reply::AgentData {
                        agent_id: session.agent_id,
                        session_id: session.session_id,
                    },
                    inventory_data: Vec::new(),
                };
                (session.circuit.send_msg(&logout, true).0, session.agent_id)
            };
            let _ = udp_socket.send_to(&logout_pkt, *addr).await;
            scene.remove_presence(&agent_id);
        }

        event_queues.write().await.clear();
        udp_clients.write().await.clear();

        info!(clients = client_count, "disconnected all clients");
    }

    cancel.cancel();

    macro_rules! join_or_warn {
        ($name:literal, $h:expr) => {
            if tokio::time::timeout(Duration::from_secs(10), $h).await.is_err() {
                warn!(concat!($name, " did not stop within 10s; continuing shutdown"));
            }
        };
    }

    join_or_warn!("udp_handle", udp_handle);
    join_or_warn!("process_handle", process_handle);
    join_or_warn!("caps_handle", caps_handle);
    join_or_warn!("heartbeat_handle", heartbeat_handle);
    join_or_warn!("sim_stats_handle", sim_stats_handle);
    join_or_warn!("coarse_handle", coarse_handle);
    join_or_warn!("timeout_handle", timeout_handle);

    if let Some(ref reg) = grid_reg {
        match tokio::time::timeout(Duration::from_secs(10), reg.shutdown()).await {
            Ok(Err(e)) => warn!(error = %e, "Failed to deregister from grid"),
            Err(_) => warn!("grid deregistration did not stop within 10s; continuing shutdown"),
            _ => {}
        }
    }

    // Save script states to DB before shutdown
    if scripts_enabled {
        let states: Vec<_> = {
            let sched = script_scheduler.lock();
            sched.all_scripts().filter_map(|inst| {
                match inst.save_state() {
                    Ok(state) => Some((inst.script_id, inst.prim_id, state)),
                    Err(e) => {
                        warn!(script = %inst.script_id, error = %e, "failed to save script state");
                        None
                    }
                }
            }).collect()
        };
        match tokio::time::timeout(Duration::from_secs(15), script_persist::save_script_states(pool.inner(), &states)).await {
            Ok(Ok(n)) => info!(count = n, "saved script states to DB"),
            Ok(Err(e)) => warn!(error = %e, "failed to save script states"),
            Err(_) => warn!("script state save timed out"),
        }
    }

    {
        let mut registry = scene_ref.registry.write();
        let _ = registry.close_all();
    }
    let dirty = scene.force_backup_all();
    if !dirty.is_empty() {
        info!(count = dirty.len(), "flushing dirty objects on shutdown");
        if tokio::time::timeout(Duration::from_secs(30), async {
            for uuid in &dirty {
                if let Some(group) = scene.get_object(uuid) {
                    let gid = group.uuid().to_string();
                    let region_uuid = scene.region_info.region_id.to_string();
                    for part in group.parts() {
                        let prim_row = persist::part_to_prim_row(part, &region_uuid, &gid);
                        let shape_row = persist::shape_to_row(part);
                        if let Err(e) = sim_store.store_object(&region_uuid, &gid, &prim_row, &shape_row).await {
                            tracing::warn!(uuid = %part.uuid, error = %e, "shutdown: failed to persist prim");
                        }
                    }
                    scene.mark_backed_up(uuid);
                }
            }
        }).await.is_err() {
            warn!("dirty prim flush timed out; some objects may not be persisted");
        }
    }
    pool.inner().close().await;
    info!("database pool closed");
    scene.shutdown();
    info!("Region simulator shut down");

    Ok(())
}

type ClientMap = Arc<RwLock<std::collections::HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>;
/// Default avatar wearables (shape, skin, hair, eyes, shirt, pants).
fn default_wearables() -> Vec<agent_wearables_update::WearableData> {
    vec![
        agent_wearables_update::WearableData {
            wearable_type: 0,
            item_id: Uuid::parse_str("66c41e39-38f9-f75a-024e-585989bfaba9").unwrap(),
            asset_id: Uuid::parse_str("66c41e39-38f9-f75a-024e-585989bfab73").unwrap(),
        },
        agent_wearables_update::WearableData {
            wearable_type: 1,
            item_id: Uuid::parse_str("77c41e39-38f9-f75a-024e-585989bfabc9").unwrap(),
            asset_id: Uuid::parse_str("77c41e39-38f9-f75a-024e-585989bbabbb").unwrap(),
        },
        agent_wearables_update::WearableData {
            wearable_type: 2,
            item_id: Uuid::parse_str("d342e6c1-b9d2-11dc-95ff-0800200c9a66").unwrap(),
            asset_id: Uuid::parse_str("d342e6c0-b9d2-11dc-95ff-0800200c9a66").unwrap(),
        },
        agent_wearables_update::WearableData {
            wearable_type: 3,
            item_id: Uuid::parse_str("cdc31054-eed8-4021-994f-4e0c6e861b50").unwrap(),
            asset_id: Uuid::parse_str("4bb6fa4d-1cd2-498a-a84c-95c1a0e745a7").unwrap(),
        },
        agent_wearables_update::WearableData {
            wearable_type: 4,
            item_id: Uuid::parse_str("77c41e39-38f9-f75a-0000-585989bf0000").unwrap(),
            asset_id: Uuid::parse_str("00000000-38f9-1111-024e-222222111110").unwrap(),
        },
        agent_wearables_update::WearableData {
            wearable_type: 5,
            item_id: Uuid::parse_str("77c41e39-38f9-f75a-0000-5859892f1111").unwrap(),
            asset_id: Uuid::parse_str("00000000-38f9-1111-024e-222222111120").unwrap(),
        },
    ]
}

use welkin_caps::handlers::EventQueueMap;

async fn process_messages(
    mut msg_rx: tokio::sync::mpsc::UnboundedReceiver<(SocketAddr, welkin_msg::IncomingPacket)>,
    dispatcher: PacketDispatcher,
    socket: Arc<UdpSocket>,
    clients: ClientMap,
    scene: Arc<Scene>,
    event_queues: EventQueueMap,
    caps_base: String,
    cancel: CancellationToken,
    script_scheduler: Arc<parking_lot::Mutex<ScriptScheduler>>,
    scripts_enabled: bool,
    grid_connector: Option<Arc<welkin_services::remote::grid::RemoteGridConnector>>,
    perms: Arc<DefaultPermissionsModule>,
    accounts_connector: Arc<welkin_services::remote::accounts::RemoteAccountsConnector>,
) {
    let mut last_agent_update: HashMap<Uuid, Instant> = HashMap::new();
    // Persistent vertical velocity per avatar (jumping and falling).
    let mut avatar_vertical: HashMap<Uuid, f32> = HashMap::new();
    let mut last_anim: HashMap<Uuid, &'static str> = HashMap::new();
    let mut always_run: HashSet<Uuid> = HashSet::new();
    let mut group_sessions = ChatSessionState::new();
    loop {
        let (src, pkt) = tokio::select! {
            _ = cancel.cancelled() => break,
            msg = msg_rx.recv() => match msg {
                Some(m) => m,
                None => break,
            },
        };

        let client = { clients.read().await.get(&src).cloned() };
        let (agent_id, session_id, circuit_code) = match &client {
            Some(c) => {
                let s = c.lock().await;
                (s.agent_id, s.session_id, s.circuit_code)
            }
            None => (Uuid::nil(), Uuid::nil(), 0),
        };

        let ctx = PacketContext {
            agent_id,
            session_id,
            circuit_code,
            addr: src,
        };

        let msg_id = pkt.id;

        match dispatcher.dispatch(&ctx, &pkt) {
            Ok(()) => {}
            Err(HandleError::Unhandled) => {
                tracing::trace!(id = ?msg_id, from = %src, "unhandled message");
            }
            Err(e) => {
                tracing::warn!(error = %e, id = ?msg_id, "handler error");
            }
        }

        if msg_id == CompleteAgentMovement::ID
            && let Some(client) = &client
        {
            {
                let mut cmap = clients.write().await;
                let stale: Vec<SocketAddr> = cmap
                    .iter()
                    .filter(|(addr, _)| **addr != src)
                    .filter(|(_, c)| {
                        if let Ok(s) = c.try_lock() {
                            s.agent_id == agent_id
                        } else {
                            false
                        }
                    })
                    .map(|(addr, _)| *addr)
                    .collect();
                for addr in &stale {
                    cmap.remove(addr);
                    info!(agent = %agent_id, old_addr = %addr, "evicted stale client session on reconnect");
                }
            }
            send_movement_complete(
                &socket,
                client,
                src,
                &scene.region_info,
                &scene,
                &event_queues,
                &caps_base,
                &accounts_connector,
            )
            .await;

            // --- Deliver the existing scene (prims and linkset children) to the new viewer ---
            {
                let mut session = client.lock().await;
                let sent = scene_sync::send_scene_objects(&scene, &mut session, &socket, src).await;
                info!(agent = %agent_id, objects = sent, "sent existing scene objects to new viewer");
            }

            // --- Issue 2A: Announce new avatar to all OTHER connected clients ---
            if let Some(new_presence) = scene.get_presence(&agent_id) {
                let new_avatar_obj = FullObjectUpdate {
                    local_id: new_presence.local_id,
                    full_id: new_presence.uuid,
                    parent_id: 0,
                    pcode: 47,
                    state: 0,
                    crc: 0,
                    material: 0,
                    click_action: 0,
                    scale: glam::Vec3::new(0.45, 0.6, 1.75),
                    position: new_presence.position,
                    velocity: glam::Vec3::ZERO,
                    acceleration: glam::Vec3::ZERO,
                    rotation: new_presence.rotation,
                    angular_velocity: glam::Vec3::ZERO,
                    update_flags: 0x2000_0001,
                    owner_id: new_presence.uuid,
                    path_curve: 0,
                    profile_curve: 0,
                    path_begin: 0,
                    path_end: 0,
                    path_scale_x: 0,
                    path_scale_y: 0,
                    path_shear_x: 0,
                    path_shear_y: 0,
                    path_twist: 0,
                    path_twist_begin: 0,
                    path_radius_offset: 0,
                    path_taper_x: 0,
                    path_taper_y: 0,
                    path_revolutions: 0,
                    path_skew: 0,
                    profile_begin: 0,
                    profile_end: 0,
                    profile_hollow: 0,
                    texture_entry: build_default_avatar_te(),
                    texture_anim: Vec::new(),
                    name_value: format!(
                        "FirstName STRING RW SV {}\nLastName STRING RW SV {}\n",
                        new_presence.first_name, new_presence.last_name
                    )
                    .into_bytes(),
                    data: Vec::new(),
                    text: String::new(),
                    text_color: [0, 0, 0, 0],
                    media_url: String::new(),
                    ps_block: Vec::new(),
                    extra_params: Vec::new(),
                    sound_id: Uuid::nil(),
                    sound_gain: 0.0,
                    sound_flags: 0,
                    sound_radius: 0.0,
                    foot_plane: Some(glam::Vec4::new(0.0, 0.0, 1.0, 0.0)),
                };
                let obj_msg = ObjectUpdate {
                    region_data: object_update::RegionData {
                        region_handle: scene.region_info.region_handle,
                        time_dilation: 65535,
                    },
                    object_data: vec![new_avatar_obj.to_object_data()],
                };
                let visual_params: Vec<avatar_appearance::VisualParam> = (0..253)
                    .map(|_| avatar_appearance::VisualParam { param_value: 128 })
                    .collect();
                let appearance_msg = AvatarAppearance {
                    sender: avatar_appearance::Sender {
                        id: new_presence.uuid,
                        is_trial: false,
                    },
                    object_data: avatar_appearance::ObjectData {
                        texture_entry: build_default_avatar_te(),
                    },
                    visual_param: visual_params,
                    appearance_data: vec![avatar_appearance::AppearanceData {
                        appearance_version: 0,
                        cof_version: 0,
                        flags: 0,
                    }],
                    appearance_hover: Vec::new(),
                    attachment_block: Vec::new(),
                };
                {
                    let all_clients = clients.read().await;
                    for (addr, c) in all_clients.iter() {
                        if *addr == src {
                            continue;
                        }
                        let mut sess = c.lock().await;
                        let (pkt_bytes, _) = sess.circuit.send_msg(&obj_msg, true);
                        let _ = socket.send_to(&pkt_bytes, *addr).await;
                        let (pkt_bytes, _) = sess.circuit.send_msg(&appearance_msg, true);
                        let _ = socket.send_to(&pkt_bytes, *addr).await;
                    }
                }
                info!(agent = %agent_id, "announced new avatar to other clients");
            }

            // --- Issue 2B: Send all EXISTING avatars to the newly logged-in user ---
            {
                let mut existing_presences = Vec::new();
                scene.for_each_presence(|p| {
                    if p.uuid != agent_id {
                        existing_presences.push((
                            p.uuid,
                            p.local_id,
                            p.position,
                            p.rotation,
                            p.first_name.clone(),
                            p.last_name.clone(),
                            p.texture_entry.clone(),
                            p.visual_params.clone(),
                        ));
                    }
                });
                if !existing_presences.is_empty() {
                    let mut session = client.lock().await;
                    for (ex_id, ex_local_id, ex_pos, ex_rot, ex_first, ex_last, ex_te, ex_vp) in &existing_presences {
                        let te = if ex_te.is_empty() { build_default_avatar_te() } else { ex_te.clone() };
                        let ex_obj = FullObjectUpdate {
                            local_id: *ex_local_id,
                            full_id: *ex_id,
                            parent_id: 0,
                            pcode: 47,
                            state: 0,
                            crc: 0,
                            material: 0,
                            click_action: 0,
                            scale: glam::Vec3::new(0.45, 0.6, 1.75),
                            position: *ex_pos,
                            velocity: glam::Vec3::ZERO,
                            acceleration: glam::Vec3::ZERO,
                            rotation: *ex_rot,
                            angular_velocity: glam::Vec3::ZERO,
                            update_flags: 0x2000_0001,
                            owner_id: *ex_id,
                            path_curve: 0,
                            profile_curve: 0,
                            path_begin: 0,
                            path_end: 0,
                            path_scale_x: 0,
                            path_scale_y: 0,
                            path_shear_x: 0,
                            path_shear_y: 0,
                            path_twist: 0,
                            path_twist_begin: 0,
                            path_radius_offset: 0,
                            path_taper_x: 0,
                            path_taper_y: 0,
                            path_revolutions: 0,
                            path_skew: 0,
                            profile_begin: 0,
                            profile_end: 0,
                            profile_hollow: 0,
                            texture_entry: te,
                            texture_anim: Vec::new(),
                            name_value: format!(
                                "FirstName STRING RW SV {}\nLastName STRING RW SV {}\n",
                                ex_first, ex_last
                            ).into_bytes(),
                            data: Vec::new(),
                            text: String::new(),
                            text_color: [0, 0, 0, 0],
                            media_url: String::new(),
                            ps_block: Vec::new(),
                            extra_params: Vec::new(),
                            sound_id: Uuid::nil(),
                            sound_gain: 0.0,
                            sound_flags: 0,
                            sound_radius: 0.0,
                            foot_plane: Some(glam::Vec4::new(0.0, 0.0, 1.0, 0.0)),
                        };
                        let obj_msg = ObjectUpdate {
                            region_data: object_update::RegionData {
                                region_handle: scene.region_info.region_handle,
                                time_dilation: 65535,
                            },
                            object_data: vec![ex_obj.to_object_data()],
                        };
                        let visual_params: Vec<avatar_appearance::VisualParam> = if ex_vp.is_empty() {
                            (0..253).map(|_| avatar_appearance::VisualParam { param_value: 128 }).collect()
                        } else {
                            ex_vp.iter().map(|&v| avatar_appearance::VisualParam { param_value: v }).collect()
                        };
                        let appearance_te = if ex_te.is_empty() { build_default_avatar_te() } else { ex_te.clone() };
                        let appearance_msg = AvatarAppearance {
                            sender: avatar_appearance::Sender {
                                id: *ex_id,
                                is_trial: false,
                            },
                            object_data: avatar_appearance::ObjectData {
                                texture_entry: appearance_te,
                            },
                            visual_param: visual_params,
                            appearance_data: vec![avatar_appearance::AppearanceData {
                                appearance_version: 0,
                                cof_version: 0,
                                flags: 0,
                            }],
                            appearance_hover: Vec::new(),
                            attachment_block: Vec::new(),
                        };
                        let (pkt_bytes, _) = session.circuit.send_msg(&obj_msg, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;
                        let (pkt_bytes, _) = session.circuit.send_msg(&appearance_msg, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;
                    }
                    info!(agent = %agent_id, count = existing_presences.len(), "sent existing avatars to new user");
                }
            }
        }

        if msg_id == StartPingCheck::ID
            && let Some(client) = &client
            && let Ok(ping) = StartPingCheck::decode(&pkt.body)
        {
            let reply = CompletePingCheck {
                ping_id: complete_ping_check::PingID {
                    ping_id: ping.ping_id.ping_id,
                },
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, false);
            let _ = socket.send_to(&pkt_bytes, src).await;
        }

        if msg_id == AgentCachedTexture::ID
            && let Some(client) = &client
            && let Ok(cached) = AgentCachedTexture::decode(&pkt.body)
        {
            let mut session = client.lock().await;
            let wearable_data = cached
                .wearable_data
                .iter()
                .map(|entry| agent_cached_texture_response::WearableData {
                    texture_id: Uuid::nil(),
                    texture_index: entry.texture_index,
                    host_name: Vec::new(),
                })
                .collect();
            let reply = AgentCachedTextureResponse {
                agent_data: agent_cached_texture_response::AgentData {
                    agent_id: session.agent_id,
                    session_id: session.session_id,
                    serial_num: cached.agent_data.serial_num,
                },
                wearable_data,
            };
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
        }

        if msg_id == AgentWearablesRequest::ID
            && let Some(client) = &client
        {
            let mut session = client.lock().await;
            let reply = AgentWearablesUpdate {
                agent_data: agent_wearables_update::AgentData {
                    agent_id: session.agent_id,
                    session_id: session.session_id,
                    serial_num: 1,
                },
                wearable_data: default_wearables(),
            };
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
        }

        if msg_id == LogoutRequest::ID
            && let Some(client) = &client
        {
            let mut session = client.lock().await;
            let reply = LogoutReply {
                agent_data: logout_reply::AgentData {
                    agent_id: session.agent_id,
                    session_id: session.session_id,
                },
                inventory_data: Vec::new(),
            };
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
            scene.remove_presence(&session.agent_id);
            event_queues.write().await.remove(&session.agent_id);
            clients.write().await.remove(&src);
            info!(agent = %session.agent_id, "logout acknowledged, presence and client removed");
        }

        if msg_id == EconomyDataRequest::ID
            && let Some(client) = &client
        {
            let mut session = client.lock().await;
            let reply = EconomyData {
                info: economy_data::Info {
                    object_capacity: 15000,
                    object_count: 0,
                    price_energy_unit: 0,
                    price_object_claim: 0,
                    price_public_object_decay: 0,
                    price_public_object_delete: 0,
                    price_parcel_claim: 0,
                    price_parcel_claim_factor: 0.0,
                    price_upload: 0,
                    price_rent_light: 0,
                    teleport_min_price: 0,
                    teleport_price_exponent: 0.0,
                    energy_efficiency: 0.0,
                    price_object_rent: 0.0,
                    price_object_scale_factor: 0.0,
                    price_parcel_rent: 0,
                    price_group_create: 0,
                },
            };
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
        }

        if msg_id == MoneyBalanceRequest::ID
            && let Ok(req) = MoneyBalanceRequest::decode(&pkt.body)
            && let Some(client) = &client
        {
            let txn_id = req.money_data.transaction_id;
            let reply = MoneyBalanceReply {
                money_data: money_balance_reply::MoneyData {
                    agent_id,
                    transaction_id: txn_id,
                    transaction_success: true,
                    money_balance: 0,
                    square_meters_credit: 0,
                    square_meters_committed: 0,
                    description: b"\0".to_vec(),
                },
                transaction_info: money_balance_reply::TransactionInfo {
                    transaction_type: 0,
                    source_id: Uuid::nil(),
                    is_source_group: false,
                    dest_id: Uuid::nil(),
                    is_dest_group: false,
                    amount: 0,
                    item_description: b"\0".to_vec(),
                },
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
            info!(agent = %agent_id, "MoneyBalanceRequest: replied with balance 0");
        }

        if msg_id == MoneyTransferRequest::ID
            && let Ok(req) = MoneyTransferRequest::decode(&pkt.body)
            && let Some(client) = &client
        {
            let md = &req.money_data;
            let dest = md.dest_id;
            let amount = md.amount;
            let desc = String::from_utf8_lossy(&md.description)
                .trim_end_matches('\0')
                .to_string();
            info!(agent = %agent_id, dest = %dest, amount = amount, desc = %desc, "MoneyTransferRequest: accepted (sample economy)");

            let reply = MoneyBalanceReply {
                money_data: money_balance_reply::MoneyData {
                    agent_id,
                    transaction_id: Uuid::new_v4(),
                    transaction_success: true,
                    money_balance: 0,
                    square_meters_credit: 0,
                    square_meters_committed: 0,
                    description: b"Transaction complete\0".to_vec(),
                },
                transaction_info: money_balance_reply::TransactionInfo {
                    transaction_type: md.transaction_type,
                    source_id: agent_id,
                    is_source_group: false,
                    dest_id: dest,
                    is_dest_group: false,
                    amount,
                    item_description: b"\0".to_vec(),
                },
            };
            {
                let mut session = client.lock().await;
                let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                let _ = socket.send_to(&pkt_bytes, src).await;
            }

            let clients_map = clients.read().await;
            for (peer_addr, peer_client) in clients_map.iter() {
                let mut sess = peer_client.lock().await;
                if sess.agent_id == dest {
                    let dest_reply = MoneyBalanceReply {
                        money_data: money_balance_reply::MoneyData {
                            agent_id: dest,
                            transaction_id: Uuid::new_v4(),
                            transaction_success: true,
                            money_balance: 0,
                            square_meters_credit: 0,
                            square_meters_committed: 0,
                            description: b"Payment received\0".to_vec(),
                        },
                        transaction_info: money_balance_reply::TransactionInfo {
                            transaction_type: md.transaction_type,
                            source_id: agent_id,
                            is_source_group: false,
                            dest_id: dest,
                            is_dest_group: false,
                            amount,
                            item_description: b"\0".to_vec(),
                        },
                    };
                    let (pkt_bytes, _) = sess.circuit.send_msg(&dest_reply, true);
                    let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                    break;
                }
            }
        }

        if msg_id == RequestPayPrice::ID
            && let Ok(req) = RequestPayPrice::decode(&pkt.body)
            && let Some(client) = &client
        {
            let object_id = req.object_data.object_id;
            let reply = PayPriceReply {
                object_data: pay_price_reply::ObjectData {
                    object_id,
                    default_pay_price: -1,
                },
                button_data: vec![
                    pay_price_reply::ButtonData { pay_button: -1 },
                    pay_price_reply::ButtonData { pay_button: -1 },
                    pay_price_reply::ButtonData { pay_button: -1 },
                    pay_price_reply::ButtonData { pay_button: -1 },
                ],
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
            info!(agent = %agent_id, object = %object_id, "RequestPayPrice: replied with defaults");
        }

        if msg_id == ObjectBuy::ID
            && let Ok(buy) = ObjectBuy::decode(&pkt.body)
            && let Some(client) = &client
        {
            for obj in &buy.object_data {
                info!(
                    agent = %agent_id,
                    local_id = obj.object_local_id,
                    sale_type = obj.sale_type,
                    "ObjectBuy: accepted (sample economy)"
                );
            }
            let reply = MoneyBalanceReply {
                money_data: money_balance_reply::MoneyData {
                    agent_id,
                    transaction_id: Uuid::new_v4(),
                    transaction_success: true,
                    money_balance: 0,
                    square_meters_credit: 0,
                    square_meters_committed: 0,
                    description: b"Purchase complete\0".to_vec(),
                },
                transaction_info: money_balance_reply::TransactionInfo {
                    transaction_type: 0,
                    source_id: agent_id,
                    is_source_group: false,
                    dest_id: Uuid::nil(),
                    is_dest_group: false,
                    amount: 0,
                    item_description: b"\0".to_vec(),
                },
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
        }

        if msg_id == ModifyLand::ID
            && agent_id != Uuid::nil()
            && let Ok(modify) = ModifyLand::decode(&pkt.body)
        {
            let allowed = perms.can_edit_terrain(agent_id);
            let changed = terraform::apply_modify_land(&scene, allowed, &modify);
            if !allowed {
                tracing::debug!(agent = %agent_id, "ModifyLand refused: no terraform permission");
            }
            terraform::broadcast_patches(&scene, &changed, &socket, &clients).await;
        }

        if msg_id == AgentUpdate::ID
            && let Ok(update) = AgentUpdate::decode(&pkt.body)
        {
            let flags = update.agent_data.control_flags;

            // Detect flying and running flags
            let flying = flags & 0x2000 != 0; // AGENT_CONTROL_FLY
            let fast = flags & 0x1000 != 0 || flags & 0x4000 != 0; // FAST_AT or FAST_UP

            // Ground-clamp body_rotation: zero pitch/roll for non-flying avatars
            let rot = if flying {
                update.agent_data.body_rotation
            } else {
                let q = update.agent_data.body_rotation;
                glam::Quat::from_xyzw(0.0, 0.0, q.z, q.w).normalize()
            };

            // SL coordinate system: forward=+X, left=+Y, up=+Z
            let forward = rot * glam::Vec3::X;
            let left = rot * glam::Vec3::Y;

            let presence_data = scene.get_presence(&update.agent_data.agent_id);

            // Speed constants matching OpenSim: walk 2.4576, run 4.096, fly 16.384
            let base_speed = if flying {
                if fast { 16.384f32 } else { 9.8304f32 }
            } else if fast || always_run.contains(&update.agent_data.agent_id) {
                4.096f32
            } else {
                2.4576f32
            };
            let speed_mod = presence_data.as_ref().map(|p| p.speed_multiplier).unwrap_or(1.0);
            let base_speed = base_speed * speed_mod;

            let mut vel = glam::Vec3::ZERO;
            if flags & 0x0001 != 0 {
                vel += forward * base_speed;
            } // AGENT_CONTROL_AT_POS (forward)
            if flags & 0x0002 != 0 {
                vel -= forward * base_speed;
            } // AGENT_CONTROL_AT_NEG (back)
            if flags & 0x0004 != 0 {
                vel += left * base_speed;
            } // AGENT_CONTROL_LEFT_POS
            if flags & 0x0008 != 0 {
                vel -= left * base_speed;
            } // AGENT_CONTROL_LEFT_NEG (right)
            if flying {
                if flags & 0x0010 != 0 {
                    vel.z += base_speed;
                } // AGENT_CONTROL_UP_POS
                if flags & 0x0020 != 0 {
                    vel.z -= base_speed;
                } // AGENT_CONTROL_UP_NEG
            }

            let prev_pos = presence_data
                .as_ref()
                .map(|p| p.position)
                .unwrap_or(glam::Vec3::new(128.0, 128.0, 22.0));
            let avatar_local_id = presence_data
                .as_ref()
                .map(|p| p.local_id)
                .unwrap_or(1);
            let now = Instant::now();
            let dt = last_agent_update.get(&update.agent_data.agent_id)
                .map(|t| now.duration_since(*t).as_secs_f32())
                .unwrap_or(1.0 / 30.0)
                .min(0.5);
            last_agent_update.insert(update.agent_data.agent_id, now);
            let moved = prev_pos + vel * dt;
            let ground_z = scene.terrain_height_at(moved.x, moved.y);
            // AGENT_CONTROL_UP_POS while walking is a jump.
            let jump = !flying && flags & 0x0010 != 0;
            let vz = avatar_vertical.entry(update.agent_data.agent_id).or_insert(0.0);
            let new_pos = avatar_move::place_on_ground(moved, flying, jump, ground_z, dt, vz);
            if !flying {
                vel.z = *vz; // let viewers extrapolate jumps and falls
            }

            scene.update_presence_position(&update.agent_data.agent_id, new_pos, rot);

            if let Some(_client) = &client {
                let terse_data = encode_terse(
                    avatar_local_id,
                    0,
                    true,
                    Some(glam::Vec4::new(0.0, 0.0, 1.0, -new_pos.z)),
                    new_pos,
                    vel,
                    glam::Vec3::ZERO,
                    rot,
                    glam::Vec3::ZERO,
                );
                let terse_msg = ImprovedTerseObjectUpdate {
                    region_data: improved_terse_object_update::RegionData {
                        region_handle: scene.region_info.region_handle,
                        time_dilation: 65535,
                    },
                    object_data: vec![improved_terse_object_update::ObjectData {
                        data: terse_data,
                        texture_entry: Vec::new(),
                    }],
                };
                // Broadcast terse update to ALL connected clients (not just the mover)
                {
                    let all_clients = clients.read().await;
                    for (addr, c) in all_clients.iter() {
                        let mut sess = c.lock().await;
                        let (pkt_bytes, _) = sess.circuit.send_msg(&terse_msg, false);
                        let _ = socket.send_to(&pkt_bytes, *addr).await;
                    }
                }
            }

            // --- Animation state machine ---
            // Determine desired animation from control flags
            let desired_anim = if flying {
                if flags & 0x0001 != 0 || flags & 0x0002 != 0 || flags & 0x0004 != 0 || flags & 0x0008 != 0 {
                    "aec4610c-757f-bc4e-c092-c6e9caf18daf" // FLY
                } else if flags & 0x0010 != 0 {
                    "62c5de58-cb33-5743-3d07-9e4cd4352864" // HOVER_UP
                } else if flags & 0x0020 != 0 {
                    "20f063ea-8306-2562-0b07-5c23464bcec5" // HOVER_DOWN
                } else {
                    "4ae8016b-31b9-03bb-c401-b1ea941db41d" // HOVER
                }
            } else if flags & 0x0001 != 0 || flags & 0x0002 != 0 || flags & 0x0004 != 0 || flags & 0x0008 != 0 {
                if fast || always_run.contains(&update.agent_data.agent_id) {
                    "05ddbff8-aaa9-92a1-2b74-8fe77a29b445" // RUN
                } else {
                    "6ed24bd8-91aa-4b12-ccc7-c97c857ab4e0" // WALK
                }
            } else if flags & 0x0100 != 0 { // AGENT_CONTROL_TURN_LEFT
                "56e0ba0d-4a9f-7f27-6117-32f2ebbf6135" // TURNLEFT
            } else if flags & 0x0200 != 0 { // AGENT_CONTROL_TURN_RIGHT
                "2d6daa51-3192-6794-8e2e-a15f8338ec30" // TURNRIGHT
            } else {
                "2408fe9e-df1d-1d7d-f4ff-1384fa7b350f" // STAND
            };

            let prev_anim = last_anim.get(&update.agent_data.agent_id).copied();
            if prev_anim != Some(desired_anim) {
                last_anim.insert(update.agent_data.agent_id, desired_anim);
                let anim_msg = AvatarAnimation {
                    sender: avatar_animation::Sender {
                        id: update.agent_data.agent_id,
                    },
                    animation_list: vec![avatar_animation::AnimationList {
                        anim_id: Uuid::parse_str(desired_anim).unwrap(),
                        anim_sequence_id: 1,
                    }],
                    animation_source_list: Vec::new(),
                    physical_avatar_event_list: Vec::new(),
                };
                // Broadcast animation to ALL clients
                let all_clients = clients.read().await;
                for (addr, c) in all_clients.iter() {
                    let mut sess = c.lock().await;
                    let (pkt_bytes, _) = sess.circuit.send_msg(&anim_msg, false);
                    let _ = socket.send_to(&pkt_bytes, *addr).await;
                }
            }
        }

        if msg_id == SetAlwaysRun::ID
            && let Ok(msg) = SetAlwaysRun::decode(&pkt.body)
        {
            if msg.agent_data.always_run {
                always_run.insert(msg.agent_data.agent_id);
            } else {
                always_run.remove(&msg.agent_data.agent_id);
            }
        }

        if msg_id == AgentRequestSit::ID
            && let Ok(req) = AgentRequestSit::decode(&pkt.body)
            && let Some(client) = &client
        {
            let target_id = req.target_object.target_id;
            let sit_offset = req.target_object.offset;
            if let Some(obj) = scene.get_object(&target_id) {
                let sit_pos = obj.root_part().group_position + sit_offset;
                let reply = AvatarSitResponse {
                    sit_object: avatar_sit_response::SitObject { id: target_id },
                    sit_transform: avatar_sit_response::SitTransform {
                        auto_pilot: true,
                        sit_position: sit_pos,
                        sit_rotation: glam::Quat::IDENTITY,
                        camera_eye_offset: glam::Vec3::new(0.0, 0.0, 1.0),
                        camera_at_offset: glam::Vec3::new(0.0, 0.0, 0.0),
                        force_mouselook: false,
                    },
                };
                let mut session = client.lock().await;
                let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                let _ = socket.send_to(&pkt_bytes, src).await;
                info!(agent = %agent_id, target = %target_id, "AgentRequestSit: sent sit response");
            } else {
                info!(agent = %agent_id, target = %target_id, "AgentRequestSit: target object not found");
            }
        }

        if msg_id == AgentSit::ID
            && let Ok(_sit) = AgentSit::decode(&pkt.body)
        {
            info!(agent = %agent_id, "AgentSit: confirmed sit");
        }

        if msg_id == MapBlockRequest::ID
            && let Ok(req) = MapBlockRequest::decode(&pkt.body)
        {
            let ri = &scene.region_info;
            let gx = ri.region_loc_x as u16;
            let gy = ri.region_loc_y as u16;
            let mut data = Vec::new();
            let mut sizes = Vec::new();

            if gx >= req.position_data.min_x && gx <= req.position_data.max_x
                && gy >= req.position_data.min_y && gy <= req.position_data.max_y
            {
                data.push(map_block_reply::Data {
                    x: gx,
                    y: gy,
                    name: ri.region_name.clone().into_bytes(),
                    access: 21,
                    region_flags: 0,
                    water_height: 20,
                    agents: 0,
                    map_image_id: Uuid::nil(),
                });
                sizes.push(map_block_reply::Size {
                    size_x: ri.region_size_x as u16,
                    size_y: ri.region_size_y as u16,
                });
            }

            if let Some(ref gc) = grid_connector {
                let zero_scope = "00000000-0000-0000-0000-000000000000";
                let region_id = ri.region_id.to_string();
                match gc.get_neighbours(zero_scope, &region_id).await {
                    Ok(neighbours) => {
                        for n in neighbours {
                            let nx = (n.loc_x / 256) as u16;
                            let ny = (n.loc_y / 256) as u16;
                            if nx >= req.position_data.min_x && nx <= req.position_data.max_x
                                && ny >= req.position_data.min_y && ny <= req.position_data.max_y
                            {
                                let map_tex = Uuid::parse_str(&n.region_map_texture)
                                    .unwrap_or(Uuid::nil());
                                data.push(map_block_reply::Data {
                                    x: nx,
                                    y: ny,
                                    name: n.region_name.into_bytes(),
                                    access: n.access as u8,
                                    region_flags: n.flags as u32,
                                    water_height: 20,
                                    agents: 0,
                                    map_image_id: map_tex,
                                });
                                sizes.push(map_block_reply::Size {
                                    size_x: n.size_x as u16,
                                    size_y: n.size_y as u16,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to query grid neighbours for map");
                    }
                }
            }

            if !data.is_empty() {
                let reply = MapBlockReply {
                    agent_data: map_block_reply::AgentData {
                        agent_id,
                        flags: req.agent_data.flags,
                    },
                    data,
                    size: sizes,
                };
                if let Some(client) = &client {
                    let mut session = client.lock().await;
                    let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                    let _ = socket.send_to(&pkt_bytes, src).await;
                }
            }
        }

        if msg_id == ChatFromViewer::ID
            && let Ok(chat) = ChatFromViewer::decode(&pkt.body)
        {
            let text = String::from_utf8_lossy(&chat.chat_data.message).to_string();
            info!(
                agent = %agent_id,
                channel = chat.chat_data.channel,
                chat_type = chat.chat_data.type_,
                "chat: {}",
                text
            );
            let sender_name = {
                let mut name = String::new();
                scene.for_each_presence(|p| {
                    if p.uuid == agent_id {
                        name = p.full_name();
                    }
                });
                if name.is_empty() {
                    "Unknown".to_string()
                } else {
                    name
                }
            };
            let sender_pos = scene
                .get_presence(&agent_id)
                .map(|p| p.position)
                .unwrap_or(glam::Vec3::ZERO);
            let mut name_bytes = sender_name.clone().into_bytes();
            name_bytes.push(0);
            let relay = ChatFromSimulator {
                chat_data: chat_from_simulator::ChatData {
                    from_name: name_bytes,
                    source_id: agent_id,
                    owner_id: agent_id,
                    source_type: 1,
                    chat_type: chat.chat_data.type_,
                    audible: 1,
                    position: sender_pos,
                    message: chat.chat_data.message.clone(),
                },
            };
            let chat_type = chat.chat_data.type_;
            if chat_type != 3 && chat_type != 4 {
                let range_sq: f32 = match chat_type {
                    0 => 10.0 * 10.0,
                    1 => 20.0 * 20.0,
                    2 => 100.0 * 100.0,
                    _ => f32::INFINITY,
                };
                let clients_map = clients.read().await;
                for (peer_addr, peer_client) in clients_map.iter() {
                    let mut peer = peer_client.lock().await;
                    if chat_type <= 2 {
                        if let Some(presence) = scene.get_presence(&peer.agent_id) {
                            if sender_pos.distance_squared(presence.position) > range_sq {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }
                    let (pkt_bytes, _) = peer.circuit.send_msg(&relay, true);
                    let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                }
            }

            // Dispatch listen events to scripts on this channel
            if scripts_enabled {
                let channel = chat.chat_data.channel;
                script_scheduler.lock().broadcast_event(ScriptEvent::Listen {
                    channel,
                    name: sender_name,
                    id: agent_id,
                    message: text,
                });
            }
        }

        if msg_id == ObjectAdd::ID
            && let Ok(add) = ObjectAdd::decode(&pkt.body)
        {
            let root = object_add::part_from_object_add(&add);
            let obj_uuid = root.uuid;
            let group = SceneObjectGroup::new(root);
            let _obj_id = scene.add_object(group);
            info!(agent = %add.agent_data.agent_id, object = %obj_uuid, "ObjectAdd: created in scene");

            if let Some(obj) = scene.get_object(&obj_uuid) {
                let prim_obj = scene_sync::full_update_for_part(obj.root_part(), 0, true);
                let obj_msg = ObjectUpdate {
                    region_data: object_update::RegionData {
                        region_handle: scene.region_info.region_handle,
                        time_dilation: 65535,
                    },
                    object_data: vec![prim_obj.to_object_data()],
                };
                let all_clients = clients.read().await;
                for (addr, c) in all_clients.iter() {
                    let mut sess = c.lock().await;
                    let (pkt_bytes, _) = sess.circuit.send_msg(&obj_msg, true);
                    let _ = socket.send_to(&pkt_bytes, *addr).await;
                }
            }
        }

        if msg_id == ObjectDelete::ID
            && let Ok(del) = ObjectDelete::decode(&pkt.body)
        {
            for obj_data in &del.object_data {
                if let Some(uuid) = scene.get_object_by_local_id(obj_data.object_local_id) {
                    let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                        let rp = obj.root_part();
                        perms.can_delete_object(agent_id, rp.owner_id, rp.group_id)
                    });
                    if !allowed {
                        warn!(agent = %agent_id, object = %uuid, "ObjectDelete: permission denied");
                        continue;
                    }
                    scene.remove_object(&uuid);
                    info!(agent = %agent_id, local_id = obj_data.object_local_id, "ObjectDelete: removed");
                }
            }
        }

        // ---- MultipleObjectUpdate handler ----
        if msg_id == MultipleObjectUpdate::ID
            && let Ok(mou) = MultipleObjectUpdate::decode(&pkt.body)
        {
            info!(agent = %mou.agent_data.agent_id, count = mou.object_data.len(), "MultipleObjectUpdate");
            for entry in &mou.object_data {
                let local_id = entry.object_local_id;
                let type_flags = entry.type_;
                let data = &entry.data;

                // Parse position (Vec3) from 12 bytes at the given offset
                let parse_vec3 = |d: &[u8], off: usize| -> Option<glam::Vec3> {
                    if d.len() < off + 12 { return None; }
                    let x = f32::from_le_bytes([d[off], d[off+1], d[off+2], d[off+3]]);
                    let y = f32::from_le_bytes([d[off+4], d[off+5], d[off+6], d[off+7]]);
                    let z = f32::from_le_bytes([d[off+8], d[off+9], d[off+10], d[off+11]]);
                    Some(glam::Vec3::new(x, y, z))
                };

                // Parse rotation (Quat) from 12 bytes (3-component normalized) at the given offset
                let parse_quat = |d: &[u8], off: usize| -> Option<glam::Quat> {
                    if d.len() < off + 12 { return None; }
                    let x = f32::from_le_bytes([d[off], d[off+1], d[off+2], d[off+3]]);
                    let y = f32::from_le_bytes([d[off+4], d[off+5], d[off+6], d[off+7]]);
                    let z = f32::from_le_bytes([d[off+8], d[off+9], d[off+10], d[off+11]]);
                    let w_sq = 1.0_f32 - x*x - y*y - z*z;
                    let w = if w_sq > 0.0 { w_sq.sqrt() } else { 0.0 };
                    Some(glam::Quat::from_xyzw(x, y, z, w))
                };

                let is_group = type_flags & 0x08 != 0;
                let has_pos = type_flags & 0x01 != 0;
                let has_rot = type_flags & 0x02 != 0;
                let has_scale = type_flags & 0x04 != 0;

                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "MultipleObjectUpdate: unknown local_id");
                        continue;
                    }
                };

                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "MultipleObjectUpdate: permission denied");
                    continue;
                }

                let mut new_pos: Option<glam::Vec3> = None;
                let mut new_rot: Option<glam::Quat> = None;
                let mut new_scale: Option<glam::Vec3> = None;
                let mut offset = 0usize;

                if has_pos {
                    new_pos = parse_vec3(data, offset);
                    offset += 12;
                }
                if has_rot {
                    new_rot = parse_quat(data, offset);
                    offset += 12;
                }
                if has_scale {
                    new_scale = parse_vec3(data, offset);
                    let _ = offset;
                }

                // Apply mutations
                scene.with_object_mut(&uuid, |group| {
                    if is_group {
                        if let Some(pos) = new_pos {
                            group.set_position(pos);
                        }
                        if let Some(rot) = new_rot {
                            group.set_rotation(rot);
                        }
                        if let Some(scale) = new_scale {
                            group.root_part_mut().scale = scale;
                        }
                    } else {
                        if let Some(pos) = new_pos {
                            group.root_part_mut().group_position = pos;
                        }
                        if let Some(rot) = new_rot {
                            group.root_part_mut().rotation_offset = rot;
                        }
                        if let Some(scale) = new_scale {
                            group.root_part_mut().scale = scale;
                        }
                    }
                });
                scene.push_object_to_physics(&uuid);

                // Read back updated values for terse broadcast
                if let Some(obj) = scene.get_object(&uuid) {
                    let root = obj.root_part();
                    let terse_local_id = root.local_id;
                    let pos = root.group_position;
                    let rot = root.rotation_offset;
                    broadcast_terse_object_update(
                        terse_local_id,
                        pos,
                        glam::Vec3::ZERO,
                        glam::Vec3::ZERO,
                        rot,
                        glam::Vec3::ZERO,
                        scene.region_info.region_handle,
                        &socket,
                        &clients,
                    ).await;
                }
                info!(local_id, type_flags, is_group, "MultipleObjectUpdate: applied");
            }
        }

        // ---- ObjectSelect handler ----
        if msg_id == ObjectSelect::ID
            && let Ok(sel) = ObjectSelect::decode(&pkt.body)
        {
            info!(agent = %sel.agent_data.agent_id, count = sel.object_data.len(), "ObjectSelect");
            for obj_data in &sel.object_data {
                let local_id = obj_data.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectSelect: unknown local_id");
                        continue;
                    }
                };

                // Mark as selected
                scene.with_object_mut(&uuid, |group| {
                    group.is_selected = true;
                });

                // Build ObjectProperties reply
                if let Some(obj) = scene.get_object(&uuid) {
                    let root = obj.root_part();
                    let props = ObjectProperties {
                        object_data: vec![object_properties::ObjectData {
                            object_id: root.uuid,
                            creator_id: root.creator_id,
                            owner_id: root.owner_id,
                            group_id: root.group_id,
                            creation_date: root.creation_date as u64,
                            base_mask: root.base_mask.bits(),
                            owner_mask: root.owner_mask.bits(),
                            group_mask: root.group_mask.bits(),
                            everyone_mask: root.everyone_mask.bits(),
                            next_owner_mask: root.next_owner_mask.bits(),
                            ownership_cost: root.ownership_cost,
                            sale_type: root.object_sale_type,
                            sale_price: root.sale_price,
                            aggregate_perms: 0,
                            aggregate_perm_textures: 0,
                            aggregate_perm_textures_owner: 0,
                            category: root.category,
                            inventory_serial: root.inventory_serial as i16,
                            item_id: Uuid::nil(),
                            folder_id: root.folder_id,
                            from_task_id: Uuid::nil(),
                            last_owner_id: root.last_owner_id,
                            name: root.name.as_bytes().to_vec(),
                            description: root.description.as_bytes().to_vec(),
                            touch_name: root.touch_name.as_bytes().to_vec(),
                            sit_name: root.sit_name.as_bytes().to_vec(),
                            texture_id: Vec::new(),
                        }],
                    };
                    // Send only to the selecting client
                    if let Some(client) = &client {
                        let mut session = client.lock().await;
                        let (pkt_bytes, _) = session.circuit.send_msg(&props, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;
                    }
                    info!(local_id, object = %uuid, "ObjectSelect: sent ObjectProperties");
                }
            }
        }

        // ---- ObjectDeselect handler ----
        if msg_id == ObjectDeselect::ID
            && let Ok(desel) = ObjectDeselect::decode(&pkt.body)
        {
            info!(agent = %desel.agent_data.agent_id, count = desel.object_data.len(), "ObjectDeselect");
            for obj_data in &desel.object_data {
                let local_id = obj_data.object_local_id;
                if let Some(uuid) = scene.get_object_by_local_id(local_id) {
                    scene.with_object_mut(&uuid, |group| {
                        group.is_selected = false;
                    });
                    info!(local_id, object = %uuid, "ObjectDeselect: deselected");
                } else {
                    warn!(local_id, "ObjectDeselect: unknown local_id");
                }
            }
        }

        // ---- RequestObjectPropertiesFamily handler ----
        if msg_id == RequestObjectPropertiesFamily::ID
            && let Ok(req) = RequestObjectPropertiesFamily::decode(&pkt.body)
        {
            let object_id = req.object_data.object_id;
            let request_flags = req.object_data.request_flags;
            info!(agent = %req.agent_data.agent_id, object = %object_id, flags = request_flags, "RequestObjectPropertiesFamily");

            if let Some(obj) = scene.get_object(&object_id) {
                let root = obj.root_part();
                let reply = ObjectPropertiesFamily {
                    object_data: object_properties_family::ObjectData {
                        request_flags,
                        object_id: root.uuid,
                        owner_id: root.owner_id,
                        group_id: root.group_id,
                        base_mask: root.base_mask.bits(),
                        owner_mask: root.owner_mask.bits(),
                        group_mask: root.group_mask.bits(),
                        everyone_mask: root.everyone_mask.bits(),
                        next_owner_mask: root.next_owner_mask.bits(),
                        ownership_cost: root.ownership_cost,
                        sale_type: root.object_sale_type,
                        sale_price: root.sale_price,
                        category: root.category,
                        last_owner_id: root.last_owner_id,
                        name: root.name.as_bytes().to_vec(),
                        description: root.description.as_bytes().to_vec(),
                    },
                };
                if let Some(client) = &client {
                    let mut session = client.lock().await;
                    let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                    let _ = socket.send_to(&pkt_bytes, src).await;
                }
                info!(object = %object_id, "RequestObjectPropertiesFamily: replied");
            } else {
                warn!(object = %object_id, "RequestObjectPropertiesFamily: object not found");
            }
        }

        // ---- ObjectGrab handler (touch_start) ----
        if msg_id == ObjectGrab::ID
            && let Ok(msg) = ObjectGrab::decode(&pkt.body)
        {
            let local_id = msg.object_data.local_id;
            let grab_offset = msg.object_data.grab_offset;
            let surface_count = msg.surface_info.len();
            if let Some(uuid) = scene.get_object_by_local_id(local_id) {
                info!(
                    local_id,
                    object = %uuid,
                    ?grab_offset,
                    surface_count,
                    "ObjectGrab: touch_start"
                );
                if scripts_enabled {
                    script_scheduler.lock().post_event_to_prim(
                        uuid,
                        ScriptEvent::Touch { agent_id },
                    );
                }
            } else {
                warn!(local_id, "ObjectGrab: unknown local_id");
            }
        }

        // ---- ObjectGrabUpdate handler ----
        if msg_id == ObjectGrabUpdate::ID
            && let Ok(msg) = ObjectGrabUpdate::decode(&pkt.body)
        {
            let object_id = msg.object_data.object_id;
            let grab_position = msg.object_data.grab_position;
            let surface_count = msg.surface_info.len();
            info!(
                object = %object_id,
                ?grab_position,
                surface_count,
                "ObjectGrabUpdate: touch"
            );
            if scripts_enabled {
                script_scheduler.lock().post_event_to_prim(
                    object_id,
                    ScriptEvent::TouchUpdate { agent_id },
                );
            }
        }

        // ---- ObjectDeGrab handler (touch_end) ----
        if msg_id == ObjectDeGrab::ID
            && let Ok(msg) = ObjectDeGrab::decode(&pkt.body)
        {
            let local_id = msg.object_data.local_id;
            let surface_count = msg.surface_info.len();
            if let Some(uuid) = scene.get_object_by_local_id(local_id) {
                info!(
                    local_id,
                    object = %uuid,
                    surface_count,
                    "ObjectDeGrab: touch_end"
                );
                if scripts_enabled {
                    script_scheduler.lock().post_event_to_prim(
                        uuid,
                        ScriptEvent::TouchEnd { agent_id },
                    );
                }
            } else {
                warn!(local_id, "ObjectDeGrab: unknown local_id");
            }
        }

        // ---- ObjectName handler ----
        if msg_id == ObjectName::ID
            && let Ok(msg) = ObjectName::decode(&pkt.body)
        {
            for entry in &msg.object_data {
                let local_id = entry.local_id;
                let new_name = String::from_utf8_lossy(&entry.name).to_string();
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectName: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectName: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    group.root_part_mut().name = new_name.clone();
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, name = %new_name, "ObjectName: updated");
            }
        }

        // ---- ObjectDescription handler ----
        if msg_id == ObjectDescription::ID
            && let Ok(msg) = ObjectDescription::decode(&pkt.body)
        {
            for entry in &msg.object_data {
                let local_id = entry.local_id;
                let new_desc = String::from_utf8_lossy(&entry.description).to_string();
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectDescription: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectDescription: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    group.root_part_mut().description = new_desc.clone();
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, description = %new_desc, "ObjectDescription: updated");
            }
        }

        // ---- ObjectShape handler ----
        if msg_id == ObjectShape::ID
            && let Ok(msg) = ObjectShape::decode(&pkt.body)
        {
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectShape: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectShape: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    let shape = &mut group.root_part_mut().shape;
                    shape.path_curve = entry.path_curve;
                    shape.profile_curve = entry.profile_curve;
                    shape.path_begin = entry.path_begin;
                    shape.path_end = entry.path_end;
                    shape.path_scale_x = entry.path_scale_x;
                    shape.path_scale_y = entry.path_scale_y;
                    shape.path_shear_x = entry.path_shear_x;
                    shape.path_shear_y = entry.path_shear_y;
                    shape.path_twist = entry.path_twist;
                    shape.path_twist_begin = entry.path_twist_begin;
                    shape.path_radius_offset = entry.path_radius_offset;
                    shape.path_taper_x = entry.path_taper_x;
                    shape.path_taper_y = entry.path_taper_y;
                    shape.path_revolutions = entry.path_revolutions;
                    shape.path_skew = entry.path_skew;
                    shape.profile_begin = entry.profile_begin;
                    shape.profile_end = entry.profile_end;
                    shape.profile_hollow = entry.profile_hollow;
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, "ObjectShape: updated");
            }
        }

        // ---- ObjectPermissions handler ----
        if msg_id == ObjectPermissions::ID
            && let Ok(msg) = ObjectPermissions::decode(&pkt.body)
        {
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectPermissions: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectPermissions: permission denied");
                    continue;
                }
                let mask_bits = PermissionMask::from_bits_truncate(entry.mask);
                scene.with_object_mut(&uuid, |group| {
                    let part = group.root_part_mut();
                    let target = match entry.field {
                        1 => &mut part.base_mask,
                        2 => &mut part.owner_mask,
                        4 => &mut part.group_mask,
                        8 => &mut part.everyone_mask,
                        16 => &mut part.next_owner_mask,
                        _ => {
                            warn!(field = entry.field, "ObjectPermissions: unknown field");
                            return;
                        }
                    };
                    if entry.set == 1 {
                        *target = *target | mask_bits;
                    } else {
                        *target = *target & !mask_bits;
                    }
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, field = entry.field, set = entry.set, mask = entry.mask, "ObjectPermissions: updated");
            }
        }

        // ---- ObjectSaleInfo handler ----
        if msg_id == ObjectSaleInfo::ID
            && let Ok(msg) = ObjectSaleInfo::decode(&pkt.body)
        {
            for entry in &msg.object_data {
                let local_id = entry.local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectSaleInfo: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectSaleInfo: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    let part = group.root_part_mut();
                    part.object_sale_type = entry.sale_type;
                    part.sale_price = entry.sale_price;
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, sale_type = entry.sale_type, sale_price = entry.sale_price, "ObjectSaleInfo: updated");
            }
        }

        // ---- ObjectExtraParams handler ----
        if msg_id == ObjectExtraParams::ID
            && let Ok(msg) = ObjectExtraParams::decode(&pkt.body)
        {
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectExtraParams: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectExtraParams: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    group.root_part_mut().shape.extra_params = entry.param_data.clone();
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, param_type = entry.param_type, param_size = entry.param_size, "ObjectExtraParams: updated");
            }
        }

        // ---- ObjectGroup handler ----
        if msg_id == ObjectGroup::ID
            && let Ok(msg) = ObjectGroup::decode(&pkt.body)
        {
            let new_group_id = msg.agent_data.group_id;
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectGroup: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectGroup: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    group.root_part_mut().group_id = new_group_id;
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, group = %new_group_id, "ObjectGroup: updated");
            }
        }

        // ---- ObjectOwner handler ----
        if msg_id == ObjectOwner::ID
            && let Ok(msg) = ObjectOwner::decode(&pkt.body)
        {
            let new_owner_id = msg.header_data.owner_id;
            let new_group_id = msg.header_data.group_id;
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectOwner: unknown local_id");
                        continue;
                    }
                };
                let allowed = scene.get_object(&uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !allowed {
                    warn!(agent = %agent_id, object = %uuid, "ObjectOwner: permission denied");
                    continue;
                }
                scene.with_object_mut(&uuid, |group| {
                    let part = group.root_part_mut();
                    part.last_owner_id = part.owner_id;
                    if new_owner_id.is_nil() {
                        // Deed to group
                        part.group_id = new_group_id;
                    } else {
                        // Transfer ownership
                        part.owner_id = new_owner_id;
                        part.group_id = new_group_id;
                    }
                });
                if let Some(obj) = scene.get_object(&uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(local_id, object = %uuid, owner = %new_owner_id, group = %new_group_id, "ObjectOwner: updated");
            }
        }

        // ---- RezObject handler ----
        if msg_id == RezObject::ID
            && let Ok(msg) = RezObject::decode(&pkt.body)
        {
            let name = String::from_utf8_lossy(&msg.inventory_data.name).to_string();
            let item_id = msg.inventory_data.item_id;
            let position = msg.rez_data.ray_end;
            let mut root = SceneObjectPart::new(&name, msg.agent_data.agent_id);
            root.creator_id = msg.inventory_data.creator_id;
            root.group_position = position;
            root.group_id = msg.agent_data.group_id;
            root.base_mask = PermissionMask::from_bits_truncate(msg.inventory_data.base_mask);
            root.owner_mask = PermissionMask::from_bits_truncate(msg.inventory_data.owner_mask);
            root.group_mask = PermissionMask::from_bits_truncate(msg.inventory_data.group_mask);
            root.everyone_mask = PermissionMask::from_bits_truncate(msg.inventory_data.everyone_mask);
            root.next_owner_mask = PermissionMask::from_bits_truncate(msg.inventory_data.next_owner_mask);
            let obj_uuid = root.uuid;
            let group = SceneObjectGroup::new(root);
            scene.add_object(group);
            if let Some(obj) = scene.get_object(&obj_uuid) {
                broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
            }
            if scripts_enabled {
                script_scheduler.lock().post_event_to_prim(
                    obj_uuid,
                    ScriptEvent::OnRez { start_param: 0 },
                );
            }
            info!(
                agent = %msg.agent_data.agent_id,
                object = %obj_uuid,
                %item_id,
                ?position,
                %name,
                "RezObject: created prim in scene"
            );
        }

        // ---- DeRezObject handler ----
        if msg_id == DeRezObject::ID
            && let Ok(msg) = DeRezObject::decode(&pkt.body)
        {
            let destination = msg.agent_block.destination;
            let mut kill_ids: Vec<u32> = Vec::new();
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "DeRezObject: unknown local_id");
                        continue;
                    }
                };
                // Save script state before removal (for inventory persistence)
                if scripts_enabled && destination != 1 {
                    let sched = script_scheduler.lock();
                    for sid in sched.scripts_on_prim(&uuid) {
                        if let Some(inst) = sched.get_script(&sid) {
                            match inst.save_state() {
                                Ok(_state) => {
                                    tracing::debug!(script = %sid, object = %uuid, "saved script state before derez");
                                }
                                Err(e) => {
                                    tracing::warn!(script = %sid, error = %e, "failed to save script state before derez");
                                }
                            }
                        }
                    }
                }
                match destination {
                    1 => {
                        // TakeCopy: copy to inventory, leave in scene
                        info!(local_id, object = %uuid, "DeRezObject: copied to inventory (TakeCopy)");
                    }
                    4 => {
                        // Take: remove from scene, take to inventory
                        if let Some(_removed) = scene.remove_object(&uuid) {
                            kill_ids.push(local_id);
                            info!(local_id, object = %uuid, "DeRezObject: taken to inventory");
                        }
                    }
                    6 => {
                        // Delete
                        if let Some(_removed) = scene.remove_object(&uuid) {
                            kill_ids.push(local_id);
                            info!(local_id, object = %uuid, "DeRezObject: deleted");
                        }
                    }
                    9 => {
                        // ReturnToOwner
                        if let Some(_removed) = scene.remove_object(&uuid) {
                            kill_ids.push(local_id);
                            info!(local_id, object = %uuid, "DeRezObject: returned to owner");
                        }
                    }
                    other => {
                        if let Some(_removed) = scene.remove_object(&uuid) {
                            kill_ids.push(local_id);
                            info!(local_id, object = %uuid, destination = other, "DeRezObject: removed with destination code");
                        }
                    }
                }
            }
            if !kill_ids.is_empty() {
                let kill = KillObject {
                    object_data: kill_ids.iter().map(|&id| kill_object::ObjectData { id }).collect(),
                };
                let all_clients = clients.read().await;
                for (addr, c) in all_clients.iter() {
                    let mut sess = c.lock().await;
                    let (pkt_bytes, _) = sess.circuit.send_msg(&kill, true);
                    let _ = socket.send_to(&pkt_bytes, *addr).await;
                }
            }
        }

        // ---- ObjectLink handler ----
        if msg_id == ObjectLink::ID
            && let Ok(msg) = ObjectLink::decode(&pkt.body)
        {
            if msg.object_data.len() >= 2 {
                // First entry becomes the root group; remaining become children
                let root_local_id = msg.object_data[0].object_local_id;
                let root_uuid = match scene.get_object_by_local_id(root_local_id) {
                    Some(u) => u,
                    None => {
                        warn!(root_local_id, "ObjectLink: root local_id not found");
                        continue;
                    }
                };
                let link_allowed = scene.get_object(&root_uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !link_allowed {
                    warn!(agent = %agent_id, object = %root_uuid, "ObjectLink: permission denied");
                    continue;
                }
                let root_position = match scene.get_object(&root_uuid) {
                    Some(g) => g.root_part().group_position,
                    None => {
                        warn!(root_local_id, "ObjectLink: root group not found");
                        continue;
                    }
                };

                let mut kill_ids: Vec<u32> = Vec::new();
                let mut parts_to_add: Vec<SceneObjectPart> = Vec::new();

                for entry in &msg.object_data[1..] {
                    let child_local_id = entry.object_local_id;
                    let child_uuid = match scene.get_object_by_local_id(child_local_id) {
                        Some(u) => u,
                        None => {
                            warn!(child_local_id, "ObjectLink: child local_id not found");
                            continue;
                        }
                    };
                    if child_uuid == root_uuid {
                        continue; // skip if same as root
                    }
                    if let Some(child_group) = scene.remove_object(&child_uuid) {
                        let child_root_local_id = child_group.root_part().local_id;
                        kill_ids.push(child_root_local_id);
                        for part in child_group.parts() {
                            let mut p = part.clone();
                            // Recalculate offset relative to root position
                            p.offset_position = p.group_position - root_position;
                            parts_to_add.push(p);
                        }
                    }
                }

                // Add collected parts to root group
                scene.with_object_mut(&root_uuid, |group| {
                    for part in parts_to_add {
                        group.add_part(part);
                    }
                });

                // Broadcast updated root group
                if let Some(obj) = scene.get_object(&root_uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }

                // Send KillObject for removed child groups
                if !kill_ids.is_empty() {
                    let kill = KillObject {
                        object_data: kill_ids.iter().map(|&id| kill_object::ObjectData { id }).collect(),
                    };
                    let all_clients = clients.read().await;
                    for (addr, c) in all_clients.iter() {
                        let mut sess = c.lock().await;
                        let (pkt_bytes, _) = sess.circuit.send_msg(&kill, true);
                        let _ = socket.send_to(&pkt_bytes, *addr).await;
                    }
                }

                info!(
                    root_local_id,
                    object = %root_uuid,
                    linked_count = msg.object_data.len() - 1,
                    "ObjectLink: linked objects"
                );
            }
        }

        // ---- ObjectDelink handler ----
        if msg_id == ObjectDelink::ID
            && let Ok(msg) = ObjectDelink::decode(&pkt.body)
        {
            let mut updated_groups: Vec<Uuid> = Vec::new();
            let mut new_group_uuids: Vec<Uuid> = Vec::new();

            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                // Find which group contains this local_id
                let group_uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectDelink: unknown local_id");
                        continue;
                    }
                };

                let delink_allowed = scene.get_object(&group_uuid).map_or(false, |obj| {
                    let rp = obj.root_part();
                    perms.can_edit_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask)
                });
                if !delink_allowed {
                    warn!(agent = %agent_id, object = %group_uuid, "ObjectDelink: permission denied");
                    continue;
                }

                // Check if this is a child part (not the root)
                let part_info = scene.with_object_mut(&group_uuid, |group| {
                    let root_local_id = group.root_part().local_id;
                    if local_id == root_local_id {
                        // Cannot delink the root part itself
                        return None;
                    }
                    // Find the child part's uuid by local_id
                    let part_uuid = group.get_part_by_local_id(local_id)?.uuid;
                    let root_pos = group.root_part().group_position;
                    if let Some(mut part) = group.remove_part(&part_uuid) {
                        // Calculate world position from group position + offset
                        part.group_position = root_pos + part.offset_position;
                        part.offset_position = glam::Vec3::ZERO;
                        Some(part)
                    } else {
                        None
                    }
                });

                if let Some(Some(part)) = part_info {
                    // Create new group from the delinked part
                    let new_group = SceneObjectGroup::new(part);
                    let new_uuid = scene.add_object(new_group);
                    new_group_uuids.push(new_uuid);
                    if !updated_groups.contains(&group_uuid) {
                        updated_groups.push(group_uuid);
                    }
                    info!(local_id, parent = %group_uuid, new_object = %new_uuid, "ObjectDelink: part delinked to new group");
                } else {
                    warn!(local_id, "ObjectDelink: cannot delink (root part or not found)");
                }
            }

            // Broadcast updates for all affected groups
            for uuid in &updated_groups {
                if let Some(obj) = scene.get_object(uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
            }
            for uuid in &new_group_uuids {
                if let Some(obj) = scene.get_object(uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
            }
        }

        // ---- ObjectDuplicate handler ----
        if msg_id == ObjectDuplicate::ID
            && let Ok(msg) = ObjectDuplicate::decode(&pkt.body)
        {
            let offset = msg.shared_data.offset;
            let copier_id = msg.agent_data.agent_id;
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let original_uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectDuplicate: unknown local_id");
                        continue;
                    }
                };
                let original = match scene.get_object(&original_uuid) {
                    Some(g) => g,
                    None => {
                        warn!(local_id, object = %original_uuid, "ObjectDuplicate: object not found");
                        continue;
                    }
                };

                {
                    let rp = original.root_part();
                    if !perms.can_copy_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask) {
                        warn!(agent = %agent_id, object = %original_uuid, "ObjectDuplicate: permission denied");
                        continue;
                    }
                }

                // Deep copy the root part
                let orig_root = original.root_part();
                let mut new_root = SceneObjectPart::new(&orig_root.name, copier_id);
                new_root.description = orig_root.description.clone();
                new_root.shape = orig_root.shape.clone();
                new_root.scale = orig_root.scale;
                new_root.rotation_offset = orig_root.rotation_offset;
                new_root.group_position = orig_root.group_position + offset;
                new_root.material = orig_root.material;
                new_root.click_action = orig_root.click_action;
                new_root.base_mask = orig_root.base_mask;
                new_root.owner_mask = orig_root.owner_mask;
                new_root.group_mask = orig_root.group_mask;
                new_root.everyone_mask = orig_root.everyone_mask;
                new_root.next_owner_mask = orig_root.next_owner_mask;
                new_root.group_id = msg.agent_data.group_id;

                let mut new_group = SceneObjectGroup::new(new_root);

                // Deep copy child parts
                let child_parts: Vec<SceneObjectPart> = original.parts()
                    .skip(1) // skip root
                    .map(|p| {
                        let mut cp = SceneObjectPart::new(&p.name, copier_id);
                        cp.description = p.description.clone();
                        cp.shape = p.shape.clone();
                        cp.scale = p.scale;
                        cp.rotation_offset = p.rotation_offset;
                        cp.offset_position = p.offset_position;
                        cp.material = p.material;
                        cp.click_action = p.click_action;
                        cp.base_mask = p.base_mask;
                        cp.owner_mask = p.owner_mask;
                        cp.group_mask = p.group_mask;
                        cp.everyone_mask = p.everyone_mask;
                        cp.next_owner_mask = p.next_owner_mask;
                        cp
                    })
                    .collect();
                for cp in child_parts {
                    new_group.add_part(cp);
                }

                let new_uuid = scene.add_object(new_group);
                if let Some(obj) = scene.get_object(&new_uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(
                    local_id,
                    original = %original_uuid,
                    copy = %new_uuid,
                    ?offset,
                    "ObjectDuplicate: duplicated"
                );
            }
        }

        // ---- ObjectDuplicateOnRay handler ----
        if msg_id == ObjectDuplicateOnRay::ID
            && let Ok(msg) = ObjectDuplicateOnRay::decode(&pkt.body)
        {
            let ray_end = msg.agent_data.ray_end;
            let copier_id = msg.agent_data.agent_id;
            for entry in &msg.object_data {
                let local_id = entry.object_local_id;
                let original_uuid = match scene.get_object_by_local_id(local_id) {
                    Some(u) => u,
                    None => {
                        warn!(local_id, "ObjectDuplicateOnRay: unknown local_id");
                        continue;
                    }
                };
                let original = match scene.get_object(&original_uuid) {
                    Some(g) => g,
                    None => {
                        warn!(local_id, object = %original_uuid, "ObjectDuplicateOnRay: object not found");
                        continue;
                    }
                };

                {
                    let rp = original.root_part();
                    if !perms.can_copy_object(agent_id, rp.owner_id, rp.group_id, rp.owner_mask) {
                        warn!(agent = %agent_id, object = %original_uuid, "ObjectDuplicateOnRay: permission denied");
                        continue;
                    }
                }

                // Deep copy root part, place at ray_end
                let orig_root = original.root_part();
                let mut new_root = SceneObjectPart::new(&orig_root.name, copier_id);
                new_root.description = orig_root.description.clone();
                new_root.shape = orig_root.shape.clone();
                new_root.scale = orig_root.scale;
                new_root.rotation_offset = orig_root.rotation_offset;
                new_root.group_position = ray_end;
                new_root.material = orig_root.material;
                new_root.click_action = orig_root.click_action;
                new_root.base_mask = orig_root.base_mask;
                new_root.owner_mask = orig_root.owner_mask;
                new_root.group_mask = orig_root.group_mask;
                new_root.everyone_mask = orig_root.everyone_mask;
                new_root.next_owner_mask = orig_root.next_owner_mask;
                new_root.group_id = msg.agent_data.group_id;

                let mut new_group = SceneObjectGroup::new(new_root);

                // Deep copy child parts
                let child_parts: Vec<SceneObjectPart> = original.parts()
                    .skip(1)
                    .map(|p| {
                        let mut cp = SceneObjectPart::new(&p.name, copier_id);
                        cp.description = p.description.clone();
                        cp.shape = p.shape.clone();
                        cp.scale = p.scale;
                        cp.rotation_offset = p.rotation_offset;
                        cp.offset_position = p.offset_position;
                        cp.material = p.material;
                        cp.click_action = p.click_action;
                        cp.base_mask = p.base_mask;
                        cp.owner_mask = p.owner_mask;
                        cp.group_mask = p.group_mask;
                        cp.everyone_mask = p.everyone_mask;
                        cp.next_owner_mask = p.next_owner_mask;
                        cp
                    })
                    .collect();
                for cp in child_parts {
                    new_group.add_part(cp);
                }

                let new_uuid = scene.add_object(new_group);
                if let Some(obj) = scene.get_object(&new_uuid) {
                    broadcast_full_object_update(&obj, scene.region_info.region_handle, &socket, &clients).await;
                }
                info!(
                    local_id,
                    original = %original_uuid,
                    copy = %new_uuid,
                    ?ray_end,
                    "ObjectDuplicateOnRay: duplicated at ray position"
                );
            }
        }

        if msg_id == AgentThrottle::ID
            && let Ok(throttle_msg) = AgentThrottle::decode(&pkt.body)
            && let Some(client) = &client
        {
            let mut session = client.lock().await;
            session
                .throttle
                .set_from_viewer(&throttle_msg.throttle.throttles);
            tracing::debug!(agent = %agent_id, "throttle updated from viewer");
        }

        if msg_id == RequestImage::ID
            && let Ok(req) = RequestImage::decode(&pkt.body)
            && let Some(client) = &client
        {
            let mut session = client.lock().await;
            for img in &req.request_image {
                let reply = ImageNotInDatabase {
                    image_id: image_not_in_database::ImageID { id: img.image },
                };
                let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                let _ = socket.send_to(&pkt_bytes, src).await;
            }
            tracing::debug!(agent = %agent_id, count = req.request_image.len(), "RequestImage: replied ImageNotInDatabase");
        }

        if msg_id == AgentDataUpdateRequest::ID
            && let Some(client) = &client
        {
            let mut session = client.lock().await;
            let name = scene
                .get_presence(&session.agent_id)
                .map(|p| (p.first_name.clone(), p.last_name.clone()))
                .unwrap_or_else(|| ("Unknown".to_string(), "User".to_string()));
            let reply = AgentDataUpdate {
                agent_data: agent_data_update::AgentData {
                    agent_id: session.agent_id,
                    first_name: name.0.into_bytes(),
                    last_name: name.1.into_bytes(),
                    group_title: Vec::new(),
                    active_group_id: Uuid::nil(),
                    group_powers: 0,
                    group_name: Vec::new(),
                },
            };
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
        }

        if msg_id == AvatarPropertiesRequest::ID
            && let Ok(req) = AvatarPropertiesRequest::decode(&pkt.body)
            && let Some(client) = &client
        {
            let target = req.agent_data.avatar_id;
            let (first, last) = scene.get_presence(&target)
                .map(|p| (p.first_name.clone(), p.last_name.clone()))
                .unwrap_or_else(|| ("Unknown".to_string(), "User".to_string()));
            let born_str = "2024-01-01";
            let about = format!("{} {}", first, last);
            let reply = AvatarPropertiesReply {
                agent_data: avatar_properties_reply::AgentData {
                    agent_id,
                    avatar_id: target,
                },
                properties_data: avatar_properties_reply::PropertiesData {
                    image_id: Uuid::nil(),
                    fl_image_id: Uuid::nil(),
                    partner_id: Uuid::nil(),
                    about_text: about.into_bytes(),
                    fl_about_text: Vec::new(),
                    born_on: born_str.as_bytes().to_vec(),
                    profile_url: Vec::new(),
                    charter_member: Vec::new(),
                    flags: 0,
                },
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
            info!(agent = %agent_id, target = %target, "sent AvatarPropertiesReply");
        }

        if msg_id == AgentSetAppearance::ID
            && let Ok(appearance) = AgentSetAppearance::decode(&pkt.body)
        {
            let texture_entry = appearance.object_data.texture_entry.clone();
            let visual_params: Vec<u8> = appearance
                .visual_param
                .iter()
                .map(|vp| vp.param_value)
                .collect();
            scene.update_presence_appearance(
                &agent_id,
                texture_entry.clone(),
                visual_params.clone(),
            );
            info!(agent = %agent_id, params = visual_params.len(), tex_len = texture_entry.len(), "AgentSetAppearance: stored and broadcasting");

            let visual_param_blocks: Vec<avatar_appearance::VisualParam> = visual_params
                .iter()
                .map(|&v| avatar_appearance::VisualParam { param_value: v })
                .collect();
            let broadcast = AvatarAppearance {
                sender: avatar_appearance::Sender {
                    id: agent_id,
                    is_trial: false,
                },
                object_data: avatar_appearance::ObjectData { texture_entry },
                visual_param: visual_param_blocks,
                appearance_data: vec![avatar_appearance::AppearanceData {
                    appearance_version: 0,
                    cof_version: 0,
                    flags: 0,
                }],
                appearance_hover: Vec::new(),
                attachment_block: Vec::new(),
            };
            let clients_map = clients.read().await;
            for (peer_addr, peer_client) in clients_map.iter() {
                let mut peer = peer_client.lock().await;
                let (pkt_bytes, _) = peer.circuit.send_msg(&broadcast, true);
                let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
            }

            // Send updated ObjectUpdate with real baked texture_entry so
            // the viewer replaces the initial default-gray textures.
            if let Some(presence) = scene.get_presence(&agent_id) {
                let avatar_obj = FullObjectUpdate {
                    local_id: presence.local_id,
                    full_id: presence.uuid,
                    parent_id: 0,
                    pcode: 47,
                    state: 0,
                    crc: 0,
                    material: 0,
                    click_action: 0,
                    scale: glam::Vec3::new(0.45, 0.6, 1.75),
                    position: presence.position,
                    velocity: glam::Vec3::ZERO,
                    acceleration: glam::Vec3::ZERO,
                    rotation: presence.rotation,
                    angular_velocity: glam::Vec3::ZERO,
                    update_flags: 0x2000_0001,
                    owner_id: presence.uuid,
                    path_curve: 0, profile_curve: 0,
                    path_begin: 0, path_end: 0,
                    path_scale_x: 0, path_scale_y: 0,
                    path_shear_x: 0, path_shear_y: 0,
                    path_twist: 0, path_twist_begin: 0,
                    path_radius_offset: 0,
                    path_taper_x: 0, path_taper_y: 0,
                    path_revolutions: 0, path_skew: 0,
                    profile_begin: 0, profile_end: 0, profile_hollow: 0,
                    texture_entry: presence.texture_entry.clone(),
                    texture_anim: Vec::new(),
                    name_value: format!(
                        "FirstName STRING RW SV {}\nLastName STRING RW SV {}\n",
                        presence.first_name, presence.last_name
                    ).into_bytes(),
                    data: Vec::new(),
                    text: String::new(),
                    text_color: [0, 0, 0, 0],
                    media_url: String::new(),
                    ps_block: Vec::new(),
                    extra_params: Vec::new(),
                    sound_id: Uuid::nil(),
                    sound_gain: 0.0,
                    sound_flags: 0,
                    sound_radius: 0.0,
                    foot_plane: Some(glam::Vec4::new(0.0, 0.0, 1.0, 0.0)),
                };
                let obj_msg = ObjectUpdate {
                    region_data: object_update::RegionData {
                        region_handle: scene.region_info.region_handle,
                        time_dilation: 65535,
                    },
                    object_data: vec![avatar_obj.to_object_data()],
                };
                let clients_map2 = clients.read().await;
                for (peer_addr, peer_client) in clients_map2.iter() {
                    let mut peer = peer_client.lock().await;
                    let (pkt_bytes, _) = peer.circuit.send_msg(&obj_msg, true);
                    let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                }
            }
        }

        if msg_id == TeleportLocationRequest::ID {
            if let Ok(req) = TeleportLocationRequest::decode(&pkt.body) {
                if req.info.region_handle == scene.region_info.region_handle {
                    if let Some(client) = &client {
                        let mut session = client.lock().await;

                        let start_msg = TeleportStart {
                            info: teleport_start::Info { teleport_flags: 0 },
                        };
                        let (pkt_bytes, _) = session.circuit.send_msg(&start_msg, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;

                        let look_rot = if req.info.look_at.length() < 0.001 {
                            glam::Quat::IDENTITY
                        } else {
                            glam::Quat::from_rotation_arc(
                                glam::Vec3::Y,
                                req.info.look_at.normalize(),
                            )
                        };
                        scene.update_presence_position(&agent_id, req.info.position, look_rot);

                        let local_msg = TeleportLocal {
                            info: teleport_local::Info {
                                agent_id,
                                location_id: 0,
                                position: req.info.position,
                                look_at: req.info.look_at,
                                teleport_flags: 0,
                            },
                        };
                        let (pkt_bytes, _) = session.circuit.send_msg(&local_msg, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;

                        info!(agent = %agent_id, pos = ?req.info.position, "local teleport complete");
                    }
                } else {
                    let mut transfer = welkin_modules::EntityTransferModule::new();
                    transfer.region_handle = scene.region_info.region_handle;
                    transfer.max_transfer_distance = 4095.0;
                    let tp_result = transfer.teleport(
                        agent_id,
                        req.info.region_handle,
                        [req.info.position.x, req.info.position.y, req.info.position.z],
                        [req.info.look_at.x, req.info.look_at.y, req.info.look_at.z],
                        welkin_modules::transfer::TeleportFlags::VIA_LOCATION,
                    );
                    if let Some(client) = &client {
                        let mut session = client.lock().await;
                        match tp_result {
                            Ok(()) => {
                                info!(
                                    agent = %agent_id,
                                    dest_handle = req.info.region_handle,
                                    "cross-region teleport initiated via EntityTransferModule"
                                );
                                let fail_msg = TeleportFailed {
                                    info: teleport_failed::Info {
                                        agent_id,
                                        reason: b"Cross-region teleport is not supported yet: the destination region cannot accept incoming agents.".to_vec(),
                                    },
                                    alert_info: Vec::new(),
                                };
                                let (pkt_bytes, _) = session.circuit.send_msg(&fail_msg, true);
                                let _ = socket.send_to(&pkt_bytes, src).await;
                            }
                            Err(reason) => {
                                let fail_msg = TeleportFailed {
                                    info: teleport_failed::Info {
                                        agent_id,
                                        reason: reason.as_bytes().to_vec(),
                                    },
                                    alert_info: Vec::new(),
                                };
                                let (pkt_bytes, _) = session.circuit.send_msg(&fail_msg, true);
                                let _ = socket.send_to(&pkt_bytes, src).await;
                                warn!(agent = %agent_id, %reason, "cross-region teleport failed");
                            }
                        }
                    }
                }
            }
        }

        // --- EstateOwnerMessage handler ---
        if msg_id == EstateOwnerMessage::ID {
            if let Ok(req) = EstateOwnerMessage::decode(&pkt.body) {
                let method = String::from_utf8_lossy(&req.method_data.method)
                    .trim_end_matches('\0')
                    .to_string();
                let params: Vec<String> = req
                    .param_list
                    .iter()
                    .map(|p| {
                        String::from_utf8_lossy(&p.parameter)
                            .trim_end_matches('\0')
                            .to_string()
                    })
                    .collect();
                match method.as_str() {
                    "setregioninfo" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: setregioninfo");
                    }
                    "setregionterrain" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: setregionterrain");
                    }
                    "restart" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: restart requested");
                    }
                    "texturebase" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: texturebase");
                    }
                    "texturedetail" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: texturedetail");
                    }
                    "setcovenantid" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: setcovenantid");
                    }
                    "estateaccessdelta" => {
                        if params.len() >= 2 {
                            let mask = params[0].parse::<u32>().unwrap_or(0);
                            let target = Uuid::parse_str(&params[1]).unwrap_or(Uuid::nil());
                            info!(
                                agent = %agent_id,
                                mask = mask,
                                target = %target,
                                "EstateOwnerMessage: estateaccessdelta"
                            );
                        }
                    }
                    "simulatormessage" => {
                        let text = params.first().map(|s| s.as_str()).unwrap_or("");
                        info!(agent = %agent_id, message = %text, "EstateOwnerMessage: simulatormessage");
                        let alert = AlertMessage {
                            alert_data: alert_message::AlertData {
                                message: text.as_bytes().to_vec(),
                            },
                            alert_info: Vec::new(),
                            agent_info: Vec::new(),
                        };
                        let all_clients = clients.read().await;
                        for (peer_addr, peer_client) in all_clients.iter() {
                            let mut peer = peer_client.lock().await;
                            let (pkt_bytes, _) = peer.circuit.send_msg(&alert, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                        }
                    }
                    "instantmessage" => {
                        let text = params.first().map(|s| s.as_str()).unwrap_or("");
                        info!(agent = %agent_id, message = %text, "EstateOwnerMessage: instantmessage");
                    }
                    "setregiondebug" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: setregiondebug");
                    }
                    "terraincollisionheight" => {
                        info!(agent = %agent_id, params = ?params, "EstateOwnerMessage: terraincollisionheight");
                    }
                    other => {
                        info!(agent = %agent_id, method = %other, params = ?params, "EstateOwnerMessage: unhandled method");
                    }
                }
            }
        }

        // --- FreezeUser handler ---
        if msg_id == FreezeUser::ID {
            if let Ok(req) = FreezeUser::decode(&pkt.body) {
                let target_id = req.data.target_id;
                let freeze = req.data.flags == 0;
                info!(
                    agent = %agent_id,
                    target = %target_id,
                    action = if freeze { "freeze" } else { "unfreeze" },
                    "FreezeUser"
                );
                if scene.get_presence(&target_id).is_some() {
                    let alert_text = if freeze {
                        "You have been frozen by an administrator."
                    } else {
                        "You have been unfrozen."
                    };
                    let all_clients = clients.read().await;
                    for (peer_addr, peer_client) in all_clients.iter() {
                        let is_target = {
                            let peer = peer_client.lock().await;
                            peer.agent_id == target_id
                        };
                        if is_target {
                            let alert = AgentAlertMessage {
                                agent_data: agent_alert_message::AgentData {
                                    agent_id: target_id,
                                },
                                alert_data: agent_alert_message::AlertData {
                                    modal: false,
                                    message: alert_text.as_bytes().to_vec(),
                                },
                            };
                            let mut peer = peer_client.lock().await;
                            let (pkt_bytes, _) = peer.circuit.send_msg(&alert, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            break;
                        }
                    }
                } else {
                    warn!(target = %target_id, "FreezeUser: target presence not found");
                }
            }
        }

        // --- EjectUser handler ---
        if msg_id == EjectUser::ID {
            if let Ok(req) = EjectUser::decode(&pkt.body) {
                let target_id = req.data.target_id;
                let also_ban = (req.data.flags & 1) != 0;
                info!(
                    agent = %agent_id,
                    target = %target_id,
                    ban = also_ban,
                    "EjectUser"
                );
                let all_clients = clients.read().await;
                for (peer_addr, peer_client) in all_clients.iter() {
                    let is_target = {
                        let peer = peer_client.lock().await;
                        peer.agent_id == target_id
                    };
                    if is_target {
                        let fail_msg = TeleportFailed {
                            info: teleport_failed::Info {
                                agent_id: target_id,
                                reason: b"You have been ejected from this region.".to_vec(),
                            },
                            alert_info: Vec::new(),
                        };
                        let mut peer = peer_client.lock().await;
                        let (pkt_bytes, _) = peer.circuit.send_msg(&fail_msg, true);
                        let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                        break;
                    }
                }
                drop(all_clients);
                scene.remove_presence(&target_id);
                info!(target = %target_id, "ejected user from region");
            }
        }

        // --- GodUpdateRegionInfo handler ---
        if msg_id == GodUpdateRegionInfo::ID {
            if let Ok(req) = GodUpdateRegionInfo::decode(&pkt.body) {
                let new_name = String::from_utf8_lossy(&req.region_info.sim_name)
                    .trim_end_matches('\0')
                    .to_string();
                info!(
                    agent = %agent_id,
                    new_name = %new_name,
                    estate_id = req.region_info.estate_id,
                    region_flags = req.region_info.region_flags,
                    "GodUpdateRegionInfo"
                );
            }
        }

        // --- RequestRegionInfo handler ---
        if msg_id == RequestRegionInfo::ID {
            if let Ok(_req) = RequestRegionInfo::decode(&pkt.body) {
                if let Some(client) = &client {
                    let reply = RegionInfo {
                        agent_data: region_info::AgentData {
                            agent_id,
                            session_id,
                        },
                        region_info: region_info::RegionInfo {
                            sim_name: scene.region_info.region_name.as_bytes().to_vec(),
                            estate_id: 1,
                            parent_estate_id: 1,
                            region_flags: 0,
                            sim_access: 13,
                            max_agents: 40,
                            billable_factor: 0.0,
                            object_bonus_factor: 1.0,
                            water_height: 20.0,
                            terrain_raise_limit: 100.0,
                            terrain_lower_limit: -100.0,
                            price_per_meter: 0,
                            redirect_grid_x: 0,
                            redirect_grid_y: 0,
                            use_estate_sun: true,
                            sun_hour: 0.0,
                        },
                        region_info2: region_info::RegionInfo2 {
                            product_sku: Vec::new(),
                            product_name: b"OpenSimulator-Rust".to_vec(),
                            max_agents32: 40,
                            hard_max_agents: 100,
                            hard_max_objects: 15000,
                        },
                        region_info3: Vec::new(),
                        region_info5: Vec::new(),
                        combat_settings: Vec::new(),
                    };
                    let mut session = client.lock().await;
                    let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                    let _ = socket.send_to(&pkt_bytes, src).await;
                    info!(agent = %agent_id, "sent RegionInfo reply");
                }
            }
        }

        if msg_id == UUIDNameRequest::ID
            && let Ok(req) = UUIDNameRequest::decode(&pkt.body)
            && let Some(client) = &client
        {
            let mut reply_blocks = Vec::new();
            for block in &req.uuid_name_block {
                let target_id = block.id;
                let mut first = b"Unknown".to_vec();
                let mut last = b"User".to_vec();
                if let Some(p) = scene.get_presence(&target_id) {
                    first = p.first_name.as_bytes().to_vec();
                    last = p.last_name.as_bytes().to_vec();
                }
                first.push(0);
                last.push(0);
                reply_blocks.push(uuid_name_reply::UUIDNameBlock {
                    id: target_id,
                    first_name: first,
                    last_name: last,
                });
            }
            let reply = UUIDNameReply {
                uuid_name_block: reply_blocks,
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
            info!(agent = %agent_id, count = req.uuid_name_block.len(), "UUIDNameRequest: replied");
        }

        if msg_id == UUIDGroupNameRequest::ID
            && let Ok(req) = UUIDGroupNameRequest::decode(&pkt.body)
            && let Some(client) = &client
        {
            let mut reply_blocks = Vec::new();
            for block in &req.uuid_name_block {
                let mut name = b"Unknown".to_vec();
                name.push(0);
                reply_blocks.push(uuid_group_name_reply::UUIDNameBlock {
                    id: block.id,
                    group_name: name,
                });
            }
            let reply = UUIDGroupNameReply {
                uuid_name_block: reply_blocks,
            };
            let mut session = client.lock().await;
            let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
            let _ = socket.send_to(&pkt_bytes, src).await;
            info!(agent = %agent_id, count = req.uuid_name_block.len(), "UUIDGroupNameRequest: replied");
        }

        if msg_id == ViewerEffect::ID
            && let Ok(ve) = ViewerEffect::decode(&pkt.body)
        {
            let clients_map = clients.read().await;
            for (peer_addr, peer_client) in clients_map.iter() {
                if *peer_addr == src {
                    continue;
                }
                let mut sess = peer_client.lock().await;
                let (pkt_bytes, _) = sess.circuit.send_msg(&ve, false);
                let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
            }
        }

        if msg_id == ScriptDialogReply::ID
            && let Ok(reply) = ScriptDialogReply::decode(&pkt.body)
        {
            let channel = reply.data.chat_channel;
            let object_id = reply.data.object_id;
            let label = String::from_utf8_lossy(&reply.data.button_label)
                .trim_end_matches('\0')
                .to_string();
            info!(
                agent = %agent_id,
                object = %object_id,
                channel = channel,
                button = %label,
                "ScriptDialogReply: button clicked"
            );
            let sender_name = {
                let mut name = String::new();
                scene.for_each_presence(|p| {
                    if p.uuid == agent_id {
                        name = p.full_name();
                    }
                });
                if name.is_empty() {
                    "Unknown".to_string()
                } else {
                    name
                }
            };
            let mut name_bytes = sender_name.into_bytes();
            name_bytes.push(0);
            let chat_msg = ChatFromSimulator {
                chat_data: chat_from_simulator::ChatData {
                    from_name: name_bytes,
                    source_id: agent_id,
                    owner_id: agent_id,
                    source_type: 1,
                    chat_type: 0,
                    audible: 1,
                    position: scene
                        .get_presence(&agent_id)
                        .map(|p| p.position)
                        .unwrap_or(glam::Vec3::ZERO),
                    message: label.into_bytes(),
                },
            };
            if channel == 0 {
                let clients_map = clients.read().await;
                for (peer_addr, peer_client) in clients_map.iter() {
                    let mut sess = peer_client.lock().await;
                    let (pkt_bytes, _) = sess.circuit.send_msg(&chat_msg, true);
                    let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                }
            }
        }

        if msg_id == ImprovedInstantMessage::ID
            && let Ok(im) = ImprovedInstantMessage::decode(&pkt.body)
        {
            let mb = &im.message_block;
            let dialog = mb.dialog;
            let to_agent = mb.to_agent_id;
            let im_session_id = mb.id;
            let now_ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as u32)
                .unwrap_or(0);

            let build_relay = |dlg: u8| -> ImprovedInstantMessage {
                ImprovedInstantMessage {
                    agent_data: improved_instant_message::AgentData {
                        agent_id,
                        session_id,
                    },
                    message_block: improved_instant_message::MessageBlock {
                        from_group: mb.from_group,
                        to_agent_id: to_agent,
                        parent_estate_id: mb.parent_estate_id,
                        region_id: scene.region_info.region_id,
                        position: mb.position,
                        offline: 0,
                        dialog: dlg,
                        id: im_session_id,
                        timestamp: now_ts,
                        from_agent_name: mb.from_agent_name.clone(),
                        message: mb.message.clone(),
                        binary_bucket: mb.binary_bucket.clone(),
                    },
                    estate_block: improved_instant_message::EstateBlock::default(),
                    meta_data: Vec::new(),
                }
            };

            match dialog {
                // 0: MessageFromAgent
                0 => {
                    let relay = build_relay(0);
                    let mut delivered = false;
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            delivered = true;
                            break;
                        }
                    }
                    if delivered {
                        info!(from = %agent_id, to = %to_agent, "IM: personal message relayed");
                    } else {
                        warn!(from = %agent_id, to = %to_agent, "IM: target not online in region, offline storage pending");
                    }
                }
                // 4: InventoryOffered
                4 => {
                    let relay = build_relay(4);
                    let clients_map = clients.read().await;
                    let mut delivered = false;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            delivered = true;
                            break;
                        }
                    }
                    if delivered {
                        info!(from = %agent_id, to = %to_agent, "IM: inventory offer relayed");
                    } else {
                        warn!(from = %agent_id, to = %to_agent, "IM: inventory offer target not online");
                    }
                }
                // 5: InventoryAccepted
                5 => {
                    let relay = build_relay(5);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, "IM: inventory accepted relayed");
                            break;
                        }
                    }
                }
                // 6: InventoryDeclined
                6 => {
                    let relay = build_relay(6);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, "IM: inventory declined relayed");
                            break;
                        }
                    }
                }
                // 20: SessionSend (from_group=true) or BusyAutoResponse (from_group=false)
                20 => {
                    if mb.from_group {
                        // SessionSend — group IM fan-out
                        if !group_sessions.is_agent_active(im_session_id, agent_id) {
                            group_sessions.invite_agent(im_session_id, agent_id);
                        }
                        let recipients: Vec<Uuid> = group_sessions
                            .active_agents(im_session_id)
                            .into_iter()
                            .filter(|a| *a != agent_id)
                            .collect();
                        let clients_map = clients.read().await;
                        let mut count = 0u32;
                        for (peer_addr, peer_client) in clients_map.iter() {
                            let mut sess = peer_client.lock().await;
                            if recipients.contains(&sess.agent_id) {
                                let relay = build_relay(20);
                                let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                                let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                                count += 1;
                            }
                        }
                        info!(from = %agent_id, group = %im_session_id, delivered = count, "IM: group session send");
                    } else {
                        // BusyAutoResponse — relay to target
                        let relay = build_relay(20);
                        let clients_map = clients.read().await;
                        for (peer_addr, peer_client) in clients_map.iter() {
                            let mut sess = peer_client.lock().await;
                            if sess.agent_id == to_agent {
                                let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                                let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                                info!(from = %agent_id, to = %to_agent, "IM: busy auto-response relayed");
                                break;
                            }
                        }
                    }
                }
                // 22: RequestTeleport (lure)
                22 => {
                    let relay = build_relay(22);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, "IM: teleport lure relayed");
                            break;
                        }
                    }
                }
                // 38: FriendshipOffered
                38 => {
                    let relay = build_relay(38);
                    let clients_map = clients.read().await;
                    let mut delivered = false;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            delivered = true;
                            break;
                        }
                    }
                    if delivered {
                        info!(from = %agent_id, to = %to_agent, "IM: friendship offer relayed");
                    } else {
                        warn!(from = %agent_id, to = %to_agent, "IM: friendship offer target not online");
                    }
                }
                // 39: FriendshipAccepted
                39 => {
                    let relay = build_relay(39);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, "IM: friendship accepted relayed");
                            break;
                        }
                    }
                }
                // 40: FriendshipDeclined
                40 => {
                    let relay = build_relay(40);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, "IM: friendship declined relayed");
                            break;
                        }
                    }
                }
                // 41: StartTyping
                41 => {
                    let relay = build_relay(41);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, false);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            break;
                        }
                    }
                }
                // 42: StopTyping
                42 => {
                    let relay = build_relay(42);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, false);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            break;
                        }
                    }
                }
                // 3: GroupInvitation
                3 => {
                    let relay = build_relay(3);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, group = %im_session_id, "IM: group invitation relayed");
                            break;
                        }
                    }
                }
                // 17: SessionGroupStart
                17 => {
                    group_sessions.invite_agent(im_session_id, agent_id);
                    let relay = build_relay(17);
                    if let Some(client) = &client {
                        let mut sess = client.lock().await;
                        let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;
                    }
                    info!(agent = %agent_id, group = %im_session_id, "IM: group session started");
                }
                // 32: GroupNotice
                32 => {
                    let recipients: Vec<Uuid> = group_sessions
                        .active_agents(im_session_id)
                        .into_iter()
                        .filter(|a| *a != agent_id)
                        .collect();
                    let clients_map = clients.read().await;
                    let mut count = 0u32;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if recipients.contains(&sess.agent_id) {
                            let relay = build_relay(32);
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            count += 1;
                        }
                    }
                    info!(from = %agent_id, group = %im_session_id, delivered = count, "IM: group notice sent");
                }
                // 33: GroupNoticeInventoryAccepted
                33 => {
                    let relay = build_relay(33);
                    let clients_map = clients.read().await;
                    for (peer_addr, peer_client) in clients_map.iter() {
                        let mut sess = peer_client.lock().await;
                        if sess.agent_id == to_agent {
                            let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                            let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                            info!(from = %agent_id, to = %to_agent, "IM: group notice inventory accepted");
                            break;
                        }
                    }
                }
                // 37: GroupNoticeRequested
                37 => {
                    let relay = build_relay(37);
                    if let Some(client) = &client {
                        let mut sess = client.lock().await;
                        let (pkt_bytes, _) = sess.circuit.send_msg(&relay, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;
                    }
                    info!(agent = %agent_id, notice = %im_session_id, "IM: group notice requested");
                }
                other => {
                    warn!(from = %agent_id, dialog = other, "IM: unhandled dialog type");
                }
            }
        }
    }
}

async fn send_movement_complete(
    socket: &Arc<UdpSocket>,
    client: &Arc<Mutex<ClientSession>>,
    addr: SocketAddr,
    region_info: &SceneRegionInfo,
    scene: &Arc<Scene>,
    event_queues: &EventQueueMap,
    caps_base: &str,
    accounts: &Arc<welkin_services::remote::accounts::RemoteAccountsConnector>,
) {
    let mut session = client.lock().await;

    let (first_name, last_name) = accounts
        .get_name(&session.agent_id.to_string())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to resolve avatar name from ROBUST");
            ("Unknown".to_string(), "User".to_string())
        });

    let movement = AgentMovementComplete {
        agent_data: agent_movement_complete::AgentData {
            agent_id: session.agent_id,
            session_id: session.session_id,
        },
        data: agent_movement_complete::Data {
            position: glam::Vec3::new(128.0, 128.0, 22.0),
            look_at: glam::Vec3::new(1.0, 0.0, 0.0),
            region_handle: region_info.region_handle,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as u32)
                .unwrap_or(0),
        },
        sim_data: agent_movement_complete::SimData {
            channel_version: b"OpenSimulator-Rust 0.1".to_vec(),
        },
    };

    let (pkt, _seq) = session.circuit.send_msg(&movement, true);
    if let Err(e) = socket.send_to(&pkt, addr).await {
        tracing::warn!(error = %e, "failed to send AgentMovementComplete");
        return;
    }
    info!(agent = %session.agent_id, "sent AgentMovementComplete");

    let handshake = RegionHandshake {
        region_info: region_handshake::RegionInfo {
            region_flags: 0,
            sim_access: 13, // Mature
            sim_name: region_info.region_name.as_bytes().to_vec(),
            sim_owner: Uuid::nil(),
            is_estate_manager: false,
            water_height: 20.0,
            billable_factor: 0.0,
            cache_id: Uuid::new_v4(),
            terrain_base0: Uuid::parse_str("0bc58228-74a0-7e83-89bc-5c23464bcec5").unwrap(),
            terrain_base1: Uuid::parse_str("63338ede-0037-c4fd-855b-015d77112fc8").unwrap(),
            terrain_base2: Uuid::parse_str("303cd381-8560-7579-23f1-f0a880799740").unwrap(),
            terrain_base3: Uuid::parse_str("beb169c7-11ea-fff2-eed5-3f368ce60d3d").unwrap(),
            terrain_detail0: Uuid::parse_str("0bc58228-74a0-7e83-89bc-5c23464bcec5").unwrap(),
            terrain_detail1: Uuid::parse_str("63338ede-0037-c4fd-855b-015d77112fc8").unwrap(),
            terrain_detail2: Uuid::parse_str("303cd381-8560-7579-23f1-f0a880799740").unwrap(),
            terrain_detail3: Uuid::parse_str("beb169c7-11ea-fff2-eed5-3f368ce60d3d").unwrap(),
            terrain_start_height00: 10.0,
            terrain_start_height01: 10.0,
            terrain_start_height10: 10.0,
            terrain_start_height11: 10.0,
            terrain_height_range00: 60.0,
            terrain_height_range01: 60.0,
            terrain_height_range10: 60.0,
            terrain_height_range11: 60.0,
        },
        region_info2: region_handshake::RegionInfo2 {
            region_id: region_info.region_id,
        },
        region_info3: region_handshake::RegionInfo3 {
            cpu_class_id: 0,
            cpu_ratio: 0,
            colo_name: Vec::new(),
            product_sku: Vec::new(),
            product_name: b"OpenSimulator-Rust".to_vec(),
        },
        region_info4: vec![region_handshake::RegionInfo4 {
            region_flags_extended: 0,
            region_protocols: 0,
        }],
    };

    let (pkt, _seq) = session.circuit.send_msg(&handshake, true);
    if let Err(e) = socket.send_to(&pkt, addr).await {
        tracing::warn!(error = %e, "failed to send RegionHandshake");
        return;
    }
    info!(agent = %session.agent_id, region = %region_info.region_name, "sent RegionHandshake");

    // Send the region's terrain (the scene's height field), split into packet-sized LayerData.
    let terrain_payloads = scene.terrain_layer_payloads(&terraform::all_patches());
    for data in &terrain_payloads {
        let raw_payload = terraform::layer_data_raw(data);
        let (pkt, _seq) = session.circuit.send_raw(&raw_payload, false, true);
        if let Err(e) = socket.send_to(&pkt, addr).await {
            tracing::warn!(error = %e, "failed to send LayerData");
            return;
        }
    }
    info!(agent = %session.agent_id, packets = terrain_payloads.len(), "sent terrain LayerData");

    let avatar_local_id = scene.allocate_local_id();
    let (obj_pkt, wear_pkt, app_pkt) =
        build_avatar_packets(&mut session, region_info.region_handle, avatar_local_id, &first_name, &last_name);
    for (pkt, name) in [
        (obj_pkt, "ObjectUpdate"),
        (wear_pkt, "AgentWearablesUpdate"),
        (app_pkt, "AvatarAppearance"),
    ] {
        if let Err(e) = socket.send_to(&pkt, addr).await {
            tracing::warn!(error = %e, msg = name, "failed to send avatar packet");
            return;
        }
    }
    info!(agent = %session.agent_id, "sent avatar appearance packets");

    let stand_anim = AvatarAnimation {
        sender: avatar_animation::Sender {
            id: session.agent_id,
        },
        animation_list: vec![avatar_animation::AnimationList {
            anim_id: Uuid::parse_str("2408fe9e-df1d-1d7d-f4ff-1384fa7b350f").unwrap(),
            anim_sequence_id: 0,
        }],
        animation_source_list: Vec::new(),
        physical_avatar_event_list: Vec::new(),
    };
    let (pkt, _) = session.circuit.send_msg(&stand_anim, false);
    if let Err(e) = socket.send_to(&pkt, addr).await {
        tracing::warn!(error = %e, "failed to send AvatarAnimation (stand)");
    }
    info!(agent = %session.agent_id, "sent default standing animation");

    let agent_data_msg = AgentDataUpdate {
        agent_data: agent_data_update::AgentData {
            agent_id: session.agent_id,
            first_name: first_name.as_bytes().to_vec(),
            last_name: last_name.as_bytes().to_vec(),
            group_title: Vec::new(),
            active_group_id: Uuid::nil(),
            group_powers: 0,
            group_name: Vec::new(),
        },
    };
    let (pkt, _) = session.circuit.send_msg(&agent_data_msg, true);
    if let Err(e) = socket.send_to(&pkt, addr).await {
        tracing::warn!(error = %e, "failed to send AgentDataUpdate");
        return;
    }
    info!(agent = %session.agent_id, "sent AgentDataUpdate");

    let mut presence = ScenePresence::new(session.agent_id, &first_name, &last_name, PresenceType::User);
    presence.local_id = avatar_local_id;
    presence.position = glam::Vec3::new(128.0, 128.0, 22.0);
    presence.region_handle = region_info.region_handle;
    scene.add_presence(presence);
    info!(agent = %session.agent_id, "registered presence in scene");

    let eq = Arc::new(welkin_caps::event_queue::EventQueue::new());
    {
        let mut eq_body = Map::new();
        eq_body.insert(
            "agent-id".into(),
            Llsd::String(session.agent_id.to_string()),
        );
        eq_body.insert(
            "sim-ip-and-port".into(),
            Llsd::String(format!(
                "{}:{}",
                region_info.external_host_name, region_info.internal_port
            )),
        );
        eq_body.insert(
            "seed-capability".into(),
            Llsd::String(format!("{}/CAPS/seed", caps_base)),
        );
        eq.enqueue("EstablishAgentCommunication", Llsd::Map(eq_body))
            .await;
    }
    event_queues.write().await.insert(session.agent_id, eq);
    info!(agent = %session.agent_id, "event queue created with initial event");
}

/// Broadcast a full ObjectUpdate for the given scene object to all connected clients.
async fn broadcast_full_object_update(
    obj: &SceneObjectGroup,
    region_handle: u64,
    socket: &UdpSocket,
    clients: &ClientMap,
) {
    // Root prim plus every linkset child, batched to stay under the MTU.
    let batches = scene_sync::batch_updates(scene_sync::group_updates(obj), 3);
    let all_clients = clients.read().await;
    for batch in batches {
        let obj_msg = ObjectUpdate {
            region_data: object_update::RegionData {
                region_handle,
                time_dilation: 65535,
            },
            object_data: batch.iter().map(|u| u.to_object_data()).collect(),
        };
        for (addr, c) in all_clients.iter() {
            let mut sess = c.lock().await;
            let (pkt_bytes, _) = sess.circuit.send_msg(&obj_msg, true);
            let _ = socket.send_to(&pkt_bytes, *addr).await;
        }
    }
}

/// Broadcast an ImprovedTerseObjectUpdate for one prim to all connected clients.
async fn broadcast_terse_object_update(
    local_id: u32,
    pos: glam::Vec3,
    vel: glam::Vec3,
    accel: glam::Vec3,
    rot: glam::Quat,
    ang_vel: glam::Vec3,
    region_handle: u64,
    socket: &UdpSocket,
    clients: &ClientMap,
) {
    let terse_data = encode_terse(
        local_id,
        0,
        false,
        None,
        pos,
        vel,
        accel,
        rot,
        ang_vel,
    );
    let terse_msg = ImprovedTerseObjectUpdate {
        region_data: improved_terse_object_update::RegionData {
            region_handle,
            time_dilation: 65535,
        },
        object_data: vec![improved_terse_object_update::ObjectData {
            data: terse_data,
            texture_entry: Vec::new(),
        }],
    };
    let all_clients = clients.read().await;
    for (addr, c) in all_clients.iter() {
        let mut sess = c.lock().await;
        let (pkt_bytes, _) = sess.circuit.send_msg(&terse_msg, false);
        let _ = socket.send_to(&pkt_bytes, *addr).await;
    }
}

fn build_default_prim_te() -> Vec<u8> {
    let te = TextureEntry::default();
    pack_texture_entry(&te, 6)
}

fn build_default_avatar_te() -> Vec<u8> {
    let img_default_avatar =
        uuid::Uuid::parse_str("c228d1cf-4b5d-4ba8-84f4-899a0796aa97").unwrap();
    let mut te = TextureEntry::default();
    for &idx in &[8, 9, 10, 11, 20] {
        te.faces[idx].texture = img_default_avatar;
    }
    pack_texture_entry(&te, 45)
}

fn build_avatar_packets(
    session: &mut ClientSession,
    region_handle: u64,
    local_id: u32,
    first_name: &str,
    last_name: &str,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let avatar_obj = FullObjectUpdate {
        local_id,
        full_id: session.agent_id,
        parent_id: 0,
        pcode: 47,
        state: 0,
        crc: 0,
        material: 0,
        click_action: 0,
        scale: glam::Vec3::new(0.45, 0.6, 1.75),
        position: glam::Vec3::new(128.0, 128.0, 22.0),
        velocity: glam::Vec3::ZERO,
        acceleration: glam::Vec3::ZERO,
        rotation: glam::Quat::IDENTITY,
        angular_velocity: glam::Vec3::ZERO,
        update_flags: 0x2000_0001,
        owner_id: session.agent_id,
        path_curve: 0,
        profile_curve: 0,
        path_begin: 0,
        path_end: 0,
        path_scale_x: 0,
        path_scale_y: 0,
        path_shear_x: 0,
        path_shear_y: 0,
        path_twist: 0,
        path_twist_begin: 0,
        path_radius_offset: 0,
        path_taper_x: 0,
        path_taper_y: 0,
        path_revolutions: 0,
        path_skew: 0,
        profile_begin: 0,
        profile_end: 0,
        profile_hollow: 0,
        texture_entry: build_default_avatar_te(),
        texture_anim: Vec::new(),
        name_value: format!(
            "FirstName STRING RW SV {}\nLastName STRING RW SV {}\n",
            first_name, last_name
        ).into_bytes(),
        data: Vec::new(),
        text: String::new(),
        text_color: [0, 0, 0, 0],
        media_url: String::new(),
        ps_block: Vec::new(),
        extra_params: Vec::new(),
        sound_id: Uuid::nil(),
        sound_gain: 0.0,
        sound_flags: 0,
        sound_radius: 0.0,
        foot_plane: Some(glam::Vec4::new(0.0, 0.0, 1.0, 0.0)),
    };

    let obj_msg = ObjectUpdate {
        region_data: object_update::RegionData {
            region_handle,
            time_dilation: 65535,
        },
        object_data: vec![avatar_obj.to_object_data()],
    };
    let (obj_pkt, _) = session.circuit.send_msg(&obj_msg, true);

    let wearables_msg = AgentWearablesUpdate {
        agent_data: agent_wearables_update::AgentData {
            agent_id: session.agent_id,
            session_id: session.session_id,
            serial_num: 1,
        },
        wearable_data: default_wearables(),
    };
    let (wear_pkt, _) = session.circuit.send_msg(&wearables_msg, true);

    let visual_params: Vec<avatar_appearance::VisualParam> = (0..253)
        .map(|_| avatar_appearance::VisualParam { param_value: 128 })
        .collect();
    let appearance_msg = AvatarAppearance {
        sender: avatar_appearance::Sender {
            id: session.agent_id,
            is_trial: false,
        },
        object_data: avatar_appearance::ObjectData {
            texture_entry: build_default_avatar_te(),
        },
        visual_param: visual_params,
        appearance_data: vec![avatar_appearance::AppearanceData {
            appearance_version: 0,
            cof_version: 0,
            flags: 0,
        }],
        appearance_hover: Vec::new(),
        attachment_block: Vec::new(),
    };
    let (app_pkt, _) = session.circuit.send_msg(&appearance_msg, true);

    (obj_pkt, wear_pkt, app_pkt)
}

const KNOWN_CAPS: &[&str] = &[
    "EventQueueGet",
    "GetTexture",
    "GetMesh",
    "GetMesh2",
    "SimulatorFeatures",
    "RegionInfo",
    "AgentPreferences",
    "MapLayer",
    "NewFileAgentInventory",
    "UpdateNotecardAgentInventory",
    "UpdateScriptAgent",
    "FetchInventoryDescendents2",
    "FetchInventory2",
    "FetchLib2",
    "ViewerAsset",
    "GetDisplayNames",
    "HomeLocation",
    "CreateInventoryCategory",
    "UploadBakedTexture",
    "ViewerStats",
    "UpdateAgentLanguage",
    "UpdateAgentInformation",
    "ParcelVoiceInfoRequest",
    "ProvisionVoiceAccountRequest",
];

async fn seed_caps_handler(State(base_url): State<String>, body: Bytes) -> impl IntoResponse {
    let requested = from_xml(&body).unwrap_or(Llsd::Array(Vec::new()));
    let names: Vec<String> = match &requested {
        Llsd::Array(arr) => arr.iter().map(|v| v.as_str().to_string()).collect(),
        _ => Vec::new(),
    };

    let mut map = Map::new();
    for name in &names {
        if KNOWN_CAPS.contains(&name.as_str()) {
            map.insert(name.clone(), Llsd::String(format!("{}/{}", base_url, name)));
        }
    }

    let xml = to_xml_string(&Llsd::Map(map));
    (
        axum::http::StatusCode::OK,
        [("content-type", "application/llsd+xml")],
        xml,
    )
}

async fn currency_php_handler(body: Bytes) -> impl IntoResponse {
    let body_str = String::from_utf8_lossy(&body);

    let response_xml = if body_str.contains("getCurrencyQuote") {
        r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>success</name><value><boolean>1</boolean></value></member>
<member><name>currency</name><value><struct>
<member><name>estimatedCost</name><value><string>&lt;$0.00&gt;</string></value></member>
<member><name>currencyBuy</name><value><int>0</int></value></member>
</struct></value></member>
<member><name>confirm</name><value><string></string></value></member>
</struct></value></param></params></methodResponse>"#
    } else if body_str.contains("buyCurrency") {
        r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>success</name><value><boolean>1</boolean></value></member>
</struct></value></param></params></methodResponse>"#
    } else {
        r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>success</name><value><boolean>0</boolean></value></member>
</struct></value></param></params></methodResponse>"#
    };

    (
        axum::http::StatusCode::OK,
        [("Content-Type", "text/xml")],
        response_xml,
    )
}

async fn landtool_php_handler(body: Bytes) -> impl IntoResponse {
    let body_str = String::from_utf8_lossy(&body);

    let response_xml = if body_str.contains("preflightBuyLandPrep") {
        r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>success</name><value><boolean>1</boolean></value></member>
<member><name>currency</name><value><struct>
<member><name>estimatedCost</name><value><string>&lt;$0.00&gt;</string></value></member>
</struct></value></member>
<member><name>membership</name><value><struct>
<member><name>upgrade</name><value><boolean>0</boolean></value></member>
<member><name>action</name><value><string></string></value></member>
<member><name>levels</name><value><array><data></data></array></value></member>
</struct></value></member>
<member><name>landUse</name><value><struct>
<member><name>upgrade</name><value><boolean>0</boolean></value></member>
<member><name>action</name><value><string></string></value></member>
</struct></value></member>
<member><name>confirm</name><value><string></string></value></member>
</struct></value></param></params></methodResponse>"#
    } else if body_str.contains("buyLandPrep") {
        r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>success</name><value><boolean>1</boolean></value></member>
</struct></value></param></params></methodResponse>"#
    } else {
        r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>success</name><value><boolean>0</boolean></value></member>
</struct></value></param></params></methodResponse>"#
    };

    (
        axum::http::StatusCode::OK,
        [("Content-Type", "text/xml")],
        response_xml,
    )
}

/// Redacts credentials from a connection string for safe logging.
fn redact_connection_string(url: &str) -> String {
    if let Some(at) = url.find('@')
        && let Some(scheme_end) = url.find("://")
    {
        return format!("{}://***@{}", &url[..scheme_end], &url[at + 1..]);
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
        let info = SceneRegionInfo::new("Test Region", 1000, 2000);
        assert_eq!(info.region_name, "Test Region");
        assert_eq!(info.region_loc_x, 1000);
        assert_eq!(info.region_loc_y, 2000);
        // Handle = ((loc_x * 256) << 32) | (loc_y * 256)
        let expected_handle = ((1000u64 * 256) << 32) | (2000u64 * 256);
        assert_eq!(info.region_handle, expected_handle);
        assert_eq!(info.region_size_x, 256);
        assert_eq!(info.region_size_y, 256);
    }

    #[test]
    fn test_config_database_default() {
        let config = Config::default();
        let db_url =
            config.get_string_or("DatabaseService", "ConnectionString", "sqlite:region.db");
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

    #[test]
    fn test_server_address_construction() {
        let port: u16 = 9000;
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        assert_eq!(addr.port(), 9000);
        assert!(addr.ip().is_unspecified());

        // Caps port is internal_port + 1
        let caps_port = port.wrapping_add(1);
        let caps_addr = SocketAddr::from(([0, 0, 0, 0], caps_port));
        assert_eq!(caps_addr.port(), 9001);

        // Edge case: port 65535 wraps to 0
        let max_port: u16 = 65535;
        let wrapped = max_port.wrapping_add(1);
        assert_eq!(wrapped, 0);
    }

    #[test]
    fn test_cancellation_token_propagates() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let clone = parent.clone();

        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());
        assert!(!clone.is_cancelled());

        parent.cancel();

        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
        assert!(clone.is_cancelled());
    }
}
