// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Quat, Vec3};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceType {
    User,
    Npc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    None,
    Creating,
    Running,
    Removing,
    Removed,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AgentControls: u32 {
        const AT_POS         = 1 << 0;
        const AT_NEG         = 1 << 1;
        const LEFT_POS       = 1 << 2;
        const LEFT_NEG       = 1 << 3;
        const UP_POS         = 1 << 4;
        const UP_NEG         = 1 << 5;
        const PITCH_POS      = 1 << 6;
        const PITCH_NEG      = 1 << 7;
        const YAW_POS        = 1 << 8;
        const YAW_NEG        = 1 << 9;
        const FAST_AT        = 1 << 10;
        const FAST_LEFT      = 1 << 11;
        const FAST_UP        = 1 << 12;
        const FLY            = 1 << 13;
        const STOP           = 1 << 14;
        const FINISH_ANIM    = 1 << 15;
        const STAND_UP       = 1 << 16;
        const SIT_ON_GROUND  = 1 << 17;
        const MOUSELOOK      = 1 << 18;
        const NUDGE_AT_POS   = 1 << 19;
        const NUDGE_AT_NEG   = 1 << 20;
        const NUDGE_LEFT_POS = 1 << 21;
        const NUDGE_LEFT_NEG = 1 << 22;
        const NUDGE_UP_POS   = 1 << 23;
        const NUDGE_UP_NEG   = 1 << 24;
        const TURN_LEFT      = 1 << 25;
        const TURN_RIGHT     = 1 << 26;
        const AWAY           = 1 << 27;
        const LBUTTON_DOWN   = 1 << 28;
        const LBUTTON_UP     = 1 << 29;
        const ML_LBUTTON_DOWN = 1 << 30;
        const ML_LBUTTON_UP  = 1u32 << 31;
    }
}

#[derive(Debug, Clone)]
pub struct ScenePresence {
    pub uuid: Uuid,
    pub local_id: u32,
    pub first_name: String,
    pub last_name: String,
    pub presence_type: PresenceType,
    pub is_god: bool,

    pub position: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub controls: AgentControls,
    pub body_rotation: Quat,
    pub head_rotation: Quat,
    pub camera_position: Vec3,
    pub camera_at_axis: Vec3,
    pub camera_left_axis: Vec3,
    pub camera_up_axis: Vec3,

    pub lifecycle: LifecycleState,
    pub is_flying: bool,
    pub is_sitting: bool,
    pub sitting_on: Uuid,
    pub is_frozen: bool,
    pub frozen: bool,
    pub draw_distance: f32,
    pub region_handle: u64,

    pub appearance_serial: i32,
    pub visual_params: Vec<u8>,
    pub texture_entry: Vec<u8>,

    pub current_animation: Uuid,
    pub animation_sequence: u32,

    pub attachments: Vec<(u8, Uuid)>,
    /// Wearables persisted for this avatar: (wearable_type, texture-or-folder uuid).
    pub saved_wearables: Vec<(u8, Uuid)>,
    pub speed_multiplier: f32,
}

impl ScenePresence {
    pub const MOVEMENT_THRESHOLD: f32 = 0.25;
    pub const SIGNIFICANT_MOVEMENT: f32 = 16.0;
    pub const CHILD_UPDATES_MOVEMENT: f32 = 100.0;

    pub fn new(uuid: Uuid, first_name: &str, last_name: &str, presence_type: PresenceType) -> Self {
        Self {
            uuid,
            local_id: 0,
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            presence_type,
            is_god: false,

            position: Vec3::new(128.0, 128.0, 25.0),
            rotation: Quat::IDENTITY,
            velocity: Vec3::ZERO,
            controls: AgentControls::empty(),
            body_rotation: Quat::IDENTITY,
            head_rotation: Quat::IDENTITY,
            camera_position: Vec3::ZERO,
            camera_at_axis: Vec3::NEG_Z,
            camera_left_axis: Vec3::NEG_X,
            camera_up_axis: Vec3::Y,

            lifecycle: LifecycleState::None,
            is_flying: false,
            is_sitting: false,
            sitting_on: Uuid::nil(),
            is_frozen: false,
            frozen: false,
            draw_distance: 128.0,
            region_handle: 0,

            appearance_serial: 0,
            visual_params: Vec::new(),
            texture_entry: Vec::new(),

            current_animation: Uuid::nil(),
            animation_sequence: 0,

            attachments: Vec::new(),
            saved_wearables: Vec::new(),
            speed_multiplier: 1.0,
        }
    }

    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    pub fn is_npc(&self) -> bool {
        self.presence_type == PresenceType::Npc
    }

    pub fn set_position(&mut self, pos: Vec3) {
        self.position = pos;
    }

    pub fn set_rotation(&mut self, rot: Quat) {
        self.rotation = rot;
    }

    pub fn stand_up(&mut self) {
        self.is_sitting = false;
        self.sitting_on = Uuid::nil();
    }

    pub fn sit_on(&mut self, object_uuid: Uuid) {
        self.is_sitting = true;
        self.sitting_on = object_uuid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_presence_defaults() {
        let p = ScenePresence::new(Uuid::new_v4(), "Test", "User", PresenceType::User);
        assert_eq!(p.full_name(), "Test User");
        assert!(!p.is_npc());
        assert!(!p.is_flying);
        assert!(!p.is_sitting);
        assert_eq!(p.sitting_on, Uuid::nil());
        assert_eq!(p.lifecycle, LifecycleState::None);
        assert_eq!(p.position, Vec3::new(128.0, 128.0, 25.0));
    }

    #[test]
    fn sit_and_stand() {
        let mut p = ScenePresence::new(Uuid::new_v4(), "Sit", "Test", PresenceType::User);
        let obj = Uuid::new_v4();
        p.sit_on(obj);
        assert!(p.is_sitting);
        assert_eq!(p.sitting_on, obj);

        p.stand_up();
        assert!(!p.is_sitting);
        assert_eq!(p.sitting_on, Uuid::nil());
    }

    #[test]
    fn npc_detection() {
        let npc = ScenePresence::new(Uuid::new_v4(), "NPC", "Bot", PresenceType::Npc);
        assert!(npc.is_npc());
    }

    #[test]
    fn controls_bitflags() {
        let mut c = AgentControls::empty();
        c |= AgentControls::AT_POS | AgentControls::FLY;
        assert!(c.contains(AgentControls::AT_POS));
        assert!(c.contains(AgentControls::FLY));
        assert!(!c.contains(AgentControls::LEFT_POS));
    }

    #[test]
    fn movement_constants() {
        assert!(ScenePresence::MOVEMENT_THRESHOLD < ScenePresence::SIGNIFICANT_MOVEMENT);
        assert!(ScenePresence::SIGNIFICANT_MOVEMENT < ScenePresence::CHILD_UPDATES_MOVEMENT);
    }
}
