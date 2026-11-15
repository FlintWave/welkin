//! Object interaction: inbound updates, rez/derez, selection, editing, linking.
//!
//! Provides functions for interacting with scene objects over the LLUDP
//! protocol. Incoming ObjectUpdate, ObjectUpdateCompressed,
//! ImprovedTerseObjectUpdate, KillObject, and ObjectProperties packets are
//! decoded and dispatched as events through the EventBus.

use glam::{Quat, Vec3};
use uuid::Uuid;
use welkin_msg::msgs::*;
use welkin_msg::{unpack_quat, IncomingPacket, Msg};

use crate::error::RealmWalkerError;
use crate::events::{Event, EventBus, ObjectPropertiesData, SimObject};
use crate::network::Network;

// ---------------------------------------------------------------------------
// Pcode constants (from the SL message template / OpenMetaverse)
// ---------------------------------------------------------------------------

/// LL_PCODE_LEGACY_AVATAR -- pcode value for avatar objects.
const PCODE_AVATAR: u8 = 46;

// ---------------------------------------------------------------------------
// DeRez destination constants
//
// From OpenSim DeRezAction / LibreMetaverse DeRezDestination (BSD 3-Clause).
// ---------------------------------------------------------------------------

/// Destination codes for DeRezObject.AgentBlock.destination.
pub mod derez_destination {
    /// Save into the agent's inventory.
    pub const SAVE_INTO_AGENT_INVENTORY: u8 = 0;
    /// Copy to the task (in-world script) inventory.
    pub const ACQUIRE_TO_AGENT_INVENTORY: u8 = 1;
    /// Save into a task's inventory.
    pub const SAVE_INTO_TASK_INVENTORY: u8 = 2;
    /// Attachment.
    pub const ATTACHMENT: u8 = 3;
    /// Take a copy.
    pub const TAKE_COPY: u8 = 4;
    /// God take a copy.
    pub const GOD_TAKE_COPY: u8 = 5;
    /// Delete to trash.
    pub const DELETE_TO_TRASH: u8 = 6;
    /// Attachment to inventory.
    pub const ATTACHMENT_TO_INV: u8 = 7;
    /// Attachment exists in inventory already.
    pub const ATTACHMENT_EXISTS: u8 = 8;
    /// Return to owner.
    pub const RETURN_TO_OWNER: u8 = 9;
    /// Return to last owner (used by admin tools).
    pub const RETURN_TO_LAST_OWNER: u8 = 10;
}

/// Permission field codes for the ObjectPermissions message.
pub mod perm_field {
    /// Base permissions.
    pub const BASE: u8 = 0x01;
    /// Owner permissions.
    pub const OWNER: u8 = 0x02;
    /// Group permissions.
    pub const GROUP: u8 = 0x04;
    /// Everyone permissions.
    pub const EVERYONE: u8 = 0x08;
    /// Next-owner permissions.
    pub const NEXT_OWNER: u8 = 0x10;
}

/// Permission mask bit flags.
pub mod perm_mask {
    pub const TRANSFER: u32 = 1 << 13;
    pub const MODIFY: u32 = 1 << 14;
    pub const COPY: u32 = 1 << 15;
    pub const MOVE: u32 = 1 << 19;
    /// All bits (SL PERM_ALL = 0x7FFFFFFF).
    pub const ALL: u32 = 0x7FFF_FFFF;
}

// ---------------------------------------------------------------------------
// Helpers: parse ObjectData.object_data bytes (position/velocity/rotation)
// ---------------------------------------------------------------------------

/// Decode the variable-length `object_data` field from an ObjectUpdate packet.
///
/// Layout (all little-endian):
///   - If avatar (76 bytes): 16 bytes foot collision plane, then same as prim.
///   - Prim (60 bytes): position (3xf32), velocity (3xf32), acceleration
///     (3xf32), rotation (3xf32 packed quat), angular velocity (3xf32).
fn parse_object_data(data: &[u8], is_avatar: bool) -> (Vec3, Vec3, Vec3, Quat, Vec3) {
    let offset = if is_avatar { 16 } else { 0 };
    if data.len() < offset + 60 {
        return (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO);
    }
    let d = &data[offset..];
    let f = |i: usize| f32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
    let position = Vec3::new(f(0), f(4), f(8));
    let velocity = Vec3::new(f(12), f(16), f(20));
    let acceleration = Vec3::new(f(24), f(28), f(32));
    let packed_rot = Vec3::new(f(36), f(40), f(44));
    let rotation = unpack_quat(packed_rot);
    let angular_velocity = Vec3::new(f(48), f(52), f(56));
    (position, velocity, acceleration, rotation, angular_velocity)
}

/// Decode a u16 quantized float back to f32 within [lower, upper].
fn u16_to_f32(v: u16, lower: f32, upper: f32) -> f32 {
    let delta = upper - lower;
    let mut val = v as f32 / 65535.0 * delta + lower;
    let max_error = delta / 65535.0;
    if val.abs() < max_error {
        val = 0.0;
    }
    val
}

