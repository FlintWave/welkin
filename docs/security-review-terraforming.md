# Terraforming security review

Date: 2026-11-15
Scope: the `ModifyLand` handler (`crates/welkin-region/src/terraform.rs`) and
the terrain height channel (`crates/welkin-scene/src/terrain.rs`).

## Finding HIGH-B: non-finite and absurd terrain values

A viewer could send a `ModifyLand` request with NaN or infinite height or
strength. A NaN written into the height field poisons it: the terrain encoder
and the physics heightfield both read from it. Commit e441e9e fixes this at
two layers.

- The handler drops the whole request if the target height is not finite or is
  outside 0 to 4096 m, if the strength (`seconds`) is not finite or is outside
  plus or minus 1000, if an extended brush radius is outside 0 to 64 m, or if a
  coordinate is not finite or has an absolute value above 4096.
- `TerrainChannel::set` ignores NaN and infinite heights and keeps the old
  value, so no other caller can write them either.

## Related findings

- MEDIUM-C: a viewer could send `ModifyLand` as fast as it liked. The region now
  accepts one request per avatar every 20 ms, as OpenSimulator does, and drops
  the rest (commit 25996f9).
- HIGH-A: object permission checks used to be bypassed by default. They are now
  enforced by default, and the estate owner and managers come from the
  `[Estate]` config section (commit 416fa77). The handshake reports estate owner
  and manager status to the viewer.

## Other controls

- Only avatars that pass `can_edit_terrain` can change the terrain.
- Unknown brush actions are ignored.
- Parcel-limited area edits are unsupported and do nothing, so they cannot
  edit outside the parcel.

## Not covered

This review reads the code. It did not test the handler with a live viewer or
fuzz the packet decoder.
