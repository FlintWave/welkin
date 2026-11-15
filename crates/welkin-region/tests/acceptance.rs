// SPDX-License-Identifier: GPL-3.0-or-later

//! Region server acceptance tests.
//!
//! These verify that the server infrastructure can accept viewer-like
//! connections without running a real Aurora Viewer.  Every socket binds
//! to port 0 so tests never collide with each other or with a running
//! region simulator.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use welkin_caps::handlers::{
    AssetServiceState, BakeUploadState, InventoryProxyState, ScriptLoadState, cap_routes,
};
use welkin_caps::server::CapabilityServer;
use welkin_msg::{Msg, build_packet};
use welkin_net::server::LLUDPServer;

// ---------------------------------------------------------------------------
// 1. UDP listener — UseCircuitCode round-trip
// ---------------------------------------------------------------------------

/// Send a UseCircuitCode UDP packet from a test client and verify the
/// server's receive channel picks it up and creates a client session.
#[tokio::test]
async fn udp_use_circuit_code_round_trip() {
    let server = LLUDPServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    // Clone the client map handle before moving `server` into the task.
    let clients_handle = server.clients().clone();

    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let cancel_run = cancel.clone();

    let run_handle = tokio::spawn(async move {
        server.run(msg_tx, cancel_run).await;
    });

    // "Viewer" client socket.
    let viewer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let viewer_addr = viewer.local_addr().unwrap();

    // Build a UseCircuitCode packet.
    let mut ucc = welkin_msg::msgs::UseCircuitCode::default();
    ucc.circuit_code.code = 7777;
    ucc.circuit_code.id = Uuid::from_u128(0xCAFE);
    ucc.circuit_code.session_id = Uuid::from_u128(0xBEEF);
    let mut buf = Vec::new();
    build_packet(&ucc, 1, true, false, &[], &mut buf);

    viewer.send_to(&buf, server_addr).await.unwrap();

    // Wait for the packet to arrive on the receive channel.
    let (src, pkt) = tokio::time::timeout(Duration::from_secs(3), msg_rx.recv())
        .await
        .expect("timed out waiting for packet")
        .expect("channel closed unexpectedly");

    assert_eq!(src, viewer_addr);
    assert_eq!(pkt.id, welkin_msg::msgs::UseCircuitCode::ID);

    // The server should have created a client session.
    let clients = clients_handle.read().await;
    let session_arc = clients
        .get(&viewer_addr)
        .expect("session not found for viewer address");
    let session = session_arc.lock().await;
    assert_eq!(session.circuit_code, 7777);
    assert_eq!(session.agent_id, Uuid::from_u128(0xCAFE));
    assert_eq!(session.session_id, Uuid::from_u128(0xBEEF));
    drop(session);
    drop(clients);

    cancel.cancel();
    let _ = run_handle.await;
}

fn test_cap_state() -> (AssetServiceState, InventoryProxyState, BakeUploadState) {
    let client = reqwest::Client::new();
    (
        AssetServiceState { client: client.clone(), asset_url: "http://localhost:99999".into() },
        InventoryProxyState { client: client.clone(), inventory_url: "http://localhost:99999".into() },
        BakeUploadState {
            client,
            asset_url: "http://localhost:99999".into(),
            caps_url: "http://localhost:99999".into(),
            pending: Arc::new(RwLock::new(HashMap::new())),
        },
    )
}

// ---------------------------------------------------------------------------
// 2. Caps HTTP — SimulatorFeatures & friends over a real TCP listener
// ---------------------------------------------------------------------------

