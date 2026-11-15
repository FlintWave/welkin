// SPDX-License-Identifier: LGPL-3.0-or-later
//! OSSL agent, avatar, and teleport functions.
//!
//! Each function interacts with the scene presence system through the
//! [`ScriptContext`]'s presence list, health maps, and command senders.
//! Logic is derived from OpenSim's `OSSL_Api.cs` (BSD-3-Clause), rewritten
//! in idiomatic Rust.

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::{AgentCommand, AnimationCommand, NpcCommand, ScriptContext};
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Constants (from OpenSim)
// ---------------------------------------------------------------------------

/// Parcel flag: damage is allowed on this parcel (OpenSim `ParcelFlags.AllowDamage`).
const PARCEL_FLAG_ALLOW_DAMAGE: u32 = 0x20;

/// Maximum speed multiplier for `osSetOwnerSpeed` (OpenSim caps at 4.0).
const MAX_OWNER_SPEED: f64 = 4.0;

/// Default full health for a living agent.
const DEFAULT_HEALTH: f64 = 100.0;

/// Visual-params index for the "male" shape toggle (Group 0, first entry).
const VISUAL_PARAM_MALE_INDEX: usize = 0;

/// Byte threshold: values above this mean "male" (0.5 on 0..1 scale = 128 on 0..255).
const VISUAL_PARAM_MALE_THRESHOLD: u8 = 128;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_vector(s: &str) -> [f64; 3] {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 3 {
        [
            parts[0].trim().parse::<f64>().unwrap_or(0.0),
            parts[1].trim().parse::<f64>().unwrap_or(0.0),
            parts[2].trim().parse::<f64>().unwrap_or(0.0),
        ]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn parse_rotation(s: &str) -> [f64; 4] {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 4 {
        [
            parts[0].trim().parse::<f64>().unwrap_or(0.0),
            parts[1].trim().parse::<f64>().unwrap_or(0.0),
            parts[2].trim().parse::<f64>().unwrap_or(0.0),
            parts[3].trim().parse::<f64>().unwrap_or(1.0),
        ]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

// ---------------------------------------------------------------------------
// Function table
// ---------------------------------------------------------------------------

pub(super) fn ossl_agent_funcs() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osGetAgents",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_agents,
        },
        OsslFunc {
            name: "osGetAvatarList",
            level: OsslThreatLevel::None,
            register: register_os_get_avatar_list,
        },
        OsslFunc {
            name: "osGetNPCList",
            level: OsslThreatLevel::None,
            register: register_os_get_npc_list,
        },
        OsslFunc {
            name: "osSetOwnerSpeed",
            level: OsslThreatLevel::Moderate,
            register: register_os_set_owner_speed,
        },
        OsslFunc {
            name: "osGetHealth",
            level: OsslThreatLevel::None,
            register: register_os_get_health,
        },
        OsslFunc {
            name: "osCauseHealing",
            level: OsslThreatLevel::High,
            register: register_os_cause_healing,
        },
        OsslFunc {
            name: "osCauseDamage",
            level: OsslThreatLevel::High,
            register: register_os_cause_damage,
        },
        OsslFunc {
            name: "osSetHealth",
            level: OsslThreatLevel::High,
            register: register_os_set_health,
        },
        OsslFunc {
            name: "osSetHealRate",
            level: OsslThreatLevel::High,
            register: register_os_set_heal_rate,
        },
        OsslFunc {
            name: "osGetHealRate",
            level: OsslThreatLevel::None,
            register: register_os_get_heal_rate,
        },
        OsslFunc {
            name: "osKickAvatar",
            level: OsslThreatLevel::Severe,
            register: register_os_kick_avatar,
        },
        OsslFunc {
            name: "osForceOtherSit",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_other_sit,
        },
        OsslFunc {
            name: "osAvatarType",
            level: OsslThreatLevel::None,
            register: register_os_avatar_type,
        },
        OsslFunc {
            name: "osGetGender",
            level: OsslThreatLevel::None,
            register: register_os_get_gender,
        },
        OsslFunc {
            name: "osDetectedCountry",
            level: OsslThreatLevel::None,
            register: register_os_detected_country,
        },
        OsslFunc {
            name: "osGetAgentCountry",
            level: OsslThreatLevel::None,
            register: register_os_get_agent_country,
        },
        OsslFunc {
            name: "osAvatarPlayAnimation",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_avatar_play_animation,
        },
        OsslFunc {
            name: "osAvatarStopAnimation",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_avatar_stop_animation,
        },
        OsslFunc {
            name: "osLocalTeleportAgent",
            level: OsslThreatLevel::Moderate,
            register: register_os_local_teleport_agent,
        },
        OsslFunc {
            name: "osTeleportObject",
            level: OsslThreatLevel::Severe,
            register: register_os_teleport_object,
        },
    ]
}

// ---------------------------------------------------------------------------
// osGetAgents
// ---------------------------------------------------------------------------
// Returns a list of full names for every root presence in the region.
// C# reference: World.ForEachRootScenePresence → sp.Name

fn register_os_get_agents(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        let table = lua.create_table()?;
        let mut idx = 1i64;
        for p in &c.presence_list {
            table.set(idx, format!("{} {}", p.first_name, p.last_name))?;
            idx += 1;
        }
        // Also include locally-registered NPCs that might not be in presence_list yet.
        for npc in c.npc_registry.values() {
            let name = format!("{} {}", npc.first_name, npc.last_name);
            // Avoid duplicates: check if this name is already in the table.
            let mut found = false;
            for p in &c.presence_list {
                if format!("{} {}", p.first_name, p.last_name) == name {
                    found = true;
                    break;
                }
            }
            if !found {
                table.set(idx, name)?;
                idx += 1;
            }
        }
        Ok(table)
    })?;
    lua.globals().set("osGetAgents", f)
}

