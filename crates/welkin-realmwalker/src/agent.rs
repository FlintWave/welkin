// SPDX-License-Identifier: LGPL-3.0-or-later
//! Agent movement, teleport, sit/stand, fly, and animation control.
//!
//! Provides functions for controlling the bot avatar's position and state
//! via the LLUDP protocol. Each function builds the appropriate packet and
//! sends it through the [`Network`] layer.

use glam::{Quat, Vec3};
use uuid::Uuid;
use welkin_msg::msgs::*;
use welkin_msg::{IncomingPacket, Msg};

use crate::error::RealmWalkerError;
use crate::events::{
    Event, EventBus, TeleportFailedData, TeleportProgressData, TeleportStartData,
};
use crate::network::Network;

// ---------------------------------------------------------------------------
// Agent control flags
//
// Values from the Second Life message template and the OpenMetaverse
// AgentManager.ControlFlags enum (BSD 3-Clause). These are the bitmask
// values carried in AgentUpdate.AgentData.ControlFlags (u32).
// ---------------------------------------------------------------------------

/// Agent movement and input control flags.
///
/// Each constant corresponds to a single bit in the `control_flags` field
/// of an `AgentUpdate` packet. Combine with bitwise OR to express
/// simultaneous inputs (e.g. `AT_POS | FLY` for flying forward).
pub mod control_flags {
    /// Move forward.
    pub const AT_POS: u32 = 1 << 0;
    /// Move backward.
    pub const AT_NEG: u32 = 1 << 1;
    /// Strafe left.
    pub const LEFT_POS: u32 = 1 << 2;
    /// Strafe right.
    pub const LEFT_NEG: u32 = 1 << 3;
    /// Move up / jump.
    pub const UP_POS: u32 = 1 << 4;
    /// Move down / crouch.
    pub const UP_NEG: u32 = 1 << 5;
    /// Pitch up (look up).
    pub const PITCH_POS: u32 = 1 << 6;
    /// Pitch down (look down).
    pub const PITCH_NEG: u32 = 1 << 7;
    /// Yaw positive (turn left).
    pub const YAW_POS: u32 = 1 << 8;
    /// Yaw negative (turn right).
    pub const YAW_NEG: u32 = 1 << 9;
    /// Run modifier for forward/back.
    pub const FAST_AT: u32 = 1 << 10;
    /// Run modifier for left/right.
    pub const FAST_LEFT: u32 = 1 << 11;
    /// Run modifier for up/down.
    pub const FAST_UP: u32 = 1 << 12;
    /// Flying.
    pub const FLY: u32 = 1 << 13;
    /// Stop all movement.
    pub const STOP: u32 = 1 << 14;
    /// Finish current animation.
    pub const FINISH_ANIM: u32 = 1 << 15;
    /// Stand up from sitting.
    pub const STAND_UP: u32 = 1 << 16;
    /// Sit on ground.
    pub const SIT_ON_GROUND: u32 = 1 << 17;
    /// In mouselook mode.
    pub const MOUSELOOK: u32 = 1 << 18;
    /// Nudge forward.
    pub const NUDGE_AT_POS: u32 = 1 << 19;
    /// Nudge backward.
    pub const NUDGE_AT_NEG: u32 = 1 << 20;
    /// Nudge left.
    pub const NUDGE_LEFT_POS: u32 = 1 << 21;
    /// Nudge right.
    pub const NUDGE_LEFT_NEG: u32 = 1 << 22;
    /// Nudge up.
    pub const NUDGE_UP_POS: u32 = 1 << 23;
    /// Nudge down.
    pub const NUDGE_UP_NEG: u32 = 1 << 24;
    /// Turn left.
    pub const TURN_LEFT: u32 = 1 << 25;
    /// Turn right.
    pub const TURN_RIGHT: u32 = 1 << 26;
    /// Away from keyboard.
    pub const AWAY: u32 = 1 << 27;
    /// Left mouse button pressed.
    pub const LBUTTON_DOWN: u32 = 1 << 28;
    /// Left mouse button released.
    pub const LBUTTON_UP: u32 = 1 << 29;
    /// Left mouse button pressed (mouselook).
    pub const ML_LBUTTON_DOWN: u32 = 1 << 30;
    /// Left mouse button released (mouselook).
    pub const ML_LBUTTON_UP: u32 = 1 << 31;
}

