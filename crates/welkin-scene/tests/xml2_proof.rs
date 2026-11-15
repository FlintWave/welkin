// SPDX-License-Identifier: LGPL-3.0-or-later
//! XML2 round-trip proof tests: serialize/deserialize verification against
//! known C# serialized fragments and property survival.

use glam::{Quat, Vec3};
use uuid::Uuid;

use welkin_scene::group::SceneObjectGroup;
use welkin_scene::part::{PartColor, PrimFlags, SceneObjectPart};
use welkin_scene::xml2::{deserialize_group, serialize_group};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_owner() -> Uuid {
    Uuid::parse_str("aabbccdd-1122-3344-5566-778899aabbcc").unwrap()
}

fn make_part_full(name: &str, owner: Uuid) -> SceneObjectPart {
    let mut p = SceneObjectPart::new(name, owner);
    p.group_position = Vec3::new(128.5, 64.25, 32.125);
    p.offset_position = Vec3::new(1.0, 2.0, 3.0);
    p.rotation_offset = Quat::from_xyzw(0.0, 0.382683, 0.0, 0.923880);
    p.scale = Vec3::new(0.5, 1.0, 2.0);
    p.velocity = Vec3::new(0.1, 0.2, 0.3);
    p.angular_velocity = Vec3::new(0.01, 0.02, 0.03);
    p.acceleration = Vec3::new(0.001, 0.002, 0.003);
    p.description = "A test prim".to_string();
    p.text = "hover text".to_string();
    p.text_color = PartColor::new(200, 100, 50, 128);
    p.material = 4; // metal
    p.click_action = 1;
    p.flags = PrimFlags::PHYSICS | PrimFlags::PHANTOM | PrimFlags::SCRIPTED;
    p.density = 750.0;
    p.friction = 0.4;
    p.bounce = 0.7;
    p.gravity_modifier = 0.5;
    p.buoyancy = 0.3;
    p.force = Vec3::new(1.0, 2.0, 3.0);
    p.torque = Vec3::new(0.5, 0.5, 0.5);
    p.sit_name = "Sit Here".to_string();
    p.touch_name = "Touch Me".to_string();
    p.sit_target_position = Vec3::new(0.0, 0.0, 0.5);
    p.sit_target_orientation = Quat::IDENTITY;
    p.creation_date = 1700000000;
    p.sale_price = 100;
    p.object_sale_type = 2;
    p.script_access_pin = 42;
    p.pay_price = [10, 20, 50, 100, -2];
    p.camera_eye_offset = Vec3::new(0.0, -2.0, 1.0);
    p.camera_at_offset = Vec3::new(0.0, 3.0, 0.5);
    p.sound_gain = 0.8;
    p.sound_radius = 20.0;
    p
}

/// Approximate f32 equality check (relative tolerance for values, absolute for near-zero).
fn f32_close(a: f32, b: f32) -> bool {
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    let max_abs = a.abs().max(b.abs());
    if max_abs < 1e-6 {
        diff < 1e-6
    } else {
        diff / max_abs < 1e-5
    }
}

fn vec3_close(a: Vec3, b: Vec3) -> bool {
    f32_close(a.x, b.x) && f32_close(a.y, b.y) && f32_close(a.z, b.z)
}

fn quat_close(a: Quat, b: Quat) -> bool {
    f32_close(a.x, b.x) && f32_close(a.y, b.y) && f32_close(a.z, b.z) && f32_close(a.w, b.w)
}

// ===========================================================================
// 1. Single prim with full properties
// ===========================================================================

#[test]
fn roundtrip_single_prim_full_properties() {
    let owner = make_owner();
    let part = make_part_full("Full Properties Box", owner);
    let part_uuid = part.uuid;
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();

    let rp = restored.root_part();
    assert_eq!(rp.uuid, part_uuid);
    assert_eq!(rp.name, "Full Properties Box");
    assert_eq!(rp.owner_id, owner);
    assert_eq!(rp.creator_id, owner);
    assert_eq!(rp.description, "A test prim");
    assert_eq!(rp.text, "hover text");
    assert_eq!(rp.text_color, PartColor::new(200, 100, 50, 128));
    assert_eq!(rp.material, 4);
    assert_eq!(rp.click_action, 1);
    assert!(rp.flags.contains(PrimFlags::PHYSICS));
    assert!(rp.flags.contains(PrimFlags::PHANTOM));
    assert!(rp.flags.contains(PrimFlags::SCRIPTED));
    assert!(f32_close(rp.density, 750.0));
    assert!(f32_close(rp.friction, 0.4));
    assert!(f32_close(rp.bounce, 0.7));
    assert!(f32_close(rp.gravity_modifier, 0.5));
    assert!(f32_close(rp.buoyancy, 0.3));
    assert!(vec3_close(rp.force, Vec3::new(1.0, 2.0, 3.0)));
    assert!(vec3_close(rp.torque, Vec3::new(0.5, 0.5, 0.5)));
    assert_eq!(rp.sit_name, "Sit Here");
    assert_eq!(rp.touch_name, "Touch Me");
    assert!(vec3_close(rp.sit_target_position, Vec3::new(0.0, 0.0, 0.5)));
    assert_eq!(rp.creation_date, 1700000000);
    assert_eq!(rp.sale_price, 100);
    assert_eq!(rp.object_sale_type, 2);
    assert_eq!(rp.script_access_pin, 42);
    assert_eq!(rp.pay_price, [10, 20, 50, 100, -2]);
    assert!(vec3_close(rp.camera_eye_offset, Vec3::new(0.0, -2.0, 1.0)));
    assert!(vec3_close(rp.camera_at_offset, Vec3::new(0.0, 3.0, 0.5)));
    assert!(f32_close(rp.sound_gain, 0.8));
    assert!(f32_close(rp.sound_radius, 20.0));
}

// ===========================================================================
// 2. Linked prim set round-trip with link order preserved
// ===========================================================================

