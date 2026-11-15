// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::Vec3;

use crate::PidHoverType;

const PID_P: f32 = 7000.0;
const PID_D: f32 = 2200.0;
const MIN_GROUND_FLIGHT_OFFSET: f32 = 3.0;

pub struct AvatarController {
    pub target_velocity: Vec3,
    pub flying: bool,
    pub always_run: bool,
    pub walk_multiplier: f32,
    pub run_multiplier: f32,
    pub mass: f32,
    pub buoyancy: f32,
    pub pid_active: bool,
    pub colliding: bool,
    pub colliding_ground: bool,

    pub hover_type: PidHoverType,
    pub hover_height: f32,
    pub hover_tau: f32,
    pub hover_active: bool,

    zero_flag: bool,
    zero_position: Vec3,
    free_move: bool,
    collide_normal: Vec3,
    collider_ground_filter: i32,
    have_last_fall_vel: bool,
    last_fall_vel: Vec3,
}

impl Default for AvatarController {
    fn default() -> Self {
        Self::new(80.0)
    }
}

impl AvatarController {
    pub fn new(mass: f32) -> Self {
        Self {
            target_velocity: Vec3::ZERO,
            flying: false,
            always_run: false,
            walk_multiplier: 1.0 / 1.3,
            run_multiplier: 1.0 / 0.8,
            mass,
            buoyancy: 0.0,
            pid_active: true,
            colliding: false,
            colliding_ground: false,
            hover_type: PidHoverType::Ground,
            hover_height: 0.0,
            hover_tau: 0.0,
            hover_active: false,
            zero_flag: false,
            zero_position: Vec3::ZERO,
            free_move: false,
            collide_normal: Vec3::Z,
            collider_ground_filter: 0,
            have_last_fall_vel: false,
            last_fall_vel: Vec3::ZERO,
        }
    }

