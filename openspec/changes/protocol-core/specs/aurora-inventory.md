# Aurora Crate Inventory — Server-Side Analysis

Pinned commit: `3d523b14a17bbc6484b208378c0eef291ad5ac32`

## aurora-llsd

Fully bidirectional. Binary, notation and XML LLSD all have encode + decode.
No server-side gaps.

| API | Direction | Notes |
|---|---|---|
| `to_xml` / `from_xml` | Encode + Decode | XML LLSD |
| `to_binary` / `from_binary` | Encode + Decode | Binary LLSD |
| `to_notation` / `from_notation` | Encode + Decode | Notation LLSD |
| `Llsd` enum | — | All LLSD value types |

## aurora-msg

All 483 messages have both `encode_body` and `decode_body`, generated from
`message_template.msg`. No codec-level gaps.

| API | Direction |
|---|---|
| `Msg` trait | Encode + Decode |
| `MsgId`, `MsgInfo` | — |
| `Reader` / `Writer` | Decode / Encode |
| `parse_packet` / `build_packet` | Decode / Encode |
| `msgs::*` (483 structs) | Encode + Decode |

## aurora-prim

Bidirectional for core prim data.

| API | Direction |
|---|---|
| `parse_texture_entry` / `pack_texture_entry` | Decode + Encode |
| `VolumeParams` / `RawShape` | Decode + Encode |
| `parse_extra_params` + individual `pack_*` | Decode + Encode |
| `generate_volume` | Geometry for physics |

### Gaps

1. Extra-params envelope assembler (individual packers exist, no top-level)
2. `pack_render_materials` (no public packer)
3. Particle system packing (`#[cfg(test)]` only)

## aurora-net (viewer session layer)

Server-direction gaps — new code needed, not wrapper re-exports:

| Gap | Server needs | Effort |
|---|---|---|
| Object update encoders | `encode_full`, `encode_compressed`, `encode_terse` | Medium |
| Terrain layer encoder | `encode_land_layer`, `encode_wind_layer` | Small |
| XML-RPC server | `parse_call` + `build_response` | Small |
| Login server | `parse_login_request` + `build_login_response` | Small |
| Server circuit | Multi-peer sequence/ack/resend/ping | Large |

## aurora-assets (physics integration)

| API | Direction |
|---|---|
| `parse_mesh_header` / `decode_mesh_lod` | Decode |
| `mesh_physics::decode` | Decode (physics convex hulls) |
| `mesh_physics::convex_hull` | Uses `parry3d::try_convex_hull` |

parry3d 0.31.1 confirmed — matches rapier3d 0.36.1.

Physics collider path: prim → `generate_volume` → trimesh; mesh → `mesh_physics::decode` → convex hull compound.