/// Well-known animation UUIDs from the SL protocol.
///
/// These are the default avatar animations built into the simulator.
pub mod animations {
    use uuid::Uuid;

    /// Standing idle animation.
    pub const STAND: Uuid = Uuid::from_u128(0x2408fe9e_df1d_1d7d_f4ff_a77064d98177);
    /// Walking animation.
    pub const WALK: Uuid = Uuid::from_u128(0x6ed24bd8_91aa_4b12_ccc7_c97c857ab4e0);
    /// Running animation.
    pub const RUN: Uuid = Uuid::from_u128(0x05ddbff8_aaa9_92a1_2b74_8b66b1b7eb75);
    /// Flying animation.
    pub const FLY: Uuid = Uuid::from_u128(0xaec4610c_757f_bc4e_c092_c6e9caf18daf);
    /// Hovering (flying idle) animation.
    pub const HOVER: Uuid = Uuid::from_u128(0x4ae8016b_31b9_03bb_c401_b1ea941db41d);
    /// Sitting animation.
    pub const SIT: Uuid = Uuid::from_u128(0x1a5fe8ac_a804_8a5d_7f3a_0a31f4c5860c);
    /// Sitting on ground animation.
    pub const SIT_GROUND: Uuid = Uuid::from_u128(0x1c7600d6_661f_b87b_efe2_d7421eb93c86);
    /// Crouching animation.
    pub const CROUCH: Uuid = Uuid::from_u128(0x201f3fdf_cb1f_dbec_201f_7333e328ae7b);
    /// Typing animation.
    pub const TYPE: Uuid = Uuid::from_u128(0xc541c47f_e0c0_058b_ad1a_d6ae3a4584d9);
}

/// Send a raw AgentUpdate packet to the simulator.
///
/// This is the low-level function. Higher-level helpers like `move_to`,
/// `toggle_fly`, etc. build on top of this.
pub async fn send_agent_update(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    body_rotation: Quat,
    head_rotation: Quat,
    state: u8,
    camera_center: Vec3,
    camera_at: Vec3,
    camera_left: Vec3,
    camera_up: Vec3,
    far: f32,
    control_flags: u32,
    flags: u8,
) -> Result<(), RealmWalkerError> {
    let msg = AgentUpdate {
        agent_data: agent_update::AgentData {
            agent_id,
            session_id,
            body_rotation,
            head_rotation,
            state,
            camera_center,
            camera_at_axis: camera_at,
            camera_left_axis: camera_left,
            camera_up_axis: camera_up,
            far,
            control_flags,
            flags,
        },
    };
    net.send_msg(&msg, true).await
}

/// Send a simplified AgentUpdate with just position, rotation, and control flags.
///
/// Camera axes default to looking straight ahead from the agent's position,
/// which is sufficient for bot use.
pub async fn send_simple_agent_update(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    position: Vec3,
    body_rotation: Quat,
    control_flags: u32,
) -> Result<(), RealmWalkerError> {
    // Derive a forward-looking camera from the body rotation.
    let forward = body_rotation * Vec3::X;
    let left = body_rotation * Vec3::Y;
    let up = Vec3::Z;

    send_agent_update(
        net,
        agent_id,
        session_id,
        body_rotation,
        Quat::IDENTITY,
        0,
        position + Vec3::new(0.0, 0.0, 1.0), // camera slightly above agent
        forward,
        left,
        up,
        128.0,
        control_flags,
        0,
    )
    .await
}

/// Request the server to autopilot the agent to a target position.
///
/// Uses the `GenericMessage` packet with method `"autopilot"` and three
/// float parameters (x, y, z). The server moves the avatar in a straight
/// line to the destination. Only works within the current region.
pub async fn move_to(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    target: Vec3,
) -> Result<(), RealmWalkerError> {
    let msg = GenericMessage {
        agent_data: generic_message::AgentData {
            agent_id,
            session_id,
            transaction_id: Uuid::nil(),
        },
        method_data: generic_message::MethodData {
            method: b"autopilot\0".to_vec(),
            invoice: Uuid::nil(),
        },
        param_list: vec![
            generic_message::ParamList {
                parameter: format!("{}", target.x).into_bytes(),
            },
            generic_message::ParamList {
                parameter: format!("{}", target.y).into_bytes(),
            },
            generic_message::ParamList {
                parameter: format!("{}", target.z).into_bytes(),
            },
        ],
    };
    net.send_msg(&msg, true).await
}

