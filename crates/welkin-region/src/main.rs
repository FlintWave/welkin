// SPDX-License-Identifier: GPL-3.0-or-later

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
use welkin_modules::{
    AttachmentsModule, AvatarFactoryModule, CapsModule, ChatModule, DefaultPermissionsModule,
    DynAttrsModule, EstateManagementModule, LandManagementModule, ModuleConfig, SceneRef,
    ServiceThrottleModule,
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
use welkin_scene::scene::{RegionInfo, Scene, SceneConfig};
use welkin_msg::welkin_types::PermissionMask;

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
            config.get_string_or("DatabaseService", "ConnectionString", "sqlite:region.db");

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

    fn build_region_info(&self) -> RegionInfo {
        let mut info = RegionInfo::new(&self.region_name, self.region_loc_x, self.region_loc_y);
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
    let frame_time = region_config.scene_config.frame_time;
    let physics_enabled = region_config.scene_config.physics_enabled;
    let scene = Arc::new(Scene::new(region_info, region_config.scene_config));
    scene.start();

    if physics_enabled {
        let physics =
            welkin_physics_rapier::RapierPhysicsScene::new(&scene.region_info.region_name);
        scene.set_physics(Box::new(physics));
        info!("Physics engine (Rapier) initialised");
    }

    // --- Module system ---
    let scene_ref = SceneRef::new(
        scene.region_info.region_id,
        scene.region_info.region_name.clone(),
    );
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
        if let Err(e) = registry.initialise_all(&ConfigBridge(&config)) {
            warn!(error = %e, "module initialisation failed");
        }
        if let Err(e) = registry.add_region_all(&scene_ref) {
            warn!(error = %e, "module add_region failed");
        }
        info!(count = registry.module_count(), "modules initialised");
    }

    // --- Grid service registration ---
    let grid_reg = match region_config.grid_service_url {
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
            let registration = welkin_services::registration::GridRegistration::new(
                connector,
                region_row,
                reqwest::Client::new(),
            );
            match registration.startup().await {
                Ok(()) => info!(region = %scene.region_info.region_name, "Registered with grid"),
                Err(e) => warn!(error = %e, "Failed to register region with grid"),
            }
            Some(registration)
        }
        None => {
            warn!("No GridService URL configured; skipping grid registration");
            None
        }
    };

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

    let mut caps_router = cap_server.into_router();
    for (name, method_router) in
        welkin_caps::handlers::cap_routes(asset_state, inv_state, bake_state.clone())
    {
        caps_router = caps_router.route(&format!("/{name}"), method_router);
    }
    caps_router = caps_router.merge(welkin_caps::handlers::bake_upload_route(bake_state));
    caps_router = caps_router.route(
        "/EventQueueGet",
        axum::routing::post(welkin_caps::handlers::event_queue_get)
            .with_state(event_queues.clone()),
    );
    caps_router = caps_router.route(
        "/CAPS/{*path}",
        axum::routing::post(seed_caps_handler).with_state(caps_base.clone()),
    );

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

    // --- Heartbeat loop ---
    let heartbeat_cancel = cancel.clone();
    let heartbeat_scene = Arc::clone(&scene);
    let frame_time_ms = (frame_time * 1000.0) as u64;
    let heartbeat_handle = tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(frame_time_ms.max(50)));
        loop {
            tokio::select! {
                _ = heartbeat_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let _stats = heartbeat_scene.tick();
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

    // --- Wait for shutdown signal ---
    welkin_runtime::shutdown_signal().await;
    info!("Initiating graceful shutdown");

    cancel.cancel();

    // Wait for background tasks to finish.
    let _ = udp_handle.await;
    let _ = process_handle.await;
    let _ = caps_handle.await;
    let _ = heartbeat_handle.await;
    let _ = coarse_handle.await;
    let _ = timeout_handle.await;

    if let Some(ref reg) = grid_reg
        && let Err(e) = reg.shutdown().await
    {
        warn!(error = %e, "Failed to deregister from grid");
    }

    {
        let mut registry = scene_ref.registry.write();
        let _ = registry.close_all();
    }
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
) {
    let mut last_agent_update: HashMap<Uuid, Instant> = HashMap::new();
    let mut last_anim: HashMap<Uuid, &'static str> = HashMap::new();
    let mut always_run: HashSet<Uuid> = HashSet::new();
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
            )
            .await;

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
                        ));
                    }
                });
                if !existing_presences.is_empty() {
                    let mut session = client.lock().await;
                    for (ex_id, ex_local_id, ex_pos, ex_rot, ex_first, ex_last) in &existing_presences {
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
                            texture_entry: build_default_avatar_te(),
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
                        let visual_params: Vec<avatar_appearance::VisualParam> = (0..253)
                            .map(|_| avatar_appearance::VisualParam { param_value: 128 })
                            .collect();
                        let appearance_msg = AvatarAppearance {
                            sender: avatar_appearance::Sender {
                                id: *ex_id,
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

            // Speed constants matching OpenSim: walk 2.4576, run 4.096, fly 16.384
            let base_speed = if flying {
                if fast { 16.384f32 } else { 9.8304f32 }
            } else if fast || always_run.contains(&update.agent_data.agent_id) {
                4.096f32
            } else {
                2.4576f32
            };

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

            let presence_data = scene.get_presence(&update.agent_data.agent_id);
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
            let mut new_pos = prev_pos + vel * dt;
            new_pos.x = new_pos.x.clamp(0.0, 256.0);
            new_pos.y = new_pos.y.clamp(0.0, 256.0);
            if !flying && new_pos.z < 22.0 {
                new_pos.z = 22.0;
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
            let pd = &req.position_data;
            if gx >= pd.min_x && gx <= pd.max_x && gy >= pd.min_y && gy <= pd.max_y {
                let reply = MapBlockReply {
                    agent_data: map_block_reply::AgentData {
                        agent_id,
                        flags: req.agent_data.flags,
                    },
                    data: vec![map_block_reply::Data {
                        x: gx,
                        y: gy,
                        name: ri.region_name.clone().into_bytes(),
                        access: 21,
                        region_flags: 0,
                        water_height: 20,
                        agents: 0,
                        map_image_id: Uuid::nil(),
                    }],
                    size: vec![map_block_reply::Size {
                        size_x: ri.region_size_x as u16,
                        size_y: ri.region_size_y as u16,
                    }],
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
            let mut name_bytes = sender_name.into_bytes();
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
            let clients_map = clients.read().await;
            for (peer_addr, peer_client) in clients_map.iter() {
                let mut peer = peer_client.lock().await;
                let (pkt_bytes, _) = peer.circuit.send_msg(&relay, true);
                let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
            }
        }

        if msg_id == ObjectAdd::ID
            && let Ok(add) = ObjectAdd::decode(&pkt.body)
        {
            let mut root = SceneObjectPart::default();
            root.uuid = Uuid::new_v4();
            root.owner_id = add.agent_data.agent_id;
            root.group_position = add.object_data.ray_end;
            root.scale = add.object_data.scale;
            root.rotation_offset = add.object_data.rotation;
            root.name = "Object".to_string();
            let obj_uuid = root.uuid;
            let obj_pos = root.group_position;
            let obj_scale = root.scale;
            let obj_rot = root.rotation_offset;
            let group = SceneObjectGroup::new(root);
            let _obj_id = scene.add_object(group);
            info!(agent = %add.agent_data.agent_id, object = %obj_uuid, "ObjectAdd: created in scene");

            if let Some(obj) = scene.get_object(&obj_uuid) {
                let local_id = obj.root_part().local_id;
                let prim_obj = FullObjectUpdate {
                    local_id,
                    full_id: obj_uuid,
                    parent_id: 0,
                    pcode: 9,
                    state: 0,
                    crc: 0,
                    material: 3,
                    click_action: 0,
                    scale: obj_scale,
                    position: obj_pos,
                    velocity: glam::Vec3::ZERO,
                    acceleration: glam::Vec3::ZERO,
                    rotation: obj_rot,
                    angular_velocity: glam::Vec3::ZERO,
                    update_flags: 0x0001_000D,
                    owner_id: add.agent_data.agent_id,
                    path_curve: 16,
                    profile_curve: 1,
                    path_begin: 0,
                    path_end: 0,
                    path_scale_x: 100,
                    path_scale_y: 100,
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
                    texture_entry: build_default_prim_te(),
                    texture_anim: Vec::new(),
                    name_value: Vec::new(),
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
                    foot_plane: None,
                };
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
                .unwrap_or_else(|| ("Test".to_string(), "User".to_string()));
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
                    if let Some(client) = &client {
                        let mut session = client.lock().await;
                        let fail_msg = TeleportFailed {
                            info: teleport_failed::Info {
                                agent_id,
                                reason: b"Cross-region teleport not supported".to_vec(),
                            },
                            alert_info: Vec::new(),
                        };
                        let (pkt_bytes, _) = session.circuit.send_msg(&fail_msg, true);
                        let _ = socket.send_to(&pkt_bytes, src).await;
                    }
                    warn!(agent = %agent_id, handle = req.info.region_handle, "cross-region teleport not yet supported");
                }
            }
        }
    }
}

async fn send_movement_complete(
    socket: &Arc<UdpSocket>,
    client: &Arc<Mutex<ClientSession>>,
    addr: SocketAddr,
    region_info: &RegionInfo,
    scene: &Arc<Scene>,
    event_queues: &EventQueueMap,
    caps_base: &str,
) {
    let mut session = client.lock().await;

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

    // Send terrain data — flat terrain so viewer sees ground
    let terrain_height = 21.0f32;
    let patches_per_packet = 20;
    let mut all_patches: Vec<(u32, u32)> = Vec::new();
    for py in 0..16u32 {
        for px in 0..16u32 {
            all_patches.push((px, py));
        }
    }

    for chunk in all_patches.chunks(patches_per_packet) {
        let terrain_data = build_flat_terrain_data(terrain_height, chunk);
        let raw_payload = build_layer_data_raw(&terrain_data);
        let (pkt, _seq) = session.circuit.send_raw(&raw_payload, false, true);
        if let Err(e) = socket.send_to(&pkt, addr).await {
            tracing::warn!(error = %e, "failed to send LayerData");
            return;
        }
    }
    info!(agent = %session.agent_id, "sent terrain LayerData (256 flat patches)");

    let (obj_pkt, wear_pkt, app_pkt) =
        build_avatar_packets(&mut session, region_info.region_handle);
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
            first_name: b"Test".to_vec(),
            last_name: b"User".to_vec(),
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

    let mut presence = ScenePresence::new(session.agent_id, "Test", "User", PresenceType::User);
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
    let root = obj.root_part();
    let shape = &root.shape;
    let prim_obj = FullObjectUpdate {
        local_id: root.local_id,
        full_id: root.uuid,
        parent_id: root.parent_id,
        pcode: 9,
        state: 0,
        crc: 0,
        material: root.material,
        click_action: root.click_action,
        scale: root.scale,
        position: root.group_position,
        velocity: root.velocity,
        acceleration: root.acceleration,
        rotation: root.rotation_offset,
        angular_velocity: root.angular_velocity,
        update_flags: 0x0001_000D,
        owner_id: root.owner_id,
        path_curve: shape.path_curve,
        profile_curve: shape.profile_curve,
        path_begin: shape.path_begin,
        path_end: shape.path_end,
        path_scale_x: shape.path_scale_x,
        path_scale_y: shape.path_scale_y,
        path_shear_x: shape.path_shear_x,
        path_shear_y: shape.path_shear_y,
        path_twist: shape.path_twist,
        path_twist_begin: shape.path_twist_begin,
        path_radius_offset: shape.path_radius_offset,
        path_taper_x: shape.path_taper_x,
        path_taper_y: shape.path_taper_y,
        path_revolutions: shape.path_revolutions,
        path_skew: shape.path_skew,
        profile_begin: shape.profile_begin,
        profile_end: shape.profile_end,
        profile_hollow: shape.profile_hollow,
        texture_entry: build_default_prim_te(),
        texture_anim: root.texture_animation.clone(),
        name_value: Vec::new(),
        data: Vec::new(),
        text: root.text.clone(),
        text_color: [
            root.text_color.r(),
            root.text_color.g(),
            root.text_color.b(),
            root.text_color.a(),
        ],
        media_url: root.media_url.clone(),
        ps_block: root.particle_system.clone(),
        extra_params: Vec::new(),
        sound_id: root.sound_id,
        sound_gain: root.sound_gain,
        sound_flags: root.sound_flags,
        sound_radius: root.sound_radius,
        foot_plane: None,
    };
    let obj_msg = ObjectUpdate {
        region_data: object_update::RegionData {
            region_handle,
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
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let avatar_obj = FullObjectUpdate {
        local_id: 1,
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
        name_value: b"FirstName STRING RW SV Test\nLastName STRING RW SV User\n".to_vec(),
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

fn build_flat_terrain_data(height: f32, patches: &[(u32, u32)]) -> Vec<u8> {
    let mut bits: Vec<u8> = Vec::with_capacity(512);

    let push = |bits: &mut Vec<u8>, value: u32, count: u32| {
        let mut remaining = count;
        let mut byte_index: u32 = 0;
        while remaining > 0 {
            let dsize = remaining.min(8);
            remaining -= dsize;
            let byte_val = (value >> (8 * byte_index)) & 0xFF;
            for b in (0..dsize).rev() {
                bits.push(((byte_val >> b) & 1) as u8);
            }
            byte_index += 1;
        }
    };

    // Group header
    push(&mut bits, 264, 16); // stride
    push(&mut bits, 16, 8); // patch_size
    push(&mut bits, 0x4C, 8); // type = 'L'

    for &(px, py) in patches {
        let patch_id = (px << 5) | (py & 0x1F);
        // Patch header: quant_wbits(8), dc_offset(32), range(16), patch_ids(10)
        push(&mut bits, 0, 8);
        push(&mut bits, (height - 0.5f32).to_bits(), 32);
        push(&mut bits, 1, 16);
        push(&mut bits, patch_id, 10);
        // Immediate EOB (1=present, 0=end_of_block)
        bits.push(1);
        bits.push(0);
    }

    push(&mut bits, 97, 8); // END_OF_PATCHES

    let mut bytes = vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        bytes[i / 8] |= b << (7 - (i % 8));
    }
    bytes
}

fn build_layer_data_raw(terrain_data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(terrain_data.len() + 4);
    payload.push(11); // LayerData High msg ID
    payload.push(0x4C); // LayerID.Type = 'L' (land)
    let len = terrain_data.len() as u16;
    payload.extend_from_slice(&len.to_le_bytes());
    payload.extend_from_slice(terrain_data);
    payload
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
        let info = RegionInfo::new("Test Region", 1000, 2000);
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
