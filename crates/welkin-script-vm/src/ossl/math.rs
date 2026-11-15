// SPDX-License-Identifier: GPL-3.0-or-later
//! OSSL math and utility functions (threat level None).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;

pub(crate) fn math_functions() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osMin",
            level: OsslThreatLevel::None,
            register: register_os_min,
        },
        OsslFunc {
            name: "osMax",
            level: OsslThreatLevel::None,
            register: register_os_max,
        },
        OsslFunc {
            name: "osIsUUID",
            level: OsslThreatLevel::None,
            register: register_os_is_uuid,
        },
        OsslFunc {
            name: "osRound",
            level: OsslThreatLevel::None,
            register: register_os_round,
        },
        OsslFunc {
            name: "osVecMagSquare",
            level: OsslThreatLevel::None,
            register: register_os_vec_mag_square,
        },
        OsslFunc {
            name: "osVecDistSquare",
            level: OsslThreatLevel::None,
            register: register_os_vec_dist_square,
        },
        OsslFunc {
            name: "osAngleBetween",
            level: OsslThreatLevel::None,
            register: register_os_angle_between,
        },
        OsslFunc {
            name: "osApproxEquals",
            level: OsslThreatLevel::None,
            register: register_os_approx_equals,
        },
        OsslFunc {
            name: "osIsNotValidNumber",
            level: OsslThreatLevel::None,
            register: register_os_is_not_valid_number,
        },
        OsslFunc {
            name: "osSlerp",
            level: OsslThreatLevel::None,
            register: register_os_slerp,
        },
        OsslFunc {
            name: "osOldList2ListStrided",
            level: OsslThreatLevel::None,
            register: register_os_old_list2list_strided,
        },
        OsslFunc {
            name: "osListSortInPlace",
            level: OsslThreatLevel::None,
            register: register_os_list_sort_in_place,
        },
        OsslFunc {
            name: "osListSortInPlaceStrided",
            level: OsslThreatLevel::None,
            register: register_os_list_sort_in_place_strided,
        },
    ]
}

// ---------------------------------------------------------------------------
// osMin / osMax
// ---------------------------------------------------------------------------

fn register_os_min(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (a, b): (f64, f64)| Ok(a.min(b)))?;
    lua.globals().set("osMin", f)
}

fn register_os_max(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (a, b): (f64, f64)| Ok(a.max(b)))?;
    lua.globals().set("osMax", f)
}

// ---------------------------------------------------------------------------
// osIsUUID
// ---------------------------------------------------------------------------

fn register_os_is_uuid(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f =
        lua.create_function(|_, s: String| Ok(if Uuid::parse_str(&s).is_ok() { 1i32 } else { 0 }))?;
    lua.globals().set("osIsUUID", f)
}

// ---------------------------------------------------------------------------
// osRound
// ---------------------------------------------------------------------------

fn register_os_round(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (value, digits): (f64, i32)| {
        let factor = 10f64.powi(digits);
        Ok((value * factor).round() / factor)
    })?;
    lua.globals().set("osRound", f)
}

// ---------------------------------------------------------------------------
// osVecMagSquare — squared magnitude of a 3-vector
// ---------------------------------------------------------------------------

fn register_os_vec_mag_square(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, v: Vec<f64>| {
        if v.len() < 3 {
            return Err(mlua::Error::RuntimeError(
                "osVecMagSquare: need 3-component vector".into(),
            ));
        }
        Ok(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
    })?;
    lua.globals().set("osVecMagSquare", f)
}

// ---------------------------------------------------------------------------
// osVecDistSquare — squared distance between two 3-vectors
// ---------------------------------------------------------------------------

fn register_os_vec_dist_square(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (a, b): (Vec<f64>, Vec<f64>)| {
        if a.len() < 3 || b.len() < 3 {
            return Err(mlua::Error::RuntimeError(
                "osVecDistSquare: need 3-component vectors".into(),
            ));
        }
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        Ok(dx * dx + dy * dy + dz * dz)
    })?;
    lua.globals().set("osVecDistSquare", f)
}

// ---------------------------------------------------------------------------
// osAngleBetween — angle between two rotations (quaternions)
// ---------------------------------------------------------------------------

fn register_os_angle_between(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (r1, r2): (Vec<f64>, Vec<f64>)| {
        if r1.len() < 4 || r2.len() < 4 {
            return Err(mlua::Error::RuntimeError(
                "osAngleBetween: need 4-component quaternions".into(),
            ));
        }
        // dot product of quaternions
        let dot = r1[0] * r2[0] + r1[1] * r2[1] + r1[2] * r2[2] + r1[3] * r2[3];
        // clamp to [-1, 1] to avoid NaN from acos
        let angle = 2.0 * dot.abs().clamp(0.0, 1.0).acos();
        Ok(angle)
    })?;
    lua.globals().set("osAngleBetween", f)
}

