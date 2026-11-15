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
            (
                (uy - lz) / s,
                (fz - ux) / s,
                (lx - fy) / s,
                0.25 * s,
            )
        } else if fx > ly && fx > uz {
            let s = (1.0 + fx - ly - uz).sqrt() * 2.0;
            (
                0.25 * s,
                (fy + lx) / s,
                (fz + ux) / s,
                (uy - lz) / s,
            )
        } else if ly > uz {
            let s = (1.0 + ly - fx - uz).sqrt() * 2.0;
            (
                (fy + lx) / s,
                0.25 * s,
                (lz + uy) / s,
                (fz - ux) / s,
            )
        } else {
            let s = (1.0 + uz - fx - ly).sqrt() * 2.0;
            (
                (fz + ux) / s,
                (lz + uy) / s,
                0.25 * s,
                (lx - fy) / s,
            )
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
        assert!(r >= 0.0 && r < 10.0, "llFrand result {r} out of range");
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
        let r: f64 = lua
            .load("return llLog(2.718281828459045)")
            .eval()
            .unwrap();
        assert!((r - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_mag() {
        let lua = setup();
        let r: f64 = lua
            .load("return llVecMag({x=3, y=4, z=0})")
            .eval()
            .unwrap();
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
        assert!(r.abs() < 1e-12, "angle between identical rotations should be 0, got {r}");
    }
}