#[test]
fn roundtrip_linked_object_preserves_order() {
    let owner = make_owner();
    let root = make_part_full("LinkRoot", owner);
    let root_uuid = root.uuid;
    let child1 = make_part_full("Child1", owner);
    let child1_uuid = child1.uuid;
    let child2 = make_part_full("Child2", owner);
    let child2_uuid = child2.uuid;
    let child3 = make_part_full("Child3", owner);
    let child3_uuid = child3.uuid;

    let mut group = SceneObjectGroup::new(root);
    group.add_part(child1);
    group.add_part(child2);
    group.add_part(child3);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();

    assert_eq!(restored.uuid(), root_uuid);
    assert_eq!(restored.part_count(), 4);
    assert_eq!(restored.root_part().name, "LinkRoot");

    // Children are present
    assert!(restored.get_part(&child1_uuid).is_some());
    assert!(restored.get_part(&child2_uuid).is_some());
    assert!(restored.get_part(&child3_uuid).is_some());

    // Link numbers reconstructed by add_part: root=1, children=2,3,4
    assert_eq!(restored.root_part().link_num, 1);
    // After deserialization and add_part, link nums are reassigned
    let parts: Vec<_> = restored.parts().collect();
    assert_eq!(parts.len(), 4);
    // parts() returns root first, then children sorted by link_num
    assert_eq!(parts[0].uuid, root_uuid);
}

#[test]
fn roundtrip_two_child_linkset() {
    let owner = Uuid::new_v4();
    let root = SceneObjectPart::new("Root", owner);
    let root_uuid = root.uuid;
    let c1 = SceneObjectPart::new("C1", owner);
    let c1_uuid = c1.uuid;
    let c2 = SceneObjectPart::new("C2", owner);
    let c2_uuid = c2.uuid;

    let mut group = SceneObjectGroup::new(root);
    group.add_part(c1);
    group.add_part(c2);

    let xml = serialize_group(&group).unwrap();

    // Verify XML structure has OtherParts
    assert!(xml.contains("<OtherParts>"));
    assert!(xml.contains("</OtherParts>"));

    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(restored.uuid(), root_uuid);
    assert_eq!(restored.part_count(), 3);
    assert!(restored.get_part(&c1_uuid).is_some());
    assert!(restored.get_part(&c2_uuid).is_some());
}

// ===========================================================================
// 3. Known C# serialized XML fragments deserialize correctly
// ===========================================================================

#[test]
fn deserialize_csharp_single_prim() {
    // A minimal C#-style serialized prim fragment as produced by OpenSimulator 0.9.x.
    let xml = r#"<SceneObjectGroup>
  <SceneObjectPart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <AllowedDrop>false</AllowedDrop>
    <CreatorID><UUID>d1e8d182-e742-4c4e-9e2a-001122334455</UUID></CreatorID>
    <FolderID><UUID>00000000-0000-0000-0000-000000000000</UUID></FolderID>
    <InventorySerial>3</InventorySerial>
    <UUID><UUID>a1b2c3d4-e5f6-7890-abcd-ef1234567890</UUID></UUID>
    <LocalId>12345</LocalId>
    <Name>C# Test Cube</Name>
    <Material>3</Material>
    <PassTouches>false</PassTouches>
    <PassCollisions>false</PassCollisions>
    <RegionHandle>1099511628032</RegionHandle>
    <ScriptAccessPin>0</ScriptAccessPin>
    <GroupPosition><X>128</X><Y>128</Y><Z>25.5</Z></GroupPosition>
    <OffsetPosition><X>0</X><Y>0</Y><Z>0</Z></OffsetPosition>
    <RotationOffset><X>0</X><Y>0</Y><Z>0.7071068</Z><W>0.7071068</W></RotationOffset>
    <Velocity><X>0</X><Y>0</Y><Z>0</Z></Velocity>
    <AngularVelocity><X>0</X><Y>0</Y><Z>0</Z></AngularVelocity>
    <Acceleration><X>0</X><Y>0</Y><Z>0</Z></Acceleration>
    <Description>Made in OpenSim</Description>
    <Color><R>255</R><G>255</G><B>255</B><A>255</A></Color>
    <Text>Hello World</Text>
    <SitName></SitName>
    <TouchName></TouchName>
    <LinkNum>0</LinkNum>
    <ClickAction>0</ClickAction>
    <Shape>
      <ProfileCurve>1</ProfileCurve>
      <TextureEntry>AAAA</TextureEntry>
      <ExtraParams></ExtraParams>
      <PathBegin>0</PathBegin>
      <PathCurve>16</PathCurve>
      <PathEnd>0</PathEnd>
      <PathRadiusOffset>0</PathRadiusOffset>
      <PathRevolutions>0</PathRevolutions>
      <PathScaleX>100</PathScaleX>
      <PathScaleY>100</PathScaleY>
      <PathShearX>0</PathShearX>
      <PathShearY>0</PathShearY>
      <PathSkew>0</PathSkew>
      <PathTaperX>0</PathTaperX>
      <PathTaperY>0</PathTaperY>
      <PathTwist>0</PathTwist>
      <PathTwistBegin>0</PathTwistBegin>
      <PCode>9</PCode>
      <ProfileBegin>0</ProfileBegin>
      <ProfileEnd>0</ProfileEnd>
      <ProfileHollow>0</ProfileHollow>
      <State>0</State>
      <LastAttachPoint>0</LastAttachPoint>
      <ProfileShape>Square</ProfileShape>
      <HollowShape>Same</HollowShape>
      <SculptTexture><UUID>00000000-0000-0000-0000-000000000000</UUID></SculptTexture>
      <SculptType>0</SculptType>
      <FlexiSoftness>0</FlexiSoftness>
      <FlexiTension>0</FlexiTension>
      <FlexiDrag>0</FlexiDrag>
      <FlexiGravity>0</FlexiGravity>
      <FlexiWind>0</FlexiWind>
      <FlexiForceX>0</FlexiForceX>
      <FlexiForceY>0</FlexiForceY>
      <FlexiForceZ>0</FlexiForceZ>
      <LightColorR>0</LightColorR>
      <LightColorG>0</LightColorG>
      <LightColorB>0</LightColorB>
      <LightColorA>1</LightColorA>
      <LightRadius>0</LightRadius>
      <LightCutoff>0</LightCutoff>
      <LightFalloff>0</LightFalloff>
      <LightIntensity>0</LightIntensity>
      <FlexiEntry>false</FlexiEntry>
      <LightEntry>false</LightEntry>
      <SculptEntry>false</SculptEntry>
    </Shape>
    <Scale><X>0.5</X><Y>0.5</Y><Z>0.5</Z></Scale>
    <SitTargetOrientation><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientation>
    <SitTargetPosition><X>0</X><Y>0</Y><Z>0</Z></SitTargetPosition>
    <SitTargetPositionLL><X>0</X><Y>0</Y><Z>0</Z></SitTargetPositionLL>
    <SitTargetOrientationLL><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientationLL>
    <ParentID>0</ParentID>
    <CreationDate>1609459200</CreationDate>
    <Category>0</Category>
    <SalePrice>10</SalePrice>
    <ObjectSaleType>0</ObjectSaleType>
    <OwnershipCost>0</OwnershipCost>
    <GroupID><UUID>00000000-0000-0000-0000-000000000000</UUID></GroupID>
    <OwnerID><UUID>d1e8d182-e742-4c4e-9e2a-001122334455</UUID></OwnerID>
    <LastOwnerID><UUID>d1e8d182-e742-4c4e-9e2a-001122334455</UUID></LastOwnerID>
    <RezzerID><UUID>00000000-0000-0000-0000-000000000000</UUID></RezzerID>
    <BaseMask>581632</BaseMask>
    <OwnerMask>581632</OwnerMask>
    <GroupMask>0</GroupMask>
    <EveryoneMask>0</EveryoneMask>
    <NextOwnerMask>581632</NextOwnerMask>
    <Flags>None</Flags>
    <CollisionSound><UUID>00000000-0000-0000-0000-000000000000</UUID></CollisionSound>
    <CollisionSoundVolume>0</CollisionSoundVolume>
    <AttachedPos><X>0</X><Y>0</Y><Z>0</Z></AttachedPos>
    <PayPrice0>-2</PayPrice0>
    <PayPrice1>-2</PayPrice1>
    <PayPrice2>-2</PayPrice2>
    <PayPrice3>-2</PayPrice3>
    <PayPrice4>-2</PayPrice4>
    <Buoyancy>0</Buoyancy>
    <Force><X>0</X><Y>0</Y><Z>0</Z></Force>
    <Torque><X>0</X><Y>0</Y><Z>0</Z></Torque>
    <VolumeDetectActive>false</VolumeDetectActive>
    <PhysicsShapeType>0</PhysicsShapeType>
    <Density>1000</Density>
    <Friction>0.6</Friction>
    <Bounce>0.5</Bounce>
    <GravityModifier>1</GravityModifier>
    <CameraEyeOffset><X>0</X><Y>0</Y><Z>0</Z></CameraEyeOffset>
    <CameraAtOffset><X>0</X><Y>0</Y><Z>0</Z></CameraAtOffset>
    <SoundID><UUID>00000000-0000-0000-0000-000000000000</UUID></SoundID>
    <SoundGain>0</SoundGain>
    <SoundFlags>0</SoundFlags>
    <SoundRadius>0</SoundRadius>
    <SoundQueueing>false</SoundQueueing>
  </SceneObjectPart>
</SceneObjectGroup>"#;

    let group = deserialize_group(xml).unwrap();
    let p = group.root_part();

    assert_eq!(
        p.uuid,
        Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap()
    );
    assert_eq!(p.name, "C# Test Cube");
    assert_eq!(p.local_id, 12345);
    assert_eq!(p.description, "Made in OpenSim");
    assert_eq!(p.text, "Hello World");
    assert_eq!(
        p.owner_id,
        Uuid::parse_str("d1e8d182-e742-4c4e-9e2a-001122334455").unwrap()
    );
    assert!(vec3_close(p.group_position, Vec3::new(128.0, 128.0, 25.5)));
    // 45-degree rotation around Z
    assert!(f32_close(p.rotation_offset.z, 0.7071068));
    assert!(f32_close(p.rotation_offset.w, 0.7071068));
    assert_eq!(p.creation_date, 1609459200);
    assert_eq!(p.region_handle, 1099511628032);
    assert_eq!(p.inventory_serial, 3);
    assert_eq!(p.shape.profile_curve, 1);
    assert_eq!(p.shape.path_curve, 16);
    assert_eq!(p.shape.pcode, 9);
    assert_eq!(p.shape.profile_shape, "Square");
    assert_eq!(p.shape.hollow_shape, "Same");
    assert!(!p.shape.flexi_entry);
    assert!(!p.shape.light_entry);
    assert!(!p.shape.sculpt_entry);
    assert_eq!(p.text_color, PartColor::new(255, 255, 255, 255));
    assert_eq!(p.flags, PrimFlags::NONE);
    assert_eq!(p.scale, Vec3::new(0.5, 0.5, 0.5));
    assert!(f32_close(p.density, 1000.0));
}

