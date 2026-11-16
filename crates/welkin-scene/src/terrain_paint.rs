// SPDX-License-Identifier: LGPL-3.0-or-later

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintAction {
    Flatten = 0,
    Raise = 1,
    Lower = 2,
    Smooth = 3,
    Noise = 4,
    Revert = 5,
}

impl PaintAction {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(PaintAction::Flatten),
            1 => Some(PaintAction::Raise),
            2 => Some(PaintAction::Lower),
            3 => Some(PaintAction::Smooth),
            4 => Some(PaintAction::Noise),
            5 => Some(PaintAction::Revert),
            _ => None,
        }
    }
}

fn noise(x: f64, y: f64) -> f64 {
    let n = (x as i32).wrapping_add((y * 749.0) as i32);
    let n = n.wrapping_shl(13) ^ n;
    let mut val = n.wrapping_mul(n);
    val = val.wrapping_mul(15731);
    val = val.wrapping_add(789221);
    val = n.wrapping_mul(val);
    val = val.wrapping_add(1376312589);
    let masked = val & 0x7fffffff;
    1.0 - (masked as f64) / 1073741824.0
}

fn smoothed_noise_1(x: f64, y: f64) -> f64 {
    let corners = (noise(x - 1.0, y - 1.0)
        + noise(x + 1.0, y - 1.0)
        + noise(x - 1.0, y + 1.0)
        + noise(x + 1.0, y + 1.0))
        / 16.0;
    let sides = (noise(x - 1.0, y)
        + noise(x + 1.0, y)
        + noise(x, y - 1.0)
        + noise(x, y + 1.0))
        / 8.0;
    let center = noise(x, y) / 4.0;
    corners + sides + center
}

fn interpolate(a: f64, b: f64, z: f64) -> f64 {
    a * (1.0 - z) + b * z
}

fn interpolated_noise(x: f64, y: f64) -> f64 {
    let ix = x as i32;
    let fx = x - ix as f64;
    let iy = y as i32;
    let fy = y - iy as f64;
    let v1 = smoothed_noise_1(ix as f64, iy as f64);
    let v2 = smoothed_noise_1((ix + 1) as f64, iy as f64);
    let v3 = smoothed_noise_1(ix as f64, (iy + 1) as f64);
    let v4 = smoothed_noise_1((ix + 1) as f64, (iy + 1) as f64);
    let i1 = interpolate(v1, v2, fx);
    let i2 = interpolate(v3, v4, fx);
    interpolate(i1, i2, fy)
}

pub fn perlin_noise_2d(x: f64, y: f64, octaves: u32, persistence: f64) -> f64 {
    let mut total = 0.0;
    for i in 0..octaves {
        let f = 2.0_f64.powi(i as i32);
        let a = persistence.powi(i as i32);
        total += interpolated_noise(x * f, y * f) * a;
    }
    total
}

