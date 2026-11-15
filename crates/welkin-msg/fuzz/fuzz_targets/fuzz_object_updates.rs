#![no_main]
// SPDX-License-Identifier: LGPL-3.0-or-later

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }
    let flags = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let payload = &data[4..];

    let _ = aurora_net::objects::parse_compressed(payload, flags);

    let terse_data = aurora_msg::msgs::improved_terse_object_update::ObjectData {
        data: payload.to_vec(),
        texture_entry: Vec::new(),
    };
    let _ = aurora_net::objects::parse_terse(&terse_data);
});
