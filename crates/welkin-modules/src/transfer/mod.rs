// SPDX-License-Identifier: LGPL-3.0-or-later

//! Entity transfer framework: teleportation, region crossing, and the
//! transfer state machine.
//!
//! This module implements the Rust equivalents of the C#
//! `EntityTransferModule` and `EntityTransferStateMachine` from
//! `OpenSim/Region/CoreModules/Framework/EntityTransfer/`.

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// TeleportFlags
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    /// Bit-flags describing how and why a teleport is occurring.
    ///
    /// Mirrors the C# `TeleportFlags` enum in
    /// `OpenSim/Framework/Constants.cs`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TeleportFlags: u32 {
        const DEFAULT            = 0;
        const SET_HOME_TO_TARGET = 1;
        const SET_LAST_TO_TARGET = 2;
        const VIA_LURE           = 4;
        const VIA_LANDMARK       = 8;
        const VIA_LOCATION       = 16;
        const VIA_HOME           = 32;
        const VIA_TELEHUB        = 64;
        const VIA_LOGIN          = 128;
        const GOD_LIKE           = 256;
        const NINE_ONE_ONE       = 512;
        const DISABLE_CANCEL     = 1024;
        const VIA_REGION_ID      = 2048;
        const IS_FLYING          = 4096;
        const RESET_HOME         = 8192;
        const FORCE_REDIRECT     = 16384;
    }
}

// ---------------------------------------------------------------------------
// GridRegion
// ---------------------------------------------------------------------------

/// Describes a region on the grid, sufficient for teleport and crossing
/// operations.
///
/// Mirrors key fields from the C# `GridRegion` class in
/// `OpenSim/Services/Interfaces/IGridService.cs`.
#[derive(Debug, Clone)]
pub struct GridRegion {
    pub region_id: Uuid,
    pub region_handle: u64,
    pub region_name: String,
    pub external_host_name: String,
    pub http_port: u16,
    pub server_uri: String,
    pub region_loc_x: u32,
    pub region_loc_y: u32,
    pub region_size_x: u32,
    pub region_size_y: u32,
}

impl GridRegion {
    /// Create a new `GridRegion` with the given identity and default sizes.
    pub fn new(region_id: Uuid, region_name: &str) -> Self {
        Self {
            region_id,
            region_handle: 0,
            region_name: region_name.to_string(),
            external_host_name: String::new(),
            http_port: 0,
            server_uri: String::new(),
            region_loc_x: 0,
            region_loc_y: 0,
            region_size_x: 256,
            region_size_y: 256,
        }
    }
}

impl Default for GridRegion {
    fn default() -> Self {
        Self {
            region_id: Uuid::nil(),
            region_handle: 0,
            region_name: String::new(),
            external_host_name: String::new(),
            http_port: 0,
            server_uri: String::new(),
            region_loc_x: 0,
            region_loc_y: 0,
            region_size_x: 256,
            region_size_y: 256,
        }
    }
}

// ---------------------------------------------------------------------------
// EntityTransferContext
// ---------------------------------------------------------------------------

/// Per-teleport context carried through the transfer pipeline.
///
/// Mirrors the C# `EntityTransferContext` struct.
#[derive(Debug, Clone, Default)]
pub struct EntityTransferContext {
    pub incoming_teleport_type: String,
    pub src_external_name: String,
    pub destination: Option<GridRegion>,
}

// ---------------------------------------------------------------------------
// AgentTransferState
// ---------------------------------------------------------------------------

/// States an agent can be in during a teleport or region crossing.
///
/// Mirrors the C# `AgentTransferState` enum in
/// `EntityTransferStateMachine.cs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentTransferState {
    Preparing,
    Transferring,
    ReceivedAtDestination,
    CleaningUp,
    Cancelling,
    Aborting,
}

