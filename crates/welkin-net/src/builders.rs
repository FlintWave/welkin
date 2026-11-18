// SPDX-License-Identifier: LGPL-3.0-or-later

//! Pure packet-builder helpers for high-level region messages.

use uuid::Uuid;

use crate::circuit::Circuit;
use welkin_msg::msgs;

/// Encode an `UpdateSimulator` packet (low, trusted, unencoded) on the
/// supplied circuit and return the framed bytes together with their sequence
/// number.
///
/// `sim_name` may be any string-like value; it is transmitted NUL-terminated
/// in the `SimulatorInfo` block.
pub fn build_update_simulator(
    region_id: Uuid,
    sim_name: &str,
    estate_id: u32,
    sim_access: u8,
    circuit: &mut Circuit,
) -> (Vec<u8>, u32) {
    let msg = msgs::UpdateSimulator {
        simulator_info: msgs::update_simulator::SimulatorInfo {
            region_id,
            sim_name: sim_name.as_bytes().to_vec(),
            estate_id,
            sim_access,
        },
    };
    circuit.send_msg(&msg, true)
}

/// Alias for [`build_update_simulator`].
pub fn encode_update_simulator(
    region_id: Uuid,
    sim_name: &str,
    estate_id: u32,
    sim_access: u8,
    circuit: &mut Circuit,
) -> (Vec<u8>, u32) {
    build_update_simulator(region_id, sim_name, estate_id, sim_access, circuit)
}

/// Alias for [`build_update_simulator`].
pub fn build_update_simulator_packet(
    region_id: Uuid,
    sim_name: &str,
    estate_id: u32,
    sim_access: u8,
    circuit: &mut Circuit,
) -> (Vec<u8>, u32) {
    build_update_simulator(region_id, sim_name, estate_id, sim_access, circuit)
}

/// Alias for [`build_update_simulator`].
pub fn encode_update_simulator_packet(
    region_id: Uuid,
    sim_name: &str,
    estate_id: u32,
    sim_access: u8,
    circuit: &mut Circuit,
) -> (Vec<u8>, u32) {
    build_update_simulator(region_id, sim_name, estate_id, sim_access, circuit)
}