/// Parse an ImprovedTerseObjectUpdate data block.
///
/// Layout (little-endian):
///   - local_id: u32
///   - state: u8
///   - is_avatar: u8 (1 = avatar)
///   - If avatar: 16 bytes foot collision plane
///   - position: 3xf32
///   - velocity: 3xu16 quantized [-128, 128]
///   - acceleration: 3xu16 quantized [-64, 64]
///   - rotation: 4xu16 quantized [-1, 1]
///   - angular velocity: 3xu16 quantized [-64, 64]
fn parse_terse_data(data: &[u8]) -> Option<(u32, u8, bool, Vec3, Vec3, Vec3, Quat, Vec3)> {
    if data.len() < 6 {
        return None;
    }
    let local_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let state = data[4];
    let is_avatar = data[5] != 0;

    let offset = if is_avatar { 6 + 16 } else { 6 };
    // position (12) + velocity (6) + acceleration (6) + rotation (8) + angular_velocity (6) = 38
    if data.len() < offset + 38 {
        return None;
    }
    let d = &data[offset..];

    let f = |i: usize| f32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
    let position = Vec3::new(f(0), f(4), f(8));

    let u = |i: usize| u16::from_le_bytes([d[i], d[i + 1]]);
    let velocity = Vec3::new(
        u16_to_f32(u(12), -128.0, 128.0),
        u16_to_f32(u(14), -128.0, 128.0),
        u16_to_f32(u(16), -128.0, 128.0),
    );
    let acceleration = Vec3::new(
        u16_to_f32(u(18), -64.0, 64.0),
        u16_to_f32(u(20), -64.0, 64.0),
        u16_to_f32(u(22), -64.0, 64.0),
    );
    let rotation = Quat::from_xyzw(
        u16_to_f32(u(24), -1.0, 1.0),
        u16_to_f32(u(26), -1.0, 1.0),
        u16_to_f32(u(28), -1.0, 1.0),
        u16_to_f32(u(30), -1.0, 1.0),
    );
    let angular_velocity = Vec3::new(
        u16_to_f32(u(32), -64.0, 64.0),
        u16_to_f32(u(34), -64.0, 64.0),
        u16_to_f32(u(36), -64.0, 64.0),
    );

    Some((local_id, state, is_avatar, position, velocity, acceleration, rotation, angular_velocity))
}

// ---------------------------------------------------------------------------
// Inbound packet handlers
// ---------------------------------------------------------------------------

/// Try to decode an ObjectUpdate packet and emit ObjectUpdated events.
/// Returns true if the packet was an ObjectUpdate.
pub fn try_handle_object_update(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != ObjectUpdate::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<ObjectUpdate>() else {
        return false;
    };
    let region_handle = msg.region_data.region_handle;

    for obj in &msg.object_data {
        let is_avatar = obj.p_code == PCODE_AVATAR;
        let (position, velocity, acceleration, rotation, angular_velocity) =
            parse_object_data(&obj.object_data, is_avatar);

        let text_color = if obj.text_color.len() >= 4 {
            [obj.text_color[0], obj.text_color[1], obj.text_color[2], 255 - obj.text_color[3]]
        } else {
            [0, 0, 0, 0]
        };

        let sim_obj = SimObject {
            local_id: obj.id,
            full_id: obj.full_id,
            parent_id: obj.parent_id,
            pcode: obj.p_code,
            state: obj.state,
            material: obj.material,
            click_action: obj.click_action,
            scale: obj.scale,
            position,
            rotation,
            velocity,
            acceleration,
            angular_velocity,
            update_flags: obj.update_flags,
            owner_id: obj.owner_id,
            name_value: String::from_utf8_lossy(&obj.name_value).into_owned(),
            text: String::from_utf8_lossy(&obj.text).into_owned(),
            text_color,
            is_avatar,
            region_handle,
        };
        event_bus.send(Event::ObjectUpdated(sim_obj));
    }
    true
}