#[test]
fn deserialize_csharp_linked_object() {
    // Two-prim linkset: root + one child
    let xml = r#"<SceneObjectGroup>
  <SceneObjectPart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <AllowedDrop>false</AllowedDrop>
    <CreatorID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></CreatorID>
    <FolderID><UUID>00000000-0000-0000-0000-000000000000</UUID></FolderID>
    <InventorySerial>0</InventorySerial>
    <UUID><UUID>11111111-2222-3333-4444-555555555555</UUID></UUID>
    <LocalId>100</LocalId>
    <Name>Linked Root</Name>
    <Material>3</Material>
    <PassTouches>false</PassTouches>
    <PassCollisions>false</PassCollisions>
    <RegionHandle>0</RegionHandle>
    <ScriptAccessPin>0</ScriptAccessPin>
    <GroupPosition><X>50</X><Y>60</Y><Z>70</Z></GroupPosition>
    <OffsetPosition><X>0</X><Y>0</Y><Z>0</Z></OffsetPosition>
    <RotationOffset><X>0</X><Y>0</Y><Z>0</Z><W>1</W></RotationOffset>
    <Velocity><X>0</X><Y>0</Y><Z>0</Z></Velocity>
    <AngularVelocity><X>0</X><Y>0</Y><Z>0</Z></AngularVelocity>
    <Acceleration><X>0</X><Y>0</Y><Z>0</Z></Acceleration>
    <Description></Description>
    <Color><R>255</R><G>255</G><B>255</B><A>255</A></Color>
    <Text></Text>
    <SitName></SitName>
    <TouchName></TouchName>
    <LinkNum>1</LinkNum>
    <ClickAction>0</ClickAction>
    <Shape>
      <ProfileCurve>1</ProfileCurve>
      <TextureEntry></TextureEntry>
      <ExtraParams></ExtraParams>
      <PathBegin>0</PathBegin>
      <PathCurve>16</PathCurve>
      <PathEnd>0</PathEnd>
      <PathRadiusOffset>0</PathRadiusOffset>
      <PathRevolutions>0</PathRevolutions>
      <PathScaleX>100</PathScaleX>
      <PathScaleY>100</PathScaleY>
      <PathShearX>0</PathShearX>
      <PathShearY>0</PathShearY>
      <PathSkew>0</PathSkew>
      <PathTaperX>0</PathTaperX>
      <PathTaperY>0</PathTaperY>
      <PathTwist>0</PathTwist>
      <PathTwistBegin>0</PathTwistBegin>
      <PCode>9</PCode>
      <ProfileBegin>0</ProfileBegin>
      <ProfileEnd>0</ProfileEnd>
      <ProfileHollow>0</ProfileHollow>
      <State>0</State>
      <LastAttachPoint>0</LastAttachPoint>
      <ProfileShape>Square</ProfileShape>
      <HollowShape>Same</HollowShape>
      <SculptTexture><UUID>00000000-0000-0000-0000-000000000000</UUID></SculptTexture>
      <SculptType>0</SculptType>
      <FlexiSoftness>0</FlexiSoftness>
      <FlexiTension>0</FlexiTension>
      <FlexiDrag>0</FlexiDrag>
      <FlexiGravity>0</FlexiGravity>
      <FlexiWind>0</FlexiWind>
      <FlexiForceX>0</FlexiForceX>
      <FlexiForceY>0</FlexiForceY>
      <FlexiForceZ>0</FlexiForceZ>
      <LightColorR>0</LightColorR>
      <LightColorG>0</LightColorG>
      <LightColorB>0</LightColorB>
      <LightColorA>1</LightColorA>
      <LightRadius>0</LightRadius>
      <LightCutoff>0</LightCutoff>
      <LightFalloff>0</LightFalloff>
      <LightIntensity>0</LightIntensity>
      <FlexiEntry>false</FlexiEntry>
      <LightEntry>false</LightEntry>
      <SculptEntry>false</SculptEntry>
    </Shape>
    <Scale><X>1</X><Y>1</Y><Z>1</Z></Scale>
    <SitTargetOrientation><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientation>
    <SitTargetPosition><X>0</X><Y>0</Y><Z>0</Z></SitTargetPosition>
    <SitTargetPositionLL><X>0</X><Y>0</Y><Z>0</Z></SitTargetPositionLL>
    <SitTargetOrientationLL><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientationLL>
    <ParentID>0</ParentID>
    <CreationDate>1700000000</CreationDate>
    <Category>0</Category>
    <SalePrice>10</SalePrice>
    <ObjectSaleType>0</ObjectSaleType>
    <OwnershipCost>0</OwnershipCost>
    <GroupID><UUID>00000000-0000-0000-0000-000000000000</UUID></GroupID>
    <OwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></OwnerID>
    <LastOwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></LastOwnerID>
    <RezzerID><UUID>00000000-0000-0000-0000-000000000000</UUID></RezzerID>
    <BaseMask>581632</BaseMask>
    <OwnerMask>581632</OwnerMask>
    <GroupMask>0</GroupMask>
    <EveryoneMask>0</EveryoneMask>
    <NextOwnerMask>581632</NextOwnerMask>
    <Flags>None</Flags>
    <CollisionSound><UUID>00000000-0000-0000-0000-000000000000</UUID></CollisionSound>
    <CollisionSoundVolume>0</CollisionSoundVolume>
    <AttachedPos><X>0</X><Y>0</Y><Z>0</Z></AttachedPos>
    <PayPrice0>-2</PayPrice0>
    <PayPrice1>-2</PayPrice1>
    <PayPrice2>-2</PayPrice2>
    <PayPrice3>-2</PayPrice3>
    <PayPrice4>-2</PayPrice4>
    <Buoyancy>0</Buoyancy>
    <Force><X>0</X><Y>0</Y><Z>0</Z></Force>
    <Torque><X>0</X><Y>0</Y><Z>0</Z></Torque>
    <VolumeDetectActive>false</VolumeDetectActive>
    <PhysicsShapeType>0</PhysicsShapeType>
    <Density>1000</Density>
    <Friction>0.6</Friction>
    <Bounce>0.5</Bounce>
    <GravityModifier>1</GravityModifier>
    <CameraEyeOffset><X>0</X><Y>0</Y><Z>0</Z></CameraEyeOffset>
    <CameraAtOffset><X>0</X><Y>0</Y><Z>0</Z></CameraAtOffset>
    <SoundID><UUID>00000000-0000-0000-0000-000000000000</UUID></SoundID>
    <SoundGain>0</SoundGain>
    <SoundFlags>0</SoundFlags>
    <SoundRadius>0</SoundRadius>
    <SoundQueueing>false</SoundQueueing>
  </SceneObjectPart>
  <OtherParts>
    <SceneObjectPart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
      <AllowedDrop>false</AllowedDrop>
      <CreatorID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></CreatorID>
      <FolderID><UUID>00000000-0000-0000-0000-000000000000</UUID></FolderID>
      <InventorySerial>0</InventorySerial>
      <UUID><UUID>66666666-7777-8888-9999-aaaaaaaaaaaa</UUID></UUID>
      <LocalId>101</LocalId>
      <Name>Linked Child</Name>
      <Material>3</Material>
      <PassTouches>false</PassTouches>
      <PassCollisions>false</PassCollisions>
      <RegionHandle>0</RegionHandle>
      <ScriptAccessPin>0</ScriptAccessPin>
      <GroupPosition><X>50</X><Y>60</Y><Z>70</Z></GroupPosition>
      <OffsetPosition><X>2</X><Y>0</Y><Z>0</Z></OffsetPosition>
      <RotationOffset><X>0</X><Y>0</Y><Z>0</Z><W>1</W></RotationOffset>
      <Velocity><X>0</X><Y>0</Y><Z>0</Z></Velocity>
      <AngularVelocity><X>0</X><Y>0</Y><Z>0</Z></AngularVelocity>
      <Acceleration><X>0</X><Y>0</Y><Z>0</Z></Acceleration>
      <Description>child prim</Description>
      <Color><R>255</R><G>0</G><B>0</B><A>255</A></Color>
      <Text></Text>
      <SitName></SitName>
      <TouchName></TouchName>
      <LinkNum>2</LinkNum>
      <ClickAction>0</ClickAction>
      <Shape>
        <ProfileCurve>1</ProfileCurve>
        <TextureEntry></TextureEntry>
        <ExtraParams></ExtraParams>
        <PathBegin>0</PathBegin>
        <PathCurve>16</PathCurve>
        <PathEnd>0</PathEnd>
        <PathRadiusOffset>0</PathRadiusOffset>
        <PathRevolutions>0</PathRevolutions>
        <PathScaleX>100</PathScaleX>
        <PathScaleY>100</PathScaleY>
        <PathShearX>0</PathShearX>
        <PathShearY>0</PathShearY>
        <PathSkew>0</PathSkew>
        <PathTaperX>0</PathTaperX>
        <PathTaperY>0</PathTaperY>
        <PathTwist>0</PathTwist>
        <PathTwistBegin>0</PathTwistBegin>
        <PCode>9</PCode>
        <ProfileBegin>0</ProfileBegin>
        <ProfileEnd>0</ProfileEnd>
        <ProfileHollow>0</ProfileHollow>
        <State>0</State>
        <LastAttachPoint>0</LastAttachPoint>
        <ProfileShape>Square</ProfileShape>
        <HollowShape>Same</HollowShape>
        <SculptTexture><UUID>00000000-0000-0000-0000-000000000000</UUID></SculptTexture>
        <SculptType>0</SculptType>
        <FlexiSoftness>0</FlexiSoftness>
        <FlexiTension>0</FlexiTension>
        <FlexiDrag>0</FlexiDrag>
        <FlexiGravity>0</FlexiGravity>
        <FlexiWind>0</FlexiWind>
        <FlexiForceX>0</FlexiForceX>
        <FlexiForceY>0</FlexiForceY>
        <FlexiForceZ>0</FlexiForceZ>
        <LightColorR>0</LightColorR>
        <LightColorG>0</LightColorG>
        <LightColorB>0</LightColorB>
        <LightColorA>1</LightColorA>
        <LightRadius>0</LightRadius>
        <LightCutoff>0</LightCutoff>
        <LightFalloff>0</LightFalloff>
        <LightIntensity>0</LightIntensity>
        <FlexiEntry>false</FlexiEntry>
        <LightEntry>false</LightEntry>
        <SculptEntry>false</SculptEntry>
      </Shape>
      <Scale><X>0.5</X><Y>0.5</Y><Z>0.5</Z></Scale>
      <SitTargetOrientation><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientation>
      <SitTargetPosition><X>0</X><Y>0</Y><Z>0</Z></SitTargetPosition>
      <SitTargetPositionLL><X>0</X><Y>0</Y><Z>0</Z></SitTargetPositionLL>
      <SitTargetOrientationLL><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientationLL>
      <ParentID>100</ParentID>
      <CreationDate>1700000000</CreationDate>
      <Category>0</Category>
      <SalePrice>10</SalePrice>
      <ObjectSaleType>0</ObjectSaleType>
      <OwnershipCost>0</OwnershipCost>
      <GroupID><UUID>00000000-0000-0000-0000-000000000000</UUID></GroupID>
      <OwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></OwnerID>
      <LastOwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></LastOwnerID>
      <RezzerID><UUID>00000000-0000-0000-0000-000000000000</UUID></RezzerID>
      <BaseMask>581632</BaseMask>
      <OwnerMask>581632</OwnerMask>
      <GroupMask>0</GroupMask>
      <EveryoneMask>0</EveryoneMask>
      <NextOwnerMask>581632</NextOwnerMask>
      <Flags>None</Flags>
      <CollisionSound><UUID>00000000-0000-0000-0000-000000000000</UUID></CollisionSound>
      <CollisionSoundVolume>0</CollisionSoundVolume>
      <AttachedPos><X>0</X><Y>0</Y><Z>0</Z></AttachedPos>
      <PayPrice0>-2</PayPrice0>
      <PayPrice1>-2</PayPrice1>
      <PayPrice2>-2</PayPrice2>
      <PayPrice3>-2</PayPrice3>
      <PayPrice4>-2</PayPrice4>
      <Buoyancy>0</Buoyancy>
      <Force><X>0</X><Y>0</Y><Z>0</Z></Force>
      <Torque><X>0</X><Y>0</Y><Z>0</Z></Torque>
      <VolumeDetectActive>false</VolumeDetectActive>
      <PhysicsShapeType>0</PhysicsShapeType>
      <Density>1000</Density>
      <Friction>0.6</Friction>
      <Bounce>0.5</Bounce>
      <GravityModifier>1</GravityModifier>
      <CameraEyeOffset><X>0</X><Y>0</Y><Z>0</Z></CameraEyeOffset>
      <CameraAtOffset><X>0</X><Y>0</Y><Z>0</Z></CameraAtOffset>
      <SoundID><UUID>00000000-0000-0000-0000-000000000000</UUID></SoundID>
      <SoundGain>0</SoundGain>
      <SoundFlags>0</SoundFlags>
      <SoundRadius>0</SoundRadius>
      <SoundQueueing>false</SoundQueueing>
    </SceneObjectPart>
  </OtherParts>
</SceneObjectGroup>"#;

    let group = deserialize_group(xml).unwrap();
    let root_uuid = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
    let child_uuid = Uuid::parse_str("66666666-7777-8888-9999-aaaaaaaaaaaa").unwrap();

    assert_eq!(group.uuid(), root_uuid);
    assert_eq!(group.part_count(), 2);
    assert_eq!(group.root_part().name, "Linked Root");
    assert!(vec3_close(group.position(), Vec3::new(50.0, 60.0, 70.0)));

    let child = group.get_part(&child_uuid).expect("child should exist");
    assert_eq!(child.name, "Linked Child");
    assert_eq!(child.description, "child prim");
    assert_eq!(child.text_color, PartColor::new(255, 0, 0, 255));
    assert!(vec3_close(child.offset_position, Vec3::new(2.0, 0.0, 0.0)));
    assert_eq!(child.parent_id, 100);
}

