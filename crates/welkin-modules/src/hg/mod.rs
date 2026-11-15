// SPDX-License-Identifier: LGPL-3.0-or-later

//! Hypergrid region modules for cross-grid teleportation, messaging,
//! inventory access, and identity management.
//!
//! These modules extend the base region modules with Hypergrid-aware
//! behaviour, routing operations to local or remote services depending
//! on whether the agent is a local or foreign grid user.

pub mod connector_in;
pub mod entity_transfer;
pub mod friends;
pub mod inventory_access;
pub mod lure;
pub mod messaging;
pub mod user_management;
pub mod world_map;

use uuid::Uuid;

// ---------------------------------------------------------------------------
// UUI — Universal User Identifier
// ---------------------------------------------------------------------------

/// A parsed Universal User Identifier (UUI).
///
/// Format: `"uuid;home_uri;FirstName LastName"`
///
/// Used throughout Hypergrid to identify users across grid boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniversalUserIdentifier {
    pub user_id: Uuid,
    pub home_uri: String,
    pub first_name: String,
    pub last_name: String,
}

impl UniversalUserIdentifier {
    /// Parse a UUI string.
    ///
    /// Accepts the full format `"uuid;home_uri;FirstName LastName"` as well
    /// as the short form `"uuid;home_uri"` (names default to "Unknown User").
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(3, ';').collect();
        if parts.len() < 2 {
            return None;
        }

        let user_id = Uuid::parse_str(parts[0].trim()).ok()?;
        let home_uri = parts[1].trim().to_string();

        let (first_name, last_name) = if parts.len() >= 3 {
            let name = parts[2].trim();
            let name_parts: Vec<&str> = name.splitn(2, ' ').collect();
            if name_parts.len() == 2 {
                (name_parts[0].to_string(), name_parts[1].to_string())
            } else {
                (name.to_string(), String::new())
            }
        } else {
            ("Unknown".to_string(), "User".to_string())
        };

        Some(Self {
            user_id,
            home_uri,
            first_name,
            last_name,
        })
    }

    /// Format this UUI as a string.
    pub fn to_uui_string(&self) -> String {
        format!(
            "{};{};{} {}",
            self.user_id, self.home_uri, self.first_name, self.last_name
        )
    }
}

impl std::fmt::Display for UniversalUserIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{};{};{} {}",
            self.user_id, self.home_uri, self.first_name, self.last_name
        )
    }
}

// ---------------------------------------------------------------------------
// Grid URI helpers
// ---------------------------------------------------------------------------

/// Normalise a grid URI for comparison: lowercase, strip trailing slash.
pub fn normalise_grid_uri(uri: &str) -> String {
    let mut s = uri.trim().to_lowercase();
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// Check whether two grid URIs refer to the same grid.
pub fn is_same_grid(uri_a: &str, uri_b: &str) -> bool {
    normalise_grid_uri(uri_a) == normalise_grid_uri(uri_b)
}

/// Determine whether `user_home_uri` is the local grid.
pub fn is_local_grid_user(user_home_uri: &str, local_grid_uri: &str) -> bool {
    if user_home_uri.is_empty() {
        return true; // no home URI means local
    }
    is_same_grid(user_home_uri, local_grid_uri)
}

/// Extract the home URI from a service URL map (the "HomeURI" key).
pub fn get_home_uri(service_urls: &std::collections::HashMap<String, String>) -> Option<&str> {
    service_urls.get("HomeURI").map(|s| s.as_str())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_uui() {
        let uui = UniversalUserIdentifier::parse(
            "550e8400-e29b-41d4-a716-446655440000;http://grid.example.com:8002;John Doe",
        )
        .unwrap();
        assert_eq!(
            uui.user_id,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(uui.home_uri, "http://grid.example.com:8002");
        assert_eq!(uui.first_name, "John");
        assert_eq!(uui.last_name, "Doe");
    }

    #[test]
    fn parse_short_uui_no_name() {
        let uui = UniversalUserIdentifier::parse(
            "550e8400-e29b-41d4-a716-446655440000;http://grid.example.com:8002",
        )
        .unwrap();
        assert_eq!(uui.first_name, "Unknown");
        assert_eq!(uui.last_name, "User");
    }

    #[test]
    fn parse_invalid_uuid_returns_none() {
        assert!(UniversalUserIdentifier::parse("not-a-uuid;http://grid.example.com").is_none());
    }

    #[test]
    fn parse_no_semicolon_returns_none() {
        assert!(UniversalUserIdentifier::parse("550e8400-e29b-41d4-a716-446655440000").is_none());
    }

    #[test]
    fn uui_roundtrip() {
        let original =
            "550e8400-e29b-41d4-a716-446655440000;http://grid.example.com:8002;Jane Smith";
        let parsed = UniversalUserIdentifier::parse(original).unwrap();
        assert_eq!(parsed.to_uui_string(), original);
    }

    #[test]
    fn normalise_grid_uri_strips_trailing_slash() {
        assert_eq!(
            normalise_grid_uri("http://Grid.Example.Com:8002/"),
            "http://grid.example.com:8002"
        );
    }

    #[test]
    fn is_same_grid_case_insensitive() {
        assert!(is_same_grid(
            "http://grid.example.com:8002/",
            "http://GRID.EXAMPLE.COM:8002"
        ));
    }

    #[test]
    fn is_same_grid_different_grids() {
        assert!(!is_same_grid(
            "http://grid-a.example.com:8002",
            "http://grid-b.example.com:8002"
        ));
    }

    #[test]
    fn is_local_grid_user_empty_uri() {
        assert!(is_local_grid_user("", "http://mygrid.com:8002"));
    }

    #[test]
    fn is_local_grid_user_same() {
        assert!(is_local_grid_user(
            "http://mygrid.com:8002",
            "http://mygrid.com:8002"
        ));
    }

    #[test]
    fn is_local_grid_user_foreign() {
        assert!(!is_local_grid_user(
            "http://othergrid.com:8002",
            "http://mygrid.com:8002"
        ));
    }

    #[test]
    fn uui_display() {
        let uui = UniversalUserIdentifier {
            user_id: Uuid::nil(),
            home_uri: "http://example.com".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
        };
        let s = format!("{}", uui);
        assert!(s.contains("http://example.com"));
        assert!(s.contains("Test User"));
    }
}
