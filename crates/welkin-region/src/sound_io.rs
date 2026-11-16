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
            }
            | welkin_script_vm::builtins::SoundCommand::TriggerSound {
                prim_id,
                sound_id,
                volume,
            } => {
                if let Some((group_uuid, position, owner_id)) =
                    scene.with_object(&prim_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
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
                        if presence.position.distance(position) > 100.0 {
                            continue;
                        }

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
                if let Some((group_uuid, position, owner_id)) =
                    scene.with_object(&prim_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
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
                if let Some((group_uuid, _position, owner_id)) =
                    scene.with_object(&prim_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
                    let flags = loop_flags(master, slave);
                    let _ = scene.with_object_mut(&prim_id, |g| {
                        let p = g.root_part_mut();
                        p.sound_id = sound_id;
                        p.sound_gain = volume as f32;
                        p.sound_flags = flags;
                    });

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

            welkin_script_vm::builtins::SoundCommand::StopSound { prim_id } => {
                if let Some((group_uuid, _position, owner_id)) =
                    scene.with_object(&prim_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
                    let _ = scene.with_object_mut(&prim_id, |g| {
                        let p = g.root_part_mut();
                        p.sound_id = uuid::Uuid::nil();
                        p.sound_gain = 0.0;
                        p.sound_flags = 0;
                    });

                    let msg = AttachedSound {
                        data_block: attached_sound::DataBlock {
                            sound_id: uuid::Uuid::nil(),
                            object_id: group_uuid,
                            owner_id,
                            gain: 0.0,
                            flags: 0x20,
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

            welkin_script_vm::builtins::SoundCommand::AdjustVolume { prim_id, volume } => {
                if let Some((group_uuid, _position, _owner_id)) =
                    scene.with_object(&prim_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
                    let _ = scene.with_object_mut(&prim_id, |g| {
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
                if let Some((group_uuid, _position, owner_id)) =
                    scene.with_object(&prim_id, |g| (g.uuid(), g.position(), g.root_part().owner_id))
                {
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
                        let pkt = {
                            let mut s = client.lock().await;
                            s.circuit.send_msg(&msg, true).0
                        };
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }
            }

            welkin_script_vm::builtins::SoundCommand::SetRadius { prim_id, radius } => {
                let _ = scene.with_object_mut(&prim_id, |g| {
                    let p = g.root_part_mut();
                    p.sound_radius = radius as f32;
                });
            }
            welkin_script_vm::builtins::SoundCommand::CollisionSound { prim_id, sound_id, volume } => {
                let _ = scene.with_object_mut(&prim_id, |g| {
                    let p = g.root_part_mut();
                    p.collision_sound = sound_id;
                    p.collision_sound_volume = volume as f32;
                });
            }
            welkin_script_vm::builtins::SoundCommand::SetQueueing { prim_id, enabled } => {
                let _ = scene.with_object_mut(&prim_id, |g| {
                    let p = g.root_part_mut();
                    p.sound_queueing = enabled;
                });
            }
        }
    }
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
}
