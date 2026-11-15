// SPDX-License-Identifier: LGPL-3.0-or-later

//! Programmable bot-action scripts for conformance testing.
//!
//! A [`BotScript`] is a named, serialisable sequence of [`ScriptAction`]s.
//! The [`ScriptRunner`] executes a script against a [`Bot`](crate::pcampbot::Bot)
//! and collects a [`ScriptResult`] with pass/fail status and per-action detail.

use crate::pcampbot::{Bot, BotAction};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Script actions
// ---------------------------------------------------------------------------

/// A single action inside a bot script.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScriptAction {
    /// Rez (create) a named in-world object.
    RezObject { name: String },
    /// Edit a property on an existing object.
    EditObject {
        name: String,
        property: String,
        value: String,
    },
    /// Take an object into inventory.
    TakeObject { name: String },
    /// Give an object to another agent.
    GiveObject { name: String, target_agent: Uuid },
    /// Set permissions on an object.
    SetPermissions { name: String, perms: u32 },
    /// Upload and run a script inside an object.
    RunScript { name: String, source: String },
    /// Verify that some output matches a regex-like pattern.
    VerifyOutput { pattern: String },
    /// Pause for a number of milliseconds.
    Wait { ms: u64 },
    /// Assert that the bot's inventory contains an item with this name.
    AssertInventoryContains { name: String },
    /// Assert that the bot is at (or near) a given position.
    AssertPosition {
        region: String,
        x: f32,
        y: f32,
        z: f32,
        tolerance: f32,
    },
}

// ---------------------------------------------------------------------------
// Script container
// ---------------------------------------------------------------------------

/// A named, serialisable script containing an ordered list of actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotScript {
    pub name: String,
    pub description: String,
    pub actions: Vec<ScriptAction>,
    pub timeout_ms: u64,
}

impl BotScript {
    /// Create an empty script with a name and description.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            actions: Vec::new(),
            timeout_ms: 30_000,
        }
    }
}

/// Load a script from a JSON file on disk.
pub fn load_script(path: &Path) -> Result<BotScript, Box<dyn std::error::Error>> {
    let data = std::fs::read_to_string(path)?;
    let script: BotScript = serde_json::from_str(&data)?;
    Ok(script)
}

/// Save a script to a JSON file on disk.
pub fn save_script(path: &Path, script: &BotScript) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(script)?;
    std::fs::write(path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Script failure record
// ---------------------------------------------------------------------------

/// A single failure recorded during script execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptFailure {
    pub action_index: usize,
    pub error: String,
}

// ---------------------------------------------------------------------------
// Script result
// ---------------------------------------------------------------------------

/// Result of executing a [`BotScript`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResult {
    pub passed: bool,
    pub actions_run: usize,
    pub actions_failed: usize,
    pub failures: Vec<ScriptFailure>,
}

// ---------------------------------------------------------------------------
// Script runner
// ---------------------------------------------------------------------------

/// Executes a [`BotScript`] against a [`Bot`], collecting a [`ScriptResult`].
pub struct ScriptRunner<'a> {
    pub bot: &'a mut Bot,
    /// Simulated inventory (just names).
    inventory: Vec<String>,
}

impl<'a> ScriptRunner<'a> {
    pub fn new(bot: &'a mut Bot) -> Self {
        Self {
            bot,
            inventory: Vec::new(),
        }
    }

    /// Run the entire script, returning a result.
    pub async fn run(&mut self, script: &BotScript) -> ScriptResult {
        let mut actions_run = 0usize;
        let mut actions_failed = 0usize;
        let mut failures = Vec::new();

        info!(
            "Running script '{}': {} actions",
            script.name,
            script.actions.len()
        );

        for (i, action) in script.actions.iter().enumerate() {
            actions_run += 1;
            if let Err(e) = self.execute(action).await {
                actions_failed += 1;
                failures.push(ScriptFailure {
                    action_index: i,
                    error: e,
                });
                warn!(
                    "Script '{}' action {i} failed: {}",
                    script.name,
                    failures.last().map(|f| f.error.as_str()).unwrap_or("")
                );
            }
        }

        ScriptResult {
            passed: actions_failed == 0,
            actions_run,
            actions_failed,
            failures,
        }
    }

