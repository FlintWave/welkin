// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Quat, Vec3};
use uuid::Uuid;
use welkin_msg::msgs::{ObjectUpdate, object_update};
use welkin_msg::objects::FullObjectUpdate;
use welkin_scene::part::SceneObjectPart;

pub fn full_update_for_part(
    part: &SceneObjectPart,
    parent_local_id: u32,
    is_root: bool,
) -> FullObjectUpdate {
    let position = if is_root {
        part.group_position
    } else {
        part.offset_position
    };

    let texture_entry = if part.shape.texture_entry.is_empty() {
        crate::build_default_prim_te()
    } else {
        part.shape.texture_entry.clone()
    };

    FullObjectUpdate {
        local_id: part.local_id,
        full_id: part.uuid,
        parent_id: parent_local_id,
        pcode: 9,
        state: 0,
        crc: 0,
        material: part.material,
        click_action: part.click_action,
        scale: part.scale,
        position,
        velocity: part.velocity,
        acceleration: part.acceleration,
        rotation: part.rotation_offset,
        angular_velocity: part.angular_velocity,
        update_flags: 0x0001_000D,
        owner_id: part.owner_id,
        path_curve: part.shape.path_curve,
        profile_curve: part.shape.profile_curve,
        path_begin: part.shape.path_begin,
        path_end: part.shape.path_end,
        path_scale_x: part.shape.path_scale_x,
        path_scale_y: part.shape.path_scale_y,
        path_shear_x: part.shape.path_shear_x,
        path_shear_y: part.shape.path_shear_y,
        path_twist: part.shape.path_twist,
        path_twist_begin: part.shape.path_twist_begin,
        path_radius_offset: part.shape.path_radius_offset,
        path_taper_x: part.shape.path_taper_x,
        path_taper_y: part.shape.path_taper_y,
        path_revolutions: part.shape.path_revolutions,
        path_skew: part.shape.path_skew,
        profile_begin: part.shape.profile_begin,
        profile_end: part.shape.profile_end,
        profile_hollow: part.shape.profile_hollow,
        texture_entry,
        texture_anim: part.texture_animation.clone(),
        name_value: Vec::new(),
        data: Vec::new(),
        text: part.text.clone(),
        text_color: [
            part.text_color.r(),
            part.text_color.g(),
            part.text_color.b(),
            part.text_color.a(),
        ],
        media_url: part.media_url.clone(),
        ps_block: part.particle_system.clone(),
        extra_params: Vec::new(),
        sound_id: part.sound_id,
        sound_gain: part.sound_gain,
        sound_flags: part.sound_flags,
        sound_radius: part.sound_radius,
        foot_plane: None,
    }
}

pub fn scene_object_updates(scene: &welkin_scene::scene::Scene) -> Vec<FullObjectUpdate> {
    let mut updates = Vec::new();

    scene.for_each_object(|group: &welkin_scene::group::SceneObjectGroup| {
        let root = group.root_part();

        if root.local_id != 0 {
            updates.push(full_update_for_part(root, 0, true));
        }

        let root_local_id = root.local_id;

        for part in group.parts() {
            if part.local_id == 0 {
                continue;
            }
            if (part as *const SceneObjectPart) == (root as *const SceneObjectPart) {
                continue;
            }
            updates.push(full_update_for_part(part, root_local_id, false));
        }
    });

    updates
}

pub fn batch_updates(
    updates: Vec<FullObjectUpdate>,
    per_packet: usize,
) -> Vec<Vec<FullObjectUpdate>> {
    let chunk_size = per_packet.max(1);
    let mut batches = Vec::new();
    let mut iter = updates.into_iter();
    loop {
        let chunk: Vec<FullObjectUpdate> = iter.by_ref().take(chunk_size).collect();
        if chunk.is_empty() {
            break;
        }
        batches.push(chunk);
    }
    batches
}

