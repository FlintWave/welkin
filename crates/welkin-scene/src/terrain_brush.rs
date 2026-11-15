// SPDX-License-Identifier: GPL-3.0-or-later

use crate::terrain::TerrainChannel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushType {
    Raise,
    Lower,
    Flatten,
    Smooth,
    Noise,
    Revert,
}

#[derive(Debug, Clone)]
pub struct BrushParams {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub strength: f32,
}

fn falloff(dist: f32, radius: f32) -> f32 {
    (1.0 - (dist / radius).powi(2)).max(0.0)
}

fn deterministic_noise(x: u32, y: u32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 32768.0 - 1.0
}

fn neighbor_average(terrain: &TerrainChannel, x: u32, y: u32) -> f32 {
    let sx = terrain.size_x();
    let sy = terrain.size_y();
    let mut sum = 0.0f32;
    let mut count = 0u32;

    if x > 0 {
        sum += terrain.get(x - 1, y);
        count += 1;
    }
    if x + 1 < sx {
        sum += terrain.get(x + 1, y);
        count += 1;
    }
    if y > 0 {
        sum += terrain.get(x, y - 1);
        count += 1;
    }
    if y + 1 < sy {
        sum += terrain.get(x, y + 1);
        count += 1;
    }

    if count > 0 {
        sum / count as f32
    } else {
        terrain.get(x, y)
    }
}

