// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VehicleType {
    None = 0,
    Sled = 1,
    Car = 2,
    Boat = 3,
    Airplane = 4,
    Balloon = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleFlags(pub u32);

impl VehicleFlags {
    pub const HOVER_WATER_ONLY: Self = Self(0x01);
    pub const HOVER_TERRAIN_ONLY: Self = Self(0x02);
    pub const HOVER_GLOBAL_HEIGHT: Self = Self(0x04);
    pub const HOVER_UP_ONLY: Self = Self(0x08);
    pub const NO_DEFLECTION_UP: Self = Self(0x10);
    pub const LIMIT_MOTOR_UP: Self = Self(0x20);
    pub const LIMIT_ROLL_ONLY: Self = Self(0x40);
    pub const MOUSELOOK_STEER: Self = Self(0x80);
    pub const MOUSELOOK_BANK: Self = Self(0x100);
    pub const CAMERA_DECOUPLED: Self = Self(0x200);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for VehicleFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for VehicleFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for VehicleFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl std::ops::BitOrAssign for VehicleFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAndAssign for VehicleFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

pub struct VehicleState {
    pub vehicle_type: VehicleType,
    pub flags: VehicleFlags,
    pub reference_frame: Quat,
    pub roll_reference_frame: Quat,

    pub linear_motor_direction: Vec3,
    pub linear_friction_timescale: Vec3,
    pub linear_motor_decay_timescale: f32,
    pub linear_motor_timescale: f32,
    pub linear_motor_offset: Vec3,

    pub angular_motor_direction: Vec3,
    pub angular_motor_timescale: f32,
    pub angular_motor_decay_timescale: f32,
    pub angular_friction_timescale: Vec3,

    pub angular_deflection_efficiency: f32,
    pub angular_deflection_timescale: f32,
    pub linear_deflection_efficiency: f32,
    pub linear_deflection_timescale: f32,

    pub banking_efficiency: f32,
    pub banking_mix: f32,
    pub banking_timescale: f32,

    pub hover_height: f32,
    pub hover_efficiency: f32,
    pub hover_timescale: f32,
    pub buoyancy: f32,

    pub vertical_attraction_efficiency: f32,
    pub vertical_attraction_timescale: f32,

    lm_effect: f32,
    lm_decay: f32,
    am_effect: f32,
    am_decay: f32,
    ffactor: f32,
    amp_wr: f32,
    am_damp_x: f32,
    am_damp_y: f32,
    am_damp_z: f32,
    pub grav_mod: f32,
}

impl Default for VehicleState {
    fn default() -> Self {
        Self::new()
    }
}

impl VehicleState {
    pub fn new() -> Self {
        Self {
            vehicle_type: VehicleType::None,
            flags: VehicleFlags(0),
            reference_frame: Quat::IDENTITY,
            roll_reference_frame: Quat::IDENTITY,
            linear_motor_direction: Vec3::ZERO,
            linear_friction_timescale: Vec3::splat(1000.0),
            linear_motor_decay_timescale: 120.0,
            linear_motor_timescale: 1000.0,
            linear_motor_offset: Vec3::ZERO,
            angular_motor_direction: Vec3::ZERO,
            angular_motor_timescale: 1000.0,
            angular_motor_decay_timescale: 1000.0,
            angular_friction_timescale: Vec3::splat(1000.0),
            angular_deflection_efficiency: 0.0,
            angular_deflection_timescale: 1000.0,
            linear_deflection_efficiency: 0.0,
            linear_deflection_timescale: 1000.0,
            banking_efficiency: 0.0,
            banking_mix: 1.0,
            banking_timescale: 1000.0,
            hover_height: 0.0,
            hover_efficiency: 0.0,
            hover_timescale: 1000.0,
            buoyancy: 0.0,
            vertical_attraction_efficiency: 0.0,
            vertical_attraction_timescale: 1000.0,
            lm_effect: 0.0,
            lm_decay: 0.0,
            am_effect: 0.0,
            am_decay: 0.0,
            ffactor: 1.0,
            amp_wr: 1.0,
            am_damp_x: 0.0,
            am_damp_y: 0.0,
            am_damp_z: 0.0,
            grav_mod: 1.0,
        }
    }

