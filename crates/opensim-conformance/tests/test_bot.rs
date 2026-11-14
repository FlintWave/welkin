// SPDX-License-Identifier: GPL-3.0-or-later

use aurora_llsd::{Llsd, llsd_map};
use aurora_msg::{parse_packet, Msg};
use aurora_net::circuit::{Circuit, Resend};
use aurora_net::stats::NetStats;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
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

#[tokio::test]
#[ignore]
async fn test_bot_login_handshake() {
    let login_uri = std::env::var("OPENSIM_LOGIN_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:9100".to_string());

    let params = llsd_map! {
        "first" => "Foo",
        "last" => "Bar",
        "passwd" => "$1$5f4dcc3b5aa765d61d8327deb882cf99",
        "start" => "last",
        "channel" => "OpenSim Rust Conformance",
        "version" => "0.1.0",
        "agree_to_tos" => Llsd::Boolean(true),
        "read_critical" => Llsd::Boolean(true),
        "last_exec_event" => Llsd::Integer(0),
        "options" => Llsd::Array(vec![])
    };

    let xml_body = opensim_llsd::xmlrpc::build_call(
        "login_to_simulator",
        std::slice::from_ref(&params),
    );

    let client = hyper_util::client::legacy::Client::builder(
        hyper_util::rt::TokioExecutor::new(),
    )
    .build_http();
    let req = hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri(&login_uri)
        .header("Content-Type", "text/xml")
        .body(Full::new(Bytes::from(xml_body)))
        .unwrap();
    let resp = client.request(req).await.expect("login HTTP failed");
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let login_result =
        opensim_llsd::xmlrpc::parse_response(&body_bytes).expect("login failed");

    assert_eq!(login_result["login"].as_str(), "true", "login rejected");

    let circuit_code = login_result["circuit_code"].as_i32() as u32;
    let agent_id: Uuid = login_result["agent_id"].as_str().parse().unwrap();
    let session_id: Uuid = login_result["session_id"].as_str().parse().unwrap();
    let sim_ip = login_result["sim_ip"].as_str();
    let sim_port = login_result["sim_port"].as_i32() as u16;

    let sim_addr: SocketAddr = format!("{sim_ip}:{sim_port}").parse().unwrap();

    let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let stats = NetStats::default();
    let mut circuit = Circuit::new(sim_addr);

    let use_cc = aurora_msg::msgs::UseCircuitCode {
        circuit_code: aurora_msg::msgs::use_circuit_code::CircuitCode {
            code: circuit_code,
            session_id,
            id: agent_id,
        },
    };

    circuit.send_reliable_with(&sock, &stats, &use_cc, Resend::LOGIN_USE_CIRCUIT_CODE);

    let mut recv_buf = vec![0u8; 4096];
    let got_complete = tokio::time::timeout(Duration::from_secs(15), async {
        let mut sent_complete = false;
        let mut got_complete = false;

        loop {
            match tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut recv_buf))
                .await
            {
                Ok(Ok((n, _from))) => {
                    if let Ok(pkt) = parse_packet(&recv_buf[..n]) {
                        let new = circuit.on_receive(pkt.flags, pkt.sequence, &pkt.acks);
                        if new {
                            circuit.accept(pkt.flags, pkt.sequence);
                        }

                        if pkt.id
                            == <aurora_msg::msgs::AgentMovementComplete as Msg>::ID
                        {
                            got_complete = true;
                            break;
                        }
                    }

                    if !sent_complete && circuit.unacked_count() == 0 {
                        sent_complete = true;
                        let complete_msg = aurora_msg::msgs::CompleteAgentMovement {
                            agent_data:
                                aurora_msg::msgs::complete_agent_movement::AgentData {
                                    agent_id,
                                    session_id,
                                    circuit_code,
                                },
                        };
                        circuit.send_reliable_with(
                            &sock,
                            &stats,
                            &complete_msg,
                            Resend::DEFAULT,
                        );
                    }
                }
                _ => break,
            }
        }

        got_complete
    })
    .await
    .expect("receive loop timed out");

    assert!(got_complete, "AgentMovementComplete was not received");
}