pub fn apply_brush(
    terrain: &mut TerrainChannel,
    brush: BrushType,
    params: &BrushParams,
    baked: Option<&TerrainChannel>,
) {
    let sx = terrain.size_x();
    let sy = terrain.size_y();

    let x_min = ((params.center_x - params.radius).floor().max(0.0)) as u32;
    let x_max = ((params.center_x + params.radius).ceil() as u32).min(sx.saturating_sub(1));
    let y_min = ((params.center_y - params.radius).floor().max(0.0)) as u32;
    let y_max = ((params.center_y + params.radius).ceil() as u32).min(sy.saturating_sub(1));

    let flatten_target = match brush {
        BrushType::Flatten => {
            let cx = (params.center_x as u32).min(sx.saturating_sub(1));
            let cy = (params.center_y as u32).min(sy.saturating_sub(1));
            Some(terrain.get(cx, cy))
        }
        _ => None,
    };

    let mut smooth_averages: Vec<(u32, u32, f32)> = Vec::new();
    if brush == BrushType::Smooth {
        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as f32 - params.center_x;
                let dy = y as f32 - params.center_y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= params.radius {
                    smooth_averages.push((x, y, neighbor_average(terrain, x, y)));
                }
            }
        }
    }

    match brush {
        BrushType::Smooth => {
            for (x, y, avg) in smooth_averages {
                let dx = x as f32 - params.center_x;
                let dy = y as f32 - params.center_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let f = falloff(dist, params.radius);
                let t = params.strength * f;
                let h = terrain.get(x, y);
                terrain.set(x, y, h + (avg - h) * t);
            }
        }
        _ => {
            for y in y_min..=y_max {
                for x in x_min..=x_max {
                    let dx = x as f32 - params.center_x;
                    let dy = y as f32 - params.center_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > params.radius {
                        continue;
                    }
                    let f = falloff(dist, params.radius);
                    let h = terrain.get(x, y);

                    let new_h = match brush {
                        BrushType::Raise => h + params.strength * f,
                        BrushType::Lower => h - params.strength * f,
                        BrushType::Flatten => {
                            let target = flatten_target.unwrap();
                            h + (target - h) * params.strength * f
                        }
                        BrushType::Noise => h + deterministic_noise(x, y) * params.strength * f,
                        BrushType::Revert => {
                            if let Some(b) = baked {
                                let bh = b.get(x, y);
                                h + (bh - h) * params.strength * f
                            } else {
                                h
                            }
                        }
                        BrushType::Smooth => unreachable!(),
                    };

                    terrain.set(x, y, new_h);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_terrain(height: f32) -> TerrainChannel {
        let mut t = TerrainChannel::new(256, 256);
        for y in 0..256 {
            for x in 0..256 {
                t.set(x, y, height);
            }
        }
        t
    }

    #[test]
    fn test_raise_brush() {
        let mut terrain = flat_terrain(20.0);
        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 5.0,
            strength: 2.0,
        };

        let before = terrain.get(128, 128);
        apply_brush(&mut terrain, BrushType::Raise, &params, None);
        let after = terrain.get(128, 128);
        assert!(
            after > before,
            "center should be raised: {after} > {before}"
        );
    }

    #[test]
    fn test_lower_brush() {
        let mut terrain = flat_terrain(20.0);
        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 5.0,
            strength: 2.0,
        };

        let before = terrain.get(128, 128);
        apply_brush(&mut terrain, BrushType::Lower, &params, None);
        let after = terrain.get(128, 128);
        assert!(
            after < before,
            "center should be lowered: {after} < {before}"
        );
    }

    #[test]
    fn test_flatten_brush() {
        let mut terrain = flat_terrain(20.0);
        terrain.set(130, 128, 30.0);
        terrain.set(126, 128, 10.0);

        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 10.0,
            strength: 1.0,
        };

        let center_h = terrain.get(128, 128);
        apply_brush(&mut terrain, BrushType::Flatten, &params, None);

        let high_after = terrain.get(130, 128);
        let low_after = terrain.get(126, 128);
        assert!(
            (high_after - center_h).abs() < (30.0 - center_h).abs(),
            "high point should move toward center"
        );
        assert!(
            (low_after - center_h).abs() < (10.0 - center_h).abs(),
            "low point should move toward center"
        );
    }

    #[test]
    fn test_smooth_brush() {
        let mut terrain = flat_terrain(20.0);
        terrain.set(128, 128, 100.0);

        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 5.0,
            strength: 1.0,
        };

        apply_brush(&mut terrain, BrushType::Smooth, &params, None);
        let after = terrain.get(128, 128);
        assert!(
            after < 100.0,
            "spike should be reduced by smoothing: {after} < 100.0"
        );
    }

    #[test]
    fn test_brush_falloff() {
        let mut terrain = flat_terrain(20.0);
        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 10.0,
            strength: 5.0,
        };

        apply_brush(&mut terrain, BrushType::Raise, &params, None);

        let center_delta = terrain.get(128, 128) - 20.0;
        let edge_delta = terrain.get(137, 128) - 20.0;

        assert!(
            center_delta > edge_delta,
            "center should be affected more than edge: {center_delta} > {edge_delta}"
        );
        assert!(
            edge_delta > 0.0,
            "edge within radius should still be raised"
        );

        let outside = terrain.get(140, 128);
        assert!(
            (outside - 20.0).abs() < f32::EPSILON,
            "point outside radius should be unaffected"
        );
    }

    #[test]
    fn test_noise_deterministic() {
        let mut t1 = flat_terrain(20.0);
        let mut t2 = flat_terrain(20.0);
        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 5.0,
            strength: 1.0,
        };

        apply_brush(&mut t1, BrushType::Noise, &params, None);
        apply_brush(&mut t2, BrushType::Noise, &params, None);

        assert_eq!(
            t1.get(128, 128),
            t2.get(128, 128),
            "noise should be deterministic"
        );
    }

    #[test]
    fn test_revert_brush() {
        let baked = flat_terrain(20.0);
        let mut terrain = flat_terrain(50.0);

        let params = BrushParams {
            center_x: 128.0,
            center_y: 128.0,
            radius: 5.0,
            strength: 1.0,
        };

        apply_brush(&mut terrain, BrushType::Revert, &params, Some(&baked));
        let after = terrain.get(128, 128);
        assert!(
            (after - 20.0).abs() < 0.01,
            "full-strength revert at center should approach baked height: {after}"
        );
    }
}
