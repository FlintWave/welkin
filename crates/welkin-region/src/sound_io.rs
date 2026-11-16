// SPDX-License-Identifier: LGPL-3.0-or-later

use welkin_msg::msgs::*;

pub async fn relay_sound(
    cmds: Vec<welkin_script_vm::builtins::SoundCommand>,
    scene: &welkin_scene::scene::Scene,
    socket: &std::sync::Arc<tokio::net::UdpSocket>,
    clients: &std::sync::Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<
                std::net::SocketAddr,
                std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
            >,
        >,
    >,
) {
    for cmd in cmds {
        match cmd {
            welkin_script_vm::builtins::SoundCommand::PlaySound {
                prim_id,
                sound_id,
                volume,
            } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if let Some((group_uuid, owner_id, sound_queueing)) =
                    scene.with_object(&group_id, |g| (g.uuid(), g.root_part().owner_id, g.root_part().sound_queueing))
                {
                    let flags = play_flags(sound_queueing);
                    let msg = AttachedSound {
                        data_block: attached_sound::DataBlock {
                            sound_id,
                            object_id: group_uuid,
                            owner_id,
                            gain: volume as f32,
                            flags,
                        },
                    };

                    let snapshot: Vec<(
                        std::net::SocketAddr,
                        std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                    )> = {
                        let g = clients.read().await;
                        g.iter().map(|(a, c)| (*a, c.clone())).collect()
                    };

                    for (addr, client) in snapshot {
                        let pkt = {
                            let mut s = client.lock().await;
                            s.circuit.send_msg(&msg, true).0
                        };
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }
            }

            welkin_script_vm::builtins::SoundCommand::TriggerSound {
                prim_id,
                sound_id,
                volume,
            } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if let Some((group_uuid, position, owner_id, sound_radius)) =
                    scene.with_object(&group_id, |g| (g.uuid(), g.position(), g.root_part().owner_id, g.root_part().sound_radius))
                {
                    let radius = effective_radius(sound_radius);
                    if sound_radius == 0.0 {
                        let _ = scene.with_object_mut(&group_id, |g| g.root_part_mut().sound_radius = 100.0);
                    }

                    let snapshot: Vec<(
                        std::net::SocketAddr,
                        std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                    )> = {
                        let g = clients.read().await;
                        g.iter().map(|(a, c)| (*a, c.clone())).collect()
                    };

                    for (addr, client) in snapshot {
                        let agent_id = {
                            let s = client.lock().await;
                            s.agent_id
                        };
                        let Some(presence) = scene.get_presence(&agent_id) else {
                            continue;
                        };
                        if !in_trigger_range(presence.position.distance_squared(position), radius) {
                            continue;
                        }

                        let msg = SoundTrigger {
                            sound_data: sound_trigger::SoundData {
                                sound_id,
                                owner_id,
                                object_id: group_uuid,
                                parent_id: uuid::Uuid::nil(),
                                handle: scene.region_info.region_handle,
                                position,
                                gain: volume as f32,
                            },
                        };

                        let pkt = {
                            let mut s = client.lock().await;
                            s.circuit.send_msg(&msg, false).0
                        };
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }
            }

            welkin_script_vm::builtins::SoundCommand::TriggerSoundLimited {
                prim_id,
                sound_id,
                volume,
                top_ne,
                bottom_sw,
            } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if let Some((group_uuid, position, owner_id)) =
                    scene.with_object(&group_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
                    let snapshot: Vec<(
                        std::net::SocketAddr,
                        std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                    )> = {
                        let g = clients.read().await;
                        g.iter().map(|(a, c)| (*a, c.clone())).collect()
                    };

                    for (addr, client) in snapshot {
                        let agent_id = {
                            let s = client.lock().await;
                            s.agent_id
                        };
                        let Some(presence) = scene.get_presence(&agent_id) else {
                            continue;
                        };
                        if !within_box(presence.position, top_ne, bottom_sw) {
                            continue;
                        }

                        let dist = presence.position.distance(position) as f64;
                        if dist > 100.0 {
                            continue;
                        }

                        let msg = SoundTrigger {
                            sound_data: sound_trigger::SoundData {
                                sound_id,
                                owner_id,
                                object_id: group_uuid,
                                parent_id: uuid::Uuid::nil(),
                                handle: scene.region_info.region_handle,
                                position,
                                gain: limited_gain(volume as f64, dist) as f32,
                            },
                        };

                        let pkt = {
                            let mut s = client.lock().await;
                            s.circuit.send_msg(&msg, false).0
                        };
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }
            }

            welkin_script_vm::builtins::SoundCommand::LoopSound {
                prim_id,
                sound_id,
                volume,
                master,
                slave,
            } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if group_id != prim_id {
                    tracing::warn!(prim = %prim_id, "looped sounds on child prims are not supported yet; command ignored");
                    continue;
                }

                let _ = scene.with_object_mut(&prim_id, |g| {
                    let p = g.root_part_mut();
                    p.sound_id = sound_id;
                    p.sound_gain = volume as f32;
                    p.sound_flags = loop_flags_with_queue(master, slave, p.sound_queueing);
                    if p.sound_radius == 0.0 {
                        p.sound_radius = 100.0;
                    }
                });

                if let Some(group) = scene.get_object(&prim_id) {
                    crate::broadcast_full_object_update(&group, scene.region_info.region_handle, socket.as_ref(), clients).await;
                }
            }

            welkin_script_vm::builtins::SoundCommand::StopSound { prim_id } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if group_id != prim_id {
                    tracing::warn!(prim = %prim_id, "looped sounds on child prims are not supported yet; command ignored");
                    continue;
                }

                let _ = scene.with_object_mut(&prim_id, |g| {
                    let p = g.root_part_mut();
                    p.sound_id = uuid::Uuid::nil();
                    p.sound_flags = 0x20;
                    p.sound_gain = 0.0;
                });

                if let Some(group) = scene.get_object(&prim_id) {
                    crate::broadcast_full_object_update(&group, scene.region_info.region_handle, socket.as_ref(), clients).await;
                }
            }

            welkin_script_vm::builtins::SoundCommand::AdjustVolume { prim_id, volume } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if let Some((group_uuid, _position, _owner_id)) =
                    scene.with_object(&group_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
                    let _ = scene.with_object_mut(&group_id, |g| {
                        let p = g.root_part_mut();
                        p.sound_gain = volume as f32;
                    });

                    let msg = AttachedSoundGainChange {
                        data_block: attached_sound_gain_change::DataBlock {
                            object_id: group_uuid,
                            gain: volume as f32,
                        },
                    };

                    let snapshot: Vec<(
                        std::net::SocketAddr,
                        std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                    )> = {
                        let g = clients.read().await;
                        g.iter().map(|(a, c)| (*a, c.clone())).collect()
                    };

                    for (addr, client) in snapshot {
                        let pkt = {
                            let mut s = client.lock().await;
                            s.circuit.send_msg(&msg, true).0
                        };
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }
            }

            welkin_script_vm::builtins::SoundCommand::PreloadSound { prim_id, sound_id } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                if let Some((group_uuid, position, owner_id, sound_radius)) =
                    scene.with_object(&group_id, |g| (g.uuid(), g.position(), g.root_part().owner_id, g.root_part().sound_radius))
                {
                    let radius = effective_radius(sound_radius);
                    if sound_radius == 0.0 {
                        let _ = scene.with_object_mut(&group_id, |g| g.root_part_mut().sound_radius = 100.0);
                    }

                    let msg = PreloadSound {
                        data_block: vec![preload_sound::DataBlock {
                            object_id: group_uuid,
                            owner_id,
                            sound_id,
                        }],
                    };

                    let snapshot: Vec<(
                        std::net::SocketAddr,
                        std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                    )> = {
                        let g = clients.read().await;
                        g.iter().map(|(a, c)| (*a, c.clone())).collect()
                    };

                    for (addr, client) in snapshot {
                        let agent_id = {
                            let s = client.lock().await;
                            s.agent_id
                        };
                        let Some(presence) = scene.get_presence(&agent_id) else {
                            continue;
                        };
                        if !in_preload_range(presence.position.distance_squared(position), radius) {
                            continue;
                        }

                        let pkt = {
                            let mut s = client.lock().await;
                            s.circuit.send_msg(&msg, true).0
                        };
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }
            }

            welkin_script_vm::builtins::SoundCommand::SetRadius { prim_id, radius } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                let _ = scene.with_object_mut(&group_id, |g| {
                    let p = g.root_part_mut();
                    p.sound_radius = radius as f32;
                });
            }
            welkin_script_vm::builtins::SoundCommand::CollisionSound { prim_id, sound_id, volume } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                let _ = scene.with_object_mut(&group_id, |g| {
                    let p = g.root_part_mut();
                    p.collision_sound = sound_id;
                    p.collision_sound_volume = volume as f32;
                });
            }
            welkin_script_vm::builtins::SoundCommand::SetQueueing { prim_id, enabled } => {
                let group_id = scene.resolve_group_for_prim(&prim_id);
                let Some(group_id) = group_id else {
                    tracing::warn!(prim = %prim_id, "sound command for unknown prim dropped");
                    continue;
                };

                let _ = scene.with_object_mut(&group_id, |g| {
                    let p = g.root_part_mut();
                    p.sound_queueing = enabled;
                });
            }
        }
    }
}