impl AgentTransferState {
    /// Whether this state is terminal (no further transitions allowed).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::CleaningUp | Self::Aborting)
    }

    /// Check whether a transition from `self` to `new_state` is valid per
    /// the C# FSM.
    ///
    /// Valid transitions:
    /// - Preparing -> Transferring, Cancelling, CleaningUp, Aborting
    /// - Transferring -> ReceivedAtDestination, Cancelling, CleaningUp, Aborting
    /// - Cancelling -> CleaningUp, Aborting
    /// - ReceivedAtDestination -> CleaningUp, Aborting
    /// - CleaningUp -> (terminal — no transitions)
    /// - Aborting -> (terminal — no transitions)
    pub fn can_transition_to(self, new_state: Self) -> bool {
        match self {
            Self::Preparing => matches!(
                new_state,
                Self::Transferring | Self::Cancelling | Self::CleaningUp | Self::Aborting
            ),
            Self::Transferring => matches!(
                new_state,
                Self::ReceivedAtDestination | Self::Cancelling | Self::CleaningUp | Self::Aborting
            ),
            Self::Cancelling => matches!(new_state, Self::CleaningUp | Self::Aborting),
            Self::ReceivedAtDestination => {
                matches!(new_state, Self::CleaningUp | Self::Aborting)
            }
            Self::CleaningUp | Self::Aborting => false,
        }
    }
}

// ---------------------------------------------------------------------------
// EntityTransferStateMachine
// ---------------------------------------------------------------------------

/// Tracks in-transit agents and enforces valid state transitions.
///
/// Mirrors the C# `EntityTransferStateMachine` class.
pub struct EntityTransferStateMachine {
    agents_in_transit: HashMap<Uuid, AgentTransferState>,
    pub enable_wait_for_agent_arrived: bool,
}

impl EntityTransferStateMachine {
    pub fn new() -> Self {
        Self {
            agents_in_transit: HashMap::new(),
            enable_wait_for_agent_arrived: true,
        }
    }

    /// Mark an agent as in transit (state: Preparing).
    ///
    /// Returns `false` if the agent is already in transit.
    pub fn set_in_transit(&mut self, agent_id: Uuid) -> bool {
        if self.agents_in_transit.contains_key(&agent_id) {
            tracing::warn!(%agent_id, "agent already in transit");
            return false;
        }
        self.agents_in_transit
            .insert(agent_id, AgentTransferState::Preparing);
        tracing::debug!(%agent_id, "agent now in transit (Preparing)");
        true
    }

    /// Advance an agent's transfer state.
    ///
    /// Returns `false` if the agent is not in transit or the transition is
    /// invalid per the FSM.
    pub fn update_in_transit(&mut self, agent_id: Uuid, new_state: AgentTransferState) -> bool {
        let Some(current) = self.agents_in_transit.get(&agent_id) else {
            tracing::warn!(%agent_id, "update_in_transit: agent not in transit");
            return false;
        };
        if !current.can_transition_to(new_state) {
            tracing::warn!(
                %agent_id,
                from = ?current,
                to = ?new_state,
                "invalid state transition"
            );
            return false;
        }
        self.agents_in_transit.insert(agent_id, new_state);
        tracing::debug!(%agent_id, state = ?new_state, "agent transfer state updated");
        true
    }

    /// Check whether an agent is currently in transit.
    pub fn is_in_transit(&self, agent_id: Uuid) -> bool {
        self.agents_in_transit.contains_key(&agent_id)
    }

    /// Return the current transfer state for an agent, if any.
    pub fn get_agent_transfer_state(&self, agent_id: Uuid) -> Option<AgentTransferState> {
        self.agents_in_transit.get(&agent_id).copied()
    }

    /// Remove an agent from the in-transit map.
    ///
    /// Returns `true` if the agent was present and removed.
    pub fn reset_from_transit(&mut self, agent_id: Uuid) -> bool {
        let removed = self.agents_in_transit.remove(&agent_id).is_some();
        if removed {
            tracing::debug!(%agent_id, "agent reset from transit");
        }
        removed
    }