/// Try to decode an ObjectUpdateCompressed packet and emit ObjectUpdated events.
/// Returns true if the packet was an ObjectUpdateCompressed.
///
/// The compressed format packs object data into a single byte blob with flags
/// indicating which optional sections are present. We extract the core fields
/// that the SimObject struct carries.
pub fn try_handle_object_update_compressed(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != ObjectUpdateCompressed::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<ObjectUpdateCompressed>() else {
        return false;
    };
    let region_handle = msg.region_data.region_handle;

    for obj in &msg.object_data {
        let data = &obj.data;
        // Minimum size: 16 (full_id) + 4 (local_id) + 1 (pcode) + 1 (state)
        //             + 4 (crc) + 1 (material) + 1 (click_action) + 12 (scale)
        //             + 12 (position) + 12 (rotation) + 4 (flags) + 16 (owner_id) = 84
        if data.len() < 84 {
            continue;
        }
        let full_id = Uuid::from_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let local_id = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let pcode = data[20];
        let state = data[21];
        let _crc = u32::from_le_bytes([data[22], data[23], data[24], data[25]]);
        let material = data[26];
        let click_action = data[27];

        let f = |i: usize| f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        let scale = Vec3::new(f(28), f(32), f(36));
        let position = Vec3::new(f(40), f(44), f(48));
        let packed_rot = Vec3::new(f(52), f(56), f(60));
        let rotation = unpack_quat(packed_rot);

        let comp_flags = u32::from_le_bytes([data[64], data[65], data[66], data[67]]);
        let owner_id = Uuid::from_bytes([
            data[68], data[69], data[70], data[71], data[72], data[73], data[74], data[75],
            data[76], data[77], data[78], data[79], data[80], data[81], data[82], data[83],
        ]);

        let mut cursor = 84usize;

        // Flag 0x80: angular velocity (3xf32 = 12 bytes)
        let angular_velocity = if comp_flags & 0x80 != 0 {
            if data.len() < cursor + 12 {
                continue;
            }
            let v = Vec3::new(
                f32::from_le_bytes([data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]]),
                f32::from_le_bytes([data[cursor + 4], data[cursor + 5], data[cursor + 6], data[cursor + 7]]),
                f32::from_le_bytes([data[cursor + 8], data[cursor + 9], data[cursor + 10], data[cursor + 11]]),
            );
            cursor += 12;
            v
        } else {
            Vec3::ZERO
        };

        // Flag 0x20: parent_id (u32 = 4 bytes)
        let parent_id = if comp_flags & 0x20 != 0 {
            if data.len() < cursor + 4 {
                continue;
            }
            let pid = u32::from_le_bytes([data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]]);
            cursor += 4;
            pid
        } else {
            0
        };

        // Flag 0x04: text + text_color
        let (text, text_color) = if comp_flags & 0x04 != 0 {
            // Null-terminated string followed by 4 bytes RGBA (alpha inverted).
            let text_end = data[cursor..].iter().position(|&b| b == 0).unwrap_or(data.len() - cursor);
            let t = String::from_utf8_lossy(&data[cursor..cursor + text_end]).into_owned();
            cursor += text_end + 1; // skip null
            if data.len() < cursor + 4 {
                continue;
            }
            let tc = [data[cursor], data[cursor + 1], data[cursor + 2], 255 - data[cursor + 3]];
            cursor += 4;
            (t, tc)
        } else {
            (String::new(), [0, 0, 0, 0])
        };

        // Flag 0x200: media url (null-terminated)
        if comp_flags & 0x200 != 0 {
            let end = data[cursor..].iter().position(|&b| b == 0).unwrap_or(data.len() - cursor);
            cursor += end + 1;
        }

        // Flag 0x08: particle system (86 bytes)
        if comp_flags & 0x08 != 0 {
            if data.len() < cursor + 86 {
                continue;
            }
            cursor += 86;
        }

        // Extra params: at least 1 byte (0 = none)
        if data.len() <= cursor {
            continue;
        }
        if data[cursor] == 0 {
            cursor += 1;
        } else {
            // Walk the extra params chain. Each entry has: type (2) + len (4) + data (len).
            let num = data[cursor] as usize;
            cursor += 1;
            for _ in 0..num {
                if data.len() < cursor + 6 {
                    break;
                }
                let ep_len = u32::from_le_bytes([data[cursor + 2], data[cursor + 3], data[cursor + 4], data[cursor + 5]]) as usize;
                cursor += 6 + ep_len;
            }
        }

        // Flag 0x10: sound (16 uuid + 4 gain + 1 flags + 4 radius = 25 bytes)
        if comp_flags & 0x10 != 0 {
            cursor += 25.min(data.len().saturating_sub(cursor));
        }

        // Flag 0x100: name_value (null-terminated)
        let name_value = if comp_flags & 0x100 != 0 {
            let end = data[cursor..].iter().position(|&b| b == 0).unwrap_or(data.len() - cursor);
            let nv = String::from_utf8_lossy(&data[cursor..cursor + end]).into_owned();
            let _ = cursor + end + 1; // consume null
            nv
        } else {
            String::new()
        };

        let is_avatar = pcode == PCODE_AVATAR;

        let sim_obj = SimObject {
            local_id,
            full_id,
            parent_id,
            pcode,
            state,
            material,
            click_action,
            scale,
            position,
            rotation,
            velocity: Vec3::ZERO, // not in compressed format
            acceleration: Vec3::ZERO, // not in compressed format
            angular_velocity,
            update_flags: obj.update_flags,
            owner_id,
            name_value,
            text,
            text_color,
            is_avatar,
            region_handle,
        };
        event_bus.send(Event::ObjectUpdated(sim_obj));
    }
    true
}

/// Try to decode an ImprovedTerseObjectUpdate packet and emit ObjectUpdated events.
/// Returns true if the packet was an ImprovedTerseObjectUpdate.
///
/// Terse updates carry only kinematics (position, velocity, rotation) and are
/// used for moving objects and avatars. They do not carry ownership, name, or
/// other metadata. We emit a partial SimObject with the fields we have; the
/// caller is expected to merge into a local cache.
pub fn try_handle_terse_object_update(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != ImprovedTerseObjectUpdate::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<ImprovedTerseObjectUpdate>() else {
        return false;
    };
    let region_handle = msg.region_data.region_handle;

    for obj in &msg.object_data {
        let Some((local_id, state, is_avatar, position, velocity, acceleration, rotation, angular_velocity)) =
            parse_terse_data(&obj.data)
        else {
            continue;
        };

        let sim_obj = SimObject {
            local_id,
            full_id: Uuid::nil(),
            parent_id: 0,
            pcode: if is_avatar { PCODE_AVATAR } else { 0 },
            state,
            material: 0,
            click_action: 0,
            scale: Vec3::ZERO,
            position,
            rotation,
            velocity,
            acceleration,
            angular_velocity,
            update_flags: 0,
            owner_id: Uuid::nil(),
            name_value: String::new(),
            text: String::new(),
            text_color: [0, 0, 0, 0],
            is_avatar,
            region_handle,
        };
        event_bus.send(Event::ObjectUpdated(sim_obj));
    }
    true
}

/// Try to decode a KillObject packet and emit an ObjectKilled event.
/// Returns true if the packet was a KillObject.
pub fn try_handle_kill_object(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != KillObject::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<KillObject>() else {
        return false;
    };
    let local_ids: Vec<u32> = msg.object_data.iter().map(|d| d.id).collect();
    if !local_ids.is_empty() {
        event_bus.send(Event::ObjectKilled { local_ids });
    }
    true
}

