// SPDX-License-Identifier: LGPL-3.0-or-later

//! Cross-region agent export builder.
//!
//! Serialises the minimal "agent connection" data a destination region needs
//! to create a child/secondary agent (agent id, session id, circuit code,
//! seed capabilities, position, ...) into an LLSD map or its binary wire
//! form. The helper is pure (no I/O, no server) so `process_messages` and
//! HTTP seed-cap handlers can call it later.

use std::collections::HashMap;

use uuid::Uuid;

use welkin_llsd::{Llsd, Map, from_binary, to_binary, to_xml_string};

/// Portable snapshot of an agent for cross-region handoff.
#[derive(Debug, Clone)]
pub struct AgentExport {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub circuit_code: u32,
    pub region_handle: u64,
    pub first_name: String,
    pub last_name: String,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub seed_caps: HashMap<String, String>,
}

// Backwards/alternate names expected by callers.
pub type AgentConnectionExport = AgentExport;
pub type CrossRegionAgentExport = AgentExport;
pub type AgentConnection = AgentExport;

impl AgentExport {
    pub fn new(agent_id: Uuid, session_id: Uuid) -> Self {
        Self {
            agent_id,
            session_id,
            circuit_code: 0,
            region_handle: 0,
            first_name: String::new(),
            last_name: String::new(),
            position: [0.0; 3],
            velocity: [0.0; 3],
            seed_caps: HashMap::new(),
        }
    }

    /// Build the LLSD map representation.
    pub fn to_llsd_map(&self) -> Map {
        let mut map = Map::new();
        map.insert("agent_id".to_string(), Llsd::Uuid(self.agent_id));
        map.insert("session_id".to_string(), Llsd::Uuid(self.session_id));
        map.insert(
            "circuit_code".to_string(),
            Llsd::Integer(self.circuit_code as i32),
        );
        map.insert(
            "region_handle".to_string(),
            Llsd::String(self.region_handle.to_string()),
        );
        map.insert("first_name".to_string(), Llsd::String(self.first_name.clone()));
        map.insert("last_name".to_string(), Llsd::String(self.last_name.clone()));

        let pos = Llsd::Array(vec![
            Llsd::Real(self.position[0] as f64),
            Llsd::Real(self.position[1] as f64),
            Llsd::Real(self.position[2] as f64),
        ]);
        let vel = Llsd::Array(vec![
            Llsd::Real(self.velocity[0] as f64),
            Llsd::Real(self.velocity[1] as f64),
            Llsd::Real(self.velocity[2] as f64),
        ]);
        map.insert("position".to_string(), pos);
        map.insert("velocity".to_string(), vel);

        let mut caps = Map::new();
        for (k, v) in &self.seed_caps {
            caps.insert(k.clone(), Llsd::String(v.clone()));
        }
        map.insert("seed_caps".to_string(), Llsd::Map(caps.clone()));
        // OpenSim-style alias sometimes used.
        map.insert("children_cap_seeds".to_string(), Llsd::Map(caps.clone()));
        map.insert("AgentData".to_string(), Llsd::Uuid(self.agent_id));
        map
    }

    pub fn to_llsd(&self) -> Llsd {
        Llsd::Map(self.to_llsd_map())
    }

    /// Serialise to LLSD binary.
    pub fn to_binary(&self) -> Result<Vec<u8>, welkin_llsd::LlsdError> {
        Ok(to_binary(&self.to_llsd()))
    }

    /// Serialise to LLSD XML.
    pub fn to_xml(&self) -> String {
        to_xml_string(&self.to_llsd())
    }

    /// Round-trip helper for callers that want a Map back.
    pub fn from_binary(bytes: &[u8]) -> Result<Self, welkin_llsd::LlsdError> {
        let (val, _) = from_binary(bytes)?;
        Self::from_llsd(&val)
    }