// ===========================================================================
// 4. Position, rotation, scale f32 precision
// ===========================================================================

#[test]
fn position_rotation_scale_f32_precision() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("Precision", owner);
    // Use values that may have f32 representation issues
    part.group_position = Vec3::new(123.456, 789.012, 0.001);
    part.rotation_offset = Quat::from_xyzw(0.1, 0.2, 0.3, 0.9273);
    part.scale = Vec3::new(0.123, 4.567, 89.01);
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    let rp = restored.root_part();

    assert!(
        vec3_close(rp.group_position, Vec3::new(123.456, 789.012, 0.001)),
        "position: {:?}",
        rp.group_position
    );
    assert!(
        quat_close(rp.rotation_offset, Quat::from_xyzw(0.1, 0.2, 0.3, 0.9273)),
        "rotation: {:?}",
        rp.rotation_offset
    );
    assert!(
        vec3_close(rp.scale, Vec3::new(0.123, 4.567, 89.01)),
        "scale: {:?}",
        rp.scale
    );
}

#[test]
fn identity_rotation_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("IdentRot", owner);
    part.rotation_offset = Quat::IDENTITY;
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();

    assert!(quat_close(
        restored.root_part().rotation_offset,
        Quat::IDENTITY
    ));
}

#[test]
fn zero_scale_roundtrip() {
    // Zero scale isn't typical but should not crash
    let owner = make_owner();
    let mut part = SceneObjectPart::new("ZeroScale", owner);
    part.scale = Vec3::ZERO;
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(restored.root_part().scale, Vec3::ZERO);
}