    pub fn step(
        &mut self,
        position: Vec3,
        velocity: Vec3,
        capsule_half_z: f32,
        terrain_height: f32,
        water_height: f32,
        gravity_force_z: f32,
        _dt: f32,
    ) -> Vec3 {
        let tv_zero = self.target_velocity == Vec3::ZERO;
        let ctv = if tv_zero {
            Vec3::ZERO
        } else if self.always_run {
            Vec3::new(
                self.target_velocity.x * self.run_multiplier,
                self.target_velocity.y * self.run_multiplier,
                self.target_velocity.z,
            )
        } else {
            Vec3::new(
                self.target_velocity.x * self.walk_multiplier,
                self.target_velocity.y * self.walk_multiplier,
                self.target_velocity.z,
            )
        };
        let mut ctv = ctv;

        let mut vec = Vec3::ZERO;
        let aabb_min_z = position.z - capsule_half_z;

        // Terrain collision
        if aabb_min_z < terrain_height {
            if ctv.z < 0.0 {
                ctv.z = 0.0;
            }

            if !self.have_last_fall_vel {
                self.last_fall_vel = velocity;
                self.have_last_fall_vel = true;
            }

            let pidp50 = PID_P * 50.0;
            let depth = terrain_height - aabb_min_z;
            vec.z = depth * pidp50;

            if !self.flying {
                vec.z -= velocity.z * PID_D;
            }

            if depth < 0.2 {
                self.collider_ground_filter += 1;
                if self.collider_ground_filter > 2 {
                    self.colliding = true;
                    if self.collider_ground_filter > 10 {
                        self.collider_ground_filter = 10;
                        self.free_move = false;
                    }
                    self.collide_normal = Vec3::Z;
                    self.colliding_ground = true;
                    self.last_fall_vel = Vec3::ZERO;
                }
            } else {
                self.collider_ground_filter -= 5;
                if self.collider_ground_filter <= 0 {
                    self.collider_ground_filter = 0;
                    self.colliding_ground = false;
                }
            }
        } else {
            self.have_last_fall_vel = false;
            self.collider_ground_filter -= 5;
            if self.collider_ground_filter <= 0 {
                self.collider_ground_filter = 0;
                self.colliding_ground = false;
            }
        }

        // Hover PID
        let mut hover_pid_active = false;
        if self.hover_active && self.hover_tau != 0.0 && self.hover_height != 0.0 {
            hover_pid_active = true;
            let target_hover = match self.hover_type {
                PidHoverType::Ground => terrain_height + self.hover_height,
                PidHoverType::GroundAndWater => {
                    if terrain_height > water_height {
                        terrain_height + self.hover_height
                    } else {
                        water_height + self.hover_height
                    }
                }
                _ => self.hover_height,
            };

            if target_hover > terrain_height + position.z {
                let fz = target_hover - position.z;
                if fz.abs() >= 0.01 {
                    let mut fz = fz / self.hover_tau;
                    let abs_fz = fz.abs();
                    if abs_fz > 50.0 {
                        fz = 50.0 * fz.signum();
                    } else if abs_fz < 0.1 {
                        fz = 0.1 * fz.signum();
                    }
                    ctv.z = fz;
                } else {
                    ctv.z = 0.0;
                }
            }
        }

        if !self.colliding {
            self.collide_normal.z = 0.0;
        }

        // Slope adjustment for walking
        if !tv_zero
            && self.colliding
            && !self.flying
            && ctv.z == 0.0
            && self.collide_normal.z > 0.2
            && self.collide_normal.z < 0.94
        {
            let p = ctv.x * self.collide_normal.x + ctv.y * self.collide_normal.y;
            ctv.x *= (1.0 - self.collide_normal.x * self.collide_normal.x).sqrt();
            ctv.y *= (1.0 - self.collide_normal.y * self.collide_normal.y).sqrt();
            ctv.z -= p;
            if ctv.z < 0.0 {
                ctv.z *= 2.0;
            }
        }

        if !self.free_move {
            if tv_zero {
                // Stopped — position hold
                if self.colliding || self.flying {
                    if !self.zero_flag {
                        self.zero_flag = true;
                        self.zero_position = position;
                        if !self.pid_active {
                            let pidd0833 = PID_D * 0.833;
                            vec.x -= velocity.x * pidd0833;
                            vec.y -= velocity.y * pidd0833;
                        }
                    }
                    if self.pid_active {
                        let pidd2 = PID_D * 2.0;
                        let pidp5 = PID_P * 5.0;
                        vec.x = -velocity.x * pidd2 + (self.zero_position.x - position.x) * pidp5;
                        vec.y = -velocity.y * pidd2 + (self.zero_position.y - position.y) * pidp5;
                        if velocity.z > 0.0 {
                            vec.z +=
                                -velocity.z * PID_D + (self.zero_position.z - position.z) * PID_P;
                        } else {
                            vec.z += (-velocity.z * PID_D
                                + (self.zero_position.z - position.z) * PID_P)
                                * 0.2;
                        }
                    }
                } else {
                    self.zero_flag = false;
                    let pidd0833 = PID_D * 0.833;
                    vec.x += (ctv.x - velocity.x) * pidd0833;
                    vec.y += (ctv.y - velocity.y) * pidd0833;
                    if ctv.z == -9999.0 {
                        vec.z += -velocity.z * PID_D - gravity_force_z;
                    }
                }
            } else {
                // Moving
                self.pid_active = true;
                self.zero_flag = false;

                if self.colliding {
                    if !self.flying {
                        if ctv.z > 0.0 {
                            vec.z += (ctv.z - velocity.z) * PID_D * 2.0;
                            vec.x += (ctv.x - velocity.x) * PID_D;
                            vec.y += (ctv.y - velocity.y) * PID_D;
                        } else if ctv.z == 0.0 {
                            if velocity.z > 0.0 {
                                vec.z -= velocity.z * PID_D * 2.0;
                            }
                            vec.x += (ctv.x - velocity.x) * PID_D;
                            vec.y += (ctv.y - velocity.y) * PID_D;
                        } else {
                            if ctv.z < velocity.z {
                                vec.z += (ctv.z - velocity.z) * PID_D;
                            }
                            if ctv.x.abs() > velocity.x.abs() {
                                vec.x += (ctv.x - velocity.x) * PID_D;
                            }
                            if ctv.y.abs() > velocity.y.abs() {
                                vec.y += (ctv.y - velocity.y) * PID_D;
                            }
                        }
                    } else {
                        let pidd00625 = PID_D * 0.0625;
                        vec.x += (ctv.x - velocity.x) * pidd00625;
                        vec.y += (ctv.y - velocity.y) * pidd00625;
                        vec.z += (ctv.z - velocity.z) * pidd00625;
                    }
                } else if self.flying || hover_pid_active {
                    vec.x += (ctv.x - velocity.x) * PID_D;
                    vec.y += (ctv.y - velocity.y) * PID_D;
                    vec.z += (ctv.z - velocity.z) * PID_D;
                } else {
                    let pidd0833 = PID_D * 0.833;
                    vec.x += (ctv.x - velocity.x) * pidd0833;
                    vec.y += (ctv.y - velocity.y) * pidd0833;
                    if ctv.z == -9999.0 {
                        vec.z += -velocity.z * PID_D - gravity_force_z;
                    }
                }
            }

            // Speed cap at 50 m/s
            let vel_sq = velocity.length_squared();
            if vel_sq > 2500.0 {
                let breakfactor = 0.16 * self.mass;
                vec -= velocity * breakfactor;
            }
        } else {
            // Free move — brake
            let breakfactor = self.mass;
            vec.x -= breakfactor * velocity.x;
            vec.y -= breakfactor * velocity.y;
            if self.flying {
                vec.z -= 0.5 * breakfactor * velocity.z;
            } else {
                vec.z -= 0.16 * self.mass * velocity.z;
            }
        }

        // Flying anti-gravity
        if self.flying || hover_pid_active {
            vec.z -= gravity_force_z;
            if !hover_pid_active {
                let target_altitude = terrain_height + MIN_GROUND_FLIGHT_OFFSET;
                if position.z < target_altitude {
                    vec.z += (target_altitude - position.z) * PID_P * 5.0;
                }
            }
        } else if self.buoyancy != 0.0 {
            vec.z -= gravity_force_z * self.buoyancy;
        }

        vec
    }
}
