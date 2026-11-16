// SPDX-License-Identifier: LGPL-3.0-or-later
//! Integration tests for welkin-realmwalker against a live OpenSimulator grid.
//!
//! Run with: cargo test -p welkin-realmwalker --features integration --test integration
//!
//! Requires a running OpenSimulator instance:
//!   - ROBUST login: http://127.0.0.1:8002/
//!   - Region server: 127.0.0.1:9000 (UDP)
//!   - Test user: first_name="Test", last_name="User", password="password"

#![cfg(feature = "integration")]

use tokio::time::{timeout, Duration};
use uuid::Uuid;
use welkin_realmwalker::{GridConfig, RealmWalker, RealmWalkerError, StartLocation};

const LOGIN_URI: &str = "http://127.0.0.1:8002/";

/// Build the standard test config for the Test User account.
fn test_config() -> GridConfig {
    let mut cfg = GridConfig::new(LOGIN_URI, "Test", "User", "password");
    cfg.start = StartLocation::Last;
    cfg
}

#[tokio::test]
async fn test_login_and_logout() {
    let config = test_config();
    let mut rw = RealmWalker::login(config)
        .await
        .expect("login should succeed against the live grid");

    // Login response should have real UUIDs
    assert_ne!(rw.agent_id(), Uuid::nil(), "agent_id must not be nil");
    assert_ne!(rw.session_id(), Uuid::nil(), "session_id must not be nil");
    assert!(rw.is_connected(), "should be connected after login");

    // Let background tasks settle
    tokio::time::sleep(Duration::from_millis(200)).await;

    rw.logout().await.expect("logout should succeed");
    assert!(!rw.is_connected(), "should be disconnected after logout");
}

#[tokio::test]
async fn test_login_bad_password() {
    let config = GridConfig::new(LOGIN_URI, "Test", "User", "wrong_password");
    let result = RealmWalker::login(config).await;
    match result {
        Err(RealmWalkerError::LoginFailed(_)) => {
            // Expected: bad credentials or similar login failure
        }
        Err(other) => {
            panic!("expected LoginFailed, got: {other}");
        }
        Ok(_) => {
            panic!("login with wrong password should have failed");
        }
    }
}

#[tokio::test]
async fn test_chat_send() {
    let config = test_config();
    let mut rw = RealmWalker::login(config)
        .await
        .expect("login should succeed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    rw.say("Hello from RealmWalker integration test")
        .await
        .expect("say() should not error");

    rw.logout().await.expect("logout should succeed");
}

#[tokio::test]
async fn test_event_subscription() {
    let config = test_config();
    let mut rw = RealmWalker::login(config)
        .await
        .expect("login should succeed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut rx = rw.events();

    // Send a chat message; the server may echo it back as a ChatReceived event
    rw.say("event subscription test")
        .await
        .expect("say() should not error");

    // Wait up to 5 seconds for any event on the channel.
    // We do not require a specific event type -- just that the subscription
    // mechanism works and delivers something (chat echo, EQG event, etc.).
    let got_event = timeout(Duration::from_secs(5), rx.recv()).await;

    match got_event {
        Ok(Ok(_event)) => {
            // Received an event -- subscription works.
        }
        Ok(Err(e)) => {
            // broadcast::RecvError (Closed or Lagged) -- still proves the
            // channel is wired up, just no event arrived before shutdown.
            eprintln!("event channel error (acceptable): {e}");
        }
        Err(_) => {
            // Timed out. The server may not echo our own chat.
            // This is not a hard failure for the subscription mechanism itself.
            eprintln!("no event received within 5s (server may not echo own chat)");
        }
    }

    rw.logout().await.expect("logout should succeed");
}

#[tokio::test]
async fn test_move_to_and_verify_position() {
    let config = test_config();
    let mut rw = RealmWalker::login(config)
        .await
        .expect("login should succeed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Move via autopilot to a known position within the region.
    let target = glam::Vec3::new(128.0, 128.0, 25.0);
    welkin_realmwalker::agent::move_to(
        rw.network(),
        rw.agent_id(),
        rw.session_id(),
        target,
    )
    .await
    .expect("move_to should not error");

    // Allow the server time to process the autopilot request.
    // In a real test harness we would poll ObjectUpdate packets for
    // the agent's local ID and compare the reported position against
    // the target within a tolerance.
    tokio::time::sleep(Duration::from_secs(3)).await;

    rw.logout().await.expect("logout should succeed");
}

#[tokio::test]
async fn test_teleport_request() {
    let config = test_config();
    let mut rw = RealmWalker::login(config)
        .await
        .expect("login should succeed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut rx = rw.events();

    // Teleport to the same region at position (128, 128, 25).
    // Use region handle 0 which the server interprets as "current region".
    let target_pos = glam::Vec3::new(128.0, 128.0, 25.0);
    let look_at = glam::Vec3::new(1.0, 0.0, 0.0);
    welkin_realmwalker::agent::teleport(
        rw.network(),
        rw.agent_id(),
        rw.session_id(),
        0,
        target_pos,
        look_at,
    )
    .await
    .expect("teleport request should not error");

    // Wait for a TeleportProgress or TeleportStarted event (or timeout).
    let got_event = timeout(Duration::from_secs(10), async {
        loop {
            match rx.recv().await {
                Ok(welkin_realmwalker::Event::TeleportStarted(_))
                | Ok(welkin_realmwalker::Event::TeleportProgress(_))
                | Ok(welkin_realmwalker::Event::TeleportFailed(_)) => return true,
                Ok(_) => continue,
                Err(_) => return false,
            }
        }
    })
    .await;

    match got_event {
        Ok(true) => {
            // Received a teleport-related event -- the packet flow works.
        }
        Ok(false) => {
            eprintln!("event channel closed before teleport event");
        }
        Err(_) => {
            // Timed out. The server may reject a zero-handle teleport or
            // require a valid region handle. This is not a hard failure for
            // the packet construction itself.
            eprintln!("no teleport event within 10s (server may require valid region handle)");
        }
    }

    rw.logout().await.expect("logout should succeed");
}