pub fn effective_radius(radius: f32) -> f32 {
    if radius <= 0.0 {
        100.0
    } else {
        radius
    }
}

pub fn limited_gain(volume: f64, dist: f64) -> f64 {
    if dist >= 100.0 {
        0.0
    } else {
        (volume * ((100.0 - dist) / 100.0)).max(0.0)
    }
}

pub fn in_trigger_range(dist_sq: f32, radius: f32) -> bool {
    dist_sq <= radius * radius
}

pub fn in_preload_range(dist_sq: f32, radius: f32) -> bool {
    dist_sq < 4.0 * radius * radius
}

pub fn within_box(p: glam::Vec3, top_ne: [f64; 3], bottom_sw: [f64; 3]) -> bool {
    let point = [p.x as f64, p.y as f64, p.z as f64];
    for i in 0..3 {
        let min = bottom_sw[i].min(top_ne[i]);
        let max = bottom_sw[i].max(top_ne[i]);
        if point[i] < min || point[i] > max {
            return false;
        }
    }
    true
}

pub fn loop_flags(master: bool, slave: bool) -> u8 {
    let mut flags: u8 = 1;
    if master {
        flags |= 2;
    }
    if slave {
        flags |= 4;
    }
    flags
}

pub fn loop_flags_with_queue(master: bool, slave: bool, queueing: bool) -> u8 {
    loop_flags(master, slave) | if queueing { 0x10 } else { 0 }
}

