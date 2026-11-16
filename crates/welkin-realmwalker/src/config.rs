// SPDX-License-Identifier: LGPL-3.0-or-later
use std::time::Duration;

/// Configuration for connecting to an OpenSimulator grid.
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// XMLRPC login URI (e.g. `http://grid.example.com:8002`).
    pub login_uri: String,
    /// Avatar first name.
    pub first_name: String,
    /// Avatar last name.
    pub last_name: String,
    /// Plain-text password (hashed with MD5 before transmission).
    pub password: String,
    /// Where to place the avatar after login.
    pub start: StartLocation,
    /// TCP connect timeout for the login HTTP request.
    pub connect_timeout: Duration,
    /// Read timeout for the login HTTP response.
    pub read_timeout: Duration,
    /// Capacity of the internal event broadcast channel.
    pub event_channel_capacity: usize,
    /// Whether to automatically reconnect on disconnect.
    pub auto_reconnect: bool,
    /// Maximum number of automatic reconnect attempts before giving up.
    pub max_reconnect_attempts: u32,
    /// Delay between reconnect attempts.
    pub reconnect_delay: Duration,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            login_uri: String::new(),
            first_name: String::new(),
            last_name: String::new(),
            password: String::new(),
            start: StartLocation::Last,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(30),
            event_channel_capacity: 256,
            auto_reconnect: false,
            max_reconnect_attempts: 3,
            reconnect_delay: Duration::from_secs(5),
        }
    }
}

impl GridConfig {
    /// Create a minimal config with login credentials.
    pub fn new(login_uri: impl Into<String>, first: impl Into<String>, last: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            login_uri: login_uri.into(),
            first_name: first.into(),
            last_name: last.into(),
            password: password.into(),
            ..Default::default()
        }
    }

    /// Return the password hashed as `$1$` + hex MD5, the format OpenSim XMLRPC login expects.
    pub fn hashed_password(&self) -> String {
        use md5::{Digest, Md5};
        let mut hasher = Md5::new();
        hasher.update(self.password.as_bytes());
        let hash = hasher.finalize();
        format!("$1${:x}", hash)
    }
}

/// Where to place the avatar after login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartLocation {
    /// The avatar's stored home location.
    Home,
    /// The avatar's last known location.
    Last,
    /// A named region at its default landing point (128, 128, 0).
    Region(String),
    /// A named region at a specific position.
    RegionWithPosition(String, u32, u32, u32),
}

impl StartLocation {
    /// Encode as the string the XMLRPC login `start` parameter expects.
    ///
    /// - `Home` -> `"home"`
    /// - `Last` -> `"last"`
    /// - `Region("Foo")` -> `"uri:Foo&128&128&0"`
    /// - `RegionWithPosition("Foo", 10, 20, 30)` -> `"uri:Foo&10&20&30"`
    pub fn to_login_string(&self) -> String {
        match self {
            Self::Home => "home".to_owned(),
            Self::Last => "last".to_owned(),
            Self::Region(name) => format!("uri:{name}&128&128&0"),
            Self::RegionWithPosition(name, x, y, z) => format!("uri:{name}&{x}&{y}&{z}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = GridConfig::default();
        assert!(cfg.login_uri.is_empty());
        assert_eq!(cfg.connect_timeout, Duration::from_secs(30));
        assert_eq!(cfg.read_timeout, Duration::from_secs(30));
        assert_eq!(cfg.event_channel_capacity, 256);
        assert!(!cfg.auto_reconnect);
        assert_eq!(cfg.max_reconnect_attempts, 3);
        assert_eq!(cfg.reconnect_delay, Duration::from_secs(5));
        assert_eq!(cfg.start, StartLocation::Last);
    }

    #[test]
    fn new_sets_credentials() {
        let cfg = GridConfig::new("http://localhost:8002", "Test", "User", "secret");
        assert_eq!(cfg.login_uri, "http://localhost:8002");
        assert_eq!(cfg.first_name, "Test");
        assert_eq!(cfg.last_name, "User");
        assert_eq!(cfg.password, "secret");
    }

    #[test]
    fn hashed_password_format() {
        let cfg = GridConfig::new("", "", "", "secret");
        let hashed = cfg.hashed_password();
        assert!(hashed.starts_with("$1$"), "hash should start with $1$ prefix");
        // MD5 of "secret" is 5ebe2294ecd0e0f08eab7690d2a6ee69
        assert_eq!(hashed, "$1$5ebe2294ecd0e0f08eab7690d2a6ee69");
    }

    #[test]
    fn start_location_strings() {
        assert_eq!(StartLocation::Home.to_login_string(), "home");
        assert_eq!(StartLocation::Last.to_login_string(), "last");
        assert_eq!(
            StartLocation::Region("My Region".to_owned()).to_login_string(),
            "uri:My Region&128&128&0"
        );
        assert_eq!(
            StartLocation::RegionWithPosition("Foo".to_owned(), 10, 20, 30).to_login_string(),
            "uri:Foo&10&20&30"
        );
    }
}
