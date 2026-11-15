// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::VecDeque;

use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayMode {
    Forward = 0,
    Reverse = 1,
    Loop = 2,
    PingPong = 3,
}

#[derive(Clone, Copy, Debug)]
pub struct Keyframe {
    pub position: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub time_ms: i32,
}

#[derive(Clone, Copy, Debug)]
struct WorkingFrame {
    target_position: Vec3,
    target_rotation: Quat,
    _start_position: Vec3,
    start_rotation: Quat,
    time_remaining_ms: i32,
    time_total_ms: i32,
}

#[derive(Clone, Debug)]
pub struct KeyframeResult {
    pub position: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub finished: bool,
}

pub struct KeyframeMotion {
    keyframes: Vec<Keyframe>,
    mode: PlayMode,
    running: bool,
    paused: bool,
    base_position: Vec3,
    base_rotation: Quat,
    frames: VecDeque<WorkingFrame>,
    current_frame: Option<WorkingFrame>,
    iterations: i32,
    current_velocity: Vec3,
}

impl KeyframeMotion {
    pub fn new(mode: PlayMode, base_position: Vec3, base_rotation: Quat) -> Self {
        Self {
            keyframes: Vec::new(),
            mode,
            running: false,
            paused: false,
            base_position,
            base_rotation,
            frames: VecDeque::new(),
            current_frame: None,
            iterations: 0,
            current_velocity: Vec3::ZERO,
        }
    }

    pub fn set_keyframes(&mut self, frames: Vec<Keyframe>) {
        self.keyframes = frames;
    }

    pub fn is_running(&self) -> bool {
        self.running && !self.paused
    }

    pub fn start(&mut self) {
        if self.keyframes.is_empty() {
            self.running = false;
            return;
        }
        self.running = true;
        self.paused = false;
        self.iterations = 0;
        self.frames.clear();
        self.current_frame = None;
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.paused = false;
        self.frames.clear();
        self.current_frame = None;
        self.current_velocity = Vec3::ZERO;
    }

    pub fn pause(&mut self) {
        self.paused = true;
        self.current_velocity = Vec3::ZERO;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn step(
        &mut self,
        elapsed_ms: f64,
        current_position: Vec3,
        current_rotation: Quat,
    ) -> KeyframeResult {
        let default = KeyframeResult {
            position: current_position,
            rotation: current_rotation,
            velocity: Vec3::ZERO,
            finished: false,
        };

        if !self.running || self.paused {
            return default;
        }

        if self.current_frame.is_none() && self.frames.is_empty() {
            self.build_frame_list();
            if self.frames.is_empty() {
                self.running = false;
                return KeyframeResult {
                    finished: true,
                    ..default
                };
            }
            let frame = self.frames.pop_front().unwrap();
            self.current_velocity =
                (frame.target_position - current_position) / (frame.time_total_ms as f32 * 0.001);
            self.current_frame = Some(frame);
        }

        let frame = match self.current_frame.as_mut() {
            Some(f) => f,
            None => return default,
        };

        let elapsed_i = elapsed_ms.min(150.0) as i32;
        frame.time_remaining_ms -= elapsed_i;

        if frame.time_remaining_ms <= 0 {
            let pos = frame.target_position;
            let rot = frame.target_rotation;

            if let Some(next) = self.frames.pop_front() {
                self.current_velocity =
                    (next.target_position - pos) / (next.time_total_ms as f32 * 0.001);
                self.current_frame = Some(next);
                return KeyframeResult {
                    position: pos,
                    rotation: rot,
                    velocity: self.current_velocity,
                    finished: false,
                };
            }

            self.current_frame = None;
            self.current_velocity = Vec3::ZERO;

            let can_loop = self.mode == PlayMode::Loop || self.mode == PlayMode::PingPong;

            if can_loop {
                return KeyframeResult {
                    position: pos,
                    rotation: rot,
                    velocity: Vec3::ZERO,
                    finished: false,
                };
            }

            self.running = false;
            return KeyframeResult {
                position: pos,
                rotation: rot,
                velocity: Vec3::ZERO,
                finished: true,
            };
        }

        let remaining_steps = frame.time_remaining_ms as f32 / 50.0;
        let motion = (frame.target_position - current_position) / remaining_steps;
        let new_pos = current_position + motion;

        let completed =
            (frame.time_total_ms - frame.time_remaining_ms) as f32 / frame.time_total_ms as f32;
        let new_rot = frame
            .start_rotation
            .slerp(frame.target_rotation, completed)
            .normalize();

        KeyframeResult {
            position: new_pos,
            rotation: new_rot,
            velocity: self.current_velocity,
            finished: false,
        }
    }

    fn build_frame_list(&mut self) {
        self.frames.clear();

        let can_repeat =
            self.mode == PlayMode::Loop || self.mode == PlayMode::PingPong || self.iterations == 0;

        if !can_repeat {
            return;
        }

        let reverse = self.mode == PlayMode::Reverse
            || (self.mode == PlayMode::PingPong && (self.iterations & 1) != 0);

        let mut pos = self.base_position;
        let mut rot = self.base_rotation;

        let indices: Vec<usize> = if reverse {
            (0..self.keyframes.len()).rev().collect()
        } else {
            (0..self.keyframes.len()).collect()
        };

        for i in indices {
            let kf = &self.keyframes[i];
            let direction: f32 = if reverse { -1.0 } else { 1.0 };

            let target_pos = match kf.position {
                Some(p) => pos + p * direction,
                None => pos,
            };

            let target_rot = match kf.rotation {
                Some(r) => {
                    let r = if reverse { r.conjugate() } else { r };
                    (rot * r).normalize()
                }
                None => rot,
            };

            self.frames.push_back(WorkingFrame {
                target_position: target_pos,
                target_rotation: target_rot,
                _start_position: pos,
                start_rotation: rot,
                time_remaining_ms: kf.time_ms,
                time_total_ms: kf.time_ms,
            });

            pos = target_pos;
            rot = target_rot;
        }

        self.base_position = pos;
        self.base_rotation = rot;
        self.iterations += 1;
    }
}