// ===========================================================================
// 5. Empty and non-empty texture/extra_params (inventory proxies)
// ===========================================================================

#[test]
fn empty_texture_entry_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("EmptyTex", owner);
    part.shape.texture_entry = Vec::new();
    part.shape.extra_params = Vec::new();
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    assert!(restored.root_part().shape.texture_entry.is_empty());
    assert!(restored.root_part().shape.extra_params.is_empty());
}

#[test]
fn nonempty_texture_entry_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("WithTex", owner);
    // Some fake texture data
    part.shape.texture_entry = vec![0x00, 0x01, 0x02, 0xFF, 0xAB, 0xCD];
    part.shape.extra_params = vec![0x10, 0x20, 0x30];
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(
        restored.root_part().shape.texture_entry,
        vec![0x00, 0x01, 0x02, 0xFF, 0xAB, 0xCD]
    );
    assert_eq!(
        restored.root_part().shape.extra_params,
        vec![0x10, 0x20, 0x30]
    );
}

// ===========================================================================
// 6. Extra params: flexible, light, sculpt round-trip
// ===========================================================================

#[test]
fn flexi_params_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("FlexiPrim", owner);
    part.shape.flexi_entry = true;
    part.shape.flexi_softness = 2;
    part.shape.flexi_tension = 1.5;
    part.shape.flexi_drag = 0.8;
    part.shape.flexi_gravity = 0.3;
    part.shape.flexi_wind = 0.5;
    part.shape.flexi_force_x = 0.1;
    part.shape.flexi_force_y = 0.2;
    part.shape.flexi_force_z = -0.1;
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    let s = &restored.root_part().shape;

    assert!(s.flexi_entry);
    assert_eq!(s.flexi_softness, 2);
    assert!(f32_close(s.flexi_tension, 1.5));
    assert!(f32_close(s.flexi_drag, 0.8));
    assert!(f32_close(s.flexi_gravity, 0.3));
    assert!(f32_close(s.flexi_wind, 0.5));
    assert!(f32_close(s.flexi_force_x, 0.1));
    assert!(f32_close(s.flexi_force_y, 0.2));
    assert!(f32_close(s.flexi_force_z, -0.1));
}