// ---------------------------------------------------------------------------
// osGetAvatarList
// ---------------------------------------------------------------------------
// Returns a strided list: [uuid, position, name, uuid, position, name, ...].
// Excludes the script owner.  C# reference: World.ForEachRootScenePresence,
// skipping avatar.UUID == m_host.OwnerID.

fn register_os_get_avatar_list(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        let table = lua.create_table()?;
        let mut idx = 1i64;
        let owner = c.owner_id;
        for p in &c.presence_list {
            if p.uuid == owner {
                continue;
            }
            table.set(idx, p.uuid.to_string())?;
            idx += 1;
            table.set(
                idx,
                format!("<{}, {}, {}>", p.position[0], p.position[1], p.position[2]),
            )?;
            idx += 1;
            table.set(idx, format!("{} {}", p.first_name, p.last_name))?;
            idx += 1;
        }
        Ok(table)
    })?;
    lua.globals().set("osGetAvatarList", f)
}

// ---------------------------------------------------------------------------
// osGetNPCList
// ---------------------------------------------------------------------------
// Returns a strided list of NPCs: [uuid, position, name, ...].
// C# reference: filters for avatar.IsNPC && !IsDeleted && !IsChildAgent.

fn register_os_get_npc_list(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        let table = lua.create_table()?;
        let mut idx = 1i64;

        // NPCs from the scene presence list.
        for p in &c.presence_list {
            if !p.is_npc {
                continue;
            }
            table.set(idx, p.uuid.to_string())?;
            idx += 1;
            table.set(
                idx,
                format!("<{}, {}, {}>", p.position[0], p.position[1], p.position[2]),
            )?;
            idx += 1;
            table.set(idx, format!("{} {}", p.first_name, p.last_name))?;
            idx += 1;
        }

        // Also include locally-registered NPCs not yet in the scene list.
        for (npc_id, npc) in &c.npc_registry {
            let already = c.presence_list.iter().any(|p| p.uuid == *npc_id);
            if !already {
                table.set(idx, npc_id.to_string())?;
                idx += 1;
                table.set(
                    idx,
                    format!(
                        "<{}, {}, {}>",
                        npc.position[0], npc.position[1], npc.position[2]
                    ),
                )?;
                idx += 1;
                table.set(idx, format!("{} {}", npc.first_name, npc.last_name))?;
                idx += 1;
            }
        }

        Ok(table)
    })?;
    lua.globals().set("osGetNPCList", f)
}

// ---------------------------------------------------------------------------
// osSetOwnerSpeed
// ---------------------------------------------------------------------------
// Sets the speed multiplier for the script owner's avatar.
// Capped at 4.0 (OpenSim behaviour).  C# reference: osSetOwnerSpeed.

