// SPDX-License-Identifier: LGPL-3.0-or-later
//! Luau runtime helpers prepended to every generated script.

pub const RUNTIME_PRELUDE: &str = r#"-- LSL Runtime Helpers
local function __int2float(i) return i + 0.0 end
local function __float2int(f) return math.floor(f) end
local function __vec2str(v) return string.format("<%f, %f, %f>", v.x, v.y, v.z) end
local function __rot2str(r) return string.format("<%f, %f, %f, %f>", r.x, r.y, r.z, r.s) end
local function __str2key(s) return s end
local function __key2str(k) return k end
local function __tostring_lsl(v)
    if type(v) == "table" then
        if v.s ~= nil then return __rot2str(v) end
        if v.z ~= nil then return __vec2str(v) end
        return tostring(v)
    end
    return tostring(v)
end
local function __vec_add(a, b) return {x=a.x+b.x, y=a.y+b.y, z=a.z+b.z} end
local function __vec_sub(a, b) return {x=a.x-b.x, y=a.y-b.y, z=a.z-b.z} end
local function __vec_mul_scalar(v, s) return {x=v.x*s, y=v.y*s, z=v.z*s} end
local function __vec_div_scalar(v, s) return {x=v.x/s, y=v.y/s, z=v.z/s} end
local function __vec_mod_scalar(v, s) return {x=v.x%s, y=v.y%s, z=v.z%s} end
local function __vec_dot(a, b) return a.x*b.x + a.y*b.y + a.z*b.z end
local function __vec_cross(a, b) return {x=a.y*b.z-a.z*b.y, y=a.z*b.x-a.x*b.z, z=a.x*b.y-a.y*b.x} end
local function __vec_neg(v) return {x=-v.x, y=-v.y, z=-v.z} end
local function __vec_eq(a, b) return (a.x==b.x and a.y==b.y and a.z==b.z) and 1 or 0 end
local function __rot_mul(a, b)
    return {
        x = a.s*b.x + a.x*b.s + a.y*b.z - a.z*b.y,
        y = a.s*b.y - a.x*b.z + a.y*b.s + a.z*b.x,
        z = a.s*b.z + a.x*b.y - a.y*b.x + a.z*b.s,
        s = a.s*b.s - a.x*b.x - a.y*b.y - a.z*b.z,
    }
end
local function __rot_eq(a, b) return (a.x==b.x and a.y==b.y and a.z==b.z and a.s==b.s) and 1 or 0 end
local function __vec_rot(v, r)
    local q = __rot_mul(__rot_mul(r, {x=v.x, y=v.y, z=v.z, s=0}), {x=-r.x, y=-r.y, z=-r.z, s=r.s})
    return {x=q.x, y=q.y, z=q.z}
end
local function __rot_div(a, b)
    local conj = {x=-b.x, y=-b.y, z=-b.z, s=b.s}
    return __rot_mul(a, conj)
end
local function __rot_neg(r) return {x=-r.x, y=-r.y, z=-r.z, s=-r.s} end
local function __list_add(a, b)
    local r = {}
    for i=1,#a do r[#r+1] = a[i] end
    for i=1,#b do r[#r+1] = b[i] end
    return r
end
local function __list_eq(a, b) return (#a == #b) and 1 or 0 end
local function __lsl_not(v) return (v == 0) and 1 or 0 end
local function __lsl_bool(v) return (v ~= 0) and 1 or 0 end
"#;
