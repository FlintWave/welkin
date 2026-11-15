// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL math built-in functions registered as Lua globals.

use std::cell::Cell;

use mlua::prelude::*;

// ---------------------------------------------------------------------------
// Thread-local LCG for llFrand
// ---------------------------------------------------------------------------

thread_local! {
    static RNG_STATE: Cell<u64> = const { Cell::new(12345) };
}

fn pseudo_rand() -> f64 {
    RNG_STATE.with(|s| {
        let state = s
            .get()
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        s.set(state);
        (state >> 33) as f64 / (1u64 << 31) as f64
    })
}

// ---------------------------------------------------------------------------
// Vector / rotation helpers
// ---------------------------------------------------------------------------

pub(crate) fn get_vec3(val: &LuaValue) -> Result<[f64; 3], mlua::Error> {
    match val {
        LuaValue::Table(t) => {
            let x: f64 = t.get("x").or_else(|_| t.get(1))?;
            let y: f64 = t.get("y").or_else(|_| t.get(2))?;
            let z: f64 = t.get("z").or_else(|_| t.get(3))?;
            Ok([x, y, z])
        }
        _ => Err(mlua::Error::runtime("expected vector table")),
    }
}

pub(crate) fn make_vec3(lua: &Lua, v: [f64; 3]) -> Result<LuaTable, mlua::Error> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    Ok(t)
}

pub(crate) fn get_rot(val: &LuaValue) -> Result<[f64; 4], mlua::Error> {
    match val {
        LuaValue::Table(t) => {
            let x: f64 = t.get("x").or_else(|_| t.get(1))?;
            let y: f64 = t.get("y").or_else(|_| t.get(2))?;
            let z: f64 = t.get("z").or_else(|_| t.get(3))?;
            let w: f64 = t.get("w").or_else(|_| t.get(4))?;
            Ok([x, y, z, w])
        }
        _ => Err(mlua::Error::runtime("expected rotation table")),
    }
}

pub(crate) fn make_rot(lua: &Lua, r: [f64; 4]) -> Result<LuaTable, mlua::Error> {
    let t = lua.create_table()?;
    t.set("x", r[0])?;
    t.set("y", r[1])?;
    t.set("z", r[2])?;
    t.set("w", r[3])?;
    Ok(t)
}

// ---------------------------------------------------------------------------
// Quaternion <-> Euler helpers (standard aerospace / Tait-Bryan ZYX)
// ---------------------------------------------------------------------------

/// Quaternion (x,y,z,w) -> Euler angles (roll, pitch, yaw) as a vec3.
fn quat_to_euler(q: [f64; 4]) -> [f64; 3] {
    let [x, y, z, w] = q;

    // Roll (x-axis rotation)
    let sinr_cosp = 2.0 * (w * x + y * z);
    let cosr_cosp = 1.0 - 2.0 * (x * x + y * y);
    let roll = sinr_cosp.atan2(cosr_cosp);

    // Pitch (y-axis rotation)
    let sinp = 2.0 * (w * y - z * x);
    let pitch = if sinp.abs() >= 1.0 {
        std::f64::consts::FRAC_PI_2.copysign(sinp)
    } else {
        sinp.asin()
    };

    // Yaw (z-axis rotation)
    let siny_cosp = 2.0 * (w * z + x * y);
    let cosy_cosp = 1.0 - 2.0 * (y * y + z * z);
    let yaw = siny_cosp.atan2(cosy_cosp);

    [roll, pitch, yaw]
}

