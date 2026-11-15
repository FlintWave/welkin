// SPDX-License-Identifier: GPL-3.0-or-later

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
#[derive(Debug, Clone)]
pub struct EntityTransferContext {
    pub incoming_teleport_type: String,
    pub src_external_name: String,
    pub destination: Option<GridRegion>,
}

impl Default for EntityTransferContext {
    fn default() -> Self {
        Self {
            incoming_teleport_type: String::new(),
            src_external_name: String::new(),
            destination: None,
        }
    }
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
                Self::ReceivedAtDestination
                    | Self::Cancelling
                    | Self::CleaningUp
                    | Self::Aborting
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
/// Mirrors the C# `EntityTransferModule` (NonShared).  Currently a stub:
/// teleport and crossing operations log their intent and interact with the
/// state machine, but do not perform actual network operations.
pub struct EntityTransferModule {
    pub max_transfer_distance: f32,
    pub wait_for_agent_arrived_timeout_secs: u32,
    state_machine: EntityTransferStateMachine,
    scene_name: String,
}

impl EntityTransferModule {
    pub fn new() -> Self {
        Self {
            max_transfer_distance: 4095.0,
            wait_for_agent_arrived_timeout_secs: 15,
            state_machine: EntityTransferStateMachine::new(),
            scene_name: String::new(),
        }
    }

    /// Access the underlying state machine.
    pub fn state_machine(&self) -> &EntityTransferStateMachine {
        &self.state_machine
    }

    /// Mutable access to the underlying state machine.
    pub fn state_machine_mut(&mut self) -> &mut EntityTransferStateMachine {
        &mut self.state_machine
    }

    /// Initiate a teleport to the region identified by `region_handle`.
    ///
    /// Stub: validates the agent is not already in transit, marks the agent
    /// as in transit, and returns `Ok(())`.
    pub fn teleport(
        &mut self,
        agent_id: Uuid,
        _region_handle: u64,
        _position: [f32; 3],
        _look_at: [f32; 3],
        _flags: TeleportFlags,
    ) -> Result<(), String> {
        if !self.state_machine.set_in_transit(agent_id) {
            return Err("Agent is already in transit".to_string());
        }
        tracing::info!(%agent_id, "teleport initiated (stub)");

        // In a real implementation this would resolve the region handle to a
        // GridRegion, call do_teleport, etc.  For now we advance through the
        // state machine and clean up.
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::Transferring);
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::CleaningUp);
        self.state_machine.reset_from_transit(agent_id);
        Ok(())
    }

    /// Teleport an agent to their home region.
    ///
    /// Stub: always returns `true`.
    pub fn teleport_home(&mut self, agent_id: Uuid) -> bool {
        tracing::info!(%agent_id, "teleport_home (stub)");
        true
    }

    /// Perform the actual teleport to a specific destination.
    ///
    /// Stub: marks the agent in transit, advances through states, and
    /// returns `Ok(())`.
    pub fn do_teleport(
        &mut self,
        agent_id: Uuid,
        _destination: &GridRegion,
        _position: [f32; 3],
        _look_at: [f32; 3],
        _flags: TeleportFlags,
    ) -> Result<(), String> {
        if !self.state_machine.set_in_transit(agent_id) {
            return Err("Agent is already in transit".to_string());
        }
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::Transferring);
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::ReceivedAtDestination);
        self.state_machine
            .update_in_transit(agent_id, AgentTransferState::CleaningUp);
        self.state_machine.reset_from_transit(agent_id);
        tracing::info!(%agent_id, "do_teleport completed (stub)");
        Ok(())
    }

    /// Cross an agent into a neighbouring region.
    ///
    /// Stub: returns `true`.
    pub fn cross_agent_to_new_region(
        &mut self,
        agent_id: Uuid,
        _position: [f32; 3],
        _new_region: &GridRegion,
        _is_flying: bool,
    ) -> bool {
        tracing::info!(%agent_id, "cross_agent_to_new_region (stub)");
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
        self.max_transfer_distance = config.get_float(
            "EntityTransfer",
            "max_transfer_distance",
            4095.0,
        ) as f32;
        self.wait_for_agent_arrived_timeout_secs = config.get_int(
            "EntityTransfer",
            "wait_for_agent_arrived_timeout_secs",
            15,
        ) as u32;
        tracing::info!(
            max_distance = self.max_transfer_distance,
            timeout = self.wait_for_agent_arrived_timeout_secs,
            "EntityTransferModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene_name = scene.region_name.clone();
        tracing::info!(
            region = %scene.region_name,
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
