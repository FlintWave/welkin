// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL built-in constants (TRUE, FALSE, PI, STATUS_*, CHANGED_*, NULL_KEY, ...).
//!
//! The compiler emits constant names as Lua globals, so the VM must define them.
//! Values come from the `welkin-script-api` constants table, plus the generated
//! `constants_extra` table (OpenSim `LSL_Constants.cs`) for names the compiler
//! accepts that the API table lacks (particles, camera, media, NPC, vectors, ...).

use super::constants_extra::{EXTRA_CONSTANTS, Extra};
use welkin_script_api::{CONSTANTS, LslConstantValue};

/// Define every constant from the API table as a Lua global.
pub fn register_constants(lua: &mlua::Lua) -> Result<(), crate::VmError> {
    let globals = lua.globals();
    for c in CONSTANTS {
        match c.value {
            LslConstantValue::Integer(i) => globals.set(c.name, i as i64)?,
            LslConstantValue::Float(f) => globals.set(c.name, f)?,
            LslConstantValue::String(s) => globals.set(c.name, s)?,
        }
    }
    for (name, value) in EXTRA_CONSTANTS {
        match value {
            Extra::Int(i) => globals.set(*name, *i)?,
            Extra::Float(f) => globals.set(*name, *f)?,
            Extra::Str(s) => globals.set(*name, *s)?,
            Extra::Vec3(v) => {
                let t = lua.create_table()?;
                t.set("x", v[0])?;
                t.set("y", v[1])?;
                t.set("z", v[2])?;
                globals.set(*name, t)?;
            }
            Extra::Rot(r) => {
                let t = lua.create_table()?;
                t.set("x", r[0])?;
                t.set("y", r[1])?;
                t.set("z", r[2])?;
                t.set("s", r[3])?;
                globals.set(*name, t)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    fn vm() -> mlua::Lua {
        let lua = crate::new_vm().unwrap();
        let ctx = Arc::new(parking_lot::RwLock::new(crate::builtins::ScriptContext::default()));
        crate::builtins::register_all(&lua, &ctx).unwrap();
        lua
    }

    /// Every constant the compiler lets scripts use must have a runtime value,
    /// otherwise compiled scripts read nil.
    #[test]
    fn every_compiler_constant_is_defined_at_runtime() {
        let lua = vm();
        let missing: Vec<&str> = welkin_script_compiler::builtins::BUILTIN_CONSTANTS
            .iter()
            .map(|c| c.name)
            .filter(|name| !lua.load(format!("return {name} ~= nil")).eval::<bool>().unwrap())
            .collect();
        assert!(missing.is_empty(), "constants without a runtime value: {missing:?}");
    }

    #[test]
    fn spot_check_values_against_opensim() {
        let lua = vm();
        let int = |n: &str| lua.load(format!("return {n}")).eval::<i64>().unwrap();
        assert_eq!(int("TRUE"), 1);
        assert_eq!(int("FALSE"), 0);
        assert_eq!(int("STATUS_PHYSICS"), 1);
        assert_eq!(int("PSYS_PART_FLAGS"), 0);
        assert_eq!(int("CAMERA_ACTIVE"), 12);
        assert_eq!(int("CLICK_ACTION_SIT"), 1);
        assert!((lua.load("return PI").eval::<f64>().unwrap() - 3.14159274).abs() < 1e-6);
        assert_eq!(
            lua.load("return NULL_KEY").eval::<String>().unwrap(),
            "00000000-0000-0000-0000-000000000000"
        );
        let (x, y, z): (f64, f64, f64) = lua
            .load("return TOUCH_INVALID_TEXCOORD.x, TOUCH_INVALID_TEXCOORD.y, TOUCH_INVALID_TEXCOORD.z")
            .eval()
            .unwrap();
        assert_eq!((x, y, z), (-1.0, -1.0, 0.0));
        let s: f64 = lua.load("return ZERO_ROTATION.s").eval().unwrap();
        assert_eq!(s, 1.0);
    }
}