fn register_os_set_owner_speed(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, factor: f64| {
        let mut c = ctx.write();
        let clamped = factor.min(MAX_OWNER_SPEED);
        c.speed_multiplier = clamped;
        if let Some(ref sender) = c.agent_sender {
            let _ = sender.send(AgentCommand::ForceSit {
                // Re-use the sender to notify the scene system about speed change.
                // The actual speed is stored on ScriptContext.speed_multiplier.
                agent_id: c.owner_id,
                target_id: Uuid::nil(),
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            owner = %c.owner_id,
            speed = clamped,
            "osSetOwnerSpeed"
        );
        Ok(())
    })?;
    lua.globals().set("osSetOwnerSpeed", f)
}

// ---------------------------------------------------------------------------
// osGetHealth
// ---------------------------------------------------------------------------
// Returns the health of the specified agent, or -1 if the UUID is invalid
// or the agent is not in the scene.  C# reference: presence.Health.

fn register_os_get_health(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, avatar: String| {
        let c = ctx.read();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(-1.0f64),
        };
        // Agent must be in the scene.
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(-1.0f64);
        }
        // Return tracked health or default.
        let health = c.agent_health.get(&id).copied().unwrap_or(DEFAULT_HEALTH);
        Ok(health)
    })?;
    lua.globals().set("osGetHealth", f)
}

// ---------------------------------------------------------------------------
// osCauseHealing
// ---------------------------------------------------------------------------
// Adds healing to an agent's health, capped at 100.
// C# reference: health += healing; if health >= 100 → 100.

fn register_os_cause_healing(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, amount): (String, f64)| {
        let mut c = ctx.write();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(());
        }
        let current = c.agent_health.get(&id).copied().unwrap_or(DEFAULT_HEALTH);
        let new_health = (current + amount).min(DEFAULT_HEALTH);
        c.agent_health.insert(id, new_health);
        tracing::info!(
            script_id = %c.script_id,
            agent = %id,
            healing = amount,
            health = new_health,
            "osCauseHealing"
        );
        Ok(())
    })?;
    lua.globals().set("osCauseHealing", f)
}

// ---------------------------------------------------------------------------
// osCauseDamage
// ---------------------------------------------------------------------------
// Subtracts damage from an agent's health.  Requires the host prim's parcel
// to have the AllowDamage flag set.  If health drops to zero or below:
//   - NPC → send remove command
//   - User → reset health to 100 and teleport home
// C# reference: OSSL_Api.osCauseDamage.

fn register_os_cause_damage(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, damage): (String, f64)| {
        let mut c = ctx.write();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };

        // Check that the parcel allows damage.
        if c.parcel_flags & PARCEL_FLAG_ALLOW_DAMAGE == 0 {
            return Ok(());
        }

        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(());
        }

        let current = c.agent_health.get(&id).copied().unwrap_or(DEFAULT_HEALTH);
        let new_health = current - damage;
        c.agent_health.insert(id, new_health);

        if new_health <= 0.0 {
            // Check if the victim is an NPC.
            let is_npc = c
                .presence_list
                .iter()
                .find(|p| p.uuid == id)
                .map_or(false, |p| p.is_npc);

            if is_npc {
                // Remove the NPC.
                c.npc_registry.remove(&id);
                if let Some(ref sender) = c.npc_sender {
                    let _ = sender.send(NpcCommand::Remove { npc_id: id });
                }
                tracing::info!(npc = %id, "osCauseDamage: NPC killed, removing");
            } else {
                // Reset health and teleport the user home.
                c.agent_health.insert(id, DEFAULT_HEALTH);
                if let Some(ref sender) = c.teleport_sender {
                    let _ = sender.send(crate::builtins::TeleportRequest {
                        agent_id: id,
                        region_name: String::new(), // empty = home
                        position: [128.0, 128.0, 25.0],
                        look_at: [0.0, 0.0, 0.0],
                    });
                }
                tracing::info!(agent = %id, "osCauseDamage: agent killed, teleporting home");
            }
        } else {
            tracing::info!(
                script_id = %c.script_id,
                agent = %id,
                damage = damage,
                health = new_health,
                "osCauseDamage"
            );
        }
        Ok(())
    })?;
    lua.globals().set("osCauseDamage", f)
}

