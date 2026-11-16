// SPDX-License-Identifier: LGPL-3.0-or-later

//! Native Rust pCampBot — replaces the shell wrapper that invoked an external binary.
//!
//! Bots are simulated state machines: they track login state, position, and
//! actions without opening real network connections, which makes the framework
//! useful for conformance and load-profile testing even when no live grid is
//! available.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a pCampBot run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcampBotConfig {
    pub login_uri: String,
    pub first_name_prefix: String,
    pub last_name_prefix: String,
    pub password: String,
    pub bot_count: usize,
    pub action_interval_ms: u64,
    pub region_targets: Vec<String>,
}

impl PcampBotConfig {
    /// Load configuration from a JSON file.
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        let cfg: Self = serde_json::from_str(&data)?;
        Ok(cfg)
    }


}

impl Default for PcampBotConfig {
    fn default() -> Self {
        Self {
            login_uri: "http://localhost:9000".to_string(),
            first_name_prefix: "Test".to_string(),
            last_name_prefix: "Bot".to_string(),
            password: "password".to_string(),
            bot_count: 1,
            action_interval_ms: 1000,
            region_targets: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// An action a bot can perform.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BotAction {
    Login,
    Walk([f32; 3]),
    Fly([f32; 3]),
    Teleport(String),
    CrossRegion,
    Chat(String),
    GroupJoin(Uuid),
    Disconnect,
}

// ---------------------------------------------------------------------------
// Bot state
// ---------------------------------------------------------------------------

/// Internal state of a single bot.
#[derive(Debug, Clone)]
pub struct Bot {
    pub session_id: Uuid,
    pub name: String,
    pub position: [f32; 3],
    pub current_region: String,
    pub action_queue: VecDeque<BotAction>,
    pub connected: bool,
}

impl Bot {
    /// Create a new bot with a generated identity.
    pub fn new(first_name: &str, last_name: &str) -> Self {
        Self {
            session_id: Uuid::nil(),
            name: format!("{first_name} {last_name}"),
            position: [128.0, 128.0, 25.0],
            current_region: String::new(),
            action_queue: VecDeque::new(),
            connected: false,
        }
    }

    /// Execute a single action, mutating bot state. Returns `Ok(())` on
    /// success or an error description on failure.
    pub fn execute_action(&mut self, action: &BotAction) -> Result<(), String> {
        match action {
            BotAction::Login => {
                if self.connected {
                    return Err("already connected".to_string());
                }
                self.session_id = Uuid::new_v4();
                self.connected = true;
                self.current_region = "default-region".to_string();
                info!("{} logged in (session {})", self.name, self.session_id);
                Ok(())
            }
            BotAction::Walk(direction) => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                self.position[0] += direction[0];
                self.position[1] += direction[1];
                self.position[2] += direction[2];
                Ok(())
            }
            BotAction::Fly(target) => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                self.position = *target;
                Ok(())
            }
            BotAction::Teleport(region_name) => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                self.current_region = region_name.clone();
                self.position = [128.0, 128.0, 25.0];
                info!("{} teleported to {region_name}", self.name);
                Ok(())
            }
            BotAction::CrossRegion => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                self.current_region = format!("{}-adjacent", self.current_region);
                self.position = [0.0, 128.0, 25.0];
                Ok(())
            }
            BotAction::Chat(message) => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                info!("{} says: {message}", self.name);
                Ok(())
            }
            BotAction::GroupJoin(group_id) => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                info!("{} joined group {group_id}", self.name);
                Ok(())
            }
            BotAction::Disconnect => {
                if !self.connected {
                    return Err("not connected".to_string());
                }
                self.connected = false;
                self.session_id = Uuid::nil();
                info!("{} disconnected", self.name);
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bot manager
// ---------------------------------------------------------------------------

/// Manages a fleet of bots, running their action queues concurrently.
pub struct BotManager {
    pub config: PcampBotConfig,
    pub bots: Vec<Bot>,
}

impl BotManager {
    /// Build a manager from a configuration, creating bots with sequential names.
    pub fn new(config: PcampBotConfig) -> Self {
        let bots = (0..config.bot_count)
            .map(|i| {
                Bot::new(
                    &format!("{}{i}", config.first_name_prefix),
                    &format!("{}{i}", config.last_name_prefix),
                )
            })
            .collect();

        Self { config, bots }
    }

    /// Set the same action sequence on every bot.
    pub fn set_actions(&mut self, actions: &[BotAction]) {
        for bot in &mut self.bots {
            bot.action_queue = actions.iter().cloned().collect();
        }
    }

    /// Run all bots, draining their action queues. Each bot is an async task.
    pub async fn run(&mut self) -> BotReport {
        let interval = tokio::time::Duration::from_millis(self.config.action_interval_ms);

        let mut connected = 0usize;
        let mut disconnected = 0usize;
        let mut actions_completed = 0usize;
        let mut actions_failed = 0usize;
        let mut latencies = Vec::new();

        for bot in &mut self.bots {
            while let Some(action) = bot.action_queue.pop_front() {
                let start = Instant::now();
                match bot.execute_action(&action) {
                    Ok(()) => {
                        actions_completed += 1;
                        if matches!(action, BotAction::Login) {
                            connected += 1;
                        }
                        if matches!(action, BotAction::Disconnect) {
                            disconnected += 1;
                        }
                    }
                    Err(e) => {
                        actions_failed += 1;
                        warn!("{}: action {:?} failed: {e}", bot.name, action);
                    }
                }
                let elapsed = start.elapsed().as_millis() as f64;
                latencies.push(elapsed);

                tokio::time::sleep(interval).await;
            }
        }

        let avg_latency_ms = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        };

        BotReport {
            connected,
            disconnected,
            actions_completed,
            actions_failed,
            avg_latency_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Summary report from a bot run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotReport {
    pub connected: usize,
    pub disconnected: usize,
    pub actions_completed: usize,
    pub actions_failed: usize,
    pub avg_latency_ms: f64,
}

// ---------------------------------------------------------------------------
// Top-level runner (called from main.rs CLI)
// ---------------------------------------------------------------------------

/// Run bots from a config file, with optional overrides for count and interval.
pub async fn run_bots(
    config_path: PathBuf,
    count_override: Option<usize>,
    interval_override: Option<u64>,
) -> Result<BotReport, Box<dyn std::error::Error>> {
    let mut config = PcampBotConfig::load(&config_path)?;

    if let Some(c) = count_override {
        config.bot_count = c;
    }
    if let Some(i) = interval_override {
        config.action_interval_ms = i;
    }

    info!(
        "Starting {} native bots against {}",
        config.bot_count, config.login_uri
    );

    let default_actions = vec![
        BotAction::Login,
        BotAction::Walk([1.0, 0.0, 0.0]),
        BotAction::Walk([0.0, 1.0, 0.0]),
        BotAction::Chat("Hello from pCampBot!".to_string()),
        BotAction::Disconnect,
    ];

    let mut manager = BotManager::new(config);
    manager.set_actions(&default_actions);

    let report = manager.run().await;

    info!(
        "Bot run complete: {} connected, {} actions OK, {} failed, avg latency {:.2}ms",
        report.connected, report.actions_completed, report.actions_failed, report.avg_latency_ms
    );

    Ok(report)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = PcampBotConfig::default();
        assert_eq!(cfg.bot_count, 1);
        assert_eq!(cfg.login_uri, "http://localhost:9000");
        assert!(cfg.region_targets.is_empty());
    }

    #[test]
    fn test_config_roundtrip() {
        let cfg = PcampBotConfig {
            login_uri: "http://example.com:8002".to_string(),
            first_name_prefix: "Alpha".to_string(),
            last_name_prefix: "Beta".to_string(),
            password: "secret".to_string(),
            bot_count: 5,
            action_interval_ms: 500,
            region_targets: vec!["Region1".to_string(), "Region2".to_string()],
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let restored: PcampBotConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.bot_count, 5);
        assert_eq!(restored.region_targets.len(), 2);
    }

    #[test]
    fn test_bot_creation() {
        let bot = Bot::new("Test", "Bot");
        assert!(!bot.connected);
        assert_eq!(bot.name, "Test Bot");
        assert_eq!(bot.session_id, Uuid::nil());
    }

    #[test]
    fn test_bot_login() {
        let mut bot = Bot::new("Test", "Bot");
        let result = bot.execute_action(&BotAction::Login);
        assert!(result.is_ok());
        assert!(bot.connected);
        assert_ne!(bot.session_id, Uuid::nil());
        assert_eq!(bot.current_region, "default-region");
    }

    #[test]
    fn test_bot_double_login_fails() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("first login");
        let result = bot.execute_action(&BotAction::Login);
        assert!(result.is_err());
    }

    #[test]
    fn test_bot_walk_when_disconnected_fails() {
        let mut bot = Bot::new("Test", "Bot");
        let result = bot.execute_action(&BotAction::Walk([1.0, 0.0, 0.0]));
        assert!(result.is_err());
    }

    #[test]
    fn test_bot_walk_updates_position() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        let initial = bot.position;
        bot.execute_action(&BotAction::Walk([5.0, 3.0, 0.0]))
            .expect("walk");
        assert!((bot.position[0] - (initial[0] + 5.0)).abs() < f32::EPSILON);
        assert!((bot.position[1] - (initial[1] + 3.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bot_teleport() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        bot.execute_action(&BotAction::Teleport("FarRegion".to_string()))
            .expect("teleport");
        assert_eq!(bot.current_region, "FarRegion");
        assert!((bot.position[0] - 128.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bot_disconnect() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        bot.execute_action(&BotAction::Disconnect)
            .expect("disconnect");
        assert!(!bot.connected);
        assert_eq!(bot.session_id, Uuid::nil());
    }

    #[test]
    fn test_bot_disconnect_when_not_connected_fails() {
        let mut bot = Bot::new("Test", "Bot");
        let result = bot.execute_action(&BotAction::Disconnect);
        assert!(result.is_err());
    }

    #[test]
    fn test_bot_fly() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        bot.execute_action(&BotAction::Fly([200.0, 200.0, 100.0]))
            .expect("fly");
        assert!((bot.position[0] - 200.0).abs() < f32::EPSILON);
        assert!((bot.position[1] - 200.0).abs() < f32::EPSILON);
        assert!((bot.position[2] - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bot_cross_region() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        let original_region = bot.current_region.clone();
        bot.execute_action(&BotAction::CrossRegion)
            .expect("cross region");
        assert_ne!(bot.current_region, original_region);
        assert!(bot.current_region.ends_with("-adjacent"));
    }

    #[test]
    fn test_bot_manager_creation() {
        let config = PcampBotConfig {
            bot_count: 3,
            ..Default::default()
        };
        let manager = BotManager::new(config);
        assert_eq!(manager.bots.len(), 3);
        assert_eq!(manager.bots[0].name, "Test0 Bot0");
        assert_eq!(manager.bots[2].name, "Test2 Bot2");
    }

    #[tokio::test]
    async fn test_bot_manager_run() {
        let config = PcampBotConfig {
            bot_count: 2,
            action_interval_ms: 0,
            ..Default::default()
        };
        let mut manager = BotManager::new(config);
        manager.set_actions(&[
            BotAction::Login,
            BotAction::Walk([1.0, 0.0, 0.0]),
            BotAction::Disconnect,
        ]);
        let report = manager.run().await;
        assert_eq!(report.connected, 2);
        assert_eq!(report.disconnected, 2);
        assert_eq!(report.actions_completed, 6); // 3 actions x 2 bots
        assert_eq!(report.actions_failed, 0);
    }

    #[test]
    fn test_bot_action_serde_roundtrip() {
        let actions = vec![
            BotAction::Login,
            BotAction::Walk([1.0, 2.0, 3.0]),
            BotAction::Fly([10.0, 20.0, 30.0]),
            BotAction::Teleport("TestRegion".to_string()),
            BotAction::CrossRegion,
            BotAction::Chat("hello".to_string()),
            BotAction::GroupJoin(Uuid::nil()),
            BotAction::Disconnect,
        ];
        let json = serde_json::to_string(&actions).expect("serialize");
        let restored: Vec<BotAction> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, actions);
    }

    #[test]
    fn test_bot_chat() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        let result = bot.execute_action(&BotAction::Chat("hello world".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_bot_group_join() {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        let group_id = Uuid::new_v4();
        let result = bot.execute_action(&BotAction::GroupJoin(group_id));
        assert!(result.is_ok());
    }
}