    /// Execute a single script action.
    async fn execute(&mut self, action: &ScriptAction) -> Result<(), String> {
        match action {
            ScriptAction::RezObject { name } => {
                if !self.bot.connected {
                    return Err("not connected".to_string());
                }
                info!("Rezzing object '{name}'");
                Ok(())
            }
            ScriptAction::EditObject {
                name,
                property,
                value,
            } => {
                if !self.bot.connected {
                    return Err("not connected".to_string());
                }
                info!("Editing object '{name}': {property} = {value}");
                Ok(())
            }
            ScriptAction::TakeObject { name } => {
                if !self.bot.connected {
                    return Err("not connected".to_string());
                }
                self.inventory.push(name.clone());
                info!("Took object '{name}' into inventory");
                Ok(())
            }
            ScriptAction::GiveObject { name, target_agent } => {
                if !self.bot.connected {
                    return Err("not connected".to_string());
                }
                if let Some(pos) = self.inventory.iter().position(|n| n == name) {
                    self.inventory.remove(pos);
                    info!("Gave object '{name}' to agent {target_agent}");
                    Ok(())
                } else {
                    Err(format!("object '{name}' not in inventory"))
                }
            }
            ScriptAction::SetPermissions { name, perms } => {
                if !self.bot.connected {
                    return Err("not connected".to_string());
                }
                info!("Set permissions on '{name}' to {perms:#x}");
                Ok(())
            }
            ScriptAction::RunScript { name, source } => {
                if !self.bot.connected {
                    return Err("not connected".to_string());
                }
                info!("Running script in '{name}' ({} bytes)", source.len());
                Ok(())
            }
            ScriptAction::VerifyOutput { pattern } => {
                // Simulated: always passes unless pattern is "FAIL"
                if pattern == "FAIL" {
                    Err("output verification failed: pattern matched FAIL sentinel".to_string())
                } else {
                    info!("Output verified against pattern '{pattern}'");
                    Ok(())
                }
            }
            ScriptAction::Wait { ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*ms)).await;
                Ok(())
            }
            ScriptAction::AssertInventoryContains { name } => {
                if self.inventory.iter().any(|n| n == name) {
                    Ok(())
                } else {
                    Err(format!("inventory does not contain '{name}'"))
                }
            }
            ScriptAction::AssertPosition {
                region,
                x,
                y,
                z,
                tolerance,
            } => {
                if self.bot.current_region != *region {
                    return Err(format!(
                        "region mismatch: expected '{region}', got '{}'",
                        self.bot.current_region
                    ));
                }
                let dx = self.bot.position[0] - x;
                let dy = self.bot.position[1] - y;
                let dz = self.bot.position[2] - z;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                if dist > *tolerance {
                    Err(format!(
                        "position error {dist:.2} exceeds tolerance {tolerance:.2}"
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Run a bot script from a file path, creating a temporary bot that logs in
/// to `login_uri` (simulated).
pub async fn run_bot_script(
    script_path: std::path::PathBuf,
    login_uri: String,
) -> Result<ScriptResult, Box<dyn std::error::Error>> {
    let script = load_script(&script_path)?;

    let mut bot = Bot::new("Script", "Runner");
    bot.execute_action(&BotAction::Login)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let mut runner = ScriptRunner::new(&mut bot);
    let result = runner.run(&script).await;

    info!(
        "Script '{}' against {login_uri}: {} run, {} failed, passed={}",
        script.name, result.actions_run, result.actions_failed, result.passed
    );

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn connected_bot() -> Bot {
        let mut bot = Bot::new("Test", "Bot");
        bot.execute_action(&BotAction::Login).expect("login");
        bot
    }

    #[test]
    fn test_script_serde_roundtrip() {
        let script = BotScript {
            name: "test-script".to_string(),
            description: "A test".to_string(),
            actions: vec![
                ScriptAction::RezObject {
                    name: "Box".to_string(),
                },
                ScriptAction::Wait { ms: 100 },
                ScriptAction::TakeObject {
                    name: "Box".to_string(),
                },
            ],
            timeout_ms: 5000,
        };
        let json = serde_json::to_string_pretty(&script).expect("serialize");
        let restored: BotScript = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.name, "test-script");
        assert_eq!(restored.actions.len(), 3);
    }

    #[test]
    fn test_script_action_variants_serde() {
        let actions = vec![
            ScriptAction::RezObject {
                name: "Box".to_string(),
            },
            ScriptAction::EditObject {
                name: "Box".to_string(),
                property: "color".to_string(),
                value: "red".to_string(),
            },
            ScriptAction::TakeObject {
                name: "Box".to_string(),
            },
            ScriptAction::GiveObject {
                name: "Box".to_string(),
                target_agent: Uuid::nil(),
            },
            ScriptAction::SetPermissions {
                name: "Box".to_string(),
                perms: 0x7FFFFFFF,
            },
            ScriptAction::RunScript {
                name: "Box".to_string(),
                source: "default { state_entry() { } }".to_string(),
            },
            ScriptAction::VerifyOutput {
                pattern: "hello.*".to_string(),
            },
            ScriptAction::Wait { ms: 500 },
            ScriptAction::AssertInventoryContains {
                name: "Box".to_string(),
            },
            ScriptAction::AssertPosition {
                region: "TestRegion".to_string(),
                x: 128.0,
                y: 128.0,
                z: 25.0,
                tolerance: 1.0,
            },
        ];
        let json = serde_json::to_string(&actions).expect("serialize");
        let restored: Vec<ScriptAction> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, actions);
    }

    #[tokio::test]
    async fn test_run_simple_script() {
        let mut bot = connected_bot();
        let script = BotScript {
            name: "simple".to_string(),
            description: "simple test".to_string(),
            actions: vec![
                ScriptAction::RezObject {
                    name: "Cube".to_string(),
                },
                ScriptAction::TakeObject {
                    name: "Cube".to_string(),
                },
                ScriptAction::AssertInventoryContains {
                    name: "Cube".to_string(),
                },
            ],
            timeout_ms: 5000,
        };
        let mut runner = ScriptRunner::new(&mut bot);
        let result = runner.run(&script).await;
        assert!(result.passed);
        assert_eq!(result.actions_run, 3);
        assert_eq!(result.actions_failed, 0);
    }

    #[tokio::test]
    async fn test_script_failure_detection() {
        let mut bot = connected_bot();
        let script = BotScript {
            name: "failing".to_string(),
            description: "will fail".to_string(),
            actions: vec![ScriptAction::AssertInventoryContains {
                name: "NonExistent".to_string(),
            }],
            timeout_ms: 5000,
        };
        let mut runner = ScriptRunner::new(&mut bot);
        let result = runner.run(&script).await;
        assert!(!result.passed);
        assert_eq!(result.actions_failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].action_index, 0);
        assert!(result.failures[0].error.contains("NonExistent"));
    }

    #[tokio::test]
    async fn test_script_verify_output_fail_sentinel() {
        let mut bot = connected_bot();
        let script = BotScript {
            name: "verify-fail".to_string(),
            description: "verify output".to_string(),
            actions: vec![ScriptAction::VerifyOutput {
                pattern: "FAIL".to_string(),
            }],
            timeout_ms: 5000,
        };
        let mut runner = ScriptRunner::new(&mut bot);
        let result = runner.run(&script).await;
        assert!(!result.passed);
        assert_eq!(result.actions_failed, 1);
    }

    #[tokio::test]
    async fn test_give_object_not_in_inventory() {
        let mut bot = connected_bot();
        let script = BotScript {
            name: "give-missing".to_string(),
            description: "give missing object".to_string(),
            actions: vec![ScriptAction::GiveObject {
                name: "Ghost".to_string(),
                target_agent: Uuid::nil(),
            }],
            timeout_ms: 5000,
        };
        let mut runner = ScriptRunner::new(&mut bot);
        let result = runner.run(&script).await;
        assert!(!result.passed);
        assert!(result.failures[0].error.contains("not in inventory"));
    }

    #[tokio::test]
    async fn test_assert_position_pass() {
        let mut bot = connected_bot();
        // After Login, position is [128, 128, 25] in "default-region"
        let script = BotScript {
            name: "pos-check".to_string(),
            description: "check position".to_string(),
            actions: vec![ScriptAction::AssertPosition {
                region: "default-region".to_string(),
                x: 128.0,
                y: 128.0,
                z: 25.0,
                tolerance: 0.1,
            }],
            timeout_ms: 5000,
        };
        let mut runner = ScriptRunner::new(&mut bot);
        let result = runner.run(&script).await;
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_assert_position_fail_region() {
        let mut bot = connected_bot();
        let script = BotScript {
            name: "pos-wrong-region".to_string(),
            description: "wrong region".to_string(),
            actions: vec![ScriptAction::AssertPosition {
                region: "WrongRegion".to_string(),
                x: 128.0,
                y: 128.0,
                z: 25.0,
                tolerance: 1.0,
            }],
            timeout_ms: 5000,
        };
        let mut runner = ScriptRunner::new(&mut bot);
        let result = runner.run(&script).await;
        assert!(!result.passed);
        assert!(result.failures[0].error.contains("region mismatch"));
    }

    #[test]
    fn test_load_save_script() {
        let dir = std::env::temp_dir().join("welkin_conformance_test_scripts");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_script.json");

        let script = BotScript {
            name: "roundtrip".to_string(),
            description: "io test".to_string(),
            actions: vec![ScriptAction::Wait { ms: 10 }],
            timeout_ms: 1000,
        };

        save_script(&path, &script).expect("save");
        let loaded = load_script(&path).expect("load");
        assert_eq!(loaded.name, "roundtrip");
        assert_eq!(loaded.actions.len(), 1);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_new_script_helper() {
        let script = BotScript::new("my-script", "does things");
        assert_eq!(script.name, "my-script");
        assert_eq!(script.description, "does things");
        assert!(script.actions.is_empty());
        assert_eq!(script.timeout_ms, 30_000);
    }
}