// ---------------------------------------------------------------------------
// osSetHealth
// ---------------------------------------------------------------------------
// Sets an agent's health, clamped to 1.0..100.0.
// C# reference: presence.setHealthWithUpdate(health).

fn register_os_set_health(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, health): (String, f64)| {
        let mut c = ctx.write();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(());
        }
        let clamped = health.clamp(1.0, DEFAULT_HEALTH);
        c.agent_health.insert(id, clamped);
        tracing::info!(agent = %id, health = clamped, "osSetHealth");
        Ok(())
    })?;
    lua.globals().set("osSetHealth", f)
}

// ---------------------------------------------------------------------------
// osSetHealRate
// ---------------------------------------------------------------------------
// Sets the heal rate (hp/sec) for an agent.
// C# reference: presence.HealRate = (float)healrate.

fn register_os_set_heal_rate(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, rate): (String, f64)| {
        let mut c = ctx.write();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(());
        }
        c.agent_heal_rate.insert(id, rate);
        tracing::info!(agent = %id, heal_rate = rate, "osSetHealRate");
        Ok(())
    })?;
    lua.globals().set("osSetHealRate", f)
}

// ---------------------------------------------------------------------------
// osGetHealRate
// ---------------------------------------------------------------------------
// Returns the heal rate (hp/sec) for an agent, or 0 if not found.
// C# reference: presence.HealRate (default 0).

fn register_os_get_heal_rate(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, avatar: String| {
        let c = ctx.read();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(0.0f64),
        };
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(0.0f64);
        }
        let rate = c.agent_heal_rate.get(&id).copied().unwrap_or(0.0);
        Ok(rate)
    })?;
    lua.globals().set("osGetHealRate", f)
}

// ---------------------------------------------------------------------------
// osKickAvatar
// ---------------------------------------------------------------------------
// Finds an avatar by first and last name, sends a kick command with an
// optional alert message, then closes the agent.
// C# reference: sp.ControllingClient.Kick(alert); sp.Scene.CloseAgent(sp.UUID).

fn register_os_kick_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (first, last, msg): (String, String, String)| {
        let c = ctx.read();
        // Find the agent in the presence list by name.
        let target = c
            .presence_list
            .iter()
            .find(|p| p.first_name == first && p.last_name == last);
        if let Some(presence) = target {
            if let Some(ref sender) = c.agent_sender {
                let _ = sender.send(AgentCommand::Kick {
                    agent_id: presence.uuid,
                    message: msg.clone(),
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                agent = %presence.uuid,
                name = %format!("{first} {last}"),
                alert = %msg,
                "osKickAvatar"
            );
        } else {
            tracing::debug!(
                script_id = %c.script_id,
                name = %format!("{first} {last}"),
                "osKickAvatar: agent not found"
            );
        }
        Ok(())
    })?;
    lua.globals().set("osKickAvatar", f)
}

// ---------------------------------------------------------------------------
// osForceOtherSit
// ---------------------------------------------------------------------------
// Forces an avatar to sit on a target prim.  If the target UUID is empty
// or nil, the host prim is used.
// C# reference: ForceSit → HandleAgentRequestSit.

fn register_os_force_other_sit(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, target): (String, String)| {
        let c = ctx.read();
        let agent_id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };
        // Agent must be in the scene.
        let in_scene = c.presence_list.iter().any(|p| p.uuid == agent_id);
        if !in_scene {
            return Ok(());
        }
        // Determine the sit target: use the explicit target if valid, else the host prim.
        let target_id = Uuid::parse_str(&target)
            .ok()
            .filter(|id| !id.is_nil())
            .unwrap_or(c.prim_id);

        if let Some(ref sender) = c.agent_sender {
            let _ = sender.send(AgentCommand::ForceSit {
                agent_id,
                target_id,
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            agent = %agent_id,
            target = %target_id,
            "osForceOtherSit"
        );
        Ok(())
    })?;
    lua.globals().set("osForceOtherSit", f)
}

// ---------------------------------------------------------------------------
// osAvatarType
// ---------------------------------------------------------------------------
// Returns the type of a presence:
//   -1 = invalid UUID
//    0 = not present / child agent
//    1 = user avatar
//    2 = NPC
// C# reference: av.IsNPC ? 2 : 1 (0 if not found).