pub async fn send_scene_objects(
    scene: &welkin_scene::scene::Scene,
    session: &mut welkin_net::server::ClientSession,
    socket: &tokio::net::UdpSocket,
    addr: std::net::SocketAddr,
) -> usize {
    let updates = scene_object_updates(scene);
    let batches = batch_updates(updates, 3);

    for batch in &batches {
        let msg = ObjectUpdate {
            region_data: object_update::RegionData {
                region_handle: scene.region_info.region_handle,
                time_dilation: 65535,
            },
            object_data: batch.iter().map(|update| update.to_object_data()).collect(),
        };

        let (pkt, _) = session.circuit.send_msg(&msg, true);
        let _ = socket.send_to(&pkt, addr).await;
    }

    batches.iter().map(|batch| batch.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use uuid::Uuid;
    use welkin_scene::part::SceneObjectPart;

    #[test]
    fn batch_updates_chunks() {
        let part = SceneObjectPart::new("Box", Uuid::nil());
        let make = || -> Vec<FullObjectUpdate> {
            (0..7).map(|_| full_update_for_part(&part, 0, true)).collect()
        };

        let batches = batch_updates(make(), 3);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 3);
        assert_eq!(batches[1].len(), 3);
        assert_eq!(batches[2].len(), 1);

        let batches = batch_updates(make(), 0);
        assert_eq!(batches.len(), 7);
        assert!(batches.iter().all(|batch| batch.len() == 1));
    }

    #[test]
    fn full_update_for_part_fields() {
        let owner_uuid = Uuid::nil();
        let sound_uuid = Uuid::new_v4();

        let mut part = SceneObjectPart::new("Box", owner_uuid);
        part.sound_id = sound_uuid;
        part.sound_gain = 1.25;
        part.sound_radius = 9.0;
        part.group_position = Vec3::new(1.0, 2.0, 3.0);
        part.offset_position = Vec3::new(4.0, 5.0, 6.0);

        let root = full_update_for_part(&part, 7, true);
        assert_eq!(root.position, part.group_position);
        assert_eq!(root.sound_id, sound_uuid);
        assert_eq!(root.sound_gain, 1.25);
        assert_eq!(root.sound_radius, 9.0);

        let child = full_update_for_part(&part, 7, false);
        assert_eq!(child.position, part.offset_position);
        assert_eq!(child.parent_id, 7);
    }

    /// Real UDP round-trip: the scene is sent to a "viewer" socket and the
    /// datagrams are parsed back, proving prims (and linkset children) are
    /// delivered on the wire with their sound fields.
    #[tokio::test]
    async fn scene_objects_arrive_over_udp() {
        use welkin_msg::Msg;
        use welkin_msg::msgs::ObjectUpdate;
        use welkin_scene::group::SceneObjectGroup;
        use welkin_scene::scene::RegionInfo;
        use welkin_scene::scene::{Scene, SceneConfig};

        let scene = Scene::new(RegionInfo::new("T", 1000, 1000), SceneConfig::default());
        let owner = Uuid::new_v4();
        let sound = Uuid::new_v4();

        let mut loud = SceneObjectPart::new("Loud", owner);
        loud.sound_id = sound;
        loud.sound_gain = 0.5;
        loud.sound_flags = 1;
        loud.sound_radius = 20.0;
        let mut linked = SceneObjectGroup::new(loud);
        linked.add_part(SceneObjectPart::new("Child", owner));
        let linked_root = scene.add_object(linked);
        let plain_root = scene.add_object(SceneObjectGroup::new(SceneObjectPart::new("Plain", owner)));

        let viewer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut session = welkin_net::server::ClientSession::new(
            viewer.local_addr().unwrap(),
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        let sent = send_scene_objects(&scene, &mut session, &server, viewer.local_addr().unwrap()).await;
        assert_eq!(sent, 3, "two roots plus one child");

        let mut seen: Vec<welkin_msg::msgs::object_update::ObjectData> = Vec::new();
        let mut buf = vec![0u8; 2048];
        while let Ok(Ok((n, _))) = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            viewer.recv_from(&mut buf),
        )
        .await
        {
            let pkt = welkin_msg::parse_packet(&buf[..n]).expect("parse datagram");
            assert_eq!(pkt.id, ObjectUpdate::ID);
            let msg = ObjectUpdate::decode(&pkt.body).expect("decode ObjectUpdate");
            seen.extend(msg.object_data);
        }

        assert_eq!(seen.len(), 3);
        let root = seen.iter().find(|o| o.full_id == linked_root).expect("linked root delivered");
        assert_eq!(root.sound, sound);
        assert_eq!(root.gain, 0.5);
        assert_eq!(root.radius, 20.0);
        assert!(seen.iter().any(|o| o.full_id == plain_root));
        let child = seen.iter().find(|o| o.parent_id != 0).expect("child part delivered");
        assert_eq!(child.parent_id, root.id, "child references the root's local id");
    }
}
