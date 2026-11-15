// SPDX-License-Identifier: LGPL-3.0-or-later

//! Hypergrid linker -- manages hyperlinks between grids.
//!
//! Port of `OpenSim.Services.GridService.HypergridLinker`.
//!
//! The linker stores linked-region entries that act as placeholders in the
//! local grid database, allowing the map and teleport systems to resolve
//! cross-grid destinations.

use std::sync::RwLock;

use uuid::Uuid;

use crate::hypergrid::*;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A linked region entry stored in the grid database.
#[derive(Debug, Clone)]
pub struct HyperLink {
    pub region_id: Uuid,
    pub region_name: String,
    pub server_uri: String,
    pub region_loc_x: u32,
    pub region_loc_y: u32,
    pub size_x: u32,
    pub size_y: u32,
    pub owner_id: Uuid,
    pub scope_id: Uuid,
}

/// Configuration for the [`HypergridLinker`].
#[derive(Debug, Clone)]
pub struct HypergridLinkerConfig {
    pub scope_id: Uuid,
    pub gatekeeper_url: String,
    pub map_tile_directory: String,
    pub auto_mapping_x: u32,
    pub auto_mapping_y: u32,
    pub enable_auto_mapping: bool,
}

impl Default for HypergridLinkerConfig {
    fn default() -> Self {
        Self {
            scope_id: Uuid::nil(),
            gatekeeper_url: String::new(),
            map_tile_directory: "maptiles".into(),
            auto_mapping_x: 0,
            auto_mapping_y: 0,
            enable_auto_mapping: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Linker implementation
// ---------------------------------------------------------------------------

pub struct HypergridLinker {
    config: HypergridLinkerConfig,
    links: RwLock<Vec<HyperLink>>,
}

impl HypergridLinker {
    pub fn new(config: HypergridLinkerConfig) -> Self {
        Self {
            config,
            links: RwLock::new(Vec::new()),
        }
    }

    /// Return the linker's configuration.
    pub fn config(&self) -> &HypergridLinkerConfig {
        &self.config
    }

    /// Create a new hyperlink to a foreign region.
    ///
    /// Fails if the given map coordinates are already occupied.
    pub fn link_region(
        &self,
        scope_id: Uuid,
        server_uri: &str,
        region_name: &str,
        xloc: u32,
        yloc: u32,
        owner_id: Uuid,
    ) -> HgResult<HyperLink> {
        let mut links = self.links.write().unwrap();

        // Check for coordinate collision
        for link in links.iter() {
            if link.region_loc_x == xloc && link.region_loc_y == yloc {
                return Err(HypergridError::Internal(format!(
                    "Coordinates <{}, {}> are already occupied by region {} ({})",
                    xloc, yloc, link.region_name, link.region_id
                )));
            }
        }

        let hyperlink = HyperLink {
            region_id: Uuid::new_v4(),
            region_name: region_name.to_string(),
            server_uri: server_uri.to_string(),
            region_loc_x: xloc,
            region_loc_y: yloc,
            size_x: 256,
            size_y: 256,
            owner_id,
            scope_id,
        };

        links.push(hyperlink.clone());
        Ok(hyperlink)
    }

    /// Remove a hyperlink by region ID.
    pub fn unlink_region(&self, region_id: Uuid) -> HgResult<bool> {
        let mut links = self.links.write().unwrap();
        let before = links.len();
        links.retain(|l| l.region_id != region_id);
        Ok(links.len() < before)
    }

    /// Return all current hyperlinks.
    pub fn get_linked_regions(&self) -> Vec<HyperLink> {
        let links = self.links.read().unwrap();
        links.clone()
    }

    /// Look up a hyperlink by region name (case-insensitive).
    pub fn get_link_by_name(&self, region_name: &str) -> Option<HyperLink> {
        let links = self.links.read().unwrap();
        let lower = region_name.to_lowercase();
        links
            .iter()
            .find(|l| l.region_name.to_lowercase() == lower)
            .cloned()
    }

    /// Look up a hyperlink by region ID.
    pub fn get_link_by_region_id(&self, region_id: Uuid) -> Option<HyperLink> {
        let links = self.links.read().unwrap();
        links.iter().find(|l| l.region_id == region_id).cloned()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_linker() -> HypergridLinker {
        HypergridLinker::new(HypergridLinkerConfig {
            scope_id: Uuid::nil(),
            gatekeeper_url: "http://mygrid.example:9000/".into(),
            ..Default::default()
        })
    }

    #[test]
    fn test_link_region() {
        let linker = make_linker();
        let link = linker
            .link_region(
                Uuid::nil(),
                "http://foreign.example:9000/",
                "Foreign Region",
                1000,
                1000,
                Uuid::nil(),
            )
            .unwrap();
        assert_eq!(link.region_name, "Foreign Region");
        assert_eq!(link.region_loc_x, 1000);
        assert_eq!(link.region_loc_y, 1000);
        assert_eq!(link.size_x, 256);
        assert_eq!(link.size_y, 256);

        // Should be retrievable
        let found = linker.get_link_by_region_id(link.region_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().region_name, "Foreign Region");
    }

    #[test]
    fn test_unlink_region() {
        let linker = make_linker();
        let link = linker
            .link_region(
                Uuid::nil(),
                "http://foreign.example:9000/",
                "To Remove",
                2000,
                2000,
                Uuid::nil(),
            )
            .unwrap();
        let removed = linker.unlink_region(link.region_id).unwrap();
        assert!(removed, "should successfully unlink");

        let found = linker.get_link_by_region_id(link.region_id);
        assert!(found.is_none(), "should not find unlinked region");
    }

    #[test]
    fn test_get_link_by_name() {
        let linker = make_linker();
        linker
            .link_region(
                Uuid::nil(),
                "http://foreign.example:9000/",
                "Named Region",
                3000,
                3000,
                Uuid::nil(),
            )
            .unwrap();

        // Case-insensitive lookup
        let found = linker.get_link_by_name("named region");
        assert!(found.is_some());
        assert_eq!(found.unwrap().region_name, "Named Region");

        let not_found = linker.get_link_by_name("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_link_occupied_coordinates() {
        let linker = make_linker();
        linker
            .link_region(
                Uuid::nil(),
                "http://first.example:9000/",
                "First",
                5000,
                5000,
                Uuid::nil(),
            )
            .unwrap();

        let result = linker.link_region(
            Uuid::nil(),
            "http://second.example:9000/",
            "Second",
            5000,
            5000,
            Uuid::nil(),
        );
        assert!(result.is_err(), "should fail when coordinates are occupied");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("already occupied"));
    }

    #[test]
    fn test_get_linked_regions() {
        let linker = make_linker();
        linker
            .link_region(
                Uuid::nil(),
                "http://a.example:9000/",
                "Alpha",
                6000,
                6000,
                Uuid::nil(),
            )
            .unwrap();
        linker
            .link_region(
                Uuid::nil(),
                "http://b.example:9000/",
                "Beta",
                7000,
                7000,
                Uuid::nil(),
            )
            .unwrap();
        linker
            .link_region(
                Uuid::nil(),
                "http://c.example:9000/",
                "Gamma",
                8000,
                8000,
                Uuid::nil(),
            )
            .unwrap();

        let all = linker.get_linked_regions();
        assert_eq!(all.len(), 3);
        let names: Vec<&str> = all.iter().map(|l| l.region_name.as_str()).collect();
        assert!(names.contains(&"Alpha"));
        assert!(names.contains(&"Beta"));
        assert!(names.contains(&"Gamma"));
    }

    #[test]
    fn test_unlink_nonexistent() {
        let linker = make_linker();
        let removed = linker.unlink_region(Uuid::new_v4()).unwrap();
        assert!(!removed, "unlink of nonexistent region should return false");
    }
}