/// Try to decode an ObjectProperties packet and emit ObjectPropertiesReceived events.
/// Returns true if the packet was an ObjectProperties.
pub fn try_handle_object_properties(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != ObjectProperties::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<ObjectProperties>() else {
        return false;
    };
    for obj in &msg.object_data {
        let props = ObjectPropertiesData {
            object_id: obj.object_id,
            creator_id: obj.creator_id,
            owner_id: obj.owner_id,
            group_id: obj.group_id,
            creation_date: obj.creation_date,
            base_mask: obj.base_mask,
            owner_mask: obj.owner_mask,
            group_mask: obj.group_mask,
            everyone_mask: obj.everyone_mask,
            next_owner_mask: obj.next_owner_mask,
            sale_type: obj.sale_type,
            sale_price: obj.sale_price,
            name: String::from_utf8_lossy(&obj.name).into_owned(),
            description: String::from_utf8_lossy(&obj.description).into_owned(),
            touch_name: String::from_utf8_lossy(&obj.touch_name).into_owned(),
            sit_name: String::from_utf8_lossy(&obj.sit_name).into_owned(),
        };
        event_bus.send(Event::ObjectPropertiesReceived(props));
    }
    true
}

// ---------------------------------------------------------------------------
// Outbound actions
// ---------------------------------------------------------------------------

/// Rez an object from inventory into the scene.
///
/// `item_id` is the UUID of the inventory item to rez. `position` is the
/// target location in the region. The object appears at the ground under the
/// agent if no raycast is provided.
pub async fn rez_object(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    item_id: Uuid,
    owner_id: Uuid,
    position: Vec3,
    group_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = RezObject {
        agent_data: rez_object::AgentData {
            agent_id,
            session_id,
            group_id,
        },
        rez_data: rez_object::RezData {
            from_task_id: Uuid::nil(),
            bypass_raycast: 1, // TRUE: place at exact position
            ray_start: position,
            ray_end: position,
            ray_target_id: Uuid::nil(),
            ray_end_is_intersection: false,
            rez_selected: false,
            remove_item: false,
            item_flags: 0,
            group_mask: 0,
            everyone_mask: 0,
            next_owner_mask: perm_mask::ALL,
        },
        inventory_data: rez_object::InventoryData {
            item_id,
            folder_id: Uuid::nil(),
            creator_id: owner_id,
            owner_id,
            group_id,
            base_mask: perm_mask::ALL,
            owner_mask: perm_mask::ALL,
            group_mask: 0,
            everyone_mask: 0,
            next_owner_mask: perm_mask::ALL,
            group_owned: false,
            transaction_id: Uuid::new_v4(),
            type_: 6, // AssetType.Object
            inv_type: 6, // InventoryType.Object
            flags: 0,
            sale_type: 0,
            sale_price: 0,
            name: Vec::new(),
            description: Vec::new(),
            creation_date: 0,
            crc: 0,
        },
    };
    net.send_msg(&msg, true).await
}

/// Derez one or more objects (take to inventory, delete, return, etc.).
///
/// `destination` is one of the [`derez_destination`] constants.
/// `destination_id` is the folder UUID for inventory destinations, or nil
/// for delete/return.
pub async fn derez_objects(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_ids: &[u32],
    destination: u8,
    destination_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = DeRezObject {
        agent_data: de_rez_object::AgentData {
            agent_id,
            session_id,
        },
        agent_block: de_rez_object::AgentBlock {
            group_id: Uuid::nil(),
            destination,
            destination_id,
            transaction_id: Uuid::new_v4(),
            packet_count: 1,
            packet_number: 0,
        },
        object_data: local_ids
            .iter()
            .map(|&id| de_rez_object::ObjectData {
                object_local_id: id,
            })
            .collect(),
    };
    net.send_msg(&msg, true).await
}

/// Select one or more objects (equivalent to clicking them in the viewer).
///
/// Selecting an object triggers the server to send ObjectProperties back.
pub async fn select_objects(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_ids: &[u32],
) -> Result<(), RealmWalkerError> {
    let msg = ObjectSelect {
        agent_data: object_select::AgentData {
            agent_id,
            session_id,
        },
        object_data: local_ids
            .iter()
            .map(|&id| object_select::ObjectData {
                object_local_id: id,
            })
            .collect(),
    };
    net.send_msg(&msg, true).await
}

/// Deselect one or more objects.
pub async fn deselect_objects(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_ids: &[u32],
) -> Result<(), RealmWalkerError> {
    let msg = ObjectDeselect {
        agent_data: object_deselect::AgentData {
            agent_id,
            session_id,
        },
        object_data: local_ids
            .iter()
            .map(|&id| object_deselect::ObjectData {
                object_local_id: id,
            })
            .collect(),
    };
    net.send_msg(&msg, true).await
}

/// Set the name of an object.
pub async fn set_object_name(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_id: u32,
    name: &str,
) -> Result<(), RealmWalkerError> {
    let msg = ObjectName {
        agent_data: object_name::AgentData {
            agent_id,
            session_id,
        },
        object_data: vec![object_name::ObjectData {
            local_id,
            name: name.as_bytes().to_vec(),
        }],
    };
    net.send_msg(&msg, true).await
}