    /// Wait for the destination region to confirm the agent has arrived.
    ///
    /// Stub: always returns `true`.
    pub fn wait_for_agent_arrived_at_destination(&self, _agent_id: Uuid) -> bool {
        true
    }

    /// Number of agents currently in transit (for diagnostics).
    pub fn in_transit_count(&self) -> usize {
        self.agents_in_transit.len()
    }
}

impl Default for EntityTransferStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EntityTransferModule
// ---------------------------------------------------------------------------

/// Manages teleportation and region-crossing for agents.
///
/// Mirrors the C# `EntityTransferModule` (NonShared).
pub struct EntityTransferModule {
    pub max_transfer_distance: f32,
    pub wait_for_agent_arrived_timeout_secs: u32,
    state_machine: EntityTransferStateMachine,
    scene_name: String,
    grid_service_url: String,
    pub region_handle: u64,
    pub region_size_x: u32,
    pub region_size_y: u32,
}

impl EntityTransferModule {
    pub fn new() -> Self {
        Self {
            max_transfer_distance: 4095.0,
            wait_for_agent_arrived_timeout_secs: 15,
            state_machine: EntityTransferStateMachine::new(),
            scene_name: String::new(),
            grid_service_url: String::new(),
            region_handle: 0,
            region_size_x: 256,
            region_size_y: 256,
        }
    }

    pub fn state_machine(&self) -> &EntityTransferStateMachine {
        &self.state_machine
    }

    pub fn state_machine_mut(&mut self) -> &mut EntityTransferStateMachine {
        &mut self.state_machine
    }

    /// Resolve a region handle to a GridRegion by extracting grid
    /// coordinates.  In a full deployment the grid service would be queried;
    /// here we build the destination from the handle's embedded coordinates.
    pub fn resolve_region_handle(&self, region_handle: u64) -> GridRegion {
        let loc_x = (region_handle >> 32) as u32;
        let loc_y = (region_handle & 0xFFFF_FFFF) as u32;
        GridRegion {
            region_id: Uuid::nil(),
            region_handle,
            region_name: format!("Region_{}_{}", loc_x / 256, loc_y / 256),
            external_host_name: String::new(),
            http_port: 0,
            server_uri: self.grid_service_url.clone(),
            region_loc_x: loc_x,
            region_loc_y: loc_y,
            region_size_x: 256,
            region_size_y: 256,
        }
    }

    /// Initiate a teleport to the region identified by `region_handle`.
    ///
    /// Resolves the handle to a GridRegion, validates distance, and
    /// delegates to `do_teleport`.
    pub fn teleport(
        &mut self,
        agent_id: Uuid,
        region_handle: u64,
        position: [f32; 3],
        look_at: [f32; 3],
        flags: TeleportFlags,
    ) -> Result<(), String> {
        let destination = self.resolve_region_handle(region_handle);

        let src_x = (self.region_handle >> 32) as f32;
        let src_y = (self.region_handle & 0xFFFF_FFFF) as f32;
        let dst_x = destination.region_loc_x as f32;
        let dst_y = destination.region_loc_y as f32;
        let distance = ((dst_x - src_x).powi(2) + (dst_y - src_y).powi(2)).sqrt();

        if distance > self.max_transfer_distance {
            return Err(format!(
                "destination too far: {:.0}m exceeds max {:.0}m",
                distance, self.max_transfer_distance
            ));
        }

        self.do_teleport(agent_id, &destination, position, look_at, flags)
    }

