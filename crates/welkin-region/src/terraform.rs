// SPDX-License-Identifier: LGPL-3.0-or-later

//! Terraforming: viewer `ModifyLand` requests edit the scene's terrain, and the
//! changed patches are sent to every viewer as `LayerData`.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use welkin_msg::msgs::ModifyLand;
use welkin_net::server::ClientSession;
use welkin_scene::scene::Scene;
use welkin_scene::terrain_paint::PaintAction;

type ClientMap = Arc<RwLock<std::collections::HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>;

/// `LayerData` message id on the wire (High frequency, number 11) and the land layer type.
const LAYER_DATA_HIGH_ID: u8 = 11;
const LAYER_TYPE_LAND: u8 = 0x4C;

/// Wrap encoded terrain bytes in a raw `LayerData` message body.
pub fn layer_data_raw(terrain_data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(terrain_data.len() + 4);
    payload.push(LAYER_DATA_HIGH_ID);
    payload.push(LAYER_TYPE_LAND);
    payload.extend_from_slice(&(terrain_data.len() as u16).to_le_bytes());
    payload.extend_from_slice(terrain_data);
    payload
}

/// Every patch of a 256 m region.
pub fn all_patches() -> Vec<(u32, u32)> {
    (0..16u32).flat_map(|py| (0..16u32).map(move |px| (px, py))).collect()
}

/// Brush radius for one parcel block. Current viewers send it in the extended
/// block; older ones only the 0/1/2 size index (best effort: 1, 2, 4 m).
fn brush_size(msg: &ModifyLand, index: usize) -> f32 {
    match msg.modify_block_extended.get(index) {
        Some(ext) => ext.brush_size,
        None => (1u32 << u32::from(msg.modify_block.brush_size).min(4)) as f32,
    }
}

/// Apply a `ModifyLand` request. Returns the patches that changed. `allowed` is the
/// requester's terraform permission.
pub fn apply_modify_land(scene: &Scene, allowed: bool, msg: &ModifyLand) -> Vec<(u32, u32)> {
    if !allowed {
        return Vec::new();
    }
    let Some(action) = PaintAction::from_u8(msg.modify_block.action) else {
        tracing::debug!(action = msg.modify_block.action, "ModifyLand: unknown brush action");
        return Vec::new();
    };
    let (height, seconds) = (msg.modify_block.height, msg.modify_block.seconds);
    let mut changed: Vec<(u32, u32)> = Vec::new();
    for (i, parcel) in msg.parcel_data.iter().enumerate() {
        let patches = if parcel.north == parcel.south && parcel.east == parcel.west {
            // A single click: paint with a round brush.
            scene.modify_terrain(action, parcel.west, parcel.south, height, brush_size(msg, i), seconds)
        } else if parcel.local_id == -1 {
            // A dragged rectangle over the whole region.
            scene.flood_terrain(action, height, seconds, parcel.west, parcel.south, parcel.east, parcel.north)
        } else {
            tracing::debug!(local_id = parcel.local_id, "ModifyLand: parcel-limited area edit is not supported");
            Vec::new()
        };
        changed.extend(patches);
    }
    changed.sort_unstable();
    changed.dedup();
    changed
}

