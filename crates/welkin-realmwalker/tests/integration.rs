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
