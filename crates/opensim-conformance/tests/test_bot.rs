// SPDX-License-Identifier: GPL-3.0-or-later

use aurora_msg::parse_packet;
use aurora_net::circuit::{Circuit, Resend};
use aurora_net::stats::NetStats;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn test_bot_circuit_handshake() {
    let server_addr: SocketAddr = std::env::var("OPENSIM_SIM_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9000".to_string())
        .parse()
        .expect("invalid server address");

    let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let stats = NetStats::default();
    let mut circuit = Circuit::new(server_addr);

    let msg = aurora_msg::msgs::UseCircuitCode {
        circuit_code: aurora_msg::msgs::use_circuit_code::CircuitCode {
            code: 12345,
            session_id: Uuid::new_v4(),
            id: Uuid::new_v4(),
        },
    };

    circuit.send_reliable_with(&sock, &stats, &msg, Resend::LOGIN_USE_CIRCUIT_CODE);

    let mut recv_buf = vec![0u8; 4096];
    loop {
        match tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut recv_buf)).await {
            Ok(Ok((n, _from))) => {
                if let Ok(pkt) = parse_packet(&recv_buf[..n]) {
                    let new = circuit.on_receive(pkt.flags, pkt.sequence, &pkt.acks);
                    if new {
                        circuit.accept(pkt.flags, pkt.sequence);
                    }
                }
                if circuit.unacked_count() == 0 {
                    break;
                }
            }
            _ => break,
        }
    }

    assert_eq!(
        circuit.unacked_count(),
        0,
        "UseCircuitCode was not acknowledged"
    );
}
