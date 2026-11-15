# object-interaction tasks

## Phase 1: Core object editing (P0)

- [x] Handle MultipleObjectUpdate — decode flags byte, apply position/rotation/scale to SceneObjectPart
- [x] Handle ObjectSelect — mark selected, send ObjectProperties to viewer
- [x] Handle ObjectDeselect — clear selection state
- [x] Broadcast full ObjectUpdate when object first appears or properties change
- [x] Broadcast ImprovedTerseObjectUpdate for position/rotation/velocity changes
- [x] Handle RequestObjectPropertiesFamily — send full object properties to requesting viewer
- [x] Handle ObjectPropertiesRequest — batch properties response for multiple objects

## Phase 2: Object manipulation

- [x] Handle ObjectGrab — touch start, fire touch_start script event
- [x] Handle ObjectGrabUpdate — touch drag with UV/face data, fire touch event
- [x] Handle ObjectDeGrab — touch end, fire touch_end script event
- [x] Handle ObjectName — set object name string
- [x] Handle ObjectDescription — set object description string
- [x] Handle ObjectShape — modify all prim shape parameters (ProfileCurve, PathCurve, etc.)
- [x] Handle ObjectPermissions — change base/owner/group/everyone/next-owner masks
- [x] Handle ObjectSaleInfo — set sale type and price
- [x] Handle ObjectExtraParams — flexible, light, sculpt, projection, mesh parameters
- [x] Handle ObjectGroup — set object group ownership
- [x] Handle ObjectOwner — transfer ownership or deed to group

## Phase 3: Rez, derez, and linking

- [x] Handle RezObject — instantiate object from inventory item into scene
- [x] Handle DeRezObject — take to inventory, delete, or return to owner (all destination codes)
- [x] Handle ObjectLink — combine selected prims into a link set with root prim
- [x] Handle ObjectDelink — split prims out of a link set into separate objects
- [x] Handle ObjectDuplicate — copy selected objects at offset
- [x] Handle ObjectDuplicateOnRay — copy along camera ray cast to hit point