/// Build the caps router with the static handler routes, bind it to an
/// ephemeral TCP port, and issue real HTTP requests to confirm status codes.
#[tokio::test]
async fn caps_http_simulator_features() {
    // Build the same router the region binary uses.
    let cap_server = CapabilityServer::new();
    let mut router: Router = cap_server.into_router();
    let (a, i, b) = test_cap_state();
    let (sl_tx, _sl_rx) = std::sync::mpsc::channel();
    let sl = ScriptLoadState { sender: sl_tx };
    for (name, method_router) in cap_routes(a, i, b, sl) {
        router = router.route(&format!("/{name}"), method_router);
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let cancel = CancellationToken::new();
    let cancel_server = cancel.clone();
    let http_handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                cancel_server.cancelled().await;
            })
            .await
            .ok();
    });

    // Give the server a moment to start accepting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let base = format!("http://{local_addr}");

    // SimulatorFeatures — should return 200 with LLSD body.
    let resp = http_get(&format!("{base}/SimulatorFeatures")).await;
    assert_eq!(resp.0, 200, "SimulatorFeatures should return 200");
    assert!(
        resp.1.contains("MeshRezEnabled"),
        "body should contain MeshRezEnabled"
    );

    // RegionInfo — 200
    let resp = http_get(&format!("{base}/RegionInfo")).await;
    assert_eq!(resp.0, 200, "RegionInfo should return 200");

    // GetTexture without a texture_id — 400 (missing required query param)
    let resp = http_get(&format!("{base}/GetTexture")).await;
    assert!(resp.0 == 400 || resp.0 == 404, "GetTexture with no id should return 400 or 404");

    // GetMesh without a mesh_id — 400 (missing required query param)
    let resp = http_get(&format!("{base}/GetMesh")).await;
    assert!(resp.0 == 400 || resp.0 == 404, "GetMesh with no id should return 400 or 404");

    // AgentPreferences — 200
    let resp = http_get(&format!("{base}/AgentPreferences")).await;
    assert_eq!(resp.0, 200, "AgentPreferences should return 200");

    // MapLayer — 200
    let resp = http_get(&format!("{base}/MapLayer")).await;
    assert_eq!(resp.0, 200, "MapLayer should return 200");

    // Non-existent route — 404
    let resp = http_get(&format!("{base}/NoSuchCap")).await;
    assert_eq!(resp.0, 404, "unknown route should return 404");

    cancel.cancel();
    let _ = http_handle.await;
}

/// Minimal HTTP/1.1 GET using a raw TCP stream — avoids adding extra deps.
async fn http_get(url: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    // Parse "http://host:port/path"
    let without_scheme = url.strip_prefix("http://").expect("expected http:// URL");
    let (authority, path) = match without_scheme.find('/') {
        Some(i) => (&without_scheme[..i], &without_scheme[i..]),
        None => (without_scheme, "/"),
    };

    let mut stream = TcpStream::connect(authority).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let text = String::from_utf8_lossy(&response);

    // Parse status code from "HTTP/1.1 200 OK\r\n..."
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    // Body is after the first blank line.
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();

    (status, body)
}

// ---------------------------------------------------------------------------
// 3. Shutdown coordination — cancel token stops both UDP and HTTP
// ---------------------------------------------------------------------------

/// Start both UDP and HTTP servers with a shared CancellationToken,
/// cancel it, and verify both tasks complete cleanly without hanging.
#[tokio::test]
async fn shutdown_coordination() {
    // -- UDP server --
    let udp_server = LLUDPServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let (msg_tx, _msg_rx) = mpsc::unbounded_channel();

    // -- HTTP server --
    let cap_server = CapabilityServer::new();
    let mut caps_router: Router = cap_server.into_router();
    let (a, i, b) = test_cap_state();
    let (sl_tx, _sl_rx) = std::sync::mpsc::channel();
    let sl = ScriptLoadState { sender: sl_tx };
    for (name, method_router) in cap_routes(a, i, b, sl) {
        caps_router = caps_router.route(&format!("/{name}"), method_router);
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();

    // Shared cancellation.
    let cancel = CancellationToken::new();

    let udp_cancel = cancel.clone();
    let udp_handle = tokio::spawn(async move {
        udp_server.run(msg_tx, udp_cancel).await;
    });

    let caps_cancel = cancel.clone();
    let http_handle = tokio::spawn(async move {
        axum::serve(listener, caps_router)
            .with_graceful_shutdown(async move {
                caps_cancel.cancelled().await;
            })
            .await
            .ok();
    });

    // Let both servers settle.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Cancel everything.
    cancel.cancel();

    // Both tasks must finish within a reasonable deadline.
    let deadline = Duration::from_secs(5);

    tokio::time::timeout(deadline, udp_handle)
        .await
        .expect("UDP server did not shut down in time")
        .expect("UDP task panicked");

    tokio::time::timeout(deadline, http_handle)
        .await
        .expect("HTTP server did not shut down in time")
        .expect("HTTP task panicked");
}