/// Set the description of an object.
pub async fn set_object_description(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_id: u32,
    description: &str,
) -> Result<(), RealmWalkerError> {
    let msg = ObjectDescription {
        agent_data: object_description::AgentData {
            agent_id,
            session_id,
        },
        object_data: vec![object_description::ObjectData {
            local_id,
            description: description.as_bytes().to_vec(),
        }],
    };
    net.send_msg(&msg, true).await
}

/// Set permissions on an object.
///
/// `field` identifies which permission set to change (see [`perm_field`]).
/// `set` is 1 to add the mask bits, 0 to remove them.
/// `mask` is the permission bits to apply.
pub async fn set_object_permissions(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_id: u32,
    field: u8,
    set: u8,
    mask: u32,
) -> Result<(), RealmWalkerError> {
    let msg = ObjectPermissions {
        agent_data: object_permissions::AgentData {
            agent_id,
            session_id,
        },
        header_data: object_permissions::HeaderData {
            override_: false,
        },
        object_data: vec![object_permissions::ObjectData {
            object_local_id: local_id,
            field,
            set,
            mask,
        }],
    };
    net.send_msg(&msg, true).await
}

/// Link multiple objects together. The last local_id in the list becomes
/// the new root prim.
pub async fn link_objects(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_ids: &[u32],
) -> Result<(), RealmWalkerError> {
    let msg = ObjectLink {
        agent_data: object_link::AgentData {
            agent_id,
            session_id,
        },
        object_data: local_ids
            .iter()
            .map(|&id| object_link::ObjectData {
                object_local_id: id,
            })
            .collect(),
    };
    net.send_msg(&msg, true).await
}

/// Unlink one or more objects from their linkset.
pub async fn delink_objects(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    local_ids: &[u32],
) -> Result<(), RealmWalkerError> {
    let msg = ObjectDelink {
        agent_data: object_delink::AgentData {
            agent_id,
            session_id,
        },
        object_data: local_ids
            .iter()
            .map(|&id| object_delink::ObjectData {
                object_local_id: id,
            })
            .collect(),
    };
    net.send_msg(&msg, true).await
}

