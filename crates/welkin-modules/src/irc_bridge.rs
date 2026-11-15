// SPDX-License-Identifier: GPL-3.0-or-later

//! IRC bridge module: relays chat between in-world chat channels and IRC.
//!
//! Mirrors the core functionality of the C# `IRCBridgeModule` from
//! `OpenSim/Region/OptionalModules/Avatar/Chat/`.  Uses the `irc` crate for
//! protocol handling.
//!
//! The full C# implementation spans ~2,200 lines across `IRCBridgeModule.cs`,
//! `ChannelState.cs`, and `IRCConnector.cs`.  This Rust port implements the
//! core bridge functionality: configuration, message relay data structures,
//! nick formatting, and reconnect-backoff state.  Actual TCP/IRC I/O is
//! stubbed for integration testing — a real deployment would drive these
//! through a tokio task using the `irc` crate's async client.

use std::collections::HashMap;
use std::time::Duration;

use uuid::Uuid;

use crate::events::ChatMessageEvent;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// IRCBridgeConfig
// ---------------------------------------------------------------------------

/// Configuration for an IRC bridge connection.
#[derive(Debug, Clone)]
pub struct IRCBridgeConfig {
    /// IRC server hostname.
    pub server: String,
    /// IRC server port.
    pub port: u16,
    /// IRC channel to join (e.g. "#opensim").
    pub channel: String,
    /// Nick pattern for relayed messages.  `{0}` is replaced with the avatar
    /// name.  Example: `"[OS] {0}"`.
    pub nick_format: String,
    /// The bot's own nick on the IRC server.
    pub bot_nick: String,
    /// Whether to use TLS/SSL.
    pub use_ssl: bool,
    /// In-world chat channel to bridge (default 0 = public chat).
    pub in_world_channel: i32,
    /// Server password (empty if none).
    pub server_password: String,
    /// Whether to relay in-world chat to IRC.
    pub relay_to_irc: bool,
    /// Whether to relay IRC messages to in-world.
    pub relay_to_inworld: bool,
}

impl Default for IRCBridgeConfig {
    fn default() -> Self {
        Self {
            server: String::new(),
            port: 6667,
            channel: String::new(),
            nick_format: "[OS] {0}".to_string(),
            bot_nick: "OpenSimBot".to_string(),
            use_ssl: false,
            in_world_channel: 0,
            server_password: String::new(),
            relay_to_irc: true,
            relay_to_inworld: true,
        }
    }
}

// ---------------------------------------------------------------------------
// ConnectionState
// ---------------------------------------------------------------------------

/// The state of the IRC connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and joined channel.
    Connected,
    /// Connection was lost; will retry.
    Reconnecting,
}

// ---------------------------------------------------------------------------
// ReconnectBackoff
// ---------------------------------------------------------------------------

/// Exponential backoff state for IRC reconnection.
#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    /// Current delay between reconnect attempts.
    pub current_delay: Duration,
    /// Minimum delay.
    pub min_delay: Duration,
    /// Maximum delay.
    pub max_delay: Duration,
    /// Multiplier applied on each failure.
    pub multiplier: f64,
    /// Number of consecutive failures.
    pub failure_count: u32,
}

impl ReconnectBackoff {
    pub fn new() -> Self {
        Self {
            current_delay: Duration::from_secs(5),
            min_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(300),
            multiplier: 2.0,
            failure_count: 0,
        }
    }

    /// Record a failed connection attempt and advance the backoff.
    pub fn on_failure(&mut self) {
        self.failure_count += 1;
        let new_millis =
            (self.current_delay.as_millis() as f64 * self.multiplier) as u64;
        self.current_delay = Duration::from_millis(new_millis).min(self.max_delay);
    }

    /// Reset the backoff on a successful connection.
    pub fn on_success(&mut self) {
        self.failure_count = 0;
        self.current_delay = self.min_delay;
    }