/// Request a teleport to a specific region + position.
///
/// The server responds with TeleportStart, TeleportProgress, and finally
/// TeleportLocal (same sim) or TeleportFinish (different sim).
pub async fn teleport(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    region_handle: u64,
    position: Vec3,
    look_at: Vec3,
) -> Result<(), RealmWalkerError> {
    let msg = TeleportLocationRequest {
        agent_data: teleport_location_request::AgentData {
            agent_id,
            session_id,
        },
        info: teleport_location_request::Info {
            region_handle,
            position,
            look_at,
        },
    };
    net.send_msg(&msg, true).await
}

/// Request to sit on a specific object.
///
/// The `target_id` is the UUID of the object to sit on. The `offset`
/// is the desired sit position relative to the object's center.
/// After the request, the server may reply with an avatar sit animation
/// and update the agent's parent.
pub async fn request_sit(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    target_id: Uuid,
    offset: Vec3,
) -> Result<(), RealmWalkerError> {
    let msg = AgentRequestSit {
        agent_data: agent_request_sit::AgentData {
            agent_id,
            session_id,
        },
        target_object: agent_request_sit::TargetObject {
            target_id,
            offset,
        },
    };
    net.send_msg(&msg, true).await
}

/// Confirm a sit on the object previously requested via `request_sit`.
///
/// In the standard flow, the viewer calls `AgentRequestSit` then
/// immediately follows with `AgentSit` to confirm.
pub async fn sit(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = AgentSit {
        agent_data: agent_sit::AgentData {
            agent_id,
            session_id,
        },
    };
    net.send_msg(&msg, true).await
}

/// Stand up from a sitting position.
///
/// Sends an AgentUpdate with the STAND_UP control flag set.
pub async fn stand(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
) -> Result<(), RealmWalkerError> {
    send_simple_agent_update(
        net,
        agent_id,
        session_id,
        Vec3::ZERO,
        Quat::IDENTITY,
        control_flags::STAND_UP,
    )
    .await
}

/// Toggle flying on or off.
///
/// When `fly` is true, sets the FLY control flag. When false, clears it.
/// The server keeps the fly state until another AgentUpdate changes it.
pub async fn set_flying(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    fly: bool,
) -> Result<(), RealmWalkerError> {
    let flags = if fly { control_flags::FLY } else { 0 };
    send_simple_agent_update(
        net,
        agent_id,
        session_id,
        Vec3::ZERO,
        Quat::IDENTITY,
        flags,
    )
    .await
}

/// Set the always-run flag.
///
/// When true, the avatar runs instead of walking for directional movement.
pub async fn set_always_run(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    always_run: bool,
) -> Result<(), RealmWalkerError> {
    let msg = SetAlwaysRun {
        agent_data: set_always_run::AgentData {
            agent_id,
            session_id,
            always_run,
        },
    };
    net.send_msg(&msg, true).await
}

/// Start or stop an animation on the agent.
///
/// `anim_id` is the UUID of the animation asset. If `start` is true,
/// the animation begins; if false, it stops.
pub async fn animate(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    anim_id: Uuid,
    start: bool,
) -> Result<(), RealmWalkerError> {
    let msg = AgentAnimation {
        agent_data: agent_animation::AgentData {
            agent_id,
            session_id,
        },
        animation_list: vec![agent_animation::AnimationList {
            anim_id,
            start_anim: start,
        }],
        physical_avatar_event_list: Vec::new(),
    };
    net.send_msg(&msg, false).await
}