    pub fn friction_factor(&self) -> f32 {
        self.ffactor
    }

    pub fn set_type(&mut self, vtype: VehicleType, inv_timestep: f32) {
        self.lm_effect = 0.0;
        self.am_effect = 0.0;
        self.ffactor = 1.0;
        self.linear_motor_direction = Vec3::ZERO;
        self.angular_motor_direction = Vec3::ZERO;
        self.roll_reference_frame = Quat::IDENTITY;
        self.linear_motor_offset = Vec3::ZERO;
        self.reference_frame = Quat::IDENTITY;

        self.vehicle_type = vtype;
        match vtype {
            VehicleType::None => {
                self.linear_friction_timescale = Vec3::splat(1000.0);
                self.angular_friction_timescale = Vec3::splat(1000.0);
                self.linear_motor_timescale = 1000.0;
                self.linear_motor_decay_timescale = 120.0 * inv_timestep;
                self.angular_motor_timescale = 1000.0;
                self.angular_motor_decay_timescale = 1000.0 * inv_timestep;
                self.hover_height = 0.0;
                self.hover_efficiency = 1.0;
                self.hover_timescale = 1000.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 0.0;
                self.linear_deflection_timescale = 1000.0;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 1000.0;
                self.banking_efficiency = 0.0;
                self.banking_mix = 1.0;
                self.banking_timescale = 1000.0;
                self.vertical_attraction_efficiency = 0.0;
                self.vertical_attraction_timescale = 1000.0;
                self.flags = VehicleFlags(0);
            }
            VehicleType::Sled => {
                self.linear_friction_timescale = Vec3::new(30.0, 1.0, 1000.0);
                self.angular_friction_timescale = Vec3::splat(1000.0);
                self.linear_motor_timescale = 1000.0;
                self.linear_motor_decay_timescale = 120.0 * inv_timestep;
                self.angular_motor_timescale = 1000.0;
                self.angular_motor_decay_timescale = 120.0 * inv_timestep;
                self.hover_height = 0.0;
                self.hover_efficiency = 1.0;
                self.hover_timescale = 10.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 1.0;
                self.linear_deflection_timescale = 1.0;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 10.0;
                self.vertical_attraction_efficiency = 1.0;
                self.vertical_attraction_timescale = 1000.0;
                self.banking_efficiency = 0.0;
                self.banking_mix = 1.0;
                self.banking_timescale = 10.0;
                self.flags &= !(VehicleFlags::HOVER_WATER_ONLY
                    | VehicleFlags::HOVER_TERRAIN_ONLY
                    | VehicleFlags::HOVER_GLOBAL_HEIGHT
                    | VehicleFlags::HOVER_UP_ONLY);
                self.flags |= VehicleFlags::NO_DEFLECTION_UP
                    | VehicleFlags::LIMIT_ROLL_ONLY
                    | VehicleFlags::LIMIT_MOTOR_UP;
            }
            VehicleType::Car => {
                self.linear_friction_timescale = Vec3::new(100.0, 2.0, 1000.0);
                self.angular_friction_timescale = Vec3::splat(1000.0);
                self.linear_motor_timescale = 1.0;
                self.linear_motor_decay_timescale = 60.0 * inv_timestep;
                self.angular_motor_timescale = 1.0;
                self.angular_motor_decay_timescale = 0.8 * inv_timestep;
                self.hover_height = 0.0;
                self.hover_efficiency = 0.0;
                self.hover_timescale = 1000.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 1.0;
                self.linear_deflection_timescale = 2.0;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 10.0;
                self.vertical_attraction_efficiency = 1.0;
                self.vertical_attraction_timescale = 10.0;
                self.banking_efficiency = -0.2;
                self.banking_mix = 1.0;
                self.banking_timescale = 1.0;
                self.flags &= !(VehicleFlags::HOVER_WATER_ONLY
                    | VehicleFlags::HOVER_TERRAIN_ONLY
                    | VehicleFlags::HOVER_GLOBAL_HEIGHT);
                self.flags |= VehicleFlags::NO_DEFLECTION_UP
                    | VehicleFlags::LIMIT_ROLL_ONLY
                    | VehicleFlags::LIMIT_MOTOR_UP
                    | VehicleFlags::HOVER_UP_ONLY;
            }
            VehicleType::Boat => {
                self.linear_friction_timescale = Vec3::new(10.0, 3.0, 2.0);
                self.angular_friction_timescale = Vec3::splat(10.0);
                self.linear_motor_timescale = 5.0;
                self.linear_motor_decay_timescale = 60.0 * inv_timestep;
                self.angular_motor_timescale = 4.0;
                self.angular_motor_decay_timescale = 4.0 * inv_timestep;
                self.hover_height = 0.0;
                self.hover_efficiency = 0.5;
                self.hover_timescale = 2.0;
                self.buoyancy = 1.0;
                self.linear_deflection_efficiency = 0.5;
                self.linear_deflection_timescale = 3.0;
                self.angular_deflection_efficiency = 0.5;
                self.angular_deflection_timescale = 5.0;
                self.vertical_attraction_efficiency = 0.5;
                self.vertical_attraction_timescale = 5.0;
                self.banking_efficiency = -0.3;
                self.banking_mix = 0.8;
                self.banking_timescale = 1.0;
                self.flags &= !(VehicleFlags::HOVER_TERRAIN_ONLY
                    | VehicleFlags::HOVER_GLOBAL_HEIGHT
                    | VehicleFlags::HOVER_UP_ONLY);
                self.flags |= VehicleFlags::NO_DEFLECTION_UP
                    | VehicleFlags::LIMIT_MOTOR_UP
                    | VehicleFlags::HOVER_UP_ONLY
                    | VehicleFlags::HOVER_WATER_ONLY;
            }
            VehicleType::Airplane => {
                self.linear_friction_timescale = Vec3::new(200.0, 10.0, 5.0);
                self.angular_friction_timescale = Vec3::splat(20.0);
                self.linear_motor_timescale = 2.0;
                self.linear_motor_decay_timescale = 60.0 * inv_timestep;
                self.angular_motor_timescale = 4.0;
                self.angular_motor_decay_timescale = 8.0 * inv_timestep;
                self.hover_height = 0.0;
                self.hover_efficiency = 0.5;
                self.hover_timescale = 1000.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 0.5;
                self.linear_deflection_timescale = 0.5;
                self.angular_deflection_efficiency = 1.0;
                self.angular_deflection_timescale = 2.0;
                self.vertical_attraction_efficiency = 0.9;
                self.vertical_attraction_timescale = 2.0;
                self.banking_efficiency = 1.0;
                self.banking_mix = 0.7;
                self.banking_timescale = 2.0;
                self.flags &= !(VehicleFlags::HOVER_WATER_ONLY
                    | VehicleFlags::HOVER_TERRAIN_ONLY
                    | VehicleFlags::HOVER_GLOBAL_HEIGHT
                    | VehicleFlags::HOVER_UP_ONLY
                    | VehicleFlags::NO_DEFLECTION_UP
                    | VehicleFlags::LIMIT_MOTOR_UP);
                self.flags |= VehicleFlags::LIMIT_ROLL_ONLY;
            }
            VehicleType::Balloon => {
                self.linear_friction_timescale = Vec3::splat(5.0);
                self.angular_friction_timescale = Vec3::splat(10.0);
                self.linear_motor_timescale = 5.0;
                self.linear_motor_decay_timescale = 60.0 * inv_timestep;
                self.angular_motor_timescale = 6.0;
                self.angular_motor_decay_timescale = 10.0 * inv_timestep;
                self.hover_height = 5.0;
                self.hover_efficiency = 0.8;
                self.hover_timescale = 10.0;
                self.buoyancy = 1.0;
                self.linear_deflection_efficiency = 0.0;
                self.linear_deflection_timescale = 5.0 * inv_timestep;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 5.0;
                self.vertical_attraction_efficiency = 1.0;
                self.vertical_attraction_timescale = 1000.0;
                self.banking_efficiency = 0.0;
                self.banking_mix = 0.7;
                self.banking_timescale = 5.0;
                self.flags &= !(VehicleFlags::HOVER_WATER_ONLY
                    | VehicleFlags::HOVER_TERRAIN_ONLY
                    | VehicleFlags::HOVER_UP_ONLY
                    | VehicleFlags::NO_DEFLECTION_UP
                    | VehicleFlags::LIMIT_MOTOR_UP
                    | VehicleFlags::LIMIT_ROLL_ONLY
                    | VehicleFlags::HOVER_GLOBAL_HEIGHT);
            }
        }

        self.flags &= !(VehicleFlags::MOUSELOOK_STEER
            | VehicleFlags::MOUSELOOK_BANK
            | VehicleFlags::CAMERA_DECOUPLED);

        self.lm_decay = 1.0 - 1.0 / self.linear_motor_decay_timescale;
        self.am_decay = 1.0 - 1.0 / self.angular_motor_decay_timescale;
    }