    /// The delay to wait before the next reconnect attempt.
    pub fn next_delay(&self) -> Duration {
        self.current_delay
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IRCEvent — events received from the IRC side
// ---------------------------------------------------------------------------

/// An IRC event received from the server that needs to be relayed in-world.
#[derive(Debug, Clone)]
pub enum IRCEvent {
    /// A user joined the IRC channel.
    Join { nick: String },
    /// A user left the IRC channel.
    Part { nick: String, message: String },
    /// A user quit IRC.
    Quit { nick: String, message: String },
    /// A PRIVMSG (normal chat message) from a user.
    PrivMsg { nick: String, message: String },
    /// A NOTICE from a user or server.
    Notice { nick: String, message: String },
}

// ---------------------------------------------------------------------------
// IRCBridgeModule
// ---------------------------------------------------------------------------

/// Region module that bridges in-world chat to and from an IRC channel.
pub struct IRCBridgeModule {
    /// Bridge configuration.
    pub config: IRCBridgeConfig,
    /// Current connection state.
    pub state: ConnectionState,
    /// Reconnect backoff tracker.
    pub backoff: ReconnectBackoff,
    /// Whether the module is enabled.
    enabled: bool,
    /// The scene this module is attached to.
    scene: Option<SceneRef>,
    /// Pending messages from IRC to be delivered in-world (buffered when
    /// the scene is available).
    irc_to_inworld_buffer: Vec<IRCEvent>,
    /// Pending messages from in-world to be sent to IRC.
    inworld_to_irc_buffer: Vec<(String, String)>,
    /// Mapping of IRC nicks to their last-seen avatar name (for name
    /// resolution when relaying).
    nick_map: HashMap<String, String>,
}

impl IRCBridgeModule {
    pub fn new() -> Self {
        Self {
            config: IRCBridgeConfig::default(),
            state: ConnectionState::Disconnected,
            backoff: ReconnectBackoff::new(),
            enabled: false,
            scene: None,
            irc_to_inworld_buffer: Vec::new(),
            inworld_to_irc_buffer: Vec::new(),
            nick_map: HashMap::new(),
        }
    }

    /// Format an avatar name using the configured nick pattern.
    pub fn format_nick(&self, avatar_name: &str) -> String {
        self.config.nick_format.replace("{0}", avatar_name)
    }

    /// Process an in-world chat message for relay to IRC.
    ///
    /// Returns the formatted message that would be sent to the IRC channel,
    /// or `None` if relay is disabled or the channel doesn't match.
    pub fn on_chat_from_world(
        &mut self,
        sender_name: &str,
        message: &str,
        channel: i32,
    ) -> Option<String> {
        if !self.enabled || !self.config.relay_to_irc {
            return None;
        }
        if channel != self.config.in_world_channel {
            return None;
        }

        let nick = self.format_nick(sender_name);
        let formatted = format!("<{}> {}", nick, message);
        self.inworld_to_irc_buffer.push((nick, message.to_string()));
        Some(formatted)
    }

    /// Process an IRC event for relay in-world.
    ///
    /// Returns the message text that would be broadcast in the region, or
    /// `None` if relay is disabled.
    pub fn on_irc_event(&mut self, event: IRCEvent) -> Option<String> {
        if !self.enabled || !self.config.relay_to_inworld {
            return None;
        }

        let msg = match &event {
            IRCEvent::PrivMsg { nick, message } => {
                format!("[IRC] {}: {}", nick, message)
            }
            IRCEvent::Join { nick } => {
                format!("[IRC] {} has joined {}", nick, self.config.channel)
            }
            IRCEvent::Part { nick, message } => {
                if message.is_empty() {
                    format!("[IRC] {} has left {}", nick, self.config.channel)
                } else {
                    format!(
                        "[IRC] {} has left {} ({})",
                        nick, self.config.channel, message
                    )
                }
            }
            IRCEvent::Quit { nick, message } => {
                if message.is_empty() {
                    format!("[IRC] {} has quit", nick)
                } else {
                    format!("[IRC] {} has quit ({})", nick, message)
                }
            }
            IRCEvent::Notice { nick, message } => {
                format!("[IRC Notice] {}: {}", nick, message)
            }
        };

        // Broadcast via the scene event bus.
        if let Some(ref scene) = self.scene {
            let evt = ChatMessageEvent {
                region_id: scene.region_id,
                sender_id: Uuid::nil(),
                message: msg.clone(),
                channel: self.config.in_world_channel,
            };
            let _ = scene.events.on_chat_message.send(evt);
        }

        self.irc_to_inworld_buffer.push(event);
        Some(msg)
    }

    /// Simulate a connection attempt.
    ///
    /// In a real implementation this would use the `irc` crate's async
    /// client.  Here we just track state transitions.
    pub fn connect(&mut self) -> Result<(), ModuleError> {
        if self.config.server.is_empty() || self.config.channel.is_empty() {
            return Err(ModuleError::InitFailed(
                "IRC server or channel not configured".into(),
            ));
        }
        self.state = ConnectionState::Connecting;
        tracing::info!(
            server = %self.config.server,
            port = self.config.port,
            channel = %self.config.channel,
            "IRCBridge: connecting (stub)"
        );
        // In a real implementation, this would await the connection.
        // For now, transition to Connected.
        self.state = ConnectionState::Connected;
        self.backoff.on_success();
        Ok(())
    }

    /// Simulate a disconnection.
    pub fn disconnect(&mut self) {
        self.state = ConnectionState::Disconnected;
        tracing::info!("IRCBridge: disconnected");
    }

    /// Simulate a connection failure for reconnect handling.
    pub fn on_connection_lost(&mut self) {
        self.state = ConnectionState::Reconnecting;
        self.backoff.on_failure();
        tracing::warn!(
            delay = ?self.backoff.next_delay(),
            failures = self.backoff.failure_count,
            "IRCBridge: connection lost, will reconnect"
        );
    }

    /// Number of pending in-world-to-IRC messages.
    pub fn outbound_buffer_len(&self) -> usize {
        self.inworld_to_irc_buffer.len()
    }

    /// Number of pending IRC-to-in-world events.
    pub fn inbound_buffer_len(&self) -> usize {
        self.irc_to_inworld_buffer.len()
    }

    /// Drain the outbound message buffer.
    pub fn drain_outbound(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.inworld_to_irc_buffer)
    }

    /// Drain the inbound event buffer.
    pub fn drain_inbound(&mut self) -> Vec<IRCEvent> {
        std::mem::take(&mut self.irc_to_inworld_buffer)
    }
}

impl Default for IRCBridgeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for IRCBridgeModule {
    fn name(&self) -> &str {
        "IRCBridgeModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("IRC", "enabled", false);
        self.config.server = config.get_string("IRC", "server", "");
        self.config.port = config.get_int("IRC", "port", 6667) as u16;
        self.config.channel = config.get_string("IRC", "channel", "");
        self.config.nick_format =
            config.get_string("IRC", "nick_format", "[OS] {0}");
        self.config.bot_nick =
            config.get_string("IRC", "bot_nick", "OpenSimBot");
        self.config.use_ssl = config.get_bool("IRC", "use_ssl", false);
        self.config.in_world_channel =
            config.get_int("IRC", "in_world_channel", 0) as i32;
        self.config.server_password =
            config.get_string("IRC", "server_password", "");
        self.config.relay_to_irc =
            config.get_bool("IRC", "relay_to_irc", true);
        self.config.relay_to_inworld =
            config.get_bool("IRC", "relay_to_inworld", true);

        tracing::info!(
            enabled = self.enabled,
            server = %self.config.server,
            channel = %self.config.channel,
            "IRCBridgeModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        if self.enabled && !self.config.server.is_empty() {
            // In a full implementation we'd spawn a tokio task here to
            // manage the IRC connection lifecycle.
            tracing::info!(
                region = %scene.region_name,
                "IRCBridgeModule attached to region"
            );
        }
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.disconnect();
        self.irc_to_inworld_buffer.clear();
        self.inworld_to_irc_buffer.clear();
        self.nick_map.clear();
        self.scene = None;
        tracing::info!(
            region = %scene.region_name,
            "IRCBridgeModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.disconnect();
        self.irc_to_inworld_buffer.clear();
        self.inworld_to_irc_buffer.clear();
        self.nick_map.clear();
        self.scene = None;
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleConfig;
    use std::collections::HashMap as StdHashMap;

    struct TestConfig {
        strings: StdHashMap<(String, String), String>,
        ints: StdHashMap<(String, String), i64>,
        bools: StdHashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: StdHashMap::new(),
                ints: StdHashMap::new(),
                bools: StdHashMap::new(),
            }
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }

        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
                .insert((section.to_string(), key.to_string()), value);
        }
    }