/// Start multiple animations and stop others in one packet.
pub async fn animate_batch(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    start: &[Uuid],
    stop: &[Uuid],
) -> Result<(), RealmWalkerError> {
    let mut list = Vec::with_capacity(start.len() + stop.len());
    for &id in start {
        list.push(agent_animation::AnimationList {
            anim_id: id,
            start_anim: true,
        });
    }
    for &id in stop {
        list.push(agent_animation::AnimationList {
            anim_id: id,
            start_anim: false,
        });
    }
    let msg = AgentAnimation {
        agent_data: agent_animation::AgentData {
            agent_id,
            session_id,
        },
        animation_list: list,
        physical_avatar_event_list: Vec::new(),
    };
    net.send_msg(&msg, false).await
}

// ---------------------------------------------------------------------------
// Incoming packet handlers
// ---------------------------------------------------------------------------

/// Try to decode a TeleportLocal packet and emit an event.
pub fn try_handle_teleport_local(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != TeleportLocal::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<TeleportLocal>() else {
        return false;
    };
    let info = &msg.info;
    // TeleportLocal is a same-sim teleport completion. We emit a progress
    // event so the caller knows the teleport landed.
    event_bus.send(Event::TeleportProgress(TeleportProgressData {
        flags: info.teleport_flags,
        message: format!(
            "Teleport complete at ({:.0}, {:.0}, {:.0})",
            info.position.x, info.position.y, info.position.z
        ),
    }));
    true
}

/// Try to decode a TeleportProgress packet and emit an event.
pub fn try_handle_teleport_progress(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != TeleportProgress::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<TeleportProgress>() else {
        return false;
    };
    event_bus.send(Event::TeleportProgress(TeleportProgressData {
        flags: msg.info.teleport_flags,
        message: String::from_utf8_lossy(&msg.info.message).into_owned(),
    }));
    true
}

/// Try to decode a TeleportFinish packet and emit an event.
///
/// TeleportFinish signals a cross-sim teleport. The bot should connect
/// to the new simulator using the provided circuit info. For now, we
/// emit a progress event with the destination info.
pub fn try_handle_teleport_finish(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != TeleportFinish::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<TeleportFinish>() else {
        return false;
    };
    let info = &msg.info;
    let ip = format!(
        "{}.{}.{}.{}",
        info.sim_ip[0], info.sim_ip[1], info.sim_ip[2], info.sim_ip[3]
    );
    event_bus.send(Event::TeleportProgress(TeleportProgressData {
        flags: info.teleport_flags,
        message: format!("TeleportFinish to {}:{}", ip, info.sim_port),
    }));
    true
}

/// Try to decode a TeleportStart packet and emit an event.
///
/// The server sends TeleportStart at the beginning of a teleport to signal
/// that processing has begun. The flags indicate the teleport type.
pub fn try_handle_teleport_start(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != TeleportStart::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<TeleportStart>() else {
        return false;
    };
    event_bus.send(Event::TeleportStarted(TeleportStartData {
        flags: msg.info.teleport_flags,
    }));
    true
}

/// Try to decode a TeleportFailed packet and emit an event.
///
/// The server sends TeleportFailed when a teleport cannot be completed
/// (e.g. destination region offline, access denied, etc.).
pub fn try_handle_teleport_failed(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != TeleportFailed::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<TeleportFailed>() else {
        return false;
    };
    let reason = String::from_utf8_lossy(&msg.info.reason).into_owned();
    event_bus.send(Event::TeleportFailed(TeleportFailedData {
        agent_id: msg.info.agent_id,
        reason,
    }));
    true
}

/// Cancel an in-flight teleport.
///
/// Sends a TeleportCancel packet to the server. This should be called
/// between TeleportStart and TeleportFinish/TeleportLocal if the bot
/// wants to abort the teleport.
pub async fn cancel_teleport(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = TeleportCancel {
        info: teleport_cancel::Info {
            agent_id,
            session_id,
        },
    };
    net.send_msg(&msg, true).await
}