// ---------------------------------------------------------------------------
// osApproxEquals — approximate float/vector/rotation comparison
// ---------------------------------------------------------------------------

fn register_os_approx_equals(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (a, b, margin): (Vec<f64>, Vec<f64>, f64)| {
        if a.len() != b.len() {
            return Ok(0i32);
        }
        for (x, y) in a.iter().zip(b.iter()) {
            if (x - y).abs() > margin {
                return Ok(0i32);
            }
        }
        Ok(1i32)
    })?;
    lua.globals().set("osApproxEquals", f)
}

// ---------------------------------------------------------------------------
// osIsNotValidNumber — returns 1 if NaN or infinity
// ---------------------------------------------------------------------------

fn register_os_is_not_valid_number(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, d: f64| {
        Ok(if d.is_nan() || d.is_infinite() {
            1i32
        } else {
            0
        })
    })?;
    lua.globals().set("osIsNotValidNumber", f)
}

// ---------------------------------------------------------------------------
// osSlerp — quaternion slerp (also accepts vectors for lerp)
// ---------------------------------------------------------------------------

fn register_os_slerp(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (r1, r2, t): (Vec<f64>, Vec<f64>, f64)| {
        if r1.len() == 4 && r2.len() == 4 {
            // Quaternion slerp
            let mut dot = r1[0] * r2[0] + r1[1] * r2[1] + r1[2] * r2[2] + r1[3] * r2[3];

            // If negative dot, negate one quat to take shorter path
            let mut q2 = r2.clone();
            if dot < 0.0 {
                dot = -dot;
                for v in &mut q2 {
                    *v = -*v;
                }
            }

            let dot = dot.clamp(0.0, 1.0);

            if dot > 0.9995 {
                // Very close: use linear interpolation
                let result: Vec<f64> = r1
                    .iter()
                    .zip(q2.iter())
                    .map(|(a, b)| a + t * (b - a))
                    .collect();
                // Normalize
                let mag = (result[0] * result[0]
                    + result[1] * result[1]
                    + result[2] * result[2]
                    + result[3] * result[3])
                    .sqrt();
                if mag > 0.0 {
                    Ok(result.into_iter().map(|v| v / mag).collect::<Vec<f64>>())
                } else {
                    Ok(r1)
                }
            } else {
                let theta = dot.acos();
                let sin_theta = theta.sin();
                let s1 = ((1.0 - t) * theta).sin() / sin_theta;
                let s2 = (t * theta).sin() / sin_theta;
                Ok(r1
                    .iter()
                    .zip(q2.iter())
                    .map(|(a, b)| a * s1 + b * s2)
                    .collect::<Vec<f64>>())
            }
        } else if r1.len() == 3 && r2.len() == 3 {
            // Vector lerp
            Ok(r1
                .iter()
                .zip(r2.iter())
                .map(|(a, b)| a + t * (b - a))
                .collect::<Vec<f64>>())
        } else {
            Err(mlua::Error::RuntimeError(
                "osSlerp: need 3- or 4-component vectors".into(),
            ))
        }
    })?;
    lua.globals().set("osSlerp", f)
}

// ---------------------------------------------------------------------------
// osOldList2ListStrided — strided list extraction
// ---------------------------------------------------------------------------

fn register_os_old_list2list_strided(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (src, start, end, stride): (Vec<mlua::Value>, i32, i32, i32)| {
            let len = src.len() as i32;
            if stride <= 0 || len == 0 {
                return Ok(Vec::new());
            }
            let start = start.clamp(0, len - 1) as usize;
            let end = end.clamp(0, len - 1) as usize;
            let stride = stride as usize;

            let mut result = Vec::new();
            let mut i = start;
            while i <= end {
                result.push(src[i].clone());
                i += stride;
            }
            Ok(result)
        },
    )?;
    lua.globals().set("osOldList2ListStrided", f)
}

// ---------------------------------------------------------------------------
// osListSortInPlace
// ---------------------------------------------------------------------------

fn register_os_list_sort_in_place(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, _stride, ascending): (Vec<String>, i32, i32)| {
        let mut sorted = src;
        sorted.sort();
        if ascending == 0 {
            sorted.reverse();
        }
        Ok(sorted)
    })?;
    lua.globals().set("osListSortInPlace", f)
}

// ---------------------------------------------------------------------------
// osListSortInPlaceStrided
// ---------------------------------------------------------------------------