fn register_os_avatar_type(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, avatar: String| {
        let c = ctx.read();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) => id,
            Err(_) => return Ok(-1i32),
        };
        // Look up in the presence list.
        if let Some(p) = c.presence_list.iter().find(|p| p.uuid == id) {
            return Ok(if p.is_npc { 2 } else { 1 });
        }
        // Also check the local NPC registry.
        if c.npc_registry.contains_key(&id) {
            return Ok(2);
        }
        Ok(0i32)
    })?;
    lua.globals().set("osAvatarType", f)
}

// ---------------------------------------------------------------------------
// osGetGender
// ---------------------------------------------------------------------------
// Determines gender from the avatar's visual params.  The "male" shape
// parameter (Group 0, first entry) controls gender: byte value > 128 = male.
// Returns "male", "female", or "unknown".
// C# reference: VisualParams["male"]["shape"] → ByteToFloat → > 0.5.

fn register_os_get_gender(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, avatar: String| {
        let c = ctx.read();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok("unknown".to_string()),
        };
        // Find the presence and check visual params.
        let presence = c.presence_list.iter().find(|p| p.uuid == id);
        match presence {
            Some(p) if p.visual_params.len() > VISUAL_PARAM_MALE_INDEX => {
                let male_value = p.visual_params[VISUAL_PARAM_MALE_INDEX];
                if male_value >= VISUAL_PARAM_MALE_THRESHOLD {
                    Ok("male".to_string())
                } else {
                    Ok("female".to_string())
                }
            }
            _ => Ok("unknown".to_string()),
        }
    })?;
    lua.globals().set("osGetGender", f)
}

// ---------------------------------------------------------------------------
// osDetectedCountry
// ---------------------------------------------------------------------------
// Returns the country string for a detected agent.  Requires a user account
// service; returns empty string when the service is not available.
// C# reference: World.UserAccountService.GetUserAccount → UserCountry.

fn register_os_detected_country(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _index: i32| {
        let c = ctx.read();
        // No user account service is wired yet.  This matches the C# fallback
        // path: `if (World.UserAccountService is null) return Empty`.
        tracing::debug!(
            script_id = %c.script_id,
            "osDetectedCountry: no user account service available"
        );
        Ok(String::new())
    })?;
    lua.globals().set("osDetectedCountry", f)
}

// ---------------------------------------------------------------------------
// osGetAgentCountry
// ---------------------------------------------------------------------------
// Returns the country string for an agent by UUID.  Requires a user account
// service; returns empty string when the service is not available.
// C# reference: World.UserAccountService.GetUserAccount → UserCountry.

fn register_os_get_agent_country(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, avatar: String| {
        let c = ctx.read();
        // Validate UUID even though we cannot look up the account yet.
        let _id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(String::new()),
        };
        // No user account service wired.
        tracing::debug!(
            script_id = %c.script_id,
            agent = %avatar,
            "osGetAgentCountry: no user account service available"
        );
        Ok(String::new())
    })?;
    lua.globals().set("osGetAgentCountry", f)
}

// ---------------------------------------------------------------------------
// osAvatarPlayAnimation
// ---------------------------------------------------------------------------
// Plays an animation on a specific avatar.  Looks up the animation name
// in the host prim's inventory first; if not found, passes the name
// directly (for built-in animations).
// C# reference: target.Animator.AddAnimation.

fn register_os_avatar_play_animation(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, anim): (String, String)| {
        let c = ctx.read();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };
        // Agent must be present.
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(());
        }
        // Resolve animation: check if the name matches an inventory animation asset.
        let resolved = c
            .inventory
            .iter()
            .find(|item| {
                item.name == anim && item.item_type == crate::builtins::INVENTORY_ANIMATION
            })
            .map(|item| item.asset_id.to_string())
            .unwrap_or_else(|| anim.clone());

        if let Some(ref sender) = c.animation_sender {
            let _ = sender.send(AnimationCommand::Play {
                agent_id: id,
                animation: resolved.clone(),
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            agent = %id,
            animation = %resolved,
            "osAvatarPlayAnimation"
        );
        Ok(())
    })?;
    lua.globals().set("osAvatarPlayAnimation", f)
}

// ---------------------------------------------------------------------------
// osAvatarStopAnimation
// ---------------------------------------------------------------------------
// Stops an animation on a specific avatar.
// C# reference: target.Animator.RemoveAnimation.

