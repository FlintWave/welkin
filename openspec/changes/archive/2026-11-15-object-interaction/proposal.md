# Object Interaction

## Summary

Implement all object manipulation UDP packet handlers so users can select,
edit, move, rotate, scale, link, copy, rez, and derez objects. This is the
most critical gap blocking basic usability — without these handlers, the
viewer's build tools and object inspector are non-functional.

## Motivation

The parity audit found only 18 UDP packet handlers implemented. All object
editing packets are missing (~15 handlers). A user connecting with Aurora or
Firestorm cannot click an object, open its properties, move it, resize it,
link prims, copy objects, or rez items from inventory. Every in-world
creation and editing workflow depends on these packets.

## Scope

### In scope

- **Core editing:** MultipleObjectUpdate (position, rotation, scale via flags
  byte), ObjectSelect, ObjectDeselect
- **Touch/drag:** ObjectGrab, ObjectGrabUpdate, ObjectDeGrab
- **Linking:** ObjectLink, ObjectDelink
- **Copying:** ObjectDuplicate, ObjectDuplicateOnRay
- **Properties:** ObjectName, ObjectDescription, ObjectShape,
  ObjectPermissions, ObjectSaleInfo, ObjectExtraParams, ObjectGroup,
  ObjectOwner
- **Inspector:** RequestObjectPropertiesFamily, ObjectPropertiesRequest
- **Rez/derez:** RezObject (from inventory), DeRezObject (to inventory or
  delete)
- **Broadcast:** ObjectUpdate and ImprovedTerseObjectUpdate to all viewers in
  the region when any object state changes

### Out of scope

- Script-driven object manipulation (covered by `script-engine`)
- Physics mesh upload (covered by `prim-meshing`)
- Attachment rez/detach packets (separate change)

## Phase

This belongs immediately after the current `firestorm-session` work. It is a
prerequisite for any meaningful grid operation and should be prioritized above
all P1 and P2 items.

## Risk

Medium. The packet formats are well-documented in the message template and
OpenSim's C# source. The main complexity is in MultipleObjectUpdate's flags
byte decoding and in RezObject's inventory-to-scene instantiation path.
Permission checks must be correct from the start to prevent griefing on
multi-user grids.

## References

- `OpenSim/Region/Framework/Scenes/SceneObjectGroup.cs` — object group operations
- `OpenSim/Region/Framework/Scenes/SceneObjectPart.cs` — per-prim state
- `OpenSim/Region/ClientStack/Linden/UDP/LLClientView.cs` — packet send/receive
- `OpenSim/Region/Framework/Scenes/Scene.Inventory.cs` — rez/derez logic
- `OpenSim/Region/CoreModules/Framework/InventoryAccess/InventoryAccessModule.cs` — inventory rez
