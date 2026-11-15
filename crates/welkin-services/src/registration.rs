// SPDX-License-Identifier: GPL-3.0-or-later

//! Region registration lifecycle — register, deregister, and neighbour notification.

use std::sync::Arc;

use welkin_data::grid::RegionRow;
use serde_json::json;
use tracing::{info, warn};

use crate::remote::RemoteError;
use crate::remote::grid::RemoteGridConnector;

/// Manages a single region's lifecycle on the grid: registration, deregistration,
/// and neighbour notification via the HelloNeighbour protocol.
pub struct GridRegistration {
    connector: Arc<RemoteGridConnector>,
    region: RegionRow,
    neighbours: tokio::sync::RwLock<Vec<RegionRow>>,
    client: reqwest::Client,
}

impl GridRegistration {
    /// Create a new `GridRegistration` for the given region.
    pub fn new(
        connector: Arc<RemoteGridConnector>,
        region: RegionRow,
        client: reqwest::Client,
    ) -> Self {
        Self {
            connector,
            region,
            neighbours: tokio::sync::RwLock::new(Vec::new()),
            client,
        }
    }

    /// Register the region with the grid, fetch neighbours, and send
    /// HelloNeighbour notifications.
    pub async fn startup(&self) -> Result<(), RemoteError> {
        self.connector.register_region(&self.region).await?;

        let nbrs = self
            .connector
            .get_neighbours(&self.region.scope_id, &self.region.uuid)
            .await?;

        let count = nbrs.len();
        {
            let mut guard = self.neighbours.write().await;
            *guard = nbrs;
        }

        self.notify_neighbours().await?;

        info!(
            "Region {} registered with grid, {} neighbours",
            self.region.region_name, count
        );
        Ok(())
    }

    /// Deregister the region from the grid.
    pub async fn shutdown(&self) -> Result<(), RemoteError> {
        self.connector
            .deregister_region(&self.region.uuid)
            .await?;
        info!(
            "Region {} deregistered from grid",
            self.region.region_name
        );
        Ok(())
    }

    /// Send a HelloNeighbour notification to every cached neighbour.
    ///
    /// Individual failures are logged as warnings but do not fail the overall
    /// operation.
    pub async fn notify_neighbours(&self) -> Result<(), RemoteError> {
        let neighbours = self.neighbours.read().await.clone();

        let body = json!({
            "region_id": self.region.uuid,
            "region_name": self.region.region_name,
            "region_locx": self.region.loc_x,
            "region_locy": self.region.loc_y,
            "region_size_x": self.region.size_x,
            "region_size_y": self.region.size_y,
            "region_port": self.region.server_port.unwrap_or(0),
            "internal_ep_address": self.region.server_ip,
            "internal_ep_port": self.region.server_port.unwrap_or(0),
            "external_host_name": self.region.server_uri,
            "http_port": self.region.server_http_port.unwrap_or(0),
        });

        for nbr in &neighbours {
            let url = format!(
                "{}/region/{}/",
                nbr.server_uri.trim_end_matches('/'),
                self.region.uuid
            );

            if let Err(e) = self.client.post(&url).json(&body).send().await {
                warn!(
                    "HelloNeighbour to {} ({}) failed: {}",
                    nbr.region_name, url, e
                );
            }
        }

        Ok(())
    }

    /// Re-fetch the neighbour list from the grid and send HelloNeighbour
    /// notifications to all of them.
    pub async fn refresh_neighbours(&self) -> Result<(), RemoteError> {
        let nbrs = self
            .connector
            .get_neighbours(&self.region.scope_id, &self.region.uuid)
            .await?;

        {
            let mut guard = self.neighbours.write().await;
            *guard = nbrs;
        }

        self.notify_neighbours().await
    }

    /// Return a snapshot of the cached neighbours list.
    pub async fn neighbours(&self) -> Vec<RegionRow> {
        self.neighbours.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal `RegionRow` for testing.
    fn test_region(name: &str) -> RegionRow {
        RegionRow {
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            region_handle: 1099511628032,
            region_name: name.into(),
            region_secret: String::new(),
            server_ip: "127.0.0.1".into(),
            server_port: Some(9000),
            server_uri: "http://127.0.0.1:9000".into(),
            loc_x: 256000,
            loc_y: 256000,
            loc_z: 0,
            region_map_texture: "00000000-0000-0000-0000-000000000000".into(),
            server_http_port: Some(9000),
            owner_uuid: "00000000-0000-0000-0000-000000000000".into(),
            access: 1,
            scope_id: "00000000-0000-0000-0000-000000000000".into(),
            size_x: 256,
            size_y: 256,
            flags: 0,
            last_seen: 0,
            principal_id: "00000000-0000-0000-0000-000000000000".into(),
            token: String::new(),
            parcel_map_texture: "00000000-0000-0000-0000-000000000000".into(),
        }
    }

    #[test]
    fn test_construction() {
        let connector = Arc::new(RemoteGridConnector::new("http://grid.example.com:8003"));
        let region = test_region("TestRegion");
        let client = reqwest::Client::new();
        let reg = GridRegistration::new(connector, region, client);

        // Neighbours start empty.
        let nbrs = reg.neighbours.blocking_read();
        assert!(nbrs.is_empty());
    }

    #[test]
    fn test_region_helper_produces_valid_row() {
        let r = test_region("TestRegion");
        assert_eq!(r.region_name, "TestRegion");
        assert_eq!(r.server_port, Some(9000));
    }
}