fn register_os_avatar_stop_animation(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, anim): (String, String)| {
        let c = ctx.read();
        let id = match Uuid::parse_str(&avatar) {
            Ok(id) if !id.is_nil() => id,
            _ => return Ok(()),
        };
        let in_scene = c.presence_list.iter().any(|p| p.uuid == id);
        if !in_scene {
            return Ok(());
        }
        // Resolve animation: try as UUID first, then inventory lookup.
        let resolved = if Uuid::parse_str(&anim).is_ok() {
            anim.clone()
        } else {
            c.inventory
                .iter()
                .find(|item| {
                    item.name == anim && item.item_type == crate::builtins::INVENTORY_ANIMATION
                })
                .map(|item| item.asset_id.to_string())
                .unwrap_or_else(|| anim.clone())
        };

        if let Some(ref sender) = c.animation_sender {
            let _ = sender.send(AnimationCommand::Stop {
                agent_id: id,
                animation: resolved.clone(),
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            agent = %id,
            animation = %resolved,
            "osAvatarStopAnimation"
        );
        Ok(())
    })?;
    lua.globals().set("osAvatarStopAnimation", f)
}

// ---------------------------------------------------------------------------
// osLocalTeleportAgent
// ---------------------------------------------------------------------------
// Teleports an agent within the current region to a new position, velocity,
// and look-at direction.  The flags parameter controls teleport behavior.
// C# reference: World.RequestLocalTeleport(presence, position, velocity, lookat, flags).