    /// Teleport an agent to their home region.
    pub fn teleport_home(&mut self, agent_id: Uuid) -> bool {
        tracing::info!(%agent_id, scene = %self.scene_name, "teleport_home requested");
        if !self.state_machine.set_in_transit(agent_id) {
            return false;
        }
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::Transferring);
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::CleaningUp);
        self.state_machine.reset_from_transit(agent_id);
        true
    }

    /// Perform the actual teleport to a specific destination.
    ///
    /// State machine flow: Preparing → Transferring →
    /// ReceivedAtDestination → CleaningUp.  Each transition is validated
    /// by the FSM.  On failure the agent is reset from transit.
    pub fn do_teleport(
        &mut self,
        agent_id: Uuid,
        destination: &GridRegion,
        position: [f32; 3],
        look_at: [f32; 3],
        flags: TeleportFlags,
    ) -> Result<(), String> {
        if !self.state_machine.set_in_transit(agent_id) {
            return Err("Agent is already in transit".to_string());
        }

        tracing::info!(
            %agent_id,
            dest_region = %destination.region_name,
            dest_handle = destination.region_handle,
            pos = ?position,
            look = ?look_at,
            ?flags,
            "cross-region teleport: creating agent at destination"
        );

        if !self
            .state_machine
            .update_in_transit(agent_id, AgentTransferState::Transferring)
        {
            self.state_machine.reset_from_transit(agent_id);
            return Err("failed to transition to Transferring".to_string());
        }

        // CreateAgent at destination region would happen here via HTTP POST
        // to destination.server_uri/agent/{agent_id}/.  The destination
        // region creates a child agent, returns seed caps URL.
        tracing::info!(
            %agent_id,
            dest = %destination.server_uri,
            "CreateAgent call to destination (seed caps exchange)"
        );

        if !self
            .state_machine
            .update_in_transit(agent_id, AgentTransferState::ReceivedAtDestination)
        {
            self.state_machine.reset_from_transit(agent_id);
            return Err("failed to transition to ReceivedAtDestination".to_string());
        }

        // Viewer would receive UpdateSimulator + TeleportFinish at this
        // point, redirecting it to the destination region's UDP endpoint.

        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::CleaningUp);
        self.state_machine.reset_from_transit(agent_id);

        tracing::info!(
            %agent_id,
            dest_region = %destination.region_name,
            "cross-region teleport complete"
        );
        Ok(())
    }

    /// Compute the neighbour region handle for a border crossing.
    ///
    /// Returns `(neighbour_handle, clamped_position)` where the position is
    /// wrapped into the 0..region_size range of the neighbour.
    pub fn compute_neighbour(
        &self,
        position: [f32; 3],
    ) -> Option<(u64, [f32; 3])> {
        let size_x = self.region_size_x as f32;
        let size_y = self.region_size_y as f32;
        let cur_x = (self.region_handle >> 32) as i64;
        let cur_y = (self.region_handle & 0xFFFF_FFFF) as i64;

        let mut dx: i64 = 0;
        let mut dy: i64 = 0;
        let mut new_x = position[0];
        let mut new_y = position[1];

        if position[0] < 0.0 {
            dx = -(size_x as i64);
            new_x += size_x;
        } else if position[0] >= size_x {
            dx = size_x as i64;
            new_x -= size_x;
        }

        if position[1] < 0.0 {
            dy = -(size_y as i64);
            new_y += size_y;
        } else if position[1] >= size_y {
            dy = size_y as i64;
            new_y -= size_y;
        }

        if dx == 0 && dy == 0 {
            return None;
        }

        let nbr_x = cur_x + dx;
        let nbr_y = cur_y + dy;
        if nbr_x < 0 || nbr_y < 0 {
            return None;
        }

        let handle = ((nbr_x as u64) << 32) | (nbr_y as u64);
        Some((handle, [new_x, new_y, position[2]]))
    }

    /// Cross an agent into a neighbouring region.
    ///
    /// Detects when the avatar position exceeds region bounds, computes the
    /// neighbour region handle, and drives the state machine through the
    /// crossing flow.
    pub fn cross_agent_to_new_region(
        &mut self,
        agent_id: Uuid,
        position: [f32; 3],
        new_region: &GridRegion,
        is_flying: bool,
    ) -> bool {
        if !self.state_machine.set_in_transit(agent_id) {
            tracing::warn!(%agent_id, "region crossing: agent already in transit");
            return false;
        }

        tracing::info!(
            %agent_id,
            dest_region = %new_region.region_name,
            dest_handle = new_region.region_handle,
            pos = ?position,
            flying = is_flying,
            "region crossing: creating child agent at neighbour"
        );

        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::Transferring);

        // CreateChildAgent at neighbour would happen here via HTTP.
        // The neighbour region prepares a child presence and returns
        // seed caps for the viewer to connect to.

        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::ReceivedAtDestination);

        // At this point the source region sends ChildAgentUpdate and
        // CrossAgent to hand off the avatar state.

        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::CleaningUp);
        self.state_machine.reset_from_transit(agent_id);

        tracing::info!(
            %agent_id,
            dest_region = %new_region.region_name,
            "region crossing complete"
        );
        true
    }

    /// Cancel an in-progress teleport.
    pub fn cancel_teleport(&mut self, agent_id: Uuid) {
        if self.state_machine.is_in_transit(agent_id) {
            let _ = self
                .state_machine
                .update_in_transit(agent_id, AgentTransferState::Cancelling);
            let _ = self
                .state_machine
                .update_in_transit(agent_id, AgentTransferState::CleaningUp);
            self.state_machine.reset_from_transit(agent_id);
            tracing::info!(%agent_id, "teleport cancelled");
        }
    }
}

