// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};

use crate::CapacityError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Source {
    pub description: String,
    pub reference: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Coefficient {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub source: Source,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoefficientSet {
    pub version: String,
    pub coefficients: Vec<Coefficient>,
}

impl CoefficientSet {
    pub fn get(&self, name: &str) -> Option<f64> {
        self.coefficients.iter().find(|c| c.name == name).map(|c| c.value)
    }
}

fn coeff(name: &str, value: f64, unit: &str, description: &str, reference: &str) -> Coefficient {
    Coefficient {
        name: name.into(),
        value,
        unit: unit.into(),
        source: Source {
            description: description.into(),
            reference: reference.into(),
        },
    }
}

pub fn default_coefficients() -> CoefficientSet {
    CoefficientSet {
        version: "0.1.0".into(),
        coefficients: vec![
            coeff(
                "memory_per_region_mb", 1024.0, "MB",
                "Memory per region", "OSgrid wiki tutorial",
            ),
            coeff(
                "cpu_cores_per_region", 1.0, "cores",
                "CPU cores per regularly used region", "OpenSimulator wiki",
            ),
            coeff(
                "cpu_idle_region_fraction", 0.025, "cores",
                "CPU fraction for an idle region (v0.7.6)", "OpenSimulator wiki",
            ),
            coeff(
                "memory_base_mb", 200.0, "MB",
                "Base memory for the process", "Hypergrid Business hosting guide",
            ),
            coeff(
                "memory_per_1k_prims_mb", 30.0, "MB",
                "Memory per 1000 prims", "Hypergrid Business hosting guide",
            ),
            coeff(
                "memory_per_user_mb", 50.0, "MB",
                "Memory per connected user", "Hypergrid Business hosting guide",
            ),
            coeff(
                "max_regions_per_simulator", 4.0, "regions",
                "Maximum regions per simulator instance", "OSgrid forum post",
            ),
            coeff(
                "max_regions_per_server", 16.0, "regions",
                "Maximum regions per physical server", "OSgrid forum post",
            ),
            coeff(
                "max_agents_per_region", 100.0, "agents",
                "Maximum agents per region", "Regions.ini.example",
            ),
            coeff(
                "max_prims_per_region", 15000.0, "prims",
                "Maximum prims per region", "Regions.ini.example",
            ),
            coeff(
                "client_throttle_max_bps", 187500.0, "bytes/s",
                "Maximum bandwidth per client", "OpenSimDefaults.ini",
            ),
            coeff(
                "scene_throttle_max_bps", 2500000.0, "bytes/s",
                "Maximum bandwidth per scene", "OpenSimDefaults.ini",
            ),
        ],
    }
}

pub fn load_coefficients(path: &str) -> Result<CoefficientSet, CapacityError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|e| CapacityError::Parse(e.to_string()))
}

pub fn save_coefficients(set: &CoefficientSet, path: &str) -> Result<(), CapacityError> {
    let json = serde_json::to_string_pretty(set)
        .map_err(|e| CapacityError::Parse(e.to_string()))?;
    std::fs::write(path, json)?;
    Ok(())
}