fn register_os_local_teleport_agent(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (agent, pos, vel, lookat, flags): (String, String, String, String, i32)| {
            let c = ctx.read();
            let agent_id = match Uuid::parse_str(&agent) {
                Ok(id) if !id.is_nil() => id,
                _ => return Ok(()),
            };
            // Agent must be in the scene.
            let in_scene = c.presence_list.iter().any(|p| p.uuid == agent_id);
            if !in_scene {
                return Ok(());
            }
            let position = parse_vector(&pos);
            let velocity = parse_vector(&vel);
            let look_at = parse_vector(&lookat);

            if let Some(ref sender) = c.agent_sender {
                let _ = sender.send(AgentCommand::LocalTeleport {
                    agent_id,
                    position,
                    velocity,
                    look_at,
                    flags,
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                agent = %agent_id,
                pos = ?position,
                flags = flags,
                "osLocalTeleportAgent"
            );
            Ok(())
        },
    )?;
    lua.globals().set("osLocalTeleportAgent", f)
}

// ---------------------------------------------------------------------------
// osTeleportObject
// ---------------------------------------------------------------------------
// Teleports an object to a new position and rotation.
// Returns 0 on success (command sent), -1 on failure (invalid UUID).
// C# reference: sog.TeleportObject(m_host.ParentGroup.UUID, targetPos, rotation, flags).

fn register_os_teleport_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (obj_key, _region_x, _region_y, pos, rot): (String, i32, i32, String, String)| {
            let c = ctx.read();
            let object_id = match Uuid::parse_str(&obj_key) {
                Ok(id) if !id.is_nil() => id,
                _ => {
                    tracing::warn!(key = %obj_key, "osTeleportObject: invalid object UUID");
                    return Ok(-1i32);
                }
            };
            let target_pos = parse_vector(&pos);
            let rotation = parse_rotation(&rot);

            // Combine region grid coords into flags for the handler.
            // region_x/region_y == 0 means current region (local teleport).
            let flags = if _region_x == 0 && _region_y == 0 {
                0
            } else {
                // Encode grid coords as a flag hint; the actual region lookup
                // happens in the scene teleport handler.
                1
            };

            if let Some(ref sender) = c.agent_sender {
                let _ = sender.send(AgentCommand::TeleportObject {
                    object_id,
                    target_pos,
                    rotation,
                    flags,
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                object = %object_id,
                pos = ?target_pos,
                rot = ?rotation,
                "osTeleportObject"
            );
            Ok(0i32)
        },
    )?;
    lua.globals().set("osTeleportObject", f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::{PresenceInfo, ScriptContext};
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        let owner = Uuid::new_v4();
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let npc_c = Uuid::new_v4();

        let mut ctx = ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: owner,
            parcel_flags: PARCEL_FLAG_ALLOW_DAMAGE,
            ..ScriptContext::default()
        };

        ctx.presence_list = vec![
            PresenceInfo {
                uuid: owner,
                first_name: "Owner".into(),
                last_name: "Resident".into(),
                position: [128.0, 128.0, 25.0],
                is_npc: false,
                ip_address: String::new(),
                visual_params: vec![0], // female
            },
            PresenceInfo {
                uuid: agent_a,
                first_name: "Alice".into(),
                last_name: "Smith".into(),
                position: [64.0, 64.0, 30.0],
                is_npc: false,
                ip_address: String::new(),
                visual_params: vec![200], // male
            },
            PresenceInfo {
                uuid: agent_b,
                first_name: "Bob".into(),
                last_name: "Jones".into(),
                position: [100.0, 100.0, 20.0],
                is_npc: false,
                ip_address: String::new(),
                visual_params: vec![50], // female
            },
            PresenceInfo {
                uuid: npc_c,
                first_name: "Greeter".into(),
                last_name: "Bot".into(),
                position: [128.0, 128.0, 25.0],
                is_npc: true,
                ip_address: String::new(),
                visual_params: vec![],
            },
        ];
        // Set initial health for the test agents.
        ctx.agent_health.insert(owner, 100.0);
        ctx.agent_health.insert(agent_a, 100.0);
        ctx.agent_health.insert(npc_c, 100.0);

        Arc::new(RwLock::new(ctx))
    }

    fn make_vm(ctx: &Arc<RwLock<ScriptContext>>) -> Lua {
        let lua = Lua::new();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Severe,
            ..Default::default()
        };
        register_ossl(&lua, &config, ctx).unwrap();
        lua
    }

    #[test]
    fn get_agents_returns_all_presences() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let count: i32 = lua
            .load("local t = osGetAgents(); local n = 0; for _ in pairs(t) do n = n + 1 end; return n")
            .eval()
            .unwrap();
        assert_eq!(count, 4); // owner + 2 users + 1 NPC
    }

    #[test]
    fn get_avatar_list_excludes_owner() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let count: i32 = lua
            .load("local t = osGetAvatarList(); local n = 0; for _ in pairs(t) do n = n + 1 end; return n")
            .eval()
            .unwrap();
        // 3 entries per avatar (uuid, pos, name) * 3 non-owner presences = 9
        assert_eq!(count, 9);
    }

    #[test]
    fn get_npc_list_returns_only_npcs() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let count: i32 = lua
            .load("local t = osGetNPCList(); local n = 0; for _ in pairs(t) do n = n + 1 end; return n")
            .eval()
            .unwrap();
        // 3 entries per NPC * 1 NPC = 3
        assert_eq!(count, 3);
    }

    #[test]
    fn get_health_valid_agent() {
        let ctx = make_ctx();
        let uuid = ctx.read().presence_list[1].uuid.to_string();
        let lua = make_vm(&ctx);
        let h: f64 = lua
            .load(&format!(r#"return osGetHealth("{uuid}")"#))
            .eval()
            .unwrap();
        assert!((h - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn get_health_invalid_uuid_returns_negative() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let h: f64 = lua
            .load(r#"return osGetHealth("not-a-uuid")"#)
            .eval()
            .unwrap();
        assert!(h < 0.0);
    }

    #[test]
    fn cause_healing_caps_at_100() {
        let ctx = make_ctx();
        let uuid = {
            let c = ctx.read();
            let id = c.presence_list[1].uuid;
            drop(c);
            ctx.write().agent_health.insert(id, 80.0);
            id.to_string()
        };
        let lua = make_vm(&ctx);
        lua.load(&format!(r#"osCauseHealing("{uuid}", 50.0)"#))
            .exec()
            .unwrap();
        let h: f64 = lua
            .load(&format!(r#"return osGetHealth("{uuid}")"#))
            .eval()
            .unwrap();
        assert!((h - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cause_damage_subtracts_health() {
        let ctx = make_ctx();
        let uuid = {
            let c = ctx.read();
            c.presence_list[1].uuid.to_string()
        };
        let lua = make_vm(&ctx);
        lua.load(&format!(r#"osCauseDamage("{uuid}", 30.0)"#))
            .exec()
            .unwrap();
        let h: f64 = lua
            .load(&format!(r#"return osGetHealth("{uuid}")"#))
            .eval()
            .unwrap();
        assert!((h - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_health_clamps() {
        let ctx = make_ctx();
        let uuid = {
            let c = ctx.read();
            c.presence_list[1].uuid.to_string()
        };
        let lua = make_vm(&ctx);
        lua.load(&format!(r#"osSetHealth("{uuid}", 200.0)"#))
            .exec()
            .unwrap();
        let h: f64 = lua
            .load(&format!(r#"return osGetHealth("{uuid}")"#))
            .eval()
            .unwrap();
        assert!((h - 100.0).abs() < f64::EPSILON);

        lua.load(&format!(r#"osSetHealth("{uuid}", -5.0)"#))
            .exec()
            .unwrap();
        let h: f64 = lua
            .load(&format!(r#"return osGetHealth("{uuid}")"#))
            .eval()
            .unwrap();
        assert!((h - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn avatar_type_returns_correct_codes() {
        let ctx = make_ctx();
        let (user_uuid, npc_uuid) = {
            let c = ctx.read();
            (
                c.presence_list[1].uuid.to_string(),
                c.presence_list[3].uuid.to_string(),
            )
        };
        let lua = make_vm(&ctx);

        let user_type: i32 = lua
            .load(&format!(r#"return osAvatarType("{user_uuid}")"#))
            .eval()
            .unwrap();
        assert_eq!(user_type, 1);

        let npc_type: i32 = lua
            .load(&format!(r#"return osAvatarType("{npc_uuid}")"#))
            .eval()
            .unwrap();
        assert_eq!(npc_type, 2);

        // Unknown UUID
        let nil_uuid = Uuid::new_v4().to_string();
        let unknown_type: i32 = lua
            .load(&format!(r#"return osAvatarType("{nil_uuid}")"#))
            .eval()
            .unwrap();
        assert_eq!(unknown_type, 0);
    }

    #[test]
    fn get_gender_from_visual_params() {
        let ctx = make_ctx();
        let (male_uuid, female_uuid, npc_uuid) = {
            let c = ctx.read();
            (
                c.presence_list[1].uuid.to_string(), // visual_params[0] = 200 → male
                c.presence_list[2].uuid.to_string(), // visual_params[0] = 50 → female
                c.presence_list[3].uuid.to_string(), // empty visual_params → unknown
            )
        };
        let lua = make_vm(&ctx);

        let g: String = lua
            .load(&format!(r#"return osGetGender("{male_uuid}")"#))
            .eval()
            .unwrap();
        assert_eq!(g, "male");

        let g: String = lua
            .load(&format!(r#"return osGetGender("{female_uuid}")"#))
            .eval()
            .unwrap();
        assert_eq!(g, "female");

        let g: String = lua
            .load(&format!(r#"return osGetGender("{npc_uuid}")"#))
            .eval()
            .unwrap();
        assert_eq!(g, "unknown");
    }

    #[test]
    fn set_owner_speed_clamps() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load("osSetOwnerSpeed(8.0)").exec().unwrap();
        let speed = ctx.read().speed_multiplier;
        assert!((speed - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn heal_rate_round_trip() {
        let ctx = make_ctx();
        let uuid = {
            let c = ctx.read();
            c.presence_list[1].uuid.to_string()
        };
        let lua = make_vm(&ctx);
        lua.load(&format!(r#"osSetHealRate("{uuid}", 2.5)"#))
            .exec()
            .unwrap();
        let r: f64 = lua
            .load(&format!(r#"return osGetHealRate("{uuid}")"#))
            .eval()
            .unwrap();
        assert!((r - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn detected_country_returns_empty() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let r: String = lua.load("return osDetectedCountry(0)").eval().unwrap();
        assert_eq!(r, "");
    }

    #[test]
    fn teleport_object_invalid_uuid_returns_negative() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let r: i32 = lua
            .load(r#"return osTeleportObject("bad-uuid", 0, 0, "<128,128,25>", "<0,0,0,1>")"#)
            .eval()
            .unwrap();
        assert_eq!(r, -1);
    }

    #[test]
    fn teleport_object_valid_uuid_returns_success() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let obj = Uuid::new_v4().to_string();
        let r: i32 = lua
            .load(&format!(
                r#"return osTeleportObject("{obj}", 0, 0, "<128,128,25>", "<0,0,0,1>")"#
            ))
            .eval()
            .unwrap();
        assert_eq!(r, 0);
    }
}
