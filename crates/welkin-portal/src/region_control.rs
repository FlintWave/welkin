// SPDX-License-Identifier: LGPL-3.0-or-later
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::PortalState;
use crate::error::PortalError;
use crate::sessions::RequireStaff;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionCommand {
    Create,
    Start,
    Stop,
    Snapshot,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionStatus {
    pub region_id: String,
    pub name: String,
    pub status: String,
    pub agents: u32,
    pub uptime_secs: u64,
}

#[derive(Deserialize)]
pub struct CreateRegionRequest {
    pub name: String,
    pub loc_x: i32,
    pub loc_y: i32,
    pub size_x: i32,
    pub size_y: i32,
    pub estate_id: Option<i32>,
}

fn require_operator(staff: &RequireStaff) -> Result<(), PortalError> {
    if !staff.0.role.can_manage_regions() {
        return Err(PortalError::Forbidden);
    }
    Ok(())
}

// TODO: actual region process management depends on region-simulator-core
pub async fn handle_list_regions(
    State(state): State<PortalState>,
    staff: RequireStaff,
) -> Result<impl IntoResponse, PortalError> {
    require_operator(&staff)?;
    let regions = state.region_statuses.lock().unwrap().clone();
    Ok(Json(regions))
}

pub async fn handle_create_region(
    State(state): State<PortalState>,
    staff: RequireStaff,
    Json(req): Json<CreateRegionRequest>,
) -> Result<impl IntoResponse, PortalError> {
    require_operator(&staff)?;

    let region = RegionStatus {
        region_id: uuid::Uuid::new_v4().to_string(),
        name: req.name.clone(),
        status: "stopped".into(),
        agents: 0,
        uptime_secs: 0,
    };

    state.audit_log.log_action(
        &staff.0.staff_id,
        "create_region",
        &region.region_id,
        &format!("name={} loc=({},{})", req.name, req.loc_x, req.loc_y),
        "",
    );

    state.region_statuses.lock().unwrap().push(region.clone());
    Ok((StatusCode::CREATED, Json(region)))
}

// TODO: start/stop/snapshot need region-simulator-core process management
pub async fn handle_start_region(
    State(_state): State<PortalState>,
    staff: RequireStaff,
    Path(_id): Path<String>,
) -> Result<impl IntoResponse, PortalError> {
    require_operator(&staff)?;
    return Ok(crate::error::not_implemented("start region is not implemented yet"));
}

pub async fn handle_stop_region(
    State(_state): State<PortalState>,
    staff: RequireStaff,
    Path(_id): Path<String>,
) -> Result<impl IntoResponse, PortalError> {
    require_operator(&staff)?;
    return Ok(crate::error::not_implemented("stop region is not implemented yet"));
}

pub async fn handle_snapshot_region(
    State(_state): State<PortalState>,
    staff: RequireStaff,
    Path(_id): Path<String>,
) -> Result<impl IntoResponse, PortalError> {
    require_operator(&staff)?;
    return Ok(crate::error::not_implemented("snapshot region is not implemented yet"));
}

pub async fn handle_remove_region(
    State(state): State<PortalState>,
    staff: RequireStaff,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, PortalError> {
    require_operator(&staff)?;
    let mut regions = state.region_statuses.lock().unwrap();
    let idx = regions
        .iter()
        .position(|r| r.region_id == id)
        .ok_or(PortalError::NotFound)?;
    regions.remove(idx);
    state
        .audit_log
        .log_action(&staff.0.staff_id, "remove_region", &id, "", "");
    Ok(StatusCode::NO_CONTENT)
}
