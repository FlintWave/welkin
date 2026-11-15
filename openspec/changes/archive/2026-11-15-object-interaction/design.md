# Object Interaction — Design

## Architecture

```
Viewer ──LLUDP──> Region Server ──> PacketHandler ──> Scene Graph
                                         |                |
                                         v                v
                                   PermissionsModule   SceneObjectGroup
                                                       SceneObjectPart
                                         |                |
                                         v                v
                                   Reject / Allow    Mutate state
                                                         |
                                                         v
                                                   Broadcast ObjectUpdate
                                                   to all viewers in region
```

Every object mutation follows the same pattern:

1. Decode the incoming UDP packet per the message template
2. Look up the target SceneObjectPart by local ID in the scene graph
3. Check permissions via PermissionsModule (CanEditObject, CanMoveObject, etc.)
4. Apply the mutation to the SceneObjectPart (and SceneObjectGroup if needed)
5. Broadcast the change to all connected viewers via ObjectUpdate or
   ImprovedTerseObjectUpdate

## MultipleObjectUpdate (the core edit packet)

This is the single most important object packet. The viewer sends it for
every move, rotate, and scale operation in the build tools.

Packet structure (from message template):
```
MultipleObjectUpdate Low 92 NotTrusted Zerocoded
  {
    AgentData Single
    { AgentID     LLUUID }
    { SessionID   LLUUID }
    }
    {
    ObjectData Variable
    { ObjectLocalID U32 }
    { Type          U8  }
    { Data          Variable 1 }
    }
```

The `Type` field is a flags byte:
- Bit 0 (0x01): Position
- Bit 1 (0x02): Rotation
- Bit 2 (0x04): Scale
- Bit 3 (0x08): Linked position/rotation (child prim in link set)

The `Data` field contains the values indicated by the flags, packed as:
- Position: 3 x f32 (12 bytes) — Vec3
- Rotation: 4 x f32 (16 bytes) — compressed Quaternion (3 components + sign)
  Actually sent as 3 x f32 (12 bytes) normalized quaternion
- Scale: 3 x f32 (12 bytes) — Vec3

Multiple objects can be updated in a single packet (ObjectData is Variable).

Reference: `LLClientView.HandleMultipleObjUpdate()` in OpenSim C#.

## ObjectSelect / ObjectDeselect

ObjectSelect tells the server which objects the viewer has selected (clicked
on, or shift-clicked to multi-select). The server must:

1. Record the selection (for edit permission tracking)
2. Send ObjectProperties back to the selecting viewer with the full property
   set (name, description, creator, owner, permissions, sale info, etc.)

ObjectDeselect clears the selection. The server should also send a final
ObjectProperties if any properties changed during the selection.

## ObjectGrab / ObjectGrabUpdate / ObjectDeGrab

These handle the viewer's touch interaction:
- ObjectGrab: mouse down on an object — fires `touch_start` script event
- ObjectGrabUpdate: mouse drag while holding — fires `touch` script event
  with UV coordinates and surface normal
- ObjectDeGrab: mouse up — fires `touch_end` script event

The grab packets include the surface touch position (ST coordinates) and
face index, used by scripts via `llDetectedTouchST()`, `llDetectedTouchFace()`.

## RezObject

Instantiates an object from an inventory item into the scene:

1. Viewer sends RezObject with inventory item UUID, rez position, and flags
2. Server fetches the inventory item via ROBUST xinventory
3. Server fetches the serialized object from the asset service
4. Server deserializes into SceneObjectGroup
5. Server places at requested position, assigns new local IDs
6. Server adds to scene graph
7. Server broadcasts ObjectUpdate to all viewers
8. If the item is no-copy, server removes it from inventory

Reference: `Scene.RezObject()` and `InventoryAccessModule.RezObject()`.

## DeRezObject

Takes an object from the scene into inventory or deletes it:

The `destination` field determines the action:
- 0 (SaveToExistingUserInventoryItem): update existing inventory item
- 1 (TakeCopy): copy to inventory (original stays in scene)
- 4 (Take): move to inventory (removed from scene)
- 5 (GodTakeCopy): god-mode copy
- 6 (Delete): delete permanently
- 9 (ReturnToOwner): return to object owner

Each destination requires different permission checks.

Reference: `Scene.DeRezObjects()` in OpenSim C#.

## Object Property Editing

Each property has its own packet type. All follow the same pattern:
decode, permission check, mutate, broadcast.

| Packet | Field(s) Modified | Permission Check |
|--------|-------------------|------------------|
| ObjectName | Name (string) | CanEditObject |
| ObjectDescription | Description (string) | CanEditObject |
| ObjectShape | ProfileCurve, PathCurve, all shape params | CanEditObject |
| ObjectPermissions | BaseMask, OwnerMask, GroupMask, EveryoneMask, NextOwnerMask | CanEditObjectPermissions |
| ObjectSaleInfo | SaleType, SalePrice | CanEditObject |
| ObjectExtraParams | Flexible, Light, Sculpt, Projection, Mesh | CanEditObject |
| ObjectGroup | GroupID | CanEditObject + group membership |
| ObjectOwner | OwnerID, GroupID | CanEditObject (deed to group) |

## Linking

ObjectLink combines multiple selected prims into a single SceneObjectGroup.
The first selected prim becomes the root. All child prims get their positions
recalculated relative to the new root.

ObjectDelink removes selected prims from a link set, creating new
SceneObjectGroups for each detached prim (or sub-group).

Both operations require CanEditObject on all involved prims and must respect
the link distance limit (54 meters between any two prims, matching OpenSim).

## Copying

ObjectDuplicate copies the selected objects at a given offset vector.
ObjectDuplicateOnRay copies along a ray cast from the camera, placing the
copy where the ray hits terrain or another object.

Both create new SceneObjectGroups with new UUIDs, new local IDs, and the
copier as the new owner. Permission: requires Copy permission on the object.

## Broadcast Strategy

Object state changes are broadcast via two packet types:

- **ObjectUpdate** (full): Sent when an object first appears to a viewer, or
  when properties change (name, shape, permissions, etc.). Contains all object
  data including shape, textures, and metadata.
- **ImprovedTerseObjectUpdate** (terse): Sent for position/rotation/velocity
  changes during movement or editing. Contains only transform and velocity
  data. Much smaller — used for high-frequency updates.

The server must track which viewers have received the full ObjectUpdate for
each object (the "kill list" / "update list" pattern from OpenSim's
EntityManager).

## Decisions

1. All permission checks go through the existing PermissionsModule — no
   inline permission logic in packet handlers.
2. ObjectUpdate broadcast is queued and batched per viewer tick, not sent
   immediately on every mutation (prevents packet storms during multi-select
   edits).
3. MultipleObjectUpdate's quaternion decoding follows OpenSim's exact format
   (3-component normalized, not 4-component) — copy from C# source.
4. DeRezObject destination codes are copied exactly from OpenSim's
   `DeRezAction` enum.