/// Euler angles (roll, pitch, yaw) -> Quaternion (x,y,z,w).
fn euler_to_quat(e: [f64; 3]) -> [f64; 4] {
    let [roll, pitch, yaw] = e;
    let (sr, cr) = (roll * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    let (sy, cy) = (yaw * 0.5).sin_cos();

    let x = sr * cp * cy - cr * sp * sy;
    let y = cr * sp * cy + sr * cp * sy;
    let z = cr * cp * sy - sr * sp * cy;
    let w = cr * cp * cy + sr * sp * sy;
    [x, y, z, w]
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all LSL math built-in functions on the Lua VM.
pub fn register_math(lua: &Lua) -> Result<(), crate::VmError> {
    let g = lua.globals();

    // -- Scalar functions ---------------------------------------------------

    let f = lua.create_function(|_, val: f64| Ok(val.abs()))?;
    g.set("llAbs", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.ceil()))?;
    g.set("llCeil", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.floor()))?;
    g.set("llFloor", f)?;

    let f = lua.create_function(|_, mag: f64| Ok(mag * pseudo_rand()))?;
    g.set("llFrand", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.ln()))?;
    g.set("llLog", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.log10()))?;
    g.set("llLog10", f)?;

    let f = lua.create_function(|_, (base, exp): (f64, f64)| Ok(base.powf(exp)))?;
    g.set("llPow", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.sqrt()))?;
    g.set("llSqrt", f)?;

    let f = lua.create_function(|_, theta: f64| Ok(theta.sin()))?;
    g.set("llSin", f)?;

    let f = lua.create_function(|_, theta: f64| Ok(theta.cos()))?;
    g.set("llCos", f)?;

    let f = lua.create_function(|_, theta: f64| Ok(theta.tan()))?;
    g.set("llTan", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.asin()))?;
    g.set("llAsin", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.acos()))?;
    g.set("llAcos", f)?;

    let f = lua.create_function(|_, (y, x): (f64, f64)| Ok(y.atan2(x)))?;
    g.set("llAtan2", f)?;

    let f = lua.create_function(|_, val: f64| Ok(val.round()))?;
    g.set("llRound", f)?;

    // -- Vector functions ---------------------------------------------------

    let f = lua.create_function(|_, v: LuaValue| {
        let [x, y, z] = get_vec3(&v)?;
        Ok((x * x + y * y + z * z).sqrt())
    })?;
    g.set("llVecMag", f)?;

    let f = lua.create_function(|lua, v: LuaValue| {
        let [x, y, z] = get_vec3(&v)?;
        let mag = (x * x + y * y + z * z).sqrt();
        if mag > 0.0 {
            make_vec3(lua, [x / mag, y / mag, z / mag])
        } else {
            make_vec3(lua, [0.0, 0.0, 0.0])
        }
    })?;
    g.set("llVecNorm", f)?;

    let f = lua.create_function(|_, (a, b): (LuaValue, LuaValue)| {
        let [ax, ay, az] = get_vec3(&a)?;
        let [bx, by, bz] = get_vec3(&b)?;
        let dx = ax - bx;
        let dy = ay - by;
        let dz = az - bz;
        Ok((dx * dx + dy * dy + dz * dz).sqrt())
    })?;
    g.set("llVecDist", f)?;

    // -- Rotation functions -------------------------------------------------

    let f = lua.create_function(|lua, r: LuaValue| {
        let q = get_rot(&r)?;
        let euler = quat_to_euler(q);
        make_vec3(lua, euler)
    })?;
    g.set("llRot2Euler", f)?;

    let f = lua.create_function(|lua, e: LuaValue| {
        let euler = get_vec3(&e)?;
        let q = euler_to_quat(euler);
        make_rot(lua, q)
    })?;
    g.set("llEuler2Rot", f)?;

    // llAxes2Rot — build rotation from three orthogonal axes.
    // Full implementation: extract rotation matrix from the axes and convert.
    let f = lua.create_function(|lua, (fwd, left, up): (LuaValue, LuaValue, LuaValue)| {
        let [fx, fy, fz] = get_vec3(&fwd)?;
        let [lx, ly, lz] = get_vec3(&left)?;
        let [ux, uy, uz] = get_vec3(&up)?;

        // Rotation matrix rows: fwd=col0, left=col1, up=col2
        // Using Shepperd's method for matrix-to-quaternion conversion.
        let trace = fx + ly + uz;
        let (x, y, z, w) = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            ((uy - lz) / s, (fz - ux) / s, (lx - fy) / s, 0.25 * s)
        } else if fx > ly && fx > uz {
            let s = (1.0 + fx - ly - uz).sqrt() * 2.0;
            (0.25 * s, (fy + lx) / s, (fz + ux) / s, (uy - lz) / s)
        } else if ly > uz {
            let s = (1.0 + ly - fx - uz).sqrt() * 2.0;
            ((fy + lx) / s, 0.25 * s, (lz + uy) / s, (fz - ux) / s)
        } else {
            let s = (1.0 + uz - fx - ly).sqrt() * 2.0;
            ((fz + ux) / s, (lz + uy) / s, 0.25 * s, (lx - fy) / s)
        };
        make_rot(lua, [x, y, z, w])
    })?;
    g.set("llAxes2Rot", f)?;

    let f = lua.create_function(|_, (a, b): (LuaValue, LuaValue)| {
        let [ax, ay, az, aw] = get_rot(&a)?;
        let [bx, by, bz, bw] = get_rot(&b)?;
        let dot = (ax * bx + ay * by + az * bz + aw * bw).abs();
        // Clamp to [-1, 1] to avoid NaN from acos
        let dot = dot.clamp(0.0, 1.0);
        Ok(2.0 * dot.acos())
    })?;
    g.set("llAngleBetween", f)?;

    // llRot2Angle — angle of rotation in radians
    let f = lua.create_function(|_, r: LuaValue| {
        let [_, _, _, w] = get_rot(&r)?;
        Ok(2.0 * w.abs().min(1.0).acos())
    })?;
    g.set("llRot2Angle", f)?;

    // llRot2Axis — axis of rotation as a unit vector
    let f = lua.create_function(|lua, r: LuaValue| {
        let [x, y, z, w] = get_rot(&r)?;
        let angle = 2.0 * w.abs().min(1.0).acos();
        let s = (angle / 2.0).sin();
        if s.abs() < 1e-10 {
            make_vec3(lua, [0.0, 0.0, 1.0])
        } else {
            let sign = if w < 0.0 { -1.0 } else { 1.0 };
            make_vec3(lua, [sign * x / s, sign * y / s, sign * z / s])
        }
    })?;
    g.set("llRot2Axis", f)?;

    // llRotBetween — quaternion that rotates v1 to v2
    let f = lua.create_function(|lua, (v1, v2): (LuaValue, LuaValue)| {
        let a = get_vec3(&v1)?;
        let b = get_vec3(&v2)?;
        let mag_a = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let mag_b = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
        if mag_a < 1e-10 || mag_b < 1e-10 {
            return make_rot(lua, [0.0, 0.0, 0.0, 1.0]);
        }
        let an = [a[0] / mag_a, a[1] / mag_a, a[2] / mag_a];
        let bn = [b[0] / mag_b, b[1] / mag_b, b[2] / mag_b];
        let dot = an[0] * bn[0] + an[1] * bn[1] + an[2] * bn[2];
        let cross = [
            an[1] * bn[2] - an[2] * bn[1],
            an[2] * bn[0] - an[0] * bn[2],
            an[0] * bn[1] - an[1] * bn[0],
        ];
        if dot < -0.999999 {
            // Antiparallel — pick a perpendicular axis
            let perp = if an[0].abs() < 0.9 {
                [0.0, -an[2], an[1]]
            } else {
                [an[2], 0.0, -an[0]]
            };
            let mag = (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt();
            return make_rot(lua, [perp[0] / mag, perp[1] / mag, perp[2] / mag, 0.0]);
        }
        let w = (1.0 + dot).sqrt() * 0.5;
        let s = 1.0 / (4.0 * w);
        let qx = cross[0] * s;
        let qy = cross[1] * s;
        let qz = cross[2] * s;
        let mag = (qx * qx + qy * qy + qz * qz + w * w).sqrt();
        make_rot(lua, [qx / mag, qy / mag, qz / mag, w / mag])
    })?;
    g.set("llRotBetween", f)?;

    // llGetUnixTime — seconds since Unix epoch
    let f = lua.create_function(|_, ()| {
        use chrono::Utc;
        Ok(Utc::now().timestamp())
    })?;
    g.set("llGetUnixTime", f)?;

    // llGetTimestamp — UTC timestamp string
    let f = lua.create_function(|_, ()| {
        use chrono::Utc;
        Ok(Utc::now().format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string())
    })?;
    g.set("llGetTimestamp", f)?;

    // llGetGMTclock — seconds since midnight UTC
    let f = lua.create_function(|_, ()| {
        use chrono::{Timelike, Utc};
        let now = Utc::now();
        Ok((now.hour() * 3600 + now.minute() * 60 + now.second()) as f64)
    })?;
    g.set("llGetGMTclock", f)?;

    // llGetWallclock — same as GMTclock in LSL (no timezone)
    let f = lua.create_function(|_, ()| {
        use chrono::{Timelike, Utc};
        let now = Utc::now();
        Ok((now.hour() * 3600 + now.minute() * 60 + now.second()) as f64)
    })?;
    g.set("llGetWallclock", f)?;

    // llGenerateKey — new random UUID
    let f = lua.create_function(|_, ()| Ok(uuid::Uuid::new_v4().to_string()))?;
    g.set("llGenerateKey", f)?;

    // -- LIST_STAT_* constants ------------------------------------------------
    g.set("LIST_STAT_RANGE", 0i64)?;
    g.set("LIST_STAT_MIN", 1i64)?;
    g.set("LIST_STAT_MAX", 2i64)?;
    g.set("LIST_STAT_MEAN", 3i64)?;
    g.set("LIST_STAT_MEDIAN", 4i64)?;
    g.set("LIST_STAT_STD_DEV", 5i64)?;
    g.set("LIST_STAT_SUM", 6i64)?;
    g.set("LIST_STAT_SUM_SQUARES", 7i64)?;
    g.set("LIST_STAT_NUM_COUNT", 8i64)?;
    g.set("LIST_STAT_GEOMETRIC_MEAN", 9i64)?;

    // llListStatistics(operation, list) -> float
    let f = lua.create_function(|_, (op, list): (i64, LuaTable)| {
        let mut nums: Vec<f64> = Vec::new();
        for pair in list.sequence_values::<LuaValue>() {
            match pair? {
                LuaValue::Integer(i) => nums.push(i as f64),
                LuaValue::Number(n) => nums.push(n),
                _ => {}
            }
        }
        if nums.is_empty() {
            return Ok(0.0);
        }
        let result = match op {
            0 => {
                // RANGE
                let min = nums.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                max - min
            }
            1 => nums.iter().cloned().fold(f64::INFINITY, f64::min),       // MIN
            2 => nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),   // MAX
            3 => nums.iter().sum::<f64>() / nums.len() as f64,            // MEAN
            4 => {
                // MEDIAN
                let mut sorted = nums.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = sorted.len() / 2;
                if sorted.len() % 2 == 0 {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                }
            }
            5 => {
                // STD_DEV
                let mean = nums.iter().sum::<f64>() / nums.len() as f64;
                let variance = nums.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / nums.len() as f64;
                variance.sqrt()
            }
            6 => nums.iter().sum(),                                        // SUM
            7 => nums.iter().map(|v| v * v).sum(),                         // SUM_SQUARES
            8 => nums.len() as f64,                                        // NUM_COUNT
            9 => {
                // GEOMETRIC_MEAN
                let product: f64 = nums.iter().product();
                product.powf(1.0 / nums.len() as f64)
            }
            _ => 0.0,
        };
        Ok(result)
    })?;
    g.set("llListStatistics", f)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Lua {
        let lua = Lua::new();
        register_math(&lua).expect("register_math failed");
        lua
    }

    #[test]
    fn test_abs() {
        let lua = setup();
        let r: f64 = lua.load("return llAbs(-5.0)").eval().unwrap();
        assert!((r - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ceil() {
        let lua = setup();
        let r: f64 = lua.load("return llCeil(2.3)").eval().unwrap();
        assert!((r - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_floor() {
        let lua = setup();
        let r: f64 = lua.load("return llFloor(2.9)").eval().unwrap();
        assert!((r - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_frand() {
        let lua = setup();
        let r: f64 = lua.load("return llFrand(10.0)").eval().unwrap();
        assert!((0.0..10.0).contains(&r), "llFrand result {r} out of range");
    }

    #[test]
    fn test_sqrt() {
        let lua = setup();
        let r: f64 = lua.load("return llSqrt(4.0)").eval().unwrap();
        assert!((r - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pow() {
        let lua = setup();
        let r: f64 = lua.load("return llPow(2.0, 10.0)").eval().unwrap();
        assert!((r - 1024.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cos_zero() {
        let lua = setup();
        let r: f64 = lua.load("return llCos(0.0)").eval().unwrap();
        assert!((r - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_sin_zero() {
        let lua = setup();
        let r: f64 = lua.load("return llSin(0.0)").eval().unwrap();
        assert!(r.abs() < 1e-12);
    }

    #[test]
    fn test_round() {
        let lua = setup();
        let r: f64 = lua.load("return llRound(2.7)").eval().unwrap();
        assert!((r - 3.0).abs() < f64::EPSILON);
        let r2: f64 = lua.load("return llRound(2.3)").eval().unwrap();
        assert!((r2 - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_atan2() {
        let lua = setup();
        let r: f64 = lua.load("return llAtan2(1.0, 1.0)").eval().unwrap();
        assert!((r - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
    }

    #[test]
    fn test_log() {
        let lua = setup();
        let r: f64 = lua.load("return llLog(2.718281828459045)").eval().unwrap();
        assert!((r - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_mag() {
        let lua = setup();
        let r: f64 = lua.load("return llVecMag({x=3, y=4, z=0})").eval().unwrap();
        assert!((r - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_vec_norm() {
        let lua = setup();
        let r: f64 = lua
            .load(
                r#"
                local n = llVecNorm({x=3, y=0, z=0})
                return n.x
            "#,
            )
            .eval()
            .unwrap();
        assert!((r - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_vec_dist() {
        let lua = setup();
        let r: f64 = lua
            .load("return llVecDist({x=0,y=0,z=0}, {x=3,y=4,z=0})")
            .eval()
            .unwrap();
        assert!((r - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_euler_rot_roundtrip() {
        let lua = setup();
        // Convert euler -> quat -> euler, should round-trip.
        let r: f64 = lua
            .load(
                r#"
                local e = {x=0.5, y=0.3, z=0.1}
                local q = llEuler2Rot(e)
                local e2 = llRot2Euler(q)
                -- Check x component round-trips
                return math.abs(e2.x - e.x)
            "#,
            )
            .eval()
            .unwrap();
        assert!(r < 1e-10, "euler round-trip error: {r}");
    }

    #[test]
    fn test_angle_between_same() {
        let lua = setup();
        let r: f64 = lua
            .load("return llAngleBetween({x=0,y=0,z=0,w=1}, {x=0,y=0,z=0,w=1})")
            .eval()
            .unwrap();
        assert!(
            r.abs() < 1e-12,
            "angle between identical rotations should be 0, got {r}"
        );
    }

    #[test]
    fn test_rot2_angle_identity() {
        let lua = setup();
        let r: f64 = lua
            .load("return llRot2Angle({x=0,y=0,z=0,w=1})")
            .eval()
            .unwrap();
        assert!(r.abs() < 1e-12, "identity rotation angle should be 0, got {r}");
    }

    #[test]
    fn test_rot2_axis_identity() {
        let lua = setup();
        let z: f64 = lua
            .load("return llRot2Axis({x=0,y=0,z=0,w=1}).z")
            .eval()
            .unwrap();
        assert!((z - 1.0).abs() < 1e-12, "identity axis z should be 1, got {z}");
    }

    #[test]
    fn test_rot_between() {
        let lua = setup();
        let angle: f64 = lua
            .load(
                r#"
                local q = llRotBetween({x=1,y=0,z=0}, {x=0,y=1,z=0})
                return llRot2Angle(q)
            "#,
            )
            .eval()
            .unwrap();
        assert!(
            (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
            "rotation from x to y should be pi/2, got {angle}"
        );
    }

    #[test]
    fn test_get_unix_time() {
        let lua = setup();
        let t: i64 = lua.load("return llGetUnixTime()").eval().unwrap();
        assert!(t > 1_700_000_000, "unix time should be recent, got {t}");
    }

    #[test]
    fn test_get_timestamp() {
        let lua = setup();
        let s: String = lua.load("return llGetTimestamp()").eval().unwrap();
        assert!(s.contains('T'), "timestamp should contain T: {s}");
        assert!(s.ends_with('Z'), "timestamp should end with Z: {s}");
    }

    #[test]
    fn test_get_gmt_clock() {
        let lua = setup();
        let r: f64 = lua.load("return llGetGMTclock()").eval().unwrap();
        assert!(r >= 0.0 && r < 86400.0, "GMTclock should be 0..86400, got {r}");
    }

    #[test]
    fn test_generate_key() {
        let lua = setup();
        let s: String = lua.load("return llGenerateKey()").eval().unwrap();
        assert_eq!(s.len(), 36, "UUID should be 36 chars, got {}", s.len());
    }

    #[test]
    fn test_axes2rot_identity() {
        let lua = setup();
        let script = r#"
            local q = llAxes2Rot({x=1,y=0,z=0}, {x=0,y=1,z=0}, {x=0,y=0,z=1})
            return q.x, q.y, q.z, q.w
        "#;
        let (x, y, z, w): (f64, f64, f64, f64) = lua.load(script).eval().unwrap();
        assert!(x.abs() < 1e-10, "identity x should be ~0, got {x}");
        assert!(y.abs() < 1e-10, "identity y should be ~0, got {y}");
        assert!(z.abs() < 1e-10, "identity z should be ~0, got {z}");
        assert!((w - 1.0).abs() < 1e-10, "identity w should be ~1, got {w}");
    }

    #[test]
    fn test_list_statistics_mean() {
        let lua = setup();
        let r: f64 = lua
            .load("return llListStatistics(LIST_STAT_MEAN, {10, 20, 30})")
            .eval()
            .unwrap();
        assert!((r - 20.0).abs() < 1e-10, "mean of 10,20,30 should be 20, got {r}");
    }

    #[test]
    fn test_list_statistics_sum() {
        let lua = setup();
        let r: f64 = lua
            .load("return llListStatistics(LIST_STAT_SUM, {1, 2, 3, 4})")
            .eval()
            .unwrap();
        assert!((r - 10.0).abs() < 1e-10, "sum should be 10, got {r}");
    }

    #[test]
    fn test_list_statistics_min() {
        let lua = setup();
        let r: f64 = lua
            .load("return llListStatistics(LIST_STAT_MIN, {5, 2, 8})")
            .eval()
            .unwrap();
        assert!((r - 2.0).abs() < 1e-10, "min should be 2, got {r}");
    }

    #[test]
    fn test_list_statistics_max() {
        let lua = setup();
        let r: f64 = lua
            .load("return llListStatistics(LIST_STAT_MAX, {5, 2, 8})")
            .eval()
            .unwrap();
        assert!((r - 8.0).abs() < 1e-10, "max should be 8, got {r}");
    }

    #[test]
    fn test_list_statistics_median_odd() {
        let lua = setup();
        let r: f64 = lua
            .load("return llListStatistics(LIST_STAT_MEDIAN, {3, 1, 2})")
            .eval()
            .unwrap();
        assert!((r - 2.0).abs() < 1e-10, "median of 1,2,3 should be 2, got {r}");
    }

    #[test]
    fn test_list_statistics_std_dev() {
        let lua = setup();
        let r: f64 = lua
            .load("return llListStatistics(LIST_STAT_STD_DEV, {2, 4, 4, 4, 5, 5, 7, 9})")
            .eval()
            .unwrap();
        assert!((r - 2.0).abs() < 1e-10, "std dev should be 2.0, got {r}");
    }
}