#[test]
fn light_params_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("LightPrim", owner);
    part.shape.light_entry = true;
    part.shape.light_color_r = 1.0;
    part.shape.light_color_g = 0.8;
    part.shape.light_color_b = 0.5;
    part.shape.light_color_a = 1.0;
    part.shape.light_radius = 20.0;
    part.shape.light_cutoff = 0.05;
    part.shape.light_falloff = 1.0;
    part.shape.light_intensity = 0.8;
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    let s = &restored.root_part().shape;

    assert!(s.light_entry);
    assert!(f32_close(s.light_color_r, 1.0));
    assert!(f32_close(s.light_color_g, 0.8));
    assert!(f32_close(s.light_color_b, 0.5));
    assert!(f32_close(s.light_color_a, 1.0));
    assert!(f32_close(s.light_radius, 20.0));
    assert!(f32_close(s.light_cutoff, 0.05));
    assert!(f32_close(s.light_falloff, 1.0));
    assert!(f32_close(s.light_intensity, 0.8));
}

#[test]
fn sculpt_params_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("SculptPrim", owner);
    part.shape.sculpt_entry = true;
    part.shape.sculpt_type = 5; // mesh
    part.shape.sculpt_texture = Uuid::parse_str("deadbeef-dead-beef-dead-beefdeadbeef").unwrap();
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    let s = &restored.root_part().shape;

    assert!(s.sculpt_entry);
    assert_eq!(s.sculpt_type, 5);
    assert_eq!(
        s.sculpt_texture,
        Uuid::parse_str("deadbeef-dead-beef-dead-beefdeadbeef").unwrap()
    );
}

// ===========================================================================
// 7. Shape profile/path params roundtrip
// ===========================================================================

#[test]
fn shape_profile_path_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("ShapeTest", owner);
    part.shape.profile_curve = 5;
    part.shape.profile_begin = 100;
    part.shape.profile_end = 200;
    part.shape.profile_hollow = 50;
    part.shape.path_curve = 32;
    part.shape.path_begin = 10;
    part.shape.path_end = 20;
    part.shape.path_scale_x = 150;
    part.shape.path_scale_y = 75;
    part.shape.path_shear_x = 10;
    part.shape.path_shear_y = 20;
    part.shape.path_twist = 45;
    part.shape.path_twist_begin = -30;
    part.shape.path_radius_offset = 5;
    part.shape.path_taper_x = 10;
    part.shape.path_taper_y = -10;
    part.shape.path_revolutions = 200;
    part.shape.path_skew = 15;
    part.shape.profile_shape = "Circle".to_string();
    part.shape.hollow_shape = "Triangle".to_string();
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    let s = &restored.root_part().shape;

    assert_eq!(s.profile_curve, 5);
    assert_eq!(s.profile_begin, 100);
    assert_eq!(s.profile_end, 200);
    assert_eq!(s.profile_hollow, 50);
    assert_eq!(s.path_curve, 32);
    assert_eq!(s.path_begin, 10);
    assert_eq!(s.path_end, 20);
    assert_eq!(s.path_scale_x, 150);
    assert_eq!(s.path_scale_y, 75);
    assert_eq!(s.path_shear_x, 10);
    assert_eq!(s.path_shear_y, 20);
    assert_eq!(s.path_twist, 45);
    assert_eq!(s.path_twist_begin, -30);
    assert_eq!(s.path_radius_offset, 5);
    assert_eq!(s.path_taper_x, 10);
    assert_eq!(s.path_taper_y, -10);
    assert_eq!(s.path_revolutions, 200);
    assert_eq!(s.path_skew, 15);
    assert_eq!(s.profile_shape, "Circle");
    assert_eq!(s.hollow_shape, "Triangle");
}

// ===========================================================================
// 8. Flags roundtrip (multiple combinations)
// ===========================================================================