    impl ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }
        fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
            self.ints
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
            self.bools
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
            default
        }
    }

    fn test_scene() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn setup_module() -> IRCBridgeModule {
        let mut module = IRCBridgeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("IRC", "enabled", true);
        config.set_string("IRC", "server", "irc.example.com");
        config.set_int("IRC", "port", 6667);
        config.set_string("IRC", "channel", "#opensim");
        module.initialise(&config).unwrap();
        let scene = test_scene();
        module.add_region(&scene).unwrap();
        module
    }

    // -- Test 1: nick formatting ----------------------------------------------

    #[test]
    fn nick_formatting() {
        let module = setup_module();
        assert_eq!(module.format_nick("Alice"), "[OS] Alice");
    }

    // -- Test 2: custom nick format -------------------------------------------

    #[test]
    fn custom_nick_format() {
        let mut module = IRCBridgeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("IRC", "enabled", true);
        config.set_string("IRC", "server", "irc.example.com");
        config.set_string("IRC", "channel", "#test");
        config.set_string("IRC", "nick_format", "OS_{0}");
        module.initialise(&config).unwrap();
        assert_eq!(module.format_nick("Bob"), "OS_Bob");
    }

    // -- Test 3: inworld-to-IRC relay -----------------------------------------

    #[test]
    fn inworld_to_irc_relay() {
        let mut module = setup_module();
        let formatted = module
            .on_chat_from_world("Charlie", "Hello IRC!", 0)
            .unwrap();
        assert_eq!(formatted, "<[OS] Charlie> Hello IRC!");
        assert_eq!(module.outbound_buffer_len(), 1);

        let drained = module.drain_outbound();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].1, "Hello IRC!");
        assert_eq!(module.outbound_buffer_len(), 0);
    }

    // -- Test 4: wrong channel is ignored -------------------------------------

    #[test]
    fn wrong_channel_ignored() {
        let mut module = setup_module();
        let result = module.on_chat_from_world("Dave", "wrong channel", 42);
        assert!(result.is_none());
        assert_eq!(module.outbound_buffer_len(), 0);
    }

    // -- Test 5: IRC event relay (PRIVMSG) ------------------------------------

    #[test]
    fn irc_privmsg_relay() {
        let mut module = setup_module();
        let scene = module.scene.clone().unwrap();
        let mut rx = scene.events.on_chat_message.subscribe();

        let msg = module
            .on_irc_event(IRCEvent::PrivMsg {
                nick: "ircuser".to_string(),
                message: "Hello from IRC".to_string(),
            })
            .unwrap();
        assert_eq!(msg, "[IRC] ircuser: Hello from IRC");

        // Should also appear on the event bus.
        let evt = rx.try_recv().unwrap();
        assert_eq!(evt.message, "[IRC] ircuser: Hello from IRC");
    }

    // -- Test 6: IRC event relay (JOIN/PART/QUIT) -----------------------------

    #[test]
    fn irc_join_part_quit_relay() {
        let mut module = setup_module();

        let join = module
            .on_irc_event(IRCEvent::Join {
                nick: "newcomer".to_string(),
            })
            .unwrap();
        assert!(join.contains("newcomer"));
        assert!(join.contains("#opensim"));

        let part = module
            .on_irc_event(IRCEvent::Part {
                nick: "leaver".to_string(),
                message: "bye".to_string(),
            })
            .unwrap();
        assert!(part.contains("leaver"));
        assert!(part.contains("bye"));

        let quit = module
            .on_irc_event(IRCEvent::Quit {
                nick: "quitter".to_string(),
                message: String::new(),
            })
            .unwrap();
        assert!(quit.contains("quitter"));
        assert!(quit.contains("quit"));
    }

    // -- Test 7: reconnect backoff --------------------------------------------

    #[test]
    fn reconnect_backoff() {
        let mut backoff = ReconnectBackoff::new();
        assert_eq!(backoff.next_delay(), Duration::from_secs(5));
        assert_eq!(backoff.failure_count, 0);

        backoff.on_failure();
        assert_eq!(backoff.failure_count, 1);
        assert_eq!(backoff.next_delay(), Duration::from_secs(10));

        backoff.on_failure();
        assert_eq!(backoff.failure_count, 2);
        assert_eq!(backoff.next_delay(), Duration::from_secs(20));

        // Keep failing until we hit the cap.
        for _ in 0..10 {
            backoff.on_failure();
        }
        assert!(backoff.next_delay() <= Duration::from_secs(300));

        // Reset on success.
        backoff.on_success();
        assert_eq!(backoff.failure_count, 0);
        assert_eq!(backoff.next_delay(), Duration::from_secs(5));
    }

    // -- Test 8: connection state transitions ----------------------------------

    #[test]
    fn connection_state_transitions() {
        let mut module = setup_module();
        assert_eq!(module.state, ConnectionState::Disconnected);

        module.connect().unwrap();
        assert_eq!(module.state, ConnectionState::Connected);

        module.on_connection_lost();
        assert_eq!(module.state, ConnectionState::Reconnecting);
        assert_eq!(module.backoff.failure_count, 1);

        module.disconnect();
        assert_eq!(module.state, ConnectionState::Disconnected);
    }

    // -- Test 9: connect fails without config ---------------------------------

    #[test]
    fn connect_fails_without_config() {
        let mut module = IRCBridgeModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(module.connect().is_err());
    }

    // -- Test 10: disabled relay returns None ----------------------------------

    #[test]
    fn disabled_relay_returns_none() {
        let mut module = IRCBridgeModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        let scene = test_scene();
        module.add_region(&scene).unwrap();

        // Module is disabled by default.
        assert!(module
            .on_chat_from_world("Eve", "hello", 0)
            .is_none());
        assert!(module
            .on_irc_event(IRCEvent::PrivMsg {
                nick: "x".into(),
                message: "y".into(),
            })
            .is_none());
    }

    // -- Test 11: RegionModule lifecycle --------------------------------------

    #[test]
    fn irc_bridge_module_lifecycle() {
        let mut module = IRCBridgeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("IRC", "enabled", true);
        config.set_string("IRC", "server", "irc.example.com");
        config.set_string("IRC", "channel", "#test");
        let scene = test_scene();

        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "IRCBridgeModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);

        // Buffer some messages.
        module.on_chat_from_world("Test", "msg", 0);
        assert_eq!(module.outbound_buffer_len(), 1);

        // remove_region clears buffers.
        module.remove_region(&scene).unwrap();
        assert_eq!(module.outbound_buffer_len(), 0);
        assert_eq!(module.inbound_buffer_len(), 0);

        module.close().unwrap();
    }

    // -- Test 12: config reads all IRC settings --------------------------------

    #[test]
    fn config_reads_all_settings() {
        let mut module = IRCBridgeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("IRC", "enabled", true);
        config.set_string("IRC", "server", "irc.freenode.net");
        config.set_int("IRC", "port", 6697);
        config.set_string("IRC", "channel", "#mychannel");
        config.set_string("IRC", "nick_format", "SIM_{0}");
        config.set_string("IRC", "bot_nick", "MyBot");
        config.set_bool("IRC", "use_ssl", true);
        config.set_int("IRC", "in_world_channel", 42);
        config.set_bool("IRC", "relay_to_irc", false);
        config.set_bool("IRC", "relay_to_inworld", false);

        module.initialise(&config).unwrap();

        assert!(module.enabled);
        assert_eq!(module.config.server, "irc.freenode.net");
        assert_eq!(module.config.port, 6697);
        assert_eq!(module.config.channel, "#mychannel");
        assert_eq!(module.config.nick_format, "SIM_{0}");
        assert_eq!(module.config.bot_nick, "MyBot");
        assert!(module.config.use_ssl);
        assert_eq!(module.config.in_world_channel, 42);
        assert!(!module.config.relay_to_irc);
        assert!(!module.config.relay_to_inworld);
    }

    // -- Test 13: IRC notice relay --------------------------------------------

    #[test]
    fn irc_notice_relay() {
        let mut module = setup_module();
        let msg = module
            .on_irc_event(IRCEvent::Notice {
                nick: "server".to_string(),
                message: "Maintenance in 5 minutes".to_string(),
            })
            .unwrap();
        assert_eq!(msg, "[IRC Notice] server: Maintenance in 5 minutes");
    }
}
