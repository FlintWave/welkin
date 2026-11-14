// SPDX-License-Identifier: GPL-3.0-or-later
//! OSSL draw-list functions — pure string manipulation, threat level None.
//!
//! These functions build up a draw-list string that is later interpreted
//! by the dynamic-texture renderer.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use crate::builtins::ScriptContext;
use super::{OsslFunc, OsslThreatLevel};

pub(crate) fn draw_functions() -> Vec<OsslFunc> {
    vec![
        // Existing 5 draw functions
        OsslFunc {
            name: "osDrawText",
            level: OsslThreatLevel::None,
            register: register_os_draw_text,
        },
        OsslFunc {
            name: "osDrawLine",
            level: OsslThreatLevel::None,
            register: register_os_draw_line,
        },
        OsslFunc {
            name: "osDrawRectangle",
            level: OsslThreatLevel::None,
            register: register_os_draw_rectangle,
        },
        OsslFunc {
            name: "osDrawEllipse",
            level: OsslThreatLevel::None,
            register: register_os_draw_ellipse,
        },
        OsslFunc {
            name: "osDrawImage",
            level: OsslThreatLevel::None,
            register: register_os_draw_image,
        },
        // New draw functions (task 2.1)
        OsslFunc {
            name: "osMovePen",
            level: OsslThreatLevel::None,
            register: register_os_move_pen,
        },
        OsslFunc {
            name: "osDrawFilledEllipse",
            level: OsslThreatLevel::None,
            register: register_os_draw_filled_ellipse,
        },
        OsslFunc {
            name: "osDrawFilledRectangle",
            level: OsslThreatLevel::None,
            register: register_os_draw_filled_rectangle,
        },
        OsslFunc {
            name: "osDrawPolygon",
            level: OsslThreatLevel::None,
            register: register_os_draw_polygon,
        },
        OsslFunc {
            name: "osDrawFilledPolygon",
            level: OsslThreatLevel::None,
            register: register_os_draw_filled_polygon,
        },
        OsslFunc {
            name: "osDrawResetTransform",
            level: OsslThreatLevel::None,
            register: register_os_draw_reset_transform,
        },
        OsslFunc {
            name: "osDrawRotationTransform",
            level: OsslThreatLevel::None,
            register: register_os_draw_rotation_transform,
        },
        OsslFunc {
            name: "osDrawScaleTransform",
            level: OsslThreatLevel::None,
            register: register_os_draw_scale_transform,
        },
        OsslFunc {
            name: "osDrawTranslationTransform",
            level: OsslThreatLevel::None,
            register: register_os_draw_translation_transform,
        },
        OsslFunc {
            name: "osSetFontName",
            level: OsslThreatLevel::None,
            register: register_os_set_font_name,
        },
        OsslFunc {
            name: "osSetFontSize",
            level: OsslThreatLevel::None,
            register: register_os_set_font_size,
        },
        OsslFunc {
            name: "osSetPenSize",
            level: OsslThreatLevel::None,
            register: register_os_set_pen_size,
        },
        OsslFunc {
            name: "osSetPenColor",
            level: OsslThreatLevel::None,
            register: register_os_set_pen_color,
        },
        OsslFunc {
            name: "osSetPenColour",
            level: OsslThreatLevel::None,
            register: register_os_set_pen_colour,
        },
        OsslFunc {
            name: "osSetPenCap",
            level: OsslThreatLevel::None,
            register: register_os_set_pen_cap,
        },
        OsslFunc {
            name: "osGetDrawStringSize",
            level: OsslThreatLevel::None,
            register: register_os_get_draw_string_size,
        },
    ]
}

// ---------------------------------------------------------------------------
// Existing draw functions (preserved from original ossl.rs)
// ---------------------------------------------------------------------------

fn register_os_draw_text(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, text): (String, String)| {
        Ok(format!("{draw_list}Text {text}; "))
    })?;
    lua.globals().set("osDrawText", f)
}