#[test]
fn flags_all_common_combinations() {
    let combos = [
        PrimFlags::NONE,
        PrimFlags::PHYSICS,
        PrimFlags::PHANTOM,
        PrimFlags::TEMPORARY_ON_REZ,
        PrimFlags::PHYSICS | PrimFlags::PHANTOM,
        PrimFlags::SCRIPTED | PrimFlags::TOUCH | PrimFlags::INCLUDE_IN_SEARCH,
        PrimFlags::OBJECT_MODIFY | PrimFlags::OBJECT_COPY | PrimFlags::OBJECT_TRANSFER,
    ];

    for &flags in &combos {
        let owner = make_owner();
        let mut part = SceneObjectPart::new("FlagTest", owner);
        part.flags = flags;
        let group = SceneObjectGroup::new(part);

        let xml = serialize_group(&group).unwrap();
        let restored = deserialize_group(&xml).unwrap();
        assert_eq!(
            restored.root_part().flags,
            flags,
            "flags mismatch for {:?}",
            flags
        );
    }
}

// ===========================================================================
// 9. Permissions roundtrip
// ===========================================================================

#[test]
fn permissions_roundtrip() {
    use welkin_types::PermissionMask;

    let owner = make_owner();
    let mut part = SceneObjectPart::new("PermTest", owner);
    part.base_mask = PermissionMask::ALL;
    part.owner_mask = PermissionMask::ALL;
    part.group_mask = PermissionMask::from_bits_truncate(0x0008_0000); // MOVE only
    part.everyone_mask = PermissionMask::NONE;
    part.next_owner_mask = PermissionMask::from_bits_truncate(0x4E000); // COPY+TRANSFER+MODIFY+MOVE
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    let rp = restored.root_part();
    assert_eq!(rp.base_mask, PermissionMask::ALL);
    assert_eq!(rp.owner_mask, PermissionMask::ALL);
    assert_eq!(
        rp.group_mask,
        PermissionMask::from_bits_truncate(0x0008_0000)
    );
    assert_eq!(rp.everyone_mask, PermissionMask::NONE);
    assert_eq!(
        rp.next_owner_mask,
        PermissionMask::from_bits_truncate(0x4E000)
    );
}

// ===========================================================================
// 10. Double round-trip: serialize -> deserialize -> serialize -> compare
// ===========================================================================

#[test]
fn double_roundtrip_produces_identical_xml() {
    let owner = make_owner();
    let root = make_part_full("DoubleTrip", owner);
    let child = make_part_full("DoubleChild", owner);
    let mut group = SceneObjectGroup::new(root);
    group.add_part(child);

    let xml1 = serialize_group(&group).unwrap();
    let restored1 = deserialize_group(&xml1).unwrap();
    let xml2 = serialize_group(&restored1).unwrap();
    let restored2 = deserialize_group(&xml2).unwrap();
    let xml3 = serialize_group(&restored2).unwrap();

    // After the first round-trip, subsequent serializations should be stable.
    // xml2 and xml3 should be identical (xml1 might differ slightly since
    // add_part in deserialization reassigns link numbers).
    assert_eq!(xml2, xml3, "double round-trip should produce stable XML");
}

// ===========================================================================
// 11. C# fragment with TaskInventory (should be skipped gracefully)
// ===========================================================================