    pub fn stop(&mut self) {
        self.lm_effect = 0.0;
        self.lm_decay = 0.0;
        self.am_effect = 0.0;
        self.am_decay = 0.0;
        self.ffactor = 1.0;
    }

    pub fn set_linear_motor(&mut self, dir: Vec3) {
        self.linear_motor_direction = dir;
        self.lm_effect = 1.0;
    }

    pub fn set_angular_motor(&mut self, dir: Vec3) {
        self.angular_motor_direction = dir;
        self.am_effect = 1.0;
    }

    pub fn step(
        &mut self,
        current_pos: Vec3,
        current_vel: Vec3,
        current_angvel: Vec3,
        current_rot: Quat,
        mass: f32,
        terrain_height: f32,
        water_height: f32,
        gravity_z: f32,
        dt: f32,
    ) -> (Vec3, Vec3) {
        let inv_timestep = 1.0 / dt;
        let rotq = current_rot * self.reference_frame;
        let irotq = rotq.inverse();

        let mut force = Vec3::ZERO;
        let mut torque = Vec3::ZERO;

        let cur_local_vel = irotq * current_vel;
        let cur_local_angvel = irotq * current_angvel;

        let mut ldamp_z: f32 = 0.0;

        let banking_efficiency;
        let mut vertical_attraction_timescale = self.vertical_attraction_timescale;

        if self.flags.contains(VehicleFlags::MOUSELOOK_STEER)
            || self.flags.contains(VehicleFlags::MOUSELOOK_BANK)
        {
            if self.flags.contains(VehicleFlags::MOUSELOOK_BANK) {
                banking_efficiency = self.banking_efficiency;
                if vertical_attraction_timescale < 149.9 {
                    vertical_attraction_timescale *= 2.0;
                }
            } else {
                banking_efficiency = 0.0;
            }
        } else {
            banking_efficiency = self.banking_efficiency;
        }

        // Linear motor
        if self.lm_effect > 0.01 && self.linear_motor_timescale < 1000.0 {
            let mut tmp_v = self.linear_motor_direction - cur_local_vel;
            tmp_v *= self.lm_effect / self.linear_motor_timescale;
            tmp_v = rotq * tmp_v;

            if self.flags.contains(VehicleFlags::LIMIT_MOTOR_UP) {
                tmp_v.z = 0.0;
            }

            force += tmp_v;
            self.lm_effect *= self.lm_decay;
            self.ffactor = 0.0;
        } else {
            self.lm_effect = 0.0;
            self.ffactor = 1.0;
        }

        // Hover
        if self.hover_timescale < 300.0 {
            let pos_z = current_pos.z - 0.21;
            let mut perr = self.hover_height - pos_z;

            if !self.flags.contains(VehicleFlags::HOVER_GLOBAL_HEIGHT) {
                if self.flags.contains(VehicleFlags::HOVER_WATER_ONLY) {
                    perr += water_height;
                } else if self.flags.contains(VehicleFlags::HOVER_TERRAIN_ONLY)
                    || terrain_height > water_height
                {
                    perr += terrain_height;
                } else {
                    perr += water_height;
                }
            } else if terrain_height > self.hover_height {
                perr = terrain_height - pos_z;
            }

            if !self.flags.contains(VehicleFlags::HOVER_UP_ONLY) || perr > -0.1 {
                ldamp_z = self.hover_efficiency * inv_timestep;
                perr *= (1.0 + ldamp_z) / self.hover_timescale;
                force.z += perr;
                ldamp_z *= -current_vel.z;
                force.z += gravity_z * self.grav_mod * (1.0 - self.buoyancy);
            } else {
                force.z += gravity_z;
            }
        } else {
            force.z += gravity_z * self.grav_mod * (1.0 - self.buoyancy);
        }

        // Linear deflection
        if self.linear_deflection_efficiency > 0.0 {
            let len = current_vel.length();
            if len > 0.01 {
                let at_axis = xrot(rotq) * len;
                let neg_at = -at_axis;
                let err_a = at_axis - current_vel;
                let len_a = err_a.length_squared();
                let err_b = neg_at - current_vel;
                let len_b = err_b.length_squared();

                if len_a > 0.01 || len_b > 0.01 {
                    let tmp_v = if len_a < len_b { err_a } else { err_b };
                    let tmp_v = tmp_v
                        * (self.linear_deflection_efficiency / self.linear_deflection_timescale);
                    force.x += tmp_v.x;
                    force.y += tmp_v.y;
                    if !self.flags.contains(VehicleFlags::NO_DEFLECTION_UP) {
                        force.z += tmp_v.z;
                    }
                }
            }
        }

        // Linear friction
        if cur_local_vel.x != 0.0 || cur_local_vel.y != 0.0 || cur_local_vel.z != 0.0 {
            let mut tmp_v = Vec3::new(
                -cur_local_vel.x / self.linear_friction_timescale.x,
                -cur_local_vel.y / self.linear_friction_timescale.y,
                -cur_local_vel.z / self.linear_friction_timescale.z,
            );
            tmp_v = rotq * tmp_v;

            if ldamp_z != 0.0 && ldamp_z.abs() > tmp_v.z.abs() {
                tmp_v.z = ldamp_z;
            }
            force += tmp_v;
        }

        // Vertical attractor
        if vertical_attraction_timescale < 300.0 {
            let ftmp =
                inv_timestep / (vertical_attraction_timescale * vertical_attraction_timescale);
            let ftmp2 = 0.5 * self.vertical_attraction_efficiency * inv_timestep;

            self.am_damp_x = ftmp2;
            self.amp_wr = 1.0 - 0.8 * self.vertical_attraction_efficiency;

            let (mut roll, pitch) = get_roll_pitch(irotq);

            if roll > HALF_PI {
                roll = PI - roll;
            } else if roll < -HALF_PI {
                roll = -PI - roll;
            }

            let mut eff_roll = pitch / HALF_PI;
            eff_roll *= eff_roll;
            eff_roll = 1.0 - eff_roll;
            eff_roll *= roll;

            torque.x += eff_roll * ftmp;

            if !self.flags.contains(VehicleFlags::LIMIT_ROLL_ONLY) {
                let mut eff_pitch = roll / HALF_PI;
                eff_pitch *= eff_pitch;
                eff_pitch = 1.0 - eff_pitch;
                eff_pitch *= pitch;
                torque.y += eff_pitch * ftmp;
            }

            // Banking
            if banking_efficiency != 0.0 && eff_roll.abs() > 0.01 {
                let mut broll = eff_roll * self.banking_efficiency;
                if self.banking_mix != 0.0 {
                    let mut vfact = (cur_local_vel.x.abs()) / 10.0;
                    if vfact > 1.0 {
                        vfact = 1.0;
                    }
                    if cur_local_vel.x >= 0.0 {
                        broll *= 1.0 + (vfact - 1.0) * self.banking_mix;
                    } else {
                        broll *= -(1.0 + (vfact - 1.0) * self.banking_mix);
                    }
                }
                broll /= self.banking_timescale;

                let z_in_local = irotq * Vec3::Z;
                let tmp_v = z_in_local * broll;
                torque += tmp_v;

                self.am_damp_z = self.banking_efficiency.abs() / self.banking_timescale;
                self.am_damp_y = self.am_damp_z;
            } else {
                self.am_damp_z = 1.0 / self.angular_friction_timescale.z;
                self.am_damp_y = self.am_damp_x;
            }
        } else {
            self.amp_wr = 1.0;
            self.am_damp_x = 1.0 / self.angular_friction_timescale.x;
            self.am_damp_y = 1.0 / self.angular_friction_timescale.y;
            self.am_damp_z = 1.0 / self.angular_friction_timescale.z;
        }

        // Angular motor (non-mousemode)
        if self.am_effect > 0.01 && self.angular_motor_timescale < 1000.0 {
            let tmp_v = self.angular_motor_direction - cur_local_angvel;
            let tmp_v = tmp_v * (self.am_effect / self.angular_motor_timescale);
            torque.x += tmp_v.x * self.amp_wr;
            torque.y += tmp_v.y * self.amp_wr;
            torque.z += tmp_v.z;
            self.am_effect *= self.am_decay;
        } else {
            self.am_effect = 0.0;
        }

        // Angular deflection
        if self.angular_deflection_efficiency > 0.0 {
            let dirv = if cur_local_vel.x > 0.01 {
                cur_local_vel
            } else if cur_local_vel.x < -0.01 {
                -cur_local_vel
            } else {
                Vec3::new(0.01, cur_local_vel.y, cur_local_vel.z)
            };

            let ftmp = self.angular_deflection_efficiency / self.angular_deflection_timescale;

            if dirv.z.abs() > 0.01 {
                torque.y -= dirv.z.atan2(dirv.x) * ftmp;
            }
            if dirv.y.abs() > 0.01 {
                torque.z += dirv.y.atan2(dirv.x) * ftmp;
            }
        }

        // Angular friction
        if cur_local_angvel.x != 0.0 || cur_local_angvel.y != 0.0 || cur_local_angvel.z != 0.0 {
            torque.x -= cur_local_angvel.x * self.am_damp_x;
            torque.y -= cur_local_angvel.y * self.am_damp_y;
            torque.z -= cur_local_angvel.z * self.am_damp_z;
        }

        // Force is acceleration — multiply by mass for the caller
        force *= mass;

        // Torque is in vehicle frame — rotate to object frame
        torque = self.reference_frame * torque;

        (force, torque)
    }
}

const PI: f32 = std::f32::consts::PI;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

fn xrot(rot: Quat) -> Vec3 {
    let rot = rot.normalize();
    rot * Vec3::X
}

pub fn get_roll_pitch(rot: Quat) -> (f32, f32) {
    let zx = rot.x * rot.z + rot.y * rot.w;

    if zx < -0.49999 {
        (0.0, -HALF_PI)
    } else if zx > 0.49999 {
        (0.0, HALF_PI)
    } else {
        let pitch = (2.0 * zx).asin();
        let minus_zy = rot.x * rot.w - rot.y * rot.z;
        let zz = rot.z * rot.z + rot.w * rot.w - 0.5;
        let roll = minus_zy.atan2(zz);
        (roll, pitch)
    }
}