/// Request the server to send detailed object properties.
///
/// The server responds with an ObjectProperties or ObjectPropertiesFamily
/// message that is dispatched as an `ObjectPropertiesReceived` event.
pub async fn request_object_properties(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    object_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = RequestObjectPropertiesFamily {
        agent_data: request_object_properties_family::AgentData {
            agent_id,
            session_id,
        },
        object_data: request_object_properties_family::ObjectData {
            request_flags: 0,
            object_id,
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

    // -- Outbound packet construction tests --

    #[test]
    fn object_select_roundtrip() {
        let agent_id = Uuid::from_u128(0x1111);
        let session_id = Uuid::from_u128(0x2222);
        let msg = ObjectSelect {
            agent_data: object_select::AgentData {
                agent_id,
                session_id,
            },
            object_data: vec![
                object_select::ObjectData { object_local_id: 42 },
                object_select::ObjectData { object_local_id: 99 },
            ],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(110));

        let decoded: ObjectSelect = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.object_data.len(), 2);
        assert_eq!(decoded.object_data[0].object_local_id, 42);
        assert_eq!(decoded.object_data[1].object_local_id, 99);
    }

    #[test]
    fn object_deselect_roundtrip() {
        let agent_id = Uuid::from_u128(0x3333);
        let session_id = Uuid::from_u128(0x4444);
        let msg = ObjectDeselect {
            agent_data: object_deselect::AgentData {
                agent_id,
                session_id,
            },
            object_data: vec![object_deselect::ObjectData {
                object_local_id: 77,
            }],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 2, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(111));

        let decoded: ObjectDeselect = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.object_data[0].object_local_id, 77);
    }

    #[test]
    fn object_name_roundtrip() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);
        let msg = ObjectName {
            agent_data: object_name::AgentData {
                agent_id,
                session_id,
            },
            object_data: vec![object_name::ObjectData {
                local_id: 123,
                name: b"Test Object".to_vec(),
            }],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 3, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(107));

        let decoded: ObjectName = pkt.decode().expect("should decode");
        assert_eq!(decoded.object_data[0].local_id, 123);
        assert_eq!(decoded.object_data[0].name, b"Test Object");
    }

    #[test]
    fn object_description_roundtrip() {
        let agent_id = Uuid::from_u128(0xcccc);
        let session_id = Uuid::from_u128(0xdddd);
        let msg = ObjectDescription {
            agent_data: object_description::AgentData {
                agent_id,
                session_id,
            },
            object_data: vec![object_description::ObjectData {
                local_id: 456,
                description: b"A fine object".to_vec(),
            }],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 4, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(108));

        let decoded: ObjectDescription = pkt.decode().expect("should decode");
        assert_eq!(decoded.object_data[0].local_id, 456);
        assert_eq!(decoded.object_data[0].description, b"A fine object");
    }

    #[test]
    fn object_permissions_roundtrip() {
        let agent_id = Uuid::from_u128(0xeeee);
        let session_id = Uuid::from_u128(0xffff);
        let msg = ObjectPermissions {
            agent_data: object_permissions::AgentData {
                agent_id,
                session_id,
            },
            header_data: object_permissions::HeaderData {
                override_: false,
            },
            object_data: vec![object_permissions::ObjectData {
                object_local_id: 789,
                field: perm_field::NEXT_OWNER,
                set: 1,
                mask: perm_mask::COPY | perm_mask::TRANSFER,
            }],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 5, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(105));

        let decoded: ObjectPermissions = pkt.decode().expect("should decode");
        assert_eq!(decoded.object_data[0].object_local_id, 789);
        assert_eq!(decoded.object_data[0].field, perm_field::NEXT_OWNER);
        assert_eq!(decoded.object_data[0].set, 1);
        assert_eq!(
            decoded.object_data[0].mask,
            perm_mask::COPY | perm_mask::TRANSFER
        );
    }

    #[test]
    fn object_link_roundtrip() {
        let agent_id = Uuid::from_u128(0x1234);
        let session_id = Uuid::from_u128(0x5678);
        let msg = ObjectLink {
            agent_data: object_link::AgentData {
                agent_id,
                session_id,
            },
            object_data: vec![
                object_link::ObjectData { object_local_id: 10 },
                object_link::ObjectData { object_local_id: 20 },
                object_link::ObjectData { object_local_id: 30 },
            ],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 6, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(115));

        let decoded: ObjectLink = pkt.decode().expect("should decode");
        assert_eq!(decoded.object_data.len(), 3);
        assert_eq!(decoded.object_data[0].object_local_id, 10);
        assert_eq!(decoded.object_data[2].object_local_id, 30);
    }

    #[test]
    fn object_delink_roundtrip() {
        let agent_id = Uuid::from_u128(0xabcd);
        let session_id = Uuid::from_u128(0xef01);
        let msg = ObjectDelink {
            agent_data: object_delink::AgentData {
                agent_id,
                session_id,
            },
            object_data: vec![object_delink::ObjectData {
                object_local_id: 55,
            }],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 7, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(116));

        let decoded: ObjectDelink = pkt.decode().expect("should decode");
        assert_eq!(decoded.object_data[0].object_local_id, 55);
    }

    #[test]
    fn derez_object_roundtrip() {
        let agent_id = Uuid::from_u128(0x2222);
        let session_id = Uuid::from_u128(0x3333);
        let transaction_id = Uuid::from_u128(0x9999);
        let destination_id = Uuid::from_u128(0x7777);
        let msg = DeRezObject {
            agent_data: de_rez_object::AgentData {
                agent_id,
                session_id,
            },
            agent_block: de_rez_object::AgentBlock {
                group_id: Uuid::nil(),
                destination: derez_destination::SAVE_INTO_AGENT_INVENTORY,
                destination_id,
                transaction_id,
                packet_count: 1,
                packet_number: 0,
            },
            object_data: vec![
                de_rez_object::ObjectData { object_local_id: 100 },
                de_rez_object::ObjectData { object_local_id: 200 },
            ],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 8, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(291));

        let decoded: DeRezObject = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(
            decoded.agent_block.destination,
            derez_destination::SAVE_INTO_AGENT_INVENTORY
        );
        assert_eq!(decoded.agent_block.destination_id, destination_id);
        assert_eq!(decoded.agent_block.transaction_id, transaction_id);
        assert_eq!(decoded.object_data.len(), 2);
        assert_eq!(decoded.object_data[0].object_local_id, 100);
    }

    #[test]
    fn rez_object_roundtrip() {
        let agent_id = Uuid::from_u128(0x4444);
        let session_id = Uuid::from_u128(0x5555);
        let item_id = Uuid::from_u128(0x8888);
        let msg = RezObject {
            agent_data: rez_object::AgentData {
                agent_id,
                session_id,
                group_id: Uuid::nil(),
            },
            rez_data: rez_object::RezData {
                from_task_id: Uuid::nil(),
                bypass_raycast: 1,
                ray_start: Vec3::new(128.0, 128.0, 25.0),
                ray_end: Vec3::new(128.0, 128.0, 25.0),
                ray_target_id: Uuid::nil(),
                ray_end_is_intersection: false,
                rez_selected: false,
                remove_item: false,
                item_flags: 0,
                group_mask: 0,
                everyone_mask: 0,
                next_owner_mask: perm_mask::ALL,
            },
            inventory_data: rez_object::InventoryData {
                item_id,
                folder_id: Uuid::nil(),
                creator_id: agent_id,
                owner_id: agent_id,
                group_id: Uuid::nil(),
                base_mask: perm_mask::ALL,
                owner_mask: perm_mask::ALL,
                group_mask: 0,
                everyone_mask: 0,
                next_owner_mask: perm_mask::ALL,
                group_owned: false,
                transaction_id: Uuid::nil(),
                type_: 6,
                inv_type: 6,
                flags: 0,
                sale_type: 0,
                sale_price: 0,
                name: b"Test".to_vec(),
                description: Vec::new(),
                creation_date: 0,
                crc: 0,
            },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 9, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(293));

        let decoded: RezObject = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.rez_data.bypass_raycast, 1);
        assert!((decoded.rez_data.ray_start.x - 128.0).abs() < 0.001);
        assert_eq!(decoded.inventory_data.item_id, item_id);
        assert_eq!(decoded.inventory_data.name, b"Test");
    }

    #[test]
    fn request_object_properties_family_roundtrip() {
        let agent_id = Uuid::from_u128(0x6666);
        let session_id = Uuid::from_u128(0x7777);
        let object_id = Uuid::from_u128(0xabcdef);
        let msg = RequestObjectPropertiesFamily {
            agent_data: request_object_properties_family::AgentData {
                agent_id,
                session_id,
            },
            object_data: request_object_properties_family::ObjectData {
                request_flags: 0,
                object_id,
            },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 10, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Medium(5));

        let decoded: RequestObjectPropertiesFamily = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.object_data.object_id, object_id);
    }

    // -- Inbound handler tests --

    #[test]
    fn handle_kill_object() {
        let msg = KillObject {
            object_data: vec![
                kill_object::ObjectData { id: 10 },
                kill_object::ObjectData { id: 20 },
                kill_object::ObjectData { id: 30 },
            ],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(16));

        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();
        assert!(try_handle_kill_object(&pkt, &event_bus));

        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ObjectKilled { local_ids } => {
                assert_eq!(local_ids, vec![10, 20, 30]);
            }
            other => panic!("expected ObjectKilled, got {:?}", other),
        }
    }

    #[test]
    fn handle_kill_object_ignores_other_packets() {
        let msg = CompletePingCheck {
            ping_id: complete_ping_check::PingID { ping_id: 1 },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        let event_bus = EventBus::new(16);
        assert!(!try_handle_kill_object(&pkt, &event_bus));
    }

    #[test]
    fn handle_object_properties() {
        let object_id = Uuid::from_u128(0x1234);
        let creator_id = Uuid::from_u128(0x5678);
        let owner_id = Uuid::from_u128(0x9abc);
        let group_id = Uuid::from_u128(0xdef0);
        let msg = ObjectProperties {
            object_data: vec![object_properties::ObjectData {
                object_id,
                creator_id,
                owner_id,
                group_id,
                creation_date: 1700000000,
                base_mask: perm_mask::ALL,
                owner_mask: perm_mask::ALL,
                group_mask: 0,
                everyone_mask: 0,
                next_owner_mask: perm_mask::COPY | perm_mask::TRANSFER,
                ownership_cost: 0,
                sale_type: 0,
                sale_price: 0,
                aggregate_perms: 0,
                aggregate_perm_textures: 0,
                aggregate_perm_textures_owner: 0,
                category: 0,
                inventory_serial: 0,
                item_id: Uuid::nil(),
                folder_id: Uuid::nil(),
                from_task_id: Uuid::nil(),
                last_owner_id: Uuid::nil(),
                name: b"My Prim".to_vec(),
                description: b"A nice prim".to_vec(),
                touch_name: b"Touch".to_vec(),
                sit_name: b"Sit".to_vec(),
                texture_id: Vec::new(),
            }],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 2, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Medium(9));

        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();
        assert!(try_handle_object_properties(&pkt, &event_bus));

        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ObjectPropertiesReceived(props) => {
                assert_eq!(props.object_id, object_id);
                assert_eq!(props.creator_id, creator_id);
                assert_eq!(props.owner_id, owner_id);
                assert_eq!(props.group_id, group_id);
                assert_eq!(props.creation_date, 1700000000);
                assert_eq!(props.name, "My Prim");
                assert_eq!(props.description, "A nice prim");
                assert_eq!(props.touch_name, "Touch");
                assert_eq!(props.sit_name, "Sit");
                assert_eq!(props.base_mask, perm_mask::ALL);
                assert_eq!(
                    props.next_owner_mask,
                    perm_mask::COPY | perm_mask::TRANSFER
                );
            }
            other => panic!("expected ObjectPropertiesReceived, got {:?}", other),
        }
    }

    #[test]
    fn handle_object_update_full() {
        use welkin_msg::objects::encode_object_data;

        let full_id = Uuid::from_u128(0xDEADBEEF);
        let owner_id = Uuid::from_u128(0xCAFE);
        let position = Vec3::new(50.0, 60.0, 70.0);
        let rotation = Quat::IDENTITY;

        let object_data_bytes = encode_object_data(
            position,
            Vec3::ZERO,
            Vec3::ZERO,
            rotation,
            Vec3::ZERO,
            None,
        );

        let msg = ObjectUpdate {
            region_data: object_update::RegionData {
                region_handle: 0x0003_E800_0003_E800,
                time_dilation: 65535,
            },
            object_data: vec![object_update::ObjectData {
                id: 42,
                state: 0,
                full_id,
                crc: 0,
                p_code: 47,
                material: 3,
                click_action: 0,
                scale: Vec3::new(1.0, 2.0, 3.0),
                object_data: object_data_bytes,
                parent_id: 0,
                update_flags: 0x0F,
                path_curve: 16,
                profile_curve: 1,
                path_begin: 0,
                path_end: 0,
                path_scale_x: 100,
                path_scale_y: 100,
                path_shear_x: 0,
                path_shear_y: 0,
                path_twist: 0,
                path_twist_begin: 0,
                path_radius_offset: 0,
                path_taper_x: 0,
                path_taper_y: 0,
                path_revolutions: 0,
                path_skew: 0,
                profile_begin: 0,
                profile_end: 0,
                profile_hollow: 0,
                texture_entry: Vec::new(),
                texture_anim: Vec::new(),
                name_value: b"Test NV\0".to_vec(),
                data: Vec::new(),
                text: b"Hover".to_vec(),
                text_color: vec![255, 0, 0, 0],
                media_url: Vec::new(),
                ps_block: Vec::new(),
                extra_params: Vec::new(),
                sound: Uuid::nil(),
                owner_id,
                gain: 0.0,
                flags: 0,
                radius: 0.0,
                joint_type: 0,
                joint_pivot: Vec3::ZERO,
                joint_axis_or_anchor: Vec3::ZERO,
            }],
        };

        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(12));

        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();
        assert!(try_handle_object_update(&pkt, &event_bus));

        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ObjectUpdated(obj) => {
                assert_eq!(obj.local_id, 42);
                assert_eq!(obj.full_id, full_id);
                assert_eq!(obj.pcode, 47);
                assert_eq!(obj.material, 3);
                assert_eq!(obj.owner_id, owner_id);
                assert!((obj.position.x - 50.0).abs() < 0.01);
                assert!((obj.position.y - 60.0).abs() < 0.01);
                assert!((obj.position.z - 70.0).abs() < 0.01);
                assert!((obj.scale.x - 1.0).abs() < 0.01);
                assert!((obj.scale.y - 2.0).abs() < 0.01);
                assert!((obj.scale.z - 3.0).abs() < 0.01);
                assert_eq!(obj.update_flags, 0x0F);
                assert_eq!(obj.text, "Hover");
                assert_eq!(obj.text_color, [255, 0, 0, 255]);
                assert!(!obj.is_avatar);
                assert_eq!(obj.region_handle, 0x0003_E800_0003_E800);
            }
            other => panic!("expected ObjectUpdated, got {:?}", other),
        }
    }

    #[test]
    fn handle_terse_update() {
        use welkin_msg::objects::{encode_terse, f32_to_u16};

        let terse_data = encode_terse(
            99,
            0,
            false,
            None,
            Vec3::new(100.0, 200.0, 30.0),
            Vec3::new(1.0, -1.0, 0.5),
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
        );

        let msg = ImprovedTerseObjectUpdate {
            region_data: improved_terse_object_update::RegionData {
                region_handle: 0x0003_E800_0003_E800,
                time_dilation: 65535,
            },
            object_data: vec![improved_terse_object_update::ObjectData {
                data: terse_data,
                texture_entry: Vec::new(),
            }],
        };

        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(15));

        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();
        assert!(try_handle_terse_object_update(&pkt, &event_bus));

        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ObjectUpdated(obj) => {
                assert_eq!(obj.local_id, 99);
                assert!(!obj.is_avatar);
                assert!((obj.position.x - 100.0).abs() < 0.01);
                assert!((obj.position.y - 200.0).abs() < 0.01);
                assert!((obj.position.z - 30.0).abs() < 0.01);
                assert!((obj.velocity.x - 1.0).abs() < 0.01);
                assert!((obj.velocity.y + 1.0).abs() < 0.01);
            }
            other => panic!("expected ObjectUpdated, got {:?}", other),
        }
    }

    #[test]
    fn handle_object_update_ignores_other_packets() {
        let msg = CompletePingCheck {
            ping_id: complete_ping_check::PingID { ping_id: 1 },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        let event_bus = EventBus::new(16);
        assert!(!try_handle_object_update(&pkt, &event_bus));
        assert!(!try_handle_terse_object_update(&pkt, &event_bus));
        assert!(!try_handle_kill_object(&pkt, &event_bus));
        assert!(!try_handle_object_properties(&pkt, &event_bus));
        assert!(!try_handle_object_update_compressed(&pkt, &event_bus));
    }

    #[test]
    fn perm_field_constants_are_distinct_bits() {
        let fields = [
            perm_field::BASE,
            perm_field::OWNER,
            perm_field::GROUP,
            perm_field::EVERYONE,
            perm_field::NEXT_OWNER,
        ];
        let mut combined = 0u8;
        for &f in &fields {
            assert_eq!(f.count_ones(), 1, "field 0x{:02x} not a single bit", f);
            assert_eq!(combined & f, 0, "field 0x{:02x} overlaps", f);
            combined |= f;
        }
    }

    #[test]
    fn perm_mask_values_match_protocol() {
        assert_eq!(perm_mask::TRANSFER, 1 << 13);
        assert_eq!(perm_mask::MODIFY, 1 << 14);
        assert_eq!(perm_mask::COPY, 1 << 15);
        assert_eq!(perm_mask::MOVE, 1 << 19);
        assert_eq!(perm_mask::ALL, 0x7FFF_FFFF);
    }

    #[test]
    fn derez_destination_constants() {
        // Verify distinctness of the destination byte values.
        let dests = [
            derez_destination::SAVE_INTO_AGENT_INVENTORY,
            derez_destination::ACQUIRE_TO_AGENT_INVENTORY,
            derez_destination::SAVE_INTO_TASK_INVENTORY,
            derez_destination::ATTACHMENT,
            derez_destination::TAKE_COPY,
            derez_destination::GOD_TAKE_COPY,
            derez_destination::DELETE_TO_TRASH,
            derez_destination::ATTACHMENT_TO_INV,
            derez_destination::ATTACHMENT_EXISTS,
            derez_destination::RETURN_TO_OWNER,
            derez_destination::RETURN_TO_LAST_OWNER,
        ];
        for i in 0..dests.len() {
            for j in (i + 1)..dests.len() {
                assert_ne!(dests[i], dests[j], "destination {} == {}", i, j);
            }
        }
    }

    #[test]
    fn parse_terse_data_too_short_returns_none() {
        assert!(parse_terse_data(&[]).is_none());
        assert!(parse_terse_data(&[0; 5]).is_none());
        // 6 bytes header but not enough for body
        assert!(parse_terse_data(&[1, 0, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn parse_object_data_too_short_returns_zeros() {
        let (pos, vel, acc, rot, ang) = parse_object_data(&[], false);
        assert_eq!(pos, Vec3::ZERO);
        assert_eq!(vel, Vec3::ZERO);
        assert_eq!(acc, Vec3::ZERO);
        assert_eq!(rot, Quat::IDENTITY);
        assert_eq!(ang, Vec3::ZERO);
    }
}
