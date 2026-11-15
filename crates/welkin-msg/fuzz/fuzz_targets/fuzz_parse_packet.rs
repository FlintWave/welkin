#![no_main]
// SPDX-License-Identifier: LGPL-3.0-or-later

use libfuzzer_sys::fuzz_target;
use aurora_msg::{parse_packet, Msg};

fuzz_target!(|data: &[u8]| {
    if let Ok(pkt) = parse_packet(data) {
        let _ = pkt.acks.len();
        let _ = pkt.flags;
        let _ = pkt.sequence;
        let mut r = aurora_msg::Reader::new(&pkt.body);
        let _ = aurora_msg::msgs::UseCircuitCode::decode_body(&mut r);
        let mut r = aurora_msg::Reader::new(&pkt.body);
        let _ = aurora_msg::msgs::CompleteAgentMovement::decode_body(&mut r);
        let mut r = aurora_msg::Reader::new(&pkt.body);
        let _ = aurora_msg::msgs::AgentMovementComplete::decode_body(&mut r);
    }
});
