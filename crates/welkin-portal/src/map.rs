// SPDX-License-Identifier: LGPL-3.0-or-later
use axum::Router;
use axum::extract::{Json, Path, State};
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use serde::{Deserialize, Serialize};

use crate::sessions::RequireStaff;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapRegion {
    pub region_id: String,
    pub name: String,
    pub loc_x: i32,
    pub loc_y: i32,
    pub size_x: i32,
    pub size_y: i32,
    pub status: String,
    pub owner: String,
    pub estate_name: String,
    pub map_image_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRequest {
    pub name: String,
    pub loc_x: i32,
    pub loc_y: i32,
    #[serde(default = "default_size")]
    pub size_x: i32,
    #[serde(default = "default_size")]
    pub size_y: i32,
}

fn default_size() -> i32 {
    256
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_coords: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandSettings {
    pub region_id: String,
    pub estate_id: i32,
    pub physics_engine: String,
    pub max_prims: i32,
    pub max_agents: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terrain_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRequest {
    pub loc_x: i32,
    pub loc_y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalGridOverlay {
    pub grid_uri: String,
    pub occupied_coords: Vec<(i32, i32)>,
}

pub fn overlaps(
    ax: i32,
    ay: i32,
    asx: i32,
    asy: i32,
    bx: i32,
    by: i32,
    bsx: i32,
    bsy: i32,
) -> bool {
    ax < bx + bsx && ax + asx > bx && ay < by + bsy && ay + asy > by
}

pub fn find_free_coords(
    occupied: &[(i32, i32, i32, i32)],
    x: i32,
    y: i32,
    sx: i32,
    sy: i32,
) -> (i32, i32) {
    let step: i32 = 256;
    for radius in 1i32..100 {
        for dx in (-radius)..=radius {
            for dy in (-radius)..=radius {
                if i32::abs(dx) != radius && i32::abs(dy) != radius {
                    continue;
                }
                let cx = x + dx * step;
                let cy = y + dy * step;
                if cx < 0 || cy < 0 {
                    continue;
                }
                let conflict = occupied
                    .iter()
                    .any(|&(ox, oy, osx, osy)| overlaps(cx, cy, sx, sy, ox, oy, osx, osy));
                if !conflict {
                    return (cx, cy);
                }
            }
        }
    }
    (x + 100 * step, y)
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static + AsRef<crate::sessions::SessionStore>,
{
    Router::new()
        .route("/portal/api/map/regions", get(handle_list_regions::<S>))
        .route("/portal/api/map/place", post(handle_place::<S>))
        .route("/portal/api/map/regions/{id}/move", put(handle_move::<S>))
        .route("/portal/api/map/regions/{id}", delete(handle_remove::<S>))
        .route(
            "/portal/api/map/regions/{id}/settings",
            get(handle_get_settings::<S>).put(handle_set_settings::<S>),
        )
        .route("/portal/api/map/overlay", get(handle_overlay::<S>))
}

async fn handle_list_regions<S>(_staff: RequireStaff, State(_state): State<S>) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("listing map regions is not implemented yet");
}

async fn handle_place<S>(
    _staff: RequireStaff,
    State(_state): State<S>,
    Json(_req): Json<PlacementRequest>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("placing map regions is not implemented yet");
}

async fn handle_move<S>(
    _staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
    Json(_req): Json<MoveRequest>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("moving map regions is not implemented yet");
}

async fn handle_remove<S>(
    _staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("removing map regions is not implemented yet");
}

async fn handle_get_settings<S>(
    _staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("reading island settings is not implemented yet");
}

async fn handle_set_settings<S>(
    _staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
    Json(_settings): Json<IslandSettings>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("updating island settings is not implemented yet");
}

async fn handle_overlay<S>(_staff: RequireStaff, State(_state): State<S>) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    return crate::error::not_implemented("fetching external grid overlay is not implemented yet");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overlap() {
        assert!(!overlaps(0, 0, 256, 256, 256, 0, 256, 256));
    }

    #[test]
    fn overlap_exact() {
        assert!(overlaps(0, 0, 256, 256, 0, 0, 256, 256));
    }

    #[test]
    fn overlap_partial() {
        assert!(overlaps(0, 0, 256, 256, 128, 128, 256, 256));
    }

    #[test]
    fn find_free_avoids_occupied() {
        let occupied = vec![(1000, 1000, 256, 256)];
        let (x, y) = find_free_coords(&occupied, 1000, 1000, 256, 256);
        assert!(!overlaps(x, y, 256, 256, 1000, 1000, 256, 256));
    }

    #[test]
    fn find_free_among_many() {
        let occupied = vec![
            (1000, 1000, 256, 256),
            (1256, 1000, 256, 256),
            (1000, 1256, 256, 256),
        ];
        let (x, y) = find_free_coords(&occupied, 1000, 1000, 256, 256);
        for &(ox, oy, osx, osy) in &occupied {
            assert!(!overlaps(x, y, 256, 256, ox, oy, osx, osy));
        }
    }

    #[test]
    fn variable_region_sizes() {
        assert!(overlaps(0, 0, 512, 512, 256, 256, 256, 256));
        assert!(!overlaps(0, 0, 256, 256, 512, 0, 256, 256));
    }
}