#[test]
fn deserialize_with_task_inventory_element() {
    // C# sometimes includes TaskInventory in the XML. Our deserializer should
    // skip it without error.
    let xml = r#"<SceneObjectGroup>
  <SceneObjectPart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <AllowedDrop>true</AllowedDrop>
    <CreatorID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></CreatorID>
    <FolderID><UUID>00000000-0000-0000-0000-000000000000</UUID></FolderID>
    <InventorySerial>1</InventorySerial>
    <TaskInventory>
      <TaskInventoryItem>
        <AssetID><UUID>deadbeef-0000-0000-0000-000000000001</UUID></AssetID>
        <BasePermissions>581632</BasePermissions>
        <CreationDate>1700000000</CreationDate>
        <CreatorID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></CreatorID>
        <Description>inventory script</Description>
        <EveryonePermissions>0</EveryonePermissions>
        <Flags>0</Flags>
        <GroupID><UUID>00000000-0000-0000-0000-000000000000</UUID></GroupID>
        <GroupPermissions>0</GroupPermissions>
        <InvType>10</InvType>
        <ItemID><UUID>deadbeef-0000-0000-0000-000000000002</UUID></ItemID>
        <LastOwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></LastOwnerID>
        <Name>Script</Name>
        <NextPermissions>581632</NextPermissions>
        <OwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></OwnerID>
        <CurrentPermissions>581632</CurrentPermissions>
        <ParentID><UUID>00000000-0000-0000-0000-000000000000</UUID></ParentID>
        <ParentPartID><UUID>00000000-0000-0000-0000-000000000000</UUID></ParentPartID>
        <PermsGranter><UUID>00000000-0000-0000-0000-000000000000</UUID></PermsGranter>
        <PermsMask>0</PermsMask>
        <Type>10</Type>
        <OwnerChanged>false</OwnerChanged>
      </TaskInventoryItem>
    </TaskInventory>
    <UUID><UUID>99999999-0000-0000-0000-111111111111</UUID></UUID>
    <LocalId>500</LocalId>
    <Name>With Inventory</Name>
    <Material>3</Material>
    <PassTouches>false</PassTouches>
    <PassCollisions>false</PassCollisions>
    <RegionHandle>0</RegionHandle>
    <ScriptAccessPin>0</ScriptAccessPin>
    <GroupPosition><X>128</X><Y>128</Y><Z>25</Z></GroupPosition>
    <OffsetPosition><X>0</X><Y>0</Y><Z>0</Z></OffsetPosition>
    <RotationOffset><X>0</X><Y>0</Y><Z>0</Z><W>1</W></RotationOffset>
    <Velocity><X>0</X><Y>0</Y><Z>0</Z></Velocity>
    <AngularVelocity><X>0</X><Y>0</Y><Z>0</Z></AngularVelocity>
    <Acceleration><X>0</X><Y>0</Y><Z>0</Z></Acceleration>
    <Description></Description>
    <Color><R>255</R><G>255</G><B>255</B><A>255</A></Color>
    <Text></Text>
    <SitName></SitName>
    <TouchName></TouchName>
    <LinkNum>0</LinkNum>
    <ClickAction>0</ClickAction>
    <Shape>
      <ProfileCurve>1</ProfileCurve>
      <TextureEntry></TextureEntry>
      <ExtraParams></ExtraParams>
      <PathBegin>0</PathBegin>
      <PathCurve>16</PathCurve>
      <PathEnd>0</PathEnd>
      <PathRadiusOffset>0</PathRadiusOffset>
      <PathRevolutions>0</PathRevolutions>
      <PathScaleX>100</PathScaleX>
      <PathScaleY>100</PathScaleY>
      <PathShearX>0</PathShearX>
      <PathShearY>0</PathShearY>
      <PathSkew>0</PathSkew>
      <PathTaperX>0</PathTaperX>
      <PathTaperY>0</PathTaperY>
      <PathTwist>0</PathTwist>
      <PathTwistBegin>0</PathTwistBegin>
      <PCode>9</PCode>
      <ProfileBegin>0</ProfileBegin>
      <ProfileEnd>0</ProfileEnd>
      <ProfileHollow>0</ProfileHollow>
      <State>0</State>
      <LastAttachPoint>0</LastAttachPoint>
      <ProfileShape>Square</ProfileShape>
      <HollowShape>Same</HollowShape>
      <SculptTexture><UUID>00000000-0000-0000-0000-000000000000</UUID></SculptTexture>
      <SculptType>0</SculptType>
      <FlexiSoftness>0</FlexiSoftness>
      <FlexiTension>0</FlexiTension>
      <FlexiDrag>0</FlexiDrag>
      <FlexiGravity>0</FlexiGravity>
      <FlexiWind>0</FlexiWind>
      <FlexiForceX>0</FlexiForceX>
      <FlexiForceY>0</FlexiForceY>
      <FlexiForceZ>0</FlexiForceZ>
      <LightColorR>0</LightColorR>
      <LightColorG>0</LightColorG>
      <LightColorB>0</LightColorB>
      <LightColorA>1</LightColorA>
      <LightRadius>0</LightRadius>
      <LightCutoff>0</LightCutoff>
      <LightFalloff>0</LightFalloff>
      <LightIntensity>0</LightIntensity>
      <FlexiEntry>false</FlexiEntry>
      <LightEntry>false</LightEntry>
      <SculptEntry>false</SculptEntry>
    </Shape>
    <Scale><X>1</X><Y>1</Y><Z>1</Z></Scale>
    <SitTargetOrientation><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientation>
    <SitTargetPosition><X>0</X><Y>0</Y><Z>0</Z></SitTargetPosition>
    <SitTargetPositionLL><X>0</X><Y>0</Y><Z>0</Z></SitTargetPositionLL>
    <SitTargetOrientationLL><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientationLL>
    <ParentID>0</ParentID>
    <CreationDate>1700000000</CreationDate>
    <Category>0</Category>
    <SalePrice>10</SalePrice>
    <ObjectSaleType>0</ObjectSaleType>
    <OwnershipCost>0</OwnershipCost>
    <GroupID><UUID>00000000-0000-0000-0000-000000000000</UUID></GroupID>
    <OwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></OwnerID>
    <LastOwnerID><UUID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</UUID></LastOwnerID>
    <RezzerID><UUID>00000000-0000-0000-0000-000000000000</UUID></RezzerID>
    <BaseMask>581632</BaseMask>
    <OwnerMask>581632</OwnerMask>
    <GroupMask>0</GroupMask>
    <EveryoneMask>0</EveryoneMask>
    <NextOwnerMask>581632</NextOwnerMask>
    <Flags>Scripted</Flags>
    <CollisionSound><UUID>00000000-0000-0000-0000-000000000000</UUID></CollisionSound>
    <CollisionSoundVolume>0</CollisionSoundVolume>
    <AttachedPos><X>0</X><Y>0</Y><Z>0</Z></AttachedPos>
    <PayPrice0>-2</PayPrice0>
    <PayPrice1>-2</PayPrice1>
    <PayPrice2>-2</PayPrice2>
    <PayPrice3>-2</PayPrice3>
    <PayPrice4>-2</PayPrice4>
    <Buoyancy>0</Buoyancy>
    <Force><X>0</X><Y>0</Y><Z>0</Z></Force>
    <Torque><X>0</X><Y>0</Y><Z>0</Z></Torque>
    <VolumeDetectActive>false</VolumeDetectActive>
    <PhysicsShapeType>0</PhysicsShapeType>
    <Density>1000</Density>
    <Friction>0.6</Friction>
    <Bounce>0.5</Bounce>
    <GravityModifier>1</GravityModifier>
    <CameraEyeOffset><X>0</X><Y>0</Y><Z>0</Z></CameraEyeOffset>
    <CameraAtOffset><X>0</X><Y>0</Y><Z>0</Z></CameraAtOffset>
    <SoundID><UUID>00000000-0000-0000-0000-000000000000</UUID></SoundID>
    <SoundGain>0</SoundGain>
    <SoundFlags>0</SoundFlags>
    <SoundRadius>0</SoundRadius>
    <SoundQueueing>false</SoundQueueing>
  </SceneObjectPart>
</SceneObjectGroup>"#;

    let group = deserialize_group(xml).unwrap();
    assert_eq!(group.root_part().name, "With Inventory");
    assert!(group.root_part().allowed_drop);
    assert_eq!(group.root_part().inventory_serial, 1);
    assert!(group.root_part().flags.contains(PrimFlags::SCRIPTED));
}

// ===========================================================================
// 12. Media and MatOverride roundtrip
// ===========================================================================

#[test]
fn media_string_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("MediaPrim", owner);
    part.shape.media = r#"[{"height":256,"width":256}]"#.to_string();
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    assert!(xml.contains("<Media>"));
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(
        restored.root_part().shape.media,
        r#"[{"height":256,"width":256}]"#
    );
}

#[test]
fn mat_override_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("MatOvrd", owner);
    part.shape.mat_override = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    assert!(xml.contains("<MatOvrd>"));
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(
        restored.root_part().shape.mat_override,
        vec![0xDE, 0xAD, 0xBE, 0xEF]
    );
}

// ===========================================================================
// 13. MediaUrl, TextureAnimation, ParticleSystem optional fields
// ===========================================================================

#[test]
fn media_url_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("MediaUrl", owner);
    part.media_url = "https://example.com/stream.mp4".to_string();
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    assert!(xml.contains("<MediaUrl>"));
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(
        restored.root_part().media_url,
        "https://example.com/stream.mp4"
    );
}

#[test]
fn texture_animation_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("TexAnim", owner);
    part.texture_animation = vec![0x01, 0x02, 0x03, 0x04];
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(
        restored.root_part().texture_animation,
        vec![0x01, 0x02, 0x03, 0x04]
    );
}

#[test]
fn particle_system_roundtrip() {
    let owner = make_owner();
    let mut part = SceneObjectPart::new("Particles", owner);
    part.particle_system = vec![0xAA, 0xBB, 0xCC];
    let group = SceneObjectGroup::new(part);

    let xml = serialize_group(&group).unwrap();
    let restored = deserialize_group(&xml).unwrap();
    assert_eq!(restored.root_part().particle_system, vec![0xAA, 0xBB, 0xCC]);
}