fn register_os_list_sort_in_place_strided(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, stride, ascending): (Vec<String>, i32, i32)| {
        if stride <= 0 {
            return Ok(src);
        }
        let stride = stride as usize;
        let len = src.len();
        if stride > len {
            return Ok(src);
        }

        // Split into stride-sized chunks, sort by first element
        let chunk_count = len / stride;
        let mut chunks: Vec<Vec<String>> = (0..chunk_count)
            .map(|i| src[i * stride..(i + 1) * stride].to_vec())
            .collect();

        chunks.sort_by(|a, b| a[0].cmp(&b[0]));
        if ascending == 0 {
            chunks.reverse();
        }

        let mut result: Vec<String> = Vec::with_capacity(len);
        for chunk in chunks {
            result.extend(chunk);
        }
        // Append any remainder
        let remainder_start = chunk_count * stride;
        if remainder_start < len {
            result.extend_from_slice(&src[remainder_start..]);
        }
        Ok(result)
    })?;
    lua.globals().set("osListSortInPlaceStrided", f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_math_vm() -> Lua {
        let lua = Lua::new();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::None,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        lua
    }

    #[test]
    fn test_min_max() {
        let lua = make_math_vm();
        let r: f64 = lua.load("return osMin(3.0, 5.0)").eval().unwrap();
        assert!((r - 3.0).abs() < f64::EPSILON);

        let r: f64 = lua.load("return osMax(3.0, 5.0)").eval().unwrap();
        assert!((r - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_uuid() {
        let lua = make_math_vm();
        let r: i32 = lua
            .load(r#"return osIsUUID("550e8400-e29b-41d4-a716-446655440000")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 1);

        let r: i32 = lua.load(r#"return osIsUUID("not-a-uuid")"#).eval().unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_round() {
        let lua = make_math_vm();
        let r: f64 = lua.load("return osRound(3.14159, 2)").eval().unwrap();
        assert!((r - 3.14).abs() < 1e-10);

        let r: f64 = lua.load("return osRound(3.14159, 0)").eval().unwrap();
        assert!((r - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_mag_square() {
        let lua = make_math_vm();
        let r: f64 = lua.load("return osVecMagSquare({3, 4, 0})").eval().unwrap();
        assert!((r - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vec_dist_square() {
        let lua = make_math_vm();
        let r: f64 = lua
            .load("return osVecDistSquare({1, 2, 3}, {4, 6, 3})")
            .eval()
            .unwrap();
        // (3^2 + 4^2 + 0^2) = 25
        assert!((r - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_approx_equals() {
        let lua = make_math_vm();
        let r: i32 = lua
            .load("return osApproxEquals({1.001}, {1.002}, 0.01)")
            .eval()
            .unwrap();
        assert_eq!(r, 1);

        let r: i32 = lua
            .load("return osApproxEquals({1.0}, {2.0}, 0.01)")
            .eval()
            .unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    fn test_is_not_valid_number() {
        let lua = make_math_vm();
        let r: i32 = lua.load("return osIsNotValidNumber(0/0)").eval().unwrap();
        assert_eq!(r, 1);

        let r: i32 = lua.load("return osIsNotValidNumber(42.0)").eval().unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    fn test_slerp_identity() {
        let lua = make_math_vm();
        // Slerp between identity quaternion and itself at t=0.5
        let r: Vec<f64> = lua
            .load("return osSlerp({0, 0, 0, 1}, {0, 0, 0, 1}, 0.5)")
            .eval()
            .unwrap();
        assert_eq!(r.len(), 4);
        assert!((r[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_slerp_vector_lerp() {
        let lua = make_math_vm();
        let r: Vec<f64> = lua
            .load("return osSlerp({0, 0, 0}, {10, 10, 10}, 0.5)")
            .eval()
            .unwrap();
        assert_eq!(r.len(), 3);
        assert!((r[0] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_angle_between_same_rotation() {
        let lua = make_math_vm();
        let r: f64 = lua
            .load("return osAngleBetween({0, 0, 0, 1}, {0, 0, 0, 1})")
            .eval()
            .unwrap();
        assert!(r.abs() < 1e-6);
    }

    #[test]
    fn test_list_sort() {
        let lua = make_math_vm();
        let r: Vec<String> = lua
            .load(r#"return osListSortInPlace({"c", "a", "b"}, 1, 1)"#)
            .eval()
            .unwrap();
        assert_eq!(r, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_list_sort_descending() {
        let lua = make_math_vm();
        let r: Vec<String> = lua
            .load(r#"return osListSortInPlace({"c", "a", "b"}, 1, 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_old_list2list_strided() {
        let lua = make_math_vm();
        let r: Vec<String> = lua
            .load(r#"return osOldList2ListStrided({"a", "b", "c", "d", "e", "f"}, 0, 5, 2)"#)
            .eval()
            .unwrap();
        assert_eq!(r, vec!["a", "c", "e"]);
    }
}