/// Send the given terrain patches to every connected viewer.
pub async fn broadcast_patches(scene: &Scene, patches: &[(u32, u32)], socket: &UdpSocket, clients: &ClientMap) {
    if patches.is_empty() {
        return;
    }
    let payloads: Vec<Vec<u8>> = scene
        .terrain_layer_payloads(patches)
        .iter()
        .map(|data| layer_data_raw(data))
        .collect();
    let all_clients = clients.read().await;
    for (addr, client) in all_clients.iter() {
        let mut session = client.lock().await;
        for raw in &payloads {
            let (pkt, _) = session.circuit.send_raw(raw, false, true);
            let _ = socket.send_to(&pkt, *addr).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::msgs::modify_land;
    use welkin_scene::scene::{RegionInfo, SceneConfig};

    fn scene() -> Scene {
        Scene::new(RegionInfo::new("T", 1000, 1000), SceneConfig::default())
    }

    fn click(action: u8, x: f32, y: f32, brush: f32, seconds: f32) -> ModifyLand {
        let mut msg = ModifyLand::default();
        msg.modify_block = modify_land::ModifyBlock { action, brush_size: 1, seconds, height: 30.0 };
        msg.parcel_data = vec![modify_land::ParcelData { local_id: -1, west: x, south: y, east: x, north: y }];
        msg.modify_block_extended = vec![modify_land::ModifyBlockExtended { brush_size: brush }];
        msg
    }

    #[test]
    fn a_click_raises_the_ground_and_reports_patches() {
        let scene = scene();
        let before = scene.terrain_height_at(100.0, 100.0);
        let changed = apply_modify_land(&scene, true, &click(1, 100.0, 100.0, 3.0, 1.5));
        assert!((scene.terrain_height_at(100.0, 100.0) - before - 1.5).abs() < 1e-3);
        assert!(changed.contains(&(6, 6)));
        assert!(changed.windows(2).all(|w| w[0] < w[1]), "sorted and unique");
    }

    #[test]
    fn without_permission_nothing_changes() {
        let scene = scene();
        let changed = apply_modify_land(&scene, false, &click(1, 100.0, 100.0, 3.0, 1.5));
        assert!(changed.is_empty());
        assert_eq!(scene.terrain_height_at(100.0, 100.0), 21.0);
    }

    #[test]
    fn unknown_action_is_ignored() {
        let scene = scene();
        assert!(apply_modify_land(&scene, true, &click(9, 100.0, 100.0, 3.0, 1.0)).is_empty());
    }

    #[test]
    fn a_dragged_rectangle_floods_the_area() {
        let scene = scene();
        let mut msg = click(1, 0.0, 0.0, 1.0, 2.0);
        msg.parcel_data = vec![modify_land::ParcelData { local_id: -1, west: 10.0, south: 10.0, east: 20.0, north: 20.0 }];
        let changed = apply_modify_land(&scene, true, &msg);
        assert_eq!(scene.terrain_height_at(15.0, 15.0), 23.0);
        assert_eq!(scene.terrain_height_at(25.0, 15.0), 21.0);
        assert!(changed.contains(&(0, 0)) && changed.contains(&(1, 1)));
    }

    #[test]
    fn parcel_limited_area_edits_are_refused() {
        let scene = scene();
        let mut msg = click(1, 0.0, 0.0, 1.0, 2.0);
        msg.parcel_data = vec![modify_land::ParcelData { local_id: 3, west: 10.0, south: 10.0, east: 20.0, north: 20.0 }];
        assert!(apply_modify_land(&scene, true, &msg).is_empty());
        assert_eq!(scene.terrain_height_at(15.0, 15.0), 21.0);
    }

    #[test]
    fn layer_data_raw_has_the_wire_header() {
        let raw = layer_data_raw(&[1, 2, 3]);
        assert_eq!(&raw[..4], &[11, 0x4C, 3, 0]);
        assert_eq!(&raw[4..], &[1, 2, 3]);
    }

    #[test]
    fn layer_data_for_edited_terrain_decodes_to_the_new_heights() {
        let scene = scene();
        let changed = apply_modify_land(&scene, true, &click(1, 100.0, 100.0, 4.0, 8.0));
        let payloads = scene.terrain_layer_payloads(&changed);
        let mut seen = 0;
        for data in &payloads {
            for (px, py, heights) in welkin_scene::terrain_layer::decode_land_layer(data).unwrap() {
                let (x, y) = (100 - px * 16, 100 - py * 16);
                if x < 16 && y < 16 {
                    let h = heights[(y * 16 + x) as usize];
                    assert!((h - 29.0).abs() < 0.6, "decoded {h}, expected ~29");
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, 1);
    }

    /// A viewer edits terrain; another connected viewer receives the changed patches
    /// over UDP and decodes the new heights.
    #[tokio::test]
    async fn edited_patches_reach_connected_viewers_over_udp() {
        let scene = scene();
        let viewer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let viewer_addr = viewer.local_addr().unwrap();
        let session = ClientSession::new(viewer_addr, 1, uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let clients: ClientMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
        clients.write().await.insert(viewer_addr, Arc::new(Mutex::new(session)));

        let changed = apply_modify_land(&scene, true, &click(1, 40.0, 40.0, 3.0, 5.0));
        broadcast_patches(&scene, &changed, &server, &clients).await;

        let mut buf = vec![0u8; 2048];
        let mut found = false;
        while let Ok(Ok((n, _))) =
            tokio::time::timeout(std::time::Duration::from_millis(300), viewer.recv_from(&mut buf)).await
        {
            let pkt = welkin_msg::parse_packet(&buf[..n]).expect("parse datagram");
            assert_eq!(pkt.id, welkin_msg::MsgId::High(11), "LayerData");
            assert_eq!(pkt.body[0], 0x4C);
            let len = u16::from_le_bytes([pkt.body[1], pkt.body[2]]) as usize;
            let decoded = welkin_scene::terrain_layer::decode_land_layer(&pkt.body[3..3 + len]).unwrap();
            for (px, py, heights) in decoded {
                if (px, py) == (2, 2) {
                    let h = heights[((40 - 32) * 16 + (40 - 32)) as usize];
                    assert!((h - 26.0).abs() < 0.6, "decoded {h}, expected ~26");
                    found = true;
                }
            }
        }
        assert!(found, "patch (2,2) was delivered");
    }
}