fn register_os_draw_line(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, start_x, start_y, end_x, end_y): (String, i32, i32, i32, i32)| {
            Ok(format!(
                "{draw_list}MoveTo {start_x},{start_y}; LineTo {end_x},{end_y}; "
            ))
        },
    )?;
    lua.globals().set("osDrawLine", f)
}

fn register_os_draw_rectangle(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, width, height): (String, i32, i32)| {
        Ok(format!("{draw_list}Rectangle {width},{height}; "))
    })?;
    lua.globals().set("osDrawRectangle", f)
}

fn register_os_draw_ellipse(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, width, height): (String, i32, i32)| {
        Ok(format!("{draw_list}Ellipse {width},{height}; "))
    })?;
    lua.globals().set("osDrawEllipse", f)
}

fn register_os_draw_image(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, width, height, image_url): (String, i32, i32, String)| {
            Ok(format!(
                "{draw_list}Image {width},{height},{image_url}; "
            ))
        },
    )?;
    lua.globals().set("osDrawImage", f)
}

// ---------------------------------------------------------------------------
// New draw functions (task 2.1)
// ---------------------------------------------------------------------------

fn register_os_move_pen(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, x, y): (String, i32, i32)| {
        Ok(format!("{draw_list}MoveTo {x},{y}; "))
    })?;
    lua.globals().set("osMovePen", f)
}

fn register_os_draw_filled_ellipse(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, width, height): (String, i32, i32)| {
        Ok(format!("{draw_list}FillEllipse {width},{height}; "))
    })?;
    lua.globals().set("osDrawFilledEllipse", f)
}

fn register_os_draw_filled_rectangle(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, width, height): (String, i32, i32)| {
        Ok(format!("{draw_list}FillRectangle {width},{height}; "))
    })?;
    lua.globals().set("osDrawFilledRectangle", f)
}

/// Helper: zip x-points and y-points into "x1,y1,x2,y2,..." for polygon commands.
fn polygon_points(x_points: &[i32], y_points: &[i32]) -> String {
    x_points
        .iter()
        .zip(y_points.iter())
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn register_os_draw_polygon(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, x_points, y_points): (String, Vec<i32>, Vec<i32>)| {
            let pts = polygon_points(&x_points, &y_points);
            Ok(format!("{draw_list}Polygon {pts}; "))
        },
    )?;
    lua.globals().set("osDrawPolygon", f)
}

fn register_os_draw_filled_polygon(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, x_points, y_points): (String, Vec<i32>, Vec<i32>)| {
            let pts = polygon_points(&x_points, &y_points);
            Ok(format!("{draw_list}FillPolygon {pts}; "))
        },
    )?;
    lua.globals().set("osDrawFilledPolygon", f)
}

fn register_os_draw_reset_transform(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, draw_list: String| {
        Ok(format!("{draw_list}ResetTransf; "))
    })?;
    lua.globals().set("osDrawResetTransform", f)
}

fn register_os_draw_rotation_transform(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, angle): (String, f64)| {
        Ok(format!("{draw_list}RotTransf {angle}; "))
    })?;
    lua.globals().set("osDrawRotationTransform", f)
}

fn register_os_draw_scale_transform(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, sx, sy): (String, f64, f64)| {
        Ok(format!("{draw_list}ScaleTransf {sx},{sy}; "))
    })?;
    lua.globals().set("osDrawScaleTransform", f)
}

fn register_os_draw_translation_transform(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, x, y): (String, f64, f64)| {
        Ok(format!("{draw_list}TransTransf {x},{y}; "))
    })?;
    lua.globals().set("osDrawTranslationTransform", f)
}

fn register_os_set_font_name(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, font_name): (String, String)| {
        Ok(format!("{draw_list}FontName {font_name}; "))
    })?;
    lua.globals().set("osSetFontName", f)
}

fn register_os_set_font_size(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, font_size): (String, i32)| {
        Ok(format!("{draw_list}FontSize {font_size}; "))
    })?;
    lua.globals().set("osSetFontSize", f)
}

fn register_os_set_pen_size(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, pen_size): (String, i32)| {
        Ok(format!("{draw_list}PenSize {pen_size}; "))
    })?;
    lua.globals().set("osSetPenSize", f)
}

