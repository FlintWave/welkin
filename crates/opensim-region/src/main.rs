// SPDX-License-Identifier: GPL-3.0-or-later

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use axum::body::Bytes;
use axum::extract::State;
use axum::response::IntoResponse;

use opensim_config::Config;
use opensim_data::DataPool;
use opensim_llsd::{from_xml, to_xml_string, Llsd, Map};
use opensim_msg::msgs::*;
use opensim_msg::Msg;
use opensim_net::dispatch::{HandleError, PacketContext, PacketDispatcher};
use opensim_net::server::ClientSession;
use opensim_scene::scene::{RegionInfo, Scene, SceneConfig};
use opensim_scene::presence::{ScenePresence, PresenceType};
use opensim_scene::group::SceneObjectGroup;
use opensim_scene::part::SceneObjectPart;
use opensim_msg::objects::FullObjectUpdate;
use opensim_modules::{
    ModuleConfig, SceneRef,
    ChatModule, DefaultPermissionsModule, AvatarFactoryModule,
    AttachmentsModule, EstateManagementModule, LandManagementModule,
    CapsModule, DynAttrsModule, ServiceThrottleModule,
};

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
        let physics = opensim_physics_rapier::RapierPhysicsScene::new(&scene.region_info.region_name);
        scene.set_physics(Box::new(physics));
        info!("Physics engine (Rapier) initialised");
    }

    // --- Module system ---
    let scene_ref = SceneRef::new(scene.region_info.region_id, scene.region_info.region_name.clone());
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
            let server_uri = format!(
                "http://{}:{}/",
                region_config.external_host_name, caps_port
            );
            let zero_uuid = "00000000-0000-0000-0000-000000000000";
            let loc_x = (region_config.region_loc_x * 256) as i32;
            let loc_y = (region_config.region_loc_y * 256) as i32;
            let region_row = opensim_data::grid::RegionRow {
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

            let connector = Arc::new(
                opensim_services::remote::grid::RemoteGridConnector::new(url),
            );
            let registration = opensim_services::registration::GridRegistration::new(
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
    let udp_server = opensim_net::server::LLUDPServer::bind(udp_addr).await?;

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
    let caps_base = format!(
        "http://{}:{}",
        region_config.external_host_name, caps_port
    );

    // --- Message processing loop ---
    let mut dispatcher = PacketDispatcher::new();
    opensim_net::handlers::register_core_handlers(&mut dispatcher);

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

    let cap_server = opensim_caps::server::CapabilityServer::new();

    let asset_url = region_config.grid_service_url.as_deref().unwrap_or("http://localhost:8003");
    let asset_state = opensim_caps::handlers::AssetServiceState {
        client: reqwest::Client::new(),
        asset_url: asset_url.to_string(),
    };

    let mut caps_router = cap_server.into_router();
    for (name, method_router) in opensim_caps::handlers::cap_routes(asset_state) {
        caps_router = caps_router.route(&format!("/{name}"), method_router);
    }
    caps_router = caps_router.route(
        "/EventQueueGet",
        axum::routing::post(opensim_caps::handlers::event_queue_get)
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
        let mut interval = tokio::time::interval(
            std::time::Duration::from_millis(frame_time_ms.max(50)),
        );
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
    opensim_runtime::shutdown_signal().await;
    info!("Initiating graceful shutdown");

    cancel.cancel();

    // Wait for background tasks to finish.
    let _ = udp_handle.await;
    let _ = process_handle.await;
    let _ = caps_handle.await;
    let _ = heartbeat_handle.await;
    let _ = coarse_handle.await;
    let _ = timeout_handle.await;

    if let Some(ref reg) = grid_reg {
        if let Err(e) = reg.shutdown().await {
            warn!(error = %e, "Failed to deregister from grid");
        }
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
use opensim_caps::handlers::EventQueueMap;

async fn process_messages(
    mut msg_rx: tokio::sync::mpsc::UnboundedReceiver<(SocketAddr, opensim_msg::IncomingPacket)>,
    dispatcher: PacketDispatcher,
    socket: Arc<UdpSocket>,
    clients: ClientMap,
    scene: Arc<Scene>,
    event_queues: EventQueueMap,
    caps_base: String,
    cancel: CancellationToken,
) {
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

        if msg_id == CompleteAgentMovement::ID {
            if let Some(client) = &client {
                send_movement_complete(&socket, client, src, &scene.region_info, &scene, &event_queues, &caps_base).await;
            }
        }

        if msg_id == StartPingCheck::ID {
            if let Some(client) = &client {
                if let Ok(ping) = StartPingCheck::decode(&pkt.body) {
                    let reply = CompletePingCheck {
                        ping_id: complete_ping_check::PingID {
                            ping_id: ping.ping_id.ping_id,
                        },
                    };
                    let mut session = client.lock().await;
                    let (pkt_bytes, _) = session.circuit.send_msg(&reply, false);
                    let _ = socket.send_to(&pkt_bytes, src).await;
                }
            }
        }

        if msg_id == AgentCachedTexture::ID {
            if let Some(client) = &client {
                let mut session = client.lock().await;
                let reply = AgentCachedTextureResponse {
                    agent_data: agent_cached_texture_response::AgentData {
                        agent_id: session.agent_id,
                        session_id: session.session_id,
                        serial_num: 0,
                    },
                    wearable_data: Vec::new(),
                };
                let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                let _ = socket.send_to(&pkt_bytes, src).await;
            }
        }

        if msg_id == AgentWearablesRequest::ID {
            if let Some(client) = &client {
                let mut session = client.lock().await;
                let reply = AgentWearablesUpdate {
                    agent_data: agent_wearables_update::AgentData {
                        agent_id: session.agent_id,
                        session_id: session.session_id,
                        serial_num: 1,
                    },
                    wearable_data: Vec::new(),
                };
                let (pkt_bytes, _) = session.circuit.send_msg(&reply, true);
                let _ = socket.send_to(&pkt_bytes, src).await;
            }
        }

        if msg_id == LogoutRequest::ID {
            if let Some(client) = &client {
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
                info!(agent = %session.agent_id, "logout acknowledged, presence removed");
            }
        }

        if msg_id == EconomyDataRequest::ID {
            if let Some(client) = &client {
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
        }

        if msg_id == AgentUpdate::ID {
            if let Ok(update) = AgentUpdate::decode(&pkt.body) {
                scene.update_presence_position(
                    &update.agent_data.agent_id,
                    update.agent_data.camera_center,
                    update.agent_data.body_rotation,
                );
            }
        }

        if msg_id == ChatFromViewer::ID {
            if let Ok(chat) = ChatFromViewer::decode(&pkt.body) {
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
                        if p.uuid == agent_id { name = p.full_name(); }
                    });
                    if name.is_empty() { "Unknown".to_string() } else { name }
                };
                let sender_pos = scene.get_presence(&agent_id)
                    .map(|p| p.position)
                    .unwrap_or(glam::Vec3::ZERO);
                let relay = ChatFromSimulator {
                    chat_data: chat_from_simulator::ChatData {
                        from_name: sender_name.into_bytes(),
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
                    if peer.agent_id == agent_id { continue; }
                    let (pkt_bytes, _) = peer.circuit.send_msg(&relay, true);
                    let _ = socket.send_to(&pkt_bytes, *peer_addr).await;
                }
            }
        }

        if msg_id == ObjectAdd::ID {
            if let Ok(add) = ObjectAdd::decode(&pkt.body) {
                let mut root = SceneObjectPart::default();
                root.uuid = Uuid::new_v4();
                root.owner_id = add.agent_data.agent_id;
                root.group_position = add.object_data.ray_end;
                root.scale = add.object_data.scale;
                root.rotation_offset = add.object_data.rotation;
                root.name = "Object".to_string();
                let group = SceneObjectGroup::new(root);
                let obj_id = scene.add_object(group);
                info!(agent = %add.agent_data.agent_id, object = %obj_id, "ObjectAdd: created in scene");
            }
        }

        if msg_id == ObjectDelete::ID {
            if let Ok(del) = ObjectDelete::decode(&pkt.body) {
                for obj_data in &del.object_data {
                    if let Some(uuid) = scene.get_object_by_local_id(obj_data.object_local_id) {
                        scene.remove_object(&uuid);
                        info!(agent = %agent_id, local_id = obj_data.object_local_id, "ObjectDelete: removed");
                    }
                }
            }
        }

        if msg_id == AgentThrottle::ID {
            if let Ok(throttle_msg) = AgentThrottle::decode(&pkt.body) {
                if let Some(client) = &client {
                    let mut session = client.lock().await;
                    session.throttle.set_from_viewer(&throttle_msg.throttle.throttles);
                    tracing::debug!(agent = %agent_id, "throttle updated from viewer");
                }
            }
        }

        if msg_id == RequestImage::ID {
            if let Ok(req) = RequestImage::decode(&pkt.body) {
                if let Some(client) = &client {
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
            }
        }

        if msg_id == AgentDataUpdateRequest::ID {
            if let Some(client) = &client {
                let mut session = client.lock().await;
                let name = scene.get_presence(&session.agent_id)
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
            position: glam::Vec3::new(128.0, 128.0, 25.0),
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
            terrain_base0: Uuid::nil(),
            terrain_base1: Uuid::nil(),
            terrain_base2: Uuid::nil(),
            terrain_base3: Uuid::nil(),
            terrain_detail0: Uuid::nil(),
            terrain_detail1: Uuid::nil(),
            terrain_detail2: Uuid::nil(),
            terrain_detail3: Uuid::nil(),
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

    let (obj_pkt, wear_pkt, app_pkt) = build_avatar_packets(&mut session, region_info.region_handle);
    for (pkt, name) in [(obj_pkt, "ObjectUpdate"), (wear_pkt, "AgentWearablesUpdate"), (app_pkt, "AvatarAppearance")] {
        if let Err(e) = socket.send_to(&pkt, addr).await {
            tracing::warn!(error = %e, msg = name, "failed to send avatar packet");
            return;
        }
    }
    info!(agent = %session.agent_id, "sent avatar appearance packets");

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

    let mut presence = ScenePresence::new(
        session.agent_id,
        "Test",
        "User",
        PresenceType::User,
    );
    presence.position = glam::Vec3::new(128.0, 128.0, 25.0);
    presence.region_handle = region_info.region_handle;
    scene.add_presence(presence);
    info!(agent = %session.agent_id, "registered presence in scene");

    let eq = Arc::new(opensim_caps::event_queue::EventQueue::new());
    {
        let mut eq_body = Map::new();
        eq_body.insert("agent-id".into(), Llsd::String(session.agent_id.to_string()));
        eq_body.insert(
            "sim-ip-and-port".into(),
            Llsd::String(format!("{}:{}", region_info.external_host_name, region_info.internal_port)),
        );
        eq_body.insert(
            "seed-capability".into(),
            Llsd::String(format!("{}/CAPS/seed", caps_base)),
        );
        eq.enqueue("EstablishAgentCommunication", Llsd::Map(eq_body)).await;
    }
    event_queues.write().await.insert(session.agent_id, eq);
    info!(agent = %session.agent_id, "event queue created with initial event");
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
        position: glam::Vec3::new(128.0, 128.0, 25.0),
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
        texture_entry: Vec::new(),
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
        wearable_data: Vec::new(),
    };
    let (wear_pkt, _) = session.circuit.send_msg(&wearables_msg, true);

    let visual_params: Vec<avatar_appearance::VisualParam> =
        (0..218).map(|_| avatar_appearance::VisualParam { param_value: 128 }).collect();
    let appearance_msg = AvatarAppearance {
        sender: avatar_appearance::Sender {
            id: session.agent_id,
            is_trial: false,
        },
        object_data: avatar_appearance::ObjectData {
            texture_entry: Vec::new(),
        },
        visual_param: visual_params,
        appearance_data: vec![avatar_appearance::AppearanceData {
            appearance_version: 1,
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
    push(&mut bits, 264, 16);   // stride
    push(&mut bits, 16, 8);     // patch_size
    push(&mut bits, 0x4C, 8);   // type = 'L'

    for &(px, py) in patches {
        let patch_id = (px << 5) | (py & 0x1F);
        // Patch header: quant_wbits(8), dc_offset(32), range(16), patch_ids(10)
        push(&mut bits, 0x11, 8);
        push(&mut bits, height.to_bits(), 32);
        push(&mut bits, 0, 16);
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
    payload.push(11);     // LayerData High msg ID
    payload.push(0x4C);   // LayerID.Type = 'L' (land)
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
];

async fn seed_caps_handler(
    State(base_url): State<String>,
    body: Bytes,
) -> impl IntoResponse {
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
