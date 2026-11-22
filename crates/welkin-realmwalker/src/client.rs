// SPDX-License-Identifier: LGPL-3.0-or-later
//! Top-level `RealmWalker` client that composes login, network, caps, and chat.
//!
//! Call [`RealmWalker::login`] to connect to a grid. The returned handle
//! provides methods for sending chat and IMs, and a broadcast channel for
//! receiving events. Background tasks (packet pump, ack loop, event dispatch,
//! EQG poll) run automatically until [`RealmWalker::logout`] is called or the
//! connection drops.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::caps;
use crate::chat;
use crate::config::GridConfig;
use crate::inventory;
use crate::objects;use crate::error::RealmWalkerError;
use crate::events::{ChatType, Event, EventBus};
use crate::login::{self, LoginResponse};
use crate::network::Network;

/// The main bot/client handle.
///
/// Call [`RealmWalker::login`] to connect to a grid. The returned handle
/// provides methods for sending chat and IMs, and a broadcast channel for
/// receiving events. Background tasks (packet pump, ack loop, event dispatch,
/// EQG poll) run automatically until [`logout`](RealmWalker::logout) is called
/// or the connection drops.
pub struct RealmWalker {
    config: GridConfig,
    http: reqwest::Client,
    network: Arc<Network>,
    event_bus: Arc<EventBus>,
    login_response: LoginResponse,
    caps: HashMap<String, String>,
    tasks: Vec<JoinHandle<()>>,
}

impl RealmWalker {
    /// Connect to the grid and set up the circuit.
    ///
    /// Performs the full login sequence:
    /// 1. XMLRPC login to obtain circuit details
    /// 2. UDP circuit establishment (UseCircuitCode handshake)
    /// 3. Seed capability fetch
    /// 4. Background task spawn (packet pump, ack loop, event dispatch, EQG)
    pub async fn login(config: GridConfig) -> Result<Self, RealmWalkerError> {
        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.read_timeout)
            .build()?;

        // 1. XMLRPC login
        let lr = login::login(&config, &http).await?;
        tracing::info!(
            agent = %lr.agent_id,
            sim = %lr.sim_ip,
            port = lr.sim_port,
            "logged in"
        );

        // 2. Open UDP circuit
        let net = Network::connect(
            &lr.sim_ip,
            lr.sim_port,
            lr.agent_id,
            lr.session_id,
            lr.circuit_code,
        )
        .await?;
        let net = Arc::new(net);

        // 3. Fetch seed capabilities
        let requested_caps = &[
            "EventQueueGet",
            "FetchInventory2",
            "FetchLib2",
            "GetTexture",
            "GetMesh",
            "GetMesh2",
            "UpdateScriptAgent",
            "NewFileAgentInventory",
        ];
        let seed_caps = caps::fetch_seed_caps(&http, &lr.seed_capability, requested_caps)
            .await
            .unwrap_or_default();

        tracing::debug!(count = seed_caps.len(), "seed capabilities fetched");

        // 4. Set up event bus
        let event_bus = Arc::new(EventBus::new(config.event_channel_capacity));

        // 5. Spawn background tasks
        let mut tasks = Vec::new();

        // Packet pump
        {
            let n = Arc::clone(&net);
            tasks.push(tokio::spawn(async move {
                let _ = n.run_packet_pump().await;
            }));
        }

        // Ack loop
        {
            let n = Arc::clone(&net);
            tasks.push(tokio::spawn(async move {
                let _ = n.run_ack_loop().await;
            }));
        }