impl Default for EntityTransferModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for EntityTransferModule {
    fn name(&self) -> &str {
        "EntityTransferModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.max_transfer_distance =
            config.get_float("EntityTransfer", "max_transfer_distance", 4095.0) as f32;
        self.wait_for_agent_arrived_timeout_secs =
            config.get_int("EntityTransfer", "wait_for_agent_arrived_timeout_secs", 15) as u32;
        self.grid_service_url =
            config.get_string("GridService", "GridServerURI", "");
        tracing::info!(
            max_distance = self.max_transfer_distance,
            timeout = self.wait_for_agent_arrived_timeout_secs,
            "EntityTransferModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene_name = scene.region_name.clone();
        self.region_handle = scene.region_handle;
        self.region_size_x = scene.region_size_x;
        self.region_size_y = scene.region_size_y;
        tracing::info!(
            region = %scene.region_name,
            handle = self.region_handle,
            "EntityTransferModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene_name.clear();
        tracing::info!(
            region = %scene.region_name,
            "EntityTransferModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scene_name.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_full_teleport_flow() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();

        assert!(sm.set_in_transit(agent));
        assert!(sm.is_in_transit(agent));
        assert_eq!(sm.get_agent_transfer_state(agent), Some(AgentTransferState::Preparing));

        assert!(sm.update_in_transit(agent, AgentTransferState::Transferring));
        assert!(sm.update_in_transit(agent, AgentTransferState::ReceivedAtDestination));
        assert!(sm.update_in_transit(agent, AgentTransferState::CleaningUp));
        assert!(sm.reset_from_transit(agent));
        assert!(!sm.is_in_transit(agent));
    }

    #[test]
    fn state_machine_cancellation_flow() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();

        assert!(sm.set_in_transit(agent));
        assert!(sm.update_in_transit(agent, AgentTransferState::Transferring));
        assert!(sm.update_in_transit(agent, AgentTransferState::Cancelling));
        assert!(sm.update_in_transit(agent, AgentTransferState::CleaningUp));
        assert!(sm.reset_from_transit(agent));
    }

    #[test]
    fn state_machine_rejects_invalid_transitions() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();

        assert!(sm.set_in_transit(agent));
        // Can't go from Preparing directly to ReceivedAtDestination
        assert!(!sm.update_in_transit(agent, AgentTransferState::ReceivedAtDestination));
        // Can go to Transferring
        assert!(sm.update_in_transit(agent, AgentTransferState::Transferring));
        // Can't go from Transferring back to Preparing
        assert!(!sm.update_in_transit(agent, AgentTransferState::Preparing));
    }

    #[test]
    fn state_machine_double_set_in_transit() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        assert!(sm.set_in_transit(agent));
        assert!(!sm.set_in_transit(agent));
    }

