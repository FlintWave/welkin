<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Object interaction summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Viewer compatibility](viewer-compatibility.md)

Status: complete (24 of 24 tasks, 3 phases). Change:
`openspec/changes/object-interaction/`.

## What was built

All object manipulation UDP packet handlers required for the viewer's build
tools and object inspector. Without these, users could not select, move,
rotate, scale, link, copy, rez, or derez objects. The work ported logic from
`OpenSim/Region/Framework/Scenes/SceneObjectGroup.cs`,
`SceneObjectPart.cs`, `LLClientView.cs`, and
`InventoryAccessModule.cs`.

| Phase | Area | Tasks |
|---|---|---|
| 1 | Core editing: MultipleObjectUpdate, select/deselect, properties, broadcast | 1-7 |
| 2 | Manipulation: grab/touch, name, description, shape, permissions, sale, owner | 8-18 |
| 3 | Rez, derez, linking, duplication | 19-24 |

## Architecture

Every object mutation follows the same path:

1. Decode the incoming UDP packet per the message template
2. Look up the target SceneObjectPart by local ID in the scene graph
3. Check permissions via PermissionsModule (CanEditObject, CanMoveObject, etc.)
4. Apply the mutation to the SceneObjectPart and SceneObjectGroup
5. Broadcast the change to all connected viewers

Two broadcast modes exist. ObjectUpdate sends full object data when an object
first appears or its properties change. ImprovedTerseObjectUpdate sends only
transform and velocity data for position/rotation/scale changes during
editing. The terse form is much smaller and used for high-frequency updates.

| Handler | Purpose |
|---|---|
| MultipleObjectUpdate | Position, rotation, scale via flags byte (0x01/0x02/0x04/0x08) |
| ObjectSelect, ObjectDeselect | Selection tracking, sends ObjectProperties on select |
| ObjectGrab, ObjectGrabUpdate, ObjectDeGrab | Touch events with UV coordinates and face index |
| ObjectName, ObjectDescription | String property editing |
| ObjectShape | All prim shape parameters (ProfileCurve, PathCurve, etc.) |
| ObjectPermissions | Base, owner, group, everyone, next-owner permission masks |
| ObjectSaleInfo | Sale type and price |
| ObjectExtraParams | Flexible, light, sculpt, projection, mesh parameters |
| ObjectGroup, ObjectOwner | Group ownership and ownership transfer |
| RequestObjectPropertiesFamily | Full property response to requesting viewer |
| ObjectPropertiesRequest | Batch properties response for multiple objects |
| RezObject | Instantiate from inventory: fetch item, deserialize, place, broadcast |
| DeRezObject | Six destination codes: save, take, take-copy, god-copy, delete, return |
| ObjectLink, ObjectDelink | Combine or split prims in link sets (54m distance limit) |
| ObjectDuplicate, ObjectDuplicateOnRay | Copy at offset or along camera ray cast |

## Key decisions

1. All permission checks go through PermissionsModule. No inline permission
   logic in packet handlers.
2. ObjectUpdate broadcast is queued and batched per viewer tick to prevent
   packet storms during multi-select edits.
3. MultipleObjectUpdate quaternion decoding follows OpenSim's exact format
   (3-component normalized, not 4-component).
4. DeRezObject destination codes are copied from OpenSim's `DeRezAction` enum.

## Commits

| Hash | Description |
|---|---|
| `8da4f35` | Phase 1: MultipleObjectUpdate, select, properties, terse broadcast |
| `1fe25ab` | Phase 2: grab, name, shape, permissions, sale, owner handlers |
| `eb634b0` | Phase 3: RezObject, DeRezObject, link, delink, duplicate handlers |