/// Accept a teleport lure (offer) from another avatar.
///
/// `lure_id` is the session UUID from the [`TeleportOfferData`] event.
/// The server will initiate the teleport sequence after receiving this.
pub async fn accept_teleport_lure(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    lure_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = TeleportLureRequest {
        info: teleport_lure_request::Info {
            agent_id,
            session_id,
            lure_id,
            teleport_flags: 0,
        },
    };
    net.send_msg(&msg, true).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::{build_packet, parse_packet, MsgId};

    #[test]
    fn control_flag_values_match_protocol() {
        // Verify key control flag values against the SL protocol spec.
        assert_eq!(control_flags::AT_POS, 0x0000_0001);
        assert_eq!(control_flags::AT_NEG, 0x0000_0002);
        assert_eq!(control_flags::LEFT_POS, 0x0000_0004);
        assert_eq!(control_flags::LEFT_NEG, 0x0000_0008);
        assert_eq!(control_flags::UP_POS, 0x0000_0010);
        assert_eq!(control_flags::UP_NEG, 0x0000_0020);
        assert_eq!(control_flags::PITCH_POS, 0x0000_0040);
        assert_eq!(control_flags::PITCH_NEG, 0x0000_0080);
        assert_eq!(control_flags::YAW_POS, 0x0000_0100);
        assert_eq!(control_flags::YAW_NEG, 0x0000_0200);
        assert_eq!(control_flags::FAST_AT, 0x0000_0400);
        assert_eq!(control_flags::FAST_LEFT, 0x0000_0800);
        assert_eq!(control_flags::FAST_UP, 0x0000_1000);
        assert_eq!(control_flags::FLY, 0x0000_2000);
        assert_eq!(control_flags::STOP, 0x0000_4000);
        assert_eq!(control_flags::FINISH_ANIM, 0x0000_8000);
        assert_eq!(control_flags::STAND_UP, 0x0001_0000);
        assert_eq!(control_flags::SIT_ON_GROUND, 0x0002_0000);
        assert_eq!(control_flags::MOUSELOOK, 0x0004_0000);
        assert_eq!(control_flags::NUDGE_AT_POS, 0x0008_0000);
        assert_eq!(control_flags::NUDGE_AT_NEG, 0x0010_0000);
        assert_eq!(control_flags::NUDGE_LEFT_POS, 0x0020_0000);
        assert_eq!(control_flags::NUDGE_LEFT_NEG, 0x0040_0000);
        assert_eq!(control_flags::NUDGE_UP_POS, 0x0080_0000);
        assert_eq!(control_flags::NUDGE_UP_NEG, 0x0100_0000);
        assert_eq!(control_flags::TURN_LEFT, 0x0200_0000);
        assert_eq!(control_flags::TURN_RIGHT, 0x0400_0000);
        assert_eq!(control_flags::AWAY, 0x0800_0000);
        assert_eq!(control_flags::LBUTTON_DOWN, 0x1000_0000);
        assert_eq!(control_flags::LBUTTON_UP, 0x2000_0000);
        assert_eq!(control_flags::ML_LBUTTON_DOWN, 0x4000_0000);
        assert_eq!(control_flags::ML_LBUTTON_UP, 0x8000_0000);
    }

    #[test]
    fn control_flags_are_distinct_bits() {
        // Every flag should be a single bit -- no overlaps.
        let all: &[u32] = &[
            control_flags::AT_POS,
            control_flags::AT_NEG,
            control_flags::LEFT_POS,
            control_flags::LEFT_NEG,
            control_flags::UP_POS,
            control_flags::UP_NEG,
            control_flags::PITCH_POS,
            control_flags::PITCH_NEG,
            control_flags::YAW_POS,
            control_flags::YAW_NEG,
            control_flags::FAST_AT,
            control_flags::FAST_LEFT,
            control_flags::FAST_UP,
            control_flags::FLY,
            control_flags::STOP,
            control_flags::FINISH_ANIM,
            control_flags::STAND_UP,
            control_flags::SIT_ON_GROUND,
            control_flags::MOUSELOOK,
            control_flags::NUDGE_AT_POS,
            control_flags::NUDGE_AT_NEG,
            control_flags::NUDGE_LEFT_POS,
            control_flags::NUDGE_LEFT_NEG,
            control_flags::NUDGE_UP_POS,
            control_flags::NUDGE_UP_NEG,
            control_flags::TURN_LEFT,
            control_flags::TURN_RIGHT,
            control_flags::AWAY,
            control_flags::LBUTTON_DOWN,
            control_flags::LBUTTON_UP,
            control_flags::ML_LBUTTON_DOWN,
            control_flags::ML_LBUTTON_UP,
        ];
        assert_eq!(all.len(), 32, "should have exactly 32 flags");
        let mut combined = 0u32;
        for &flag in all {
            assert_eq!(
                flag.count_ones(),
                1,
                "flag 0x{:08x} should be a single bit",
                flag
            );
            assert_eq!(
                combined & flag,
                0,
                "flag 0x{:08x} overlaps with existing flags",
                flag
            );
            combined |= flag;
        }
        assert_eq!(combined, u32::MAX, "all 32 bits should be covered");
    }

    #[test]
    fn agent_update_roundtrip() {
        let agent_id = Uuid::from_u128(0x1111);
        let session_id = Uuid::from_u128(0x2222);
        let body_rotation = Quat::from_rotation_z(1.0);

        let msg = AgentUpdate {
            agent_data: agent_update::AgentData {
                agent_id,
                session_id,
                body_rotation,
                head_rotation: Quat::IDENTITY,
                state: 0,
                camera_center: Vec3::new(128.0, 128.0, 25.0),
                camera_at_axis: Vec3::X,
                camera_left_axis: Vec3::Y,
                camera_up_axis: Vec3::Z,
                far: 128.0,
                control_flags: control_flags::AT_POS | control_flags::FLY,
                flags: 0,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 1, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(4));

        let decoded: AgentUpdate = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.agent_data.session_id, session_id);
        assert_eq!(
            decoded.agent_data.control_flags,
            control_flags::AT_POS | control_flags::FLY
        );
        assert!((decoded.agent_data.far - 128.0).abs() < 0.01);
        assert_eq!(
            decoded.agent_data.camera_center,
            Vec3::new(128.0, 128.0, 25.0)
        );
    }

    #[test]
    fn agent_update_byte_layout() {
        // Verify that AgentUpdate encodes as High(4) = single byte 0x04
        // in the message header.
        let msg = AgentUpdate::default();
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(4));
        assert!(AgentUpdate::ZEROCODED);
    }

    #[test]
    fn agent_animation_roundtrip() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);
        let anim_id = animations::FLY;

        let msg = AgentAnimation {
            agent_data: agent_animation::AgentData {
                agent_id,
                session_id,
            },
            animation_list: vec![agent_animation::AnimationList {
                anim_id,
                start_anim: true,
            }],
            physical_avatar_event_list: Vec::new(),
        };

        let mut buf = Vec::new();
        build_packet(&msg, 10, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(5));

        let decoded: AgentAnimation = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.animation_list.len(), 1);
        assert_eq!(decoded.animation_list[0].anim_id, anim_id);
        assert!(decoded.animation_list[0].start_anim);
    }

    #[test]
    fn agent_request_sit_roundtrip() {
        let agent_id = Uuid::from_u128(0x1234);
        let session_id = Uuid::from_u128(0x5678);
        let target_id = Uuid::from_u128(0x9abc);
        let offset = Vec3::new(0.0, 0.0, 0.5);

        let msg = AgentRequestSit {
            agent_data: agent_request_sit::AgentData {
                agent_id,
                session_id,
            },
            target_object: agent_request_sit::TargetObject {
                target_id,
                offset,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 20, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(6));

        let decoded: AgentRequestSit = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.target_object.target_id, target_id);
        assert!((decoded.target_object.offset.z - 0.5).abs() < 0.001);
    }

    #[test]
    fn agent_sit_roundtrip() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);

        let msg = AgentSit {
            agent_data: agent_sit::AgentData {
                agent_id,
                session_id,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 30, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(7));

        let decoded: AgentSit = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.agent_data.session_id, session_id);
    }

    #[test]
    fn teleport_location_request_roundtrip() {
        let agent_id = Uuid::from_u128(0x1111);
        let session_id = Uuid::from_u128(0x2222);
        let region_handle = 0x0003_E800_0003_E800_u64; // 256000, 256000

        let msg = TeleportLocationRequest {
            agent_data: teleport_location_request::AgentData {
                agent_id,
                session_id,
            },
            info: teleport_location_request::Info {
                region_handle,
                position: Vec3::new(128.0, 128.0, 25.0),
                look_at: Vec3::new(1.0, 0.0, 0.0),
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 40, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(63));

        let decoded: TeleportLocationRequest = pkt.decode().expect("should decode");
        assert_eq!(decoded.info.region_handle, region_handle);
        assert!((decoded.info.position.x - 128.0).abs() < 0.001);
    }

    #[test]
    fn generic_message_autopilot_roundtrip() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);

        let msg = GenericMessage {
            agent_data: generic_message::AgentData {
                agent_id,
                session_id,
                transaction_id: Uuid::nil(),
            },
            method_data: generic_message::MethodData {
                method: b"autopilot\0".to_vec(),
                invoice: Uuid::nil(),
            },
            param_list: vec![
                generic_message::ParamList {
                    parameter: b"128.5".to_vec(),
                },
                generic_message::ParamList {
                    parameter: b"64.25".to_vec(),
                },
                generic_message::ParamList {
                    parameter: b"22.0".to_vec(),
                },
            ],
        };

        let mut buf = Vec::new();
        build_packet(&msg, 50, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(261));

        let decoded: GenericMessage = pkt.decode().expect("should decode");
        assert_eq!(
            String::from_utf8_lossy(&decoded.method_data.method),
            "autopilot\0"
        );
        assert_eq!(decoded.param_list.len(), 3);
        assert_eq!(
            String::from_utf8_lossy(&decoded.param_list[0].parameter),
            "128.5"
        );
    }

    #[test]
    fn set_always_run_roundtrip() {
        let agent_id = Uuid::from_u128(0x1234);
        let session_id = Uuid::from_u128(0x5678);

        let msg = SetAlwaysRun {
            agent_data: set_always_run::AgentData {
                agent_id,
                session_id,
                always_run: true,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 60, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(88));

        let decoded: SetAlwaysRun = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert!(decoded.agent_data.always_run);
    }

    #[test]
    fn teleport_start_roundtrip() {
        let msg = TeleportStart {
            info: teleport_start::Info {
                teleport_flags: 0x0001_0002,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 70, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(73));

        let decoded: TeleportStart = pkt.decode().expect("should decode");
        assert_eq!(decoded.info.teleport_flags, 0x0001_0002);
    }

    #[test]
    fn teleport_failed_roundtrip() {
        let agent_id = Uuid::from_u128(0xdead);

        let msg = TeleportFailed {
            info: teleport_failed::Info {
                agent_id,
                reason: b"Region not found".to_vec(),
            },
            alert_info: vec![teleport_failed::AlertInfo {
                message: b"Teleport denied".to_vec(),
                extra_params: Vec::new(),
            }],
        };

        let mut buf = Vec::new();
        build_packet(&msg, 71, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(74));

        let decoded: TeleportFailed = pkt.decode().expect("should decode");
        assert_eq!(decoded.info.agent_id, agent_id);
        assert_eq!(decoded.info.reason, b"Region not found");
        assert_eq!(decoded.alert_info.len(), 1);
        assert_eq!(decoded.alert_info[0].message, b"Teleport denied");
    }

    #[test]
    fn teleport_cancel_roundtrip() {
        let agent_id = Uuid::from_u128(0x1111);
        let session_id = Uuid::from_u128(0x2222);

        let msg = TeleportCancel {
            info: teleport_cancel::Info {
                agent_id,
                session_id,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 72, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(72));

        let decoded: TeleportCancel = pkt.decode().expect("should decode");
        assert_eq!(decoded.info.agent_id, agent_id);
        assert_eq!(decoded.info.session_id, session_id);
    }

    #[test]
    fn teleport_lure_request_roundtrip() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);
        let lure_id = Uuid::from_u128(0xcccc);

        let msg = TeleportLureRequest {
            info: teleport_lure_request::Info {
                agent_id,
                session_id,
                lure_id,
                teleport_flags: 0,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 73, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(71));

        let decoded: TeleportLureRequest = pkt.decode().expect("should decode");
        assert_eq!(decoded.info.agent_id, agent_id);
        assert_eq!(decoded.info.session_id, session_id);
        assert_eq!(decoded.info.lure_id, lure_id);
        assert_eq!(decoded.info.teleport_flags, 0);
    }
}