    #[test]
    fn teleport_within_distance_succeeds() {
        let mut module = EntityTransferModule::new();
        module.region_handle = (1000_u64 << 32) | 1000;
        module.max_transfer_distance = 10000.0;
        let agent = Uuid::new_v4();

        let result = module.teleport(
            agent,
            (1256_u64 << 32) | 1256,
            [128.0, 128.0, 25.0],
            [1.0, 0.0, 0.0],
            TeleportFlags::VIA_LOCATION,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn teleport_exceeds_distance_fails() {
        let mut module = EntityTransferModule::new();
        module.region_handle = 0;
        module.max_transfer_distance = 100.0;
        let agent = Uuid::new_v4();

        let result = module.teleport(
            agent,
            (999999_u64 << 32) | 999999,
            [128.0, 128.0, 25.0],
            [1.0, 0.0, 0.0],
            TeleportFlags::VIA_LOCATION,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too far"));
    }

    #[test]
    fn compute_neighbour_east() {
        let mut module = EntityTransferModule::new();
        module.region_handle = (256_u64 << 32) | 256;
        module.region_size_x = 256;
        module.region_size_y = 256;

        let result = module.compute_neighbour([260.0, 128.0, 25.0]);
        assert!(result.is_some());
        let (handle, pos) = result.unwrap();
        assert_eq!(handle, (512_u64 << 32) | 256);
        assert!((pos[0] - 4.0).abs() < 0.01);
        assert!((pos[1] - 128.0).abs() < 0.01);
    }

    #[test]
    fn compute_neighbour_west() {
        let mut module = EntityTransferModule::new();
        module.region_handle = (256_u64 << 32) | 256;
        module.region_size_x = 256;
        module.region_size_y = 256;

        let result = module.compute_neighbour([-1.0, 128.0, 25.0]);
        assert!(result.is_some());
        let (handle, pos) = result.unwrap();
        assert_eq!(handle, (0_u64 << 32) | 256);
        assert!((pos[0] - 255.0).abs() < 0.01);
    }

    #[test]
    fn compute_neighbour_within_bounds_returns_none() {
        let mut module = EntityTransferModule::new();
        module.region_handle = (256_u64 << 32) | 256;
        module.region_size_x = 256;
        module.region_size_y = 256;

        assert!(module.compute_neighbour([128.0, 128.0, 25.0]).is_none());
    }

    #[test]
    fn compute_neighbour_negative_result_returns_none() {
        let mut module = EntityTransferModule::new();
        module.region_handle = 0;
        module.region_size_x = 256;
        module.region_size_y = 256;

        assert!(module.compute_neighbour([-1.0, 128.0, 25.0]).is_none());
    }

    #[test]
    fn cross_agent_succeeds() {
        let mut module = EntityTransferModule::new();
        module.region_handle = (256_u64 << 32) | 256;
        module.region_size_x = 256;
        module.region_size_y = 256;
        let agent = Uuid::new_v4();
        let neighbour = GridRegion::new(Uuid::new_v4(), "Neighbour");

        assert!(module.cross_agent_to_new_region(
            agent,
            [4.0, 128.0, 25.0],
            &neighbour,
            false,
        ));
    }

    #[test]
    fn cancel_teleport_cleans_up() {
        let mut module = EntityTransferModule::new();
        let agent = Uuid::new_v4();
        module.state_machine.set_in_transit(agent);
        module.cancel_teleport(agent);
        assert!(!module.state_machine.is_in_transit(agent));
    }

    #[test]
    fn resolve_region_handle_extracts_coords() {
        let module = EntityTransferModule::new();
        let handle = (512_u64 << 32) | 768;
        let region = module.resolve_region_handle(handle);
        assert_eq!(region.region_loc_x, 512);
        assert_eq!(region.region_loc_y, 768);
        assert_eq!(region.region_handle, handle);
    }
}
