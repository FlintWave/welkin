// SPDX-License-Identifier: GPL-3.0-or-later

use aurora_msg::packet::build_raw;
use aurora_msg::{parse_packet, PacketFlags};

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for i in (0..bytes.len()).step_by(2) {
        let high = hex_val(bytes[i])?;
        let low = hex_val(bytes[i + 1])?;
        out.push((high << 4) | low);
    }
    Some(out)
}

fn extract_json_string<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let key_pattern = format!("\"{}\"", key);
    let idx = line.find(&key_pattern)?;
    let rest = &line[idx + key_pattern.len()..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[test]
fn lludp_roundtrip() {
    let path = match std::env::var("LLUDP_RECORDING") {
        Ok(p) => p,
        Err(_) => {
            println!("set LLUDP_RECORDING=path/to/recording.jsonl");
            return;
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            println!("set LLUDP_RECORDING=path/to/recording.jsonl");
            return;
        }
    };

    let mut total: usize = 0;
    let mut success: usize = 0;
    let mut failures: usize = 0;
    let mut decode_errors: usize = 0;
    let mut re_encode_errors: usize = 0;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let hex = match extract_json_string(line, "data") {
            Some(s) => s,
            None => continue,
        };
        let bytes = match hex_decode(hex) {
            Some(b) => b,
            None => continue,
        };

        total += 1;

        let packet = match parse_packet(&bytes) {
            Ok(p) => p,
            Err(_) => {
                decode_errors += 1;
                continue;
            }
        };

        if packet.body.is_empty() {
            continue;
        }

        let zerocoded = packet.flags & PacketFlags::ZEROCODED != 0;
        let reliable = packet.flags & PacketFlags::RELIABLE != 0;
        let resent = packet.flags & PacketFlags::RESENT != 0;

        let mut body_with_id = Vec::new();
        packet.id.encode(&mut body_with_id);
        body_with_id.extend_from_slice(&packet.body);

        let mut out = Vec::new();
        build_raw(
            &body_with_id,
            zerocoded,
            packet.sequence,
            reliable,
            resent,
            &packet.acks,
            &mut out,
        );

        let reparsed = match parse_packet(&out) {
            Ok(p) => p,
            Err(_) => {
                re_encode_errors += 1;
                continue;
            }
        };

        if reparsed.id == packet.id
            && reparsed.body == packet.body
            && reparsed.sequence == packet.sequence
            && reparsed.acks == packet.acks
        {
            success += 1;
        } else {
            failures += 1;
            println!("mismatch seq={} msg_id={:?}", packet.sequence, packet.id);
        }
    }

    println!(
        "lludp roundtrip: total={} success={} failures={} decode_errors={} re_encode_errors={}",
        total, success, failures, decode_errors, re_encode_errors
    );

    assert_eq!(failures, 0);
    assert_eq!(re_encode_errors, 0);
}