pub fn paint_effect(
    terrain: &mut crate::terrain::TerrainChannel,
    baked: Option<&[f32]>,
    action: PaintAction,
    rx: f32,
    ry: f32,
    rz: f32,
    size: f32,
    strength: f32,
) {
    let width = terrain.size_x();
    let height = terrain.size_y();
    if width == 0 || height == 0 || !(size > 0.0) {
        return;
    }

    let mut start_x = (rx - size + 0.5) as i32;
    if start_x < 0 {
        start_x = 0;
    }
    let mut start_y = (ry - size + 0.5) as i32;
    if start_y < 0 {
        start_y = 0;
    }
    let mut end_x = (rx + size + 0.5) as i32;
    if end_x > width as i32 - 1 {
        end_x = width as i32 - 1;
    }
    if end_x < 0 {
        end_x = 0;
    }
    let mut end_y = (ry + size + 0.5) as i32;
    if end_y > height as i32 - 1 {
        end_y = height as i32 - 1;
    }
    if end_y < 0 {
        end_y = 0;
    }

    match action {
        PaintAction::Raise => {
            let sz = 0.5 * PI / size;
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let x = xi as u32;
                    let y = yi as u32;
                    let dx = x as f32 - rx;
                    let dy = y as f32 - ry;
                    let r = (dx * dx + dy * dy).sqrt();
                    let f = (r * sz).cos();
                    if f > 0.0 {
                        let cur = terrain.get(x, y);
                        terrain.set(x, y, cur + f * strength);
                    }
                }
            }
        }
        PaintAction::Lower => {
            let sz = 0.5 * PI / size;
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let x = xi as u32;
                    let y = yi as u32;
                    let dx = x as f32 - rx;
                    let dy = y as f32 - ry;
                    let r = (dx * dx + dy * dy).sqrt();
                    let f = (r * sz).cos();
                    if f > 0.0 {
                        let cur = terrain.get(x, y);
                        let newz = cur - f * strength;
                        terrain.set(x, y, newz.max(0.0));
                    }
                }
            }
        }
        PaintAction::Flatten => {
            let mut sz = size;
            sz *= 2.0 * sz;
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let x = xi as u32;
                    let y = yi as u32;
                    let dx = x as f32 - rx;
                    let dy = y as f32 - ry;
                    let df = (dx * dx + dy * dy) / sz;
                    if df > 1.0 {
                        continue;
                    }
                    let df = strength * (1.0 - df);
                    let cur = terrain.get(x, y);
                    if df >= 1.0 {
                        terrain.set(x, y, rz);
                    } else {
                        terrain.set(x, y, cur + (rz - cur) * df);
                    }
                }
            }
        }
        PaintAction::Noise => {
            let mut sz = size;
            sz *= sz;
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let x = xi as u32;
                    let y = yi as u32;
                    let dx = x as f32 - rx;
                    let dy = y as f32 - ry;
                    let df = (dx * dx + dy * dy) / sz;
                    if df > 1.0 {
                        continue;
                    }
                    let df = strength * (1.0 - df);
                    let noise = perlin_noise_2d(
                        (x as f64) / (width as f64),
                        (y as f64) / (height as f64),
                        8,
                        1.0,
                    );
                    let cur = terrain.get(x, y);
                    terrain.set(x, y, cur + (noise as f32) * df);
                }
            }
        }
        PaintAction::Revert => {
            let baked_slice = match baked {
                Some(b) => b,
                None => return,
            };
            if strength < 0.0 {
                return;
            }
            let strength = strength.min(1.0);
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let x = xi as u32;
                    let y = yi as u32;
                    let dx = x as f32 - rx;
                    let dy = y as f32 - ry;
                    let df = (dx * dx + dy * dy) / size;
                    if df > 1.0 {
                        continue;
                    }
                    let df = strength * (1.0 - df);
                    let idx = (y * width + x) as usize;
                    let Some(&baked_h) = baked_slice.get(idx) else {
                        continue;
                    };
                    let cur = terrain.get(x, y);
                    terrain.set(x, y, cur * (1.0 - df) + baked_h * df);
                }
            }
        }
        PaintAction::Smooth => {
            let mut ssize = (size + 0.5) as i32;
            if ssize > 4 {
                ssize = 4;
            }
            let mut sz = size;
            sz *= sz;
            let strength = strength.min(1.0);
            let mut tweak = vec![0.0f32; (width * height) as usize];
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let x = xi as u32;
                    let y = yi as u32;
                    let dx = x as f32 - rx;
                    let dy = y as f32 - ry;
                    let df = (dx * dx + dy * dy) / sz;
                    if df <= 1.0 {
                        let df = strength * (1.0 - df);
                        let mut sum = 0.0f32;
                        let mut count = 0u32;
                        for n in (xi - ssize)..=(xi + ssize) {
                            if n < 0 || n >= width as i32 {
                                continue;
                            }
                            for l in (yi - ssize)..=(yi + ssize) {
                                if l < 0 || l >= height as i32 {
                                    continue;
                                }
                                sum += terrain.get(n as u32, l as u32);
                                count += 1;
                            }
                        }
                        let avg = if count > 0 {
                            sum / count as f32
                        } else {
                            0.0
                        };
                        let cur = terrain.get(x, y);
                        tweak[(y * width + x) as usize] = df * (cur - avg);
                    }
                }
            }
            for yi in start_y..=end_y {
                for xi in start_x..=end_x {
                    let idx = (yi as u32 * width + xi as u32) as usize;
                    let t = tweak[idx];
                    if t != 0.0 {
                        let x = xi as u32;
                        let y = yi as u32;
                        let cur = terrain.get(x, y);
                        let newz = cur - t;
                        if newz > 0.0 {
                            terrain.set(x, y, newz);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainChannel;

    fn flat(h: f32) -> TerrainChannel {
        let mut t = TerrainChannel::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                t.set(x, y, h);
            }
        }
        t
    }

    #[test]
    fn raise_center_and_leaves_far_cells() {
        let mut t = flat(10.0);
        paint_effect(&mut t, None, PaintAction::Raise, 16.0, 16.0, 0.0, 5.0, 0.5);
        assert!((t.get(16, 16) - 10.5).abs() < 1e-5);
        assert_eq!(t.get(0, 0), 10.0);
        assert_eq!(t.get(16, 24), 10.0, "outside the brush");
    }

    #[test]
    fn lower_clamps_at_zero() {
        let mut t = flat(0.1);
        paint_effect(&mut t, None, PaintAction::Lower, 16.0, 16.0, 0.0, 5.0, 1.0);
        assert_eq!(t.get(16, 16), 0.0);
    }

    #[test]
    fn flatten_center_reaches_target() {
        let mut t = flat(3.0);
        paint_effect(&mut t, None, PaintAction::Flatten, 16.0, 16.0, 7.0, 5.0, 1.0);
        assert_eq!(t.get(16, 16), 7.0);
    }

    #[test]
    fn smooth_reduces_a_spike() {
        let mut t = flat(1.0);
        t.set(16, 16, 10.0);
        paint_effect(&mut t, None, PaintAction::Smooth, 16.0, 16.0, 0.0, 2.0, 1.0);
        assert!(t.get(16, 16) < 10.0);
        assert!(t.get(16, 16) > 1.0);
    }

    #[test]
    fn revert_restores_baked_height() {
        let baked = vec![2.0f32; 32 * 32];
        let mut t = flat(9.0);
        paint_effect(&mut t, Some(&baked), PaintAction::Revert, 16.0, 16.0, 0.0, 4.0, 1.0);
        assert_eq!(t.get(16, 16), 2.0);
    }

    #[test]
    fn revert_without_baked_does_nothing() {
        let mut t = flat(9.0);
        paint_effect(&mut t, None, PaintAction::Revert, 16.0, 16.0, 0.0, 4.0, 1.0);
        assert_eq!(t.get(16, 16), 9.0);
    }

    #[test]
    fn noise_is_deterministic_and_changes_terrain() {
        assert_eq!(perlin_noise_2d(0.25, 0.75, 8, 1.0), perlin_noise_2d(0.25, 0.75, 8, 1.0));
        let mut t = flat(10.0);
        paint_effect(&mut t, None, PaintAction::Noise, 16.0, 16.0, 0.0, 5.0, 1.0);
        let changed = (0..32).flat_map(|y| (0..32).map(move |x| (x, y))).filter(|&(x, y)| t.get(x, y) != 10.0).count();
        assert!(changed > 0);
    }

    #[test]
    fn from_u8_rejects_unknown_actions() {
        assert!(PaintAction::from_u8(6).is_none());
        assert_eq!(PaintAction::from_u8(1), Some(PaintAction::Raise));
    }

    #[test]
    fn region_corners_and_bad_sizes_do_not_panic() {
        let mut t = TerrainChannel::new(256, 256);
        for a in 0..6u8 {
            let action = PaintAction::from_u8(a).unwrap();
            paint_effect(&mut t, None, action, 0.0, 0.0, 0.0, 5.0, 1.0);
            paint_effect(&mut t, None, action, 255.0, 255.0, 0.0, 5.0, 1.0);
            paint_effect(&mut t, None, action, -50.0, 400.0, 0.0, 5.0, 1.0);
            paint_effect(&mut t, None, action, 10.0, 10.0, 0.0, 0.0, 1.0);
        }
    }
}