fn register_os_set_pen_color(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, color): (String, String)| {
        Ok(format!("{draw_list}PenColor {color}; "))
    })?;
    lua.globals().set("osSetPenColor", f)
}

fn register_os_set_pen_colour(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    // Deprecated alias for osSetPenColor
    let f = lua.create_function(|_, (draw_list, color): (String, String)| {
        Ok(format!("{draw_list}PenColor {color}; "))
    })?;
    lua.globals().set("osSetPenColour", f)
}

fn register_os_set_pen_cap(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, direction, cap): (String, String, String)| {
            Ok(format!("{draw_list}PenCap {direction},{cap}; "))
        },
    )?;
    lua.globals().set("osSetPenCap", f)
}

fn register_os_get_draw_string_size(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    // Stub: needs font metrics engine. Returns "0,0".
    let f = lua.create_function(|_, (_font_name, _text, _font_size): (String, String, i32)| {
        tracing::debug!("osGetDrawStringSize (stub)");
        Ok("0,0".to_string())
    })?;
    lua.globals().set("osGetDrawStringSize", f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_draw_vm() -> Lua {
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
    fn test_move_pen() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osMovePen("", 10, 20)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "MoveTo 10,20; ");
    }

    #[test]
    fn test_filled_ellipse() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osDrawFilledEllipse("", 30, 40)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "FillEllipse 30,40; ");
    }

    #[test]
    fn test_filled_rectangle() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osDrawFilledRectangle("", 50, 60)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "FillRectangle 50,60; ");
    }

    #[test]
    fn test_polygon() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osDrawPolygon("", {0, 10, 20}, {0, 20, 0})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Polygon 0,0,10,20,20,0; ");
    }

    #[test]
    fn test_filled_polygon() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osDrawFilledPolygon("", {0, 10, 20}, {0, 20, 0})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "FillPolygon 0,0,10,20,20,0; ");
    }

    #[test]
    fn test_transforms() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osDrawResetTransform("")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "ResetTransf; ");

        let r: String = lua
            .load(r#"return osDrawRotationTransform("", 45.0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "RotTransf 45; ");

        let r: String = lua
            .load(r#"return osDrawScaleTransform("", 2.0, 3.0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "ScaleTransf 2,3; ");

        let r: String = lua
            .load(r#"return osDrawTranslationTransform("", 10.0, 20.0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "TransTransf 10,20; ");
    }

    #[test]
    fn test_font_and_pen() {
        let lua = make_draw_vm();

        let r: String = lua
            .load(r#"return osSetFontName("", "Arial")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "FontName Arial; ");

        let r: String = lua
            .load(r#"return osSetFontSize("", 24)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "FontSize 24; ");

        let r: String = lua
            .load(r#"return osSetPenSize("", 3)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "PenSize 3; ");

        let r: String = lua
            .load(r#"return osSetPenColor("", "FF0000FF")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "PenColor FF0000FF; ");
    }

    #[test]
    fn test_pen_colour_alias() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osSetPenColour("", "00FF00FF")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "PenColor 00FF00FF; ");
    }

    #[test]
    fn test_pen_cap() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osSetPenCap("", "start", "round")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "PenCap start,round; ");
    }

    #[test]
    fn test_get_draw_string_size_stub() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(r#"return osGetDrawStringSize("Arial", "Hello", 12)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "0,0");
    }

    #[test]
    fn test_chaining_new_draw_commands() {
        let lua = make_draw_vm();
        let r: String = lua
            .load(
                r#"
                local dl = ""
                dl = osSetPenColor(dl, "FF0000FF")
                dl = osSetFontSize(dl, 24)
                dl = osMovePen(dl, 10, 20)
                dl = osDrawFilledRectangle(dl, 100, 50)
                dl = osDrawText(dl, "Hello")
                return dl
            "#,
            )
            .eval()
            .unwrap();
        assert_eq!(
            r,
            "PenColor FF0000FF; FontSize 24; MoveTo 10,20; FillRectangle 100,50; Text Hello; "
        );
    }
}