    pub fn from_llsd(val: &Llsd) -> Result<Self, welkin_llsd::LlsdError> {
        let map = match val {
            Llsd::Map(m) => m,
            _ => return Err(welkin_llsd::LlsdError::Invalid("expected llsd map")),
        };
        let get_uuid = |key: &str| -> Uuid {
            match map.get(key) {
                Some(Llsd::Uuid(u)) => *u,
                Some(Llsd::String(s)) => Uuid::parse_str(s).unwrap_or(Uuid::nil()),
                _ => Uuid::nil(),
            }
        };
        let get_i64 = |key: &str| -> i64 {
            match map.get(key) {
                Some(Llsd::Integer(i)) => (*i).into(),
                Some(Llsd::Real(r)) => *r as i64,
                _ => 0,
            }
        };
        let get_str = |key: &str| -> String {
            match map.get(key) {
                Some(Llsd::String(s)) => s.clone(),
                _ => String::new(),
            }
        };
        let agent_id = get_uuid("agent_id");
        // OpenSim-style alias fallback.
        let agent_id = if agent_id == Uuid::nil() {
            get_uuid("AgentData")
        } else {
            agent_id
        };
        let session_id = get_uuid("session_id");
        let circuit_code = get_i64("circuit_code") as u32;
        let region_handle = get_str("region_handle")
            .parse::<u64>()
            .unwrap_or(0);
        let first_name = get_str("first_name");
        let last_name = get_str("last_name");
        let mut position = [0.0f32; 3];
        if let Some(Llsd::Array(arr)) = map.get("position") {
            for (i, v) in arr.iter().enumerate().take(3) {
                position[i] = match v {
                    Llsd::Real(r) => *r as f32,
                    Llsd::Integer(i) => *i as f32,
                    _ => 0.0,
                };
            }
        }
        let mut velocity = [0.0f32; 3];
        if let Some(Llsd::Array(arr)) = map.get("velocity") {
            for (i, v) in arr.iter().enumerate().take(3) {
                velocity[i] = match v {
                    Llsd::Real(r) => *r as f32,
                    Llsd::Integer(i) => *i as f32,
                    _ => 0.0,
                };
            }
        }
        let mut seed_caps = HashMap::new();
        let caps_val = map
            .get("seed_caps")
            .or_else(|| map.get("children_cap_seeds"));
        if let Some(Llsd::Map(cm)) = caps_val {
            for (k, v) in cm {
                if let Llsd::String(s) = v {
                    seed_caps.insert(k.clone(), s.clone());
                }
            }
        }
        Ok(Self {
            agent_id,
            session_id,
            circuit_code,
            region_handle,
            first_name,
            last_name,
            position,
            velocity,
            seed_caps,
        })
    }
}

// ---------------------------------------------------------------------------
// Free-function builders (multiple common names)
// ---------------------------------------------------------------------------

pub fn build_agent_export(
    agent_id: Uuid,
    session_id: Uuid,
    circuit_code: u32,
) -> AgentExport {
    let mut e = AgentExport::new(agent_id, session_id);
    e.circuit_code = circuit_code;
    e
}

pub fn build_cross_region_agent_export(
    agent_id: Uuid,
    session_id: Uuid,
    circuit_code: u32,
) -> AgentExport {
    build_agent_export(agent_id, session_id, circuit_code)
}

pub fn export_agent(
    agent_id: Uuid,
    session_id: Uuid,
    circuit_code: u32,
) -> AgentExport {
    build_agent_export(agent_id, session_id, circuit_code)
}

pub fn export_agent_connection(
    agent_id: Uuid,
    session_id: Uuid,
    circuit_code: u32,
) -> AgentExport {
    build_agent_export(agent_id, session_id, circuit_code)
}

pub fn serialize_agent_export(export: &AgentExport) -> Result<Vec<u8>, welkin_llsd::LlsdError> {
    export.to_binary()
}

pub fn agent_export_to_llsd(export: &AgentExport) -> Llsd {
    export.to_llsd()
}

pub fn agent_export_to_binary(export: &AgentExport) -> Result<Vec<u8>, welkin_llsd::LlsdError> {
    export.to_binary()
}

// Preserve welkin-msg usage reference: agents identify via UUID, matching
// welkin_msg packet sender ids. No runtime coupling needed here.

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn roundtrip_binary() {
        let mut e = AgentExport::new(Uuid::new_v4(), Uuid::new_v4());
        e.circuit_code = 42;
        e.position = [1.0, 2.0, 3.0];
        e.seed_caps.insert("SeedCapability".into(), "http://x".into());
        let b = e.to_binary().unwrap();
        let r = AgentExport::from_binary(&b).unwrap();
        assert_eq!(r.agent_id, e.agent_id);
        assert_eq!(r.circuit_code, 42);
        assert_eq!(r.seed_caps.get("SeedCapability").unwrap(), "http://x");
    }

    #[test]
    fn xml_contains_agentdata() {
        let e = build_cross_region_agent_export(Uuid::new_v4(), Uuid::new_v4(), 7);
        let x = e.to_xml();
        assert!(x.contains("AgentData"));
    }
}