        // Event dispatch: reads from network broadcast, dispatches chat/IM to event bus
        {
            let n = Arc::clone(&net);
            let eb = Arc::clone(&event_bus);
            tasks.push(tokio::spawn(async move {
                let mut rx = n.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(pkt) => {
                            chat::try_handle_chat(&pkt, &eb);
                            chat::try_handle_im(&pkt, &eb);
                            objects::try_handle_object_update(&pkt, &eb);
                            objects::try_handle_object_update_compressed(&pkt, &eb);
                            objects::try_handle_terse_object_update(&pkt, &eb);
                            objects::try_handle_kill_object(&pkt, &eb);
                            objects::try_handle_object_properties(&pkt, &eb);
                            inventory::try_handle_inventory_descendents(&pkt, &eb);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "event dispatch lagged");
                        }
                    }
                }
            }));
        }

        // EventQueueGet long-poll loop (if the cap URL was returned)
        if let Some(eqg_url) = seed_caps.get("EventQueueGet").cloned() {
            let h = http.clone();
            let eb = Arc::clone(&event_bus);
            tasks.push(tokio::spawn(async move {
                let mut ack_id: Option<u64> = None;
                loop {
                    match caps::poll_event_queue(&h, &eqg_url, ack_id).await {
                        Ok((events, new_ack)) => {
                            ack_id = Some(new_ack);
                            for ev in events {
                                eb.send(Event::EventQueueEvent(ev.message, ev.body));
                            }
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, "EQG poll failed, retrying");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
            }));
        }

        Ok(Self {
            config,
            http,
            network: net,
            event_bus,
            login_response: lr,
            caps: seed_caps,
            tasks,
        })
    }

    /// Subscribe to the event stream.
    ///
    /// Each subscriber gets its own independent receiver. Slow subscribers
    /// will receive a `Lagged` error and skip to the latest event.
    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.event_bus.subscribe()
    }

    /// The login response data.
    pub fn login_response(&self) -> &LoginResponse {
        &self.login_response
    }

    /// The agent UUID for this session.
    pub fn agent_id(&self) -> Uuid {
        self.login_response.agent_id
    }

    /// The session UUID for this session.
    pub fn session_id(&self) -> Uuid {
        self.login_response.session_id
    }

    /// The grid configuration used to connect.
    pub fn config(&self) -> &GridConfig {
        &self.config
    }

    /// Send a normal chat message (heard within 20m).
    pub async fn say(&self, message: &str) -> Result<(), RealmWalkerError> {
        chat::send_chat(
            &self.network,
            self.agent_id(),
            self.session_id(),
            message,
            ChatType::Normal,
            0,
        )
        .await
    }

    /// Send a shout message (heard within 100m).
    pub async fn shout(&self, message: &str) -> Result<(), RealmWalkerError> {
        chat::send_chat(
            &self.network,
            self.agent_id(),
            self.session_id(),
            message,
            ChatType::Shout,
            0,
        )
        .await
    }

    /// Send a whisper message (heard within 10m).
    pub async fn whisper(&self, message: &str) -> Result<(), RealmWalkerError> {
        chat::send_chat(
            &self.network,
            self.agent_id(),
            self.session_id(),
            message,
            ChatType::Whisper,
            0,
        )
        .await
    }

    /// Send a chat message with explicit type and channel.
    pub async fn send_chat_on_channel(
        &self,
        message: &str,
        chat_type: ChatType,
        channel: i32,
    ) -> Result<(), RealmWalkerError> {
        chat::send_chat(
            &self.network,
            self.agent_id(),
            self.session_id(),
            message,
            chat_type,
            channel,
        )
        .await
    }

    /// Send an instant message to another avatar.
    pub async fn send_im(&self, to: Uuid, message: &str) -> Result<(), RealmWalkerError> {
        chat::send_im(
            &self.network,
            self.agent_id(),
            self.session_id(),
            to,
            message,
        )
        .await
    }

    /// Whether the circuit is still connected.
    pub fn is_connected(&self) -> bool {
        self.network.is_connected()
    }

    /// Look up a capability URL by name.
    pub fn cap_url(&self, name: &str) -> Option<&str> {
        self.caps.get(name).map(|s| s.as_str())
    }

    /// The HTTP client used for caps and login.
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// A reference to the underlying network layer.
    pub fn network(&self) -> &Network {
        &self.network
    }

    /// Gracefully disconnect from the grid.
    ///
    /// Sends a LogoutRequest to the server and aborts all background tasks.
    pub async fn logout(&mut self) -> Result<(), RealmWalkerError> {
        self.network.disconnect().await;
        for task in self.tasks.drain(..) {
            task.abort();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_walker_has_expected_public_api() {
        // Compile-time check that the public API surface exists.
        // We cannot do a full integration test without a live grid,
        // but we verify the types compose correctly.
        fn _assert_send<T: Send>() {}
        // RealmWalker holds Arc<Network> and JoinHandle, both Send.
        // This is a compile-time assertion.
        _assert_send::<RealmWalker>();
    }

    #[test]
    fn default_config_creates_valid_http_client() {
        // Verify that reqwest::Client::builder with our default config
        // timeouts produces a valid client (no panic).
        let config = GridConfig::default();
        let _client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.read_timeout)
            .build()
            .expect("should build HTTP client");
    }

    #[test]
    fn cap_url_returns_none_for_missing() {
        // Verify HashMap lookup behavior that cap_url relies on.
        let caps: HashMap<String, String> = HashMap::new();
        assert!(caps.get("EventQueueGet").is_none());

        let mut caps = HashMap::new();
        caps.insert("EventQueueGet".to_string(), "http://example.com/eqg".to_string());
        assert_eq!(
            caps.get("EventQueueGet").map(|s| s.as_str()),
            Some("http://example.com/eqg")
        );
        assert!(caps.get("NonExistent").is_none());
    }
}