pub fn play_flags(queueing: bool) -> u8 {
    if queueing {
        0x10
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn within_box_contains_center() {
        assert!(within_box(
            Vec3::new(0.5, 0.5, 0.5),
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        ));
    }

    #[test]
    fn within_box_rejects_outside() {
        assert!(!within_box(
            Vec3::new(2.0, 0.5, 0.5),
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        ));
    }

    #[test]
    fn within_box_boundary_inclusive() {
        assert!(within_box(
            Vec3::new(1.0, 0.0, 0.5),
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        ));
    }

    #[test]
    fn within_box_handles_swapped_bounds() {
        assert!(within_box(
            Vec3::new(0.5, 0.5, 0.5),
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        ));
    }

    #[test]
    fn loop_flags_base() {
        assert_eq!(loop_flags(false, false), 1);
    }

    #[test]
    fn loop_flags_master() {
        assert_eq!(loop_flags(true, false), 3);
    }

    #[test]
    fn loop_flags_slave() {
        assert_eq!(loop_flags(false, true), 5);
    }

    #[test]
    fn loop_flags_both() {
        assert_eq!(loop_flags(true, true), 7);
    }

    #[test]
    fn play_flags_false() {
        assert_eq!(play_flags(false), 0);
    }

    #[test]
    fn play_flags_true() {
        assert_eq!(play_flags(true), 0x10);
    }

    #[test]
    fn effective_radius_zero() {
        assert_eq!(effective_radius(0.0), 100.0);
    }

    #[test]
    fn effective_radius_negative() {
        assert_eq!(effective_radius(-1.0), 100.0);
    }

    #[test]
    fn effective_radius_positive() {
        assert_eq!(effective_radius(25.0), 25.0);
    }

    #[test]
    fn limited_gain_half() {
        assert!((limited_gain(1.0, 50.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn limited_gain_at_max_distance() {
        assert_eq!(limited_gain(1.0, 100.0), 0.0);
    }

    #[test]
    fn limited_gain_beyond_max_distance() {
        assert_eq!(limited_gain(1.0, 150.0), 0.0);
    }

    #[test]
    fn limited_gain_clamps_negative_volume() {
        assert_eq!(limited_gain(-1.0, 50.0), 0.0);
    }

    #[test]
    fn in_trigger_range_true() {
        assert!(in_trigger_range(2500.0, 50.0));
    }

    #[test]
    fn in_trigger_range_boundary() {
        assert!(in_trigger_range(2500.0, 50.0));
        assert!(!in_trigger_range(2501.0, 50.0));
    }

    #[test]
    fn in_preload_range_true() {
        assert!(in_preload_range(9999.0, 50.0));
    }

    #[test]
    fn in_preload_range_boundary() {
        assert!(!in_preload_range(10000.0, 50.0));
        assert!(in_preload_range(9999.999, 50.0));
    }

    #[test]
    fn loop_flags_with_queue_false() {
        assert_eq!(loop_flags_with_queue(false, false, false), loop_flags(false, false));
    }

    #[test]
    fn loop_flags_with_queue_true() {
        assert_eq!(loop_flags_with_queue(false, false, true), loop_flags(false, false) | 0x10);
    }
}
