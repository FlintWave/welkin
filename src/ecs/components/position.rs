use serde::{Deserialize, Serialize};

/// Simple 3‑D position component used for range checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}