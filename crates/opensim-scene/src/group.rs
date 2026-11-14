// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use glam::{Quat, Vec3};
use uuid::Uuid;

use crate::part::{PrimFlags, SceneObjectPart};

#[derive(Debug, Clone)]
pub struct SceneObjectGroup {
    root_part: SceneObjectPart,
    child_parts: HashMap<Uuid, SceneObjectPart>,

    pub is_selected: bool,
    pub is_deleted: bool,
    pub is_attachment: bool,
    pub attachment_point: u8,
    pub attached_avatar: Uuid,

    pub has_changed: bool,
    pub time_last_changed: i64,
    pub time_first_changed: i64,
    pub backup_needed: bool,
}

impl SceneObjectGroup {
    pub fn new(mut root: SceneObjectPart) -> Self {
        root.link_num = 0;
        Self {
            root_part: root,
            child_parts: HashMap::new(),
            is_selected: false,
            is_deleted: false,
            is_attachment: false,
            attachment_point: 0,
            attached_avatar: Uuid::nil(),
            has_changed: false,
            time_last_changed: 0,
            time_first_changed: 0,
            backup_needed: false,
        }
    }

    pub fn uuid(&self) -> Uuid {
        self.root_part.uuid
    }

    pub fn name(&self) -> &str {
        &self.root_part.name
    }

    pub fn root_part(&self) -> &SceneObjectPart {
        &self.root_part
    }

    pub fn root_part_mut(&mut self) -> &mut SceneObjectPart {
        &mut self.root_part
    }

    pub fn position(&self) -> Vec3 {
        self.root_part.group_position
    }

    pub fn rotation(&self) -> Quat {
        self.root_part.rotation_offset
    }

    pub fn set_position(&mut self, pos: Vec3) {
        self.root_part.group_position = pos;
        for child in self.child_parts.values_mut() {
            child.group_position = pos;
        }
        self.mark_changed();
    }

    pub fn set_rotation(&mut self, rot: Quat) {
        self.root_part.rotation_offset = rot;
        self.mark_changed();
    }

    pub fn add_part(&mut self, mut part: SceneObjectPart) {
        let next_link = self.part_count() as i32 + 1;
        part.link_num = next_link;
        part.group_position = self.root_part.group_position;
        self.child_parts.insert(part.uuid, part);
        if self.child_parts.len() == 1 {
            self.root_part.link_num = 1;
        }
        self.mark_changed();
    }

    pub fn remove_part(&mut self, uuid: &Uuid) -> Option<SceneObjectPart> {
        let removed = self.child_parts.remove(uuid);
        if removed.is_some() {
            self.renumber_links();
            self.mark_changed();
        }
        removed
    }

    pub fn get_part(&self, uuid: &Uuid) -> Option<&SceneObjectPart> {
        if self.root_part.uuid == *uuid {
            Some(&self.root_part)
        } else {
            self.child_parts.get(uuid)
        }
    }

    pub fn get_part_mut(&mut self, uuid: &Uuid) -> Option<&mut SceneObjectPart> {
        if self.root_part.uuid == *uuid {
            Some(&mut self.root_part)
        } else {
            self.child_parts.get_mut(uuid)
        }
    }

    pub fn get_part_by_local_id(&self, local_id: u32) -> Option<&SceneObjectPart> {
        if self.root_part.local_id == local_id {
            return Some(&self.root_part);
        }
        self.child_parts.values().find(|p| p.local_id == local_id)
    }

    pub fn parts(&self) -> impl Iterator<Item = &SceneObjectPart> {
        let mut all: Vec<&SceneObjectPart> = Vec::with_capacity(self.part_count());
        all.push(&self.root_part);
        let mut children: Vec<&SceneObjectPart> = self.child_parts.values().collect();
        children.sort_by_key(|p| p.link_num);
        all.extend(children);
        all.into_iter()
    }

    pub fn parts_mut(&mut self) -> Vec<&mut SceneObjectPart> {
        let mut all: Vec<&mut SceneObjectPart> = Vec::with_capacity(self.part_count());
        all.push(&mut self.root_part);
        let mut children: Vec<&mut SceneObjectPart> = self.child_parts.values_mut().collect();
        children.sort_by_key(|p| p.link_num);
        all.extend(children);
        all
    }

    pub fn part_count(&self) -> usize {
        1 + self.child_parts.len()
    }

    pub fn link_count(&self) -> i32 {
        self.part_count() as i32
    }

    pub fn is_physical(&self) -> bool {
        self.root_part.flags.contains(PrimFlags::PHYSICS)
    }

    pub fn is_phantom(&self) -> bool {
        self.root_part.flags.contains(PrimFlags::PHANTOM)
    }

    pub fn is_temporary(&self) -> bool {
        self.root_part.flags.contains(PrimFlags::TEMPORARY_ON_REZ)
    }

    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let half = self.root_part.scale * 0.5;
        let pos = self.root_part.group_position + self.root_part.offset_position;
        let mut min = pos - half;
        let mut max = pos + half;

        for child in self.child_parts.values() {
            let child_half = child.scale * 0.5;
            let child_pos = child.group_position + child.offset_position;
            min = min.min(child_pos - child_half);
            max = max.max(child_pos + child_half);
        }

        (min, max)
    }

    pub fn mark_changed(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if !self.has_changed {
            self.time_first_changed = now;
        }
        self.has_changed = true;
        self.time_last_changed = now;
        self.backup_needed = true;
    }

    fn renumber_links(&mut self) {
        let has_children = !self.child_parts.is_empty();
        self.root_part.link_num = if has_children { 1 } else { 0 };

        let mut sorted: Vec<(&Uuid, &mut SceneObjectPart)> = self.child_parts.iter_mut().collect();
        sorted.sort_by_key(|(_, p)| p.link_num);

        for (i, (_, part)) in sorted.into_iter().enumerate() {
            part.link_num = (i + 2) as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::SceneObjectPart;

    fn make_part() -> SceneObjectPart {
        SceneObjectPart::new("Test Part", Uuid::new_v4())
    }

    #[test]
    fn new_group_has_root_only() {
        let root = make_part();
        let root_uuid = root.uuid;
        let group = SceneObjectGroup::new(root);
        assert_eq!(group.uuid(), root_uuid);
        assert_eq!(group.part_count(), 1);
        assert_eq!(group.link_count(), 1);
        assert_eq!(group.root_part().link_num, 0);
    }

    #[test]
    fn add_child_updates_links() {
        let mut group = SceneObjectGroup::new(make_part());
        let child = make_part();
        let child_uuid = child.uuid;
        group.add_part(child);

        assert_eq!(group.part_count(), 2);
        assert_eq!(group.root_part().link_num, 1);
        assert_eq!(group.get_part(&child_uuid).unwrap().link_num, 2);
    }

    #[test]
    fn remove_part_renumbers() {
        let mut group = SceneObjectGroup::new(make_part());
        let c1 = make_part();
        let c2 = make_part();
        let c1_uuid = c1.uuid;
        let c2_uuid = c2.uuid;
        group.add_part(c1);
        group.add_part(c2);

        assert_eq!(group.part_count(), 3);
        group.remove_part(&c1_uuid);
        assert_eq!(group.part_count(), 2);
        assert_eq!(group.get_part(&c2_uuid).unwrap().link_num, 2);
    }

    #[test]
    fn set_position_propagates() {
        let mut group = SceneObjectGroup::new(make_part());
        group.add_part(make_part());
        let new_pos = Vec3::new(100.0, 200.0, 50.0);
        group.set_position(new_pos);

        assert_eq!(group.position(), new_pos);
        for part in group.parts() {
            assert_eq!(part.group_position, new_pos);
        }
    }

    #[test]
    fn parts_iter_root_first() {
        let mut group = SceneObjectGroup::new(make_part());
        let root_uuid = group.uuid();
        group.add_part(make_part());
        group.add_part(make_part());

        let uuids: Vec<Uuid> = group.parts().map(|p| p.uuid).collect();
        assert_eq!(uuids[0], root_uuid);
        assert_eq!(uuids.len(), 3);
    }

    #[test]
    fn bounding_box_encloses_all() {
        let mut root = make_part();
        root.group_position = Vec3::ZERO;
        root.scale = Vec3::ONE;
        let mut group = SceneObjectGroup::new(root);

        let mut child = make_part();
        child.offset_position = Vec3::new(10.0, 0.0, 0.0);
        child.scale = Vec3::ONE;
        group.add_part(child);

        let (min, max) = group.bounding_box();
        assert!(min.x <= -0.5);
        assert!(max.x >= 10.5);
    }

    #[test]
    fn get_by_local_id() {
        let mut root = make_part();
        root.local_id = 100;
        let mut group = SceneObjectGroup::new(root);

        let mut child = make_part();
        child.local_id = 101;
        let child_uuid = child.uuid;
        group.add_part(child);

        assert_eq!(group.get_part_by_local_id(100).unwrap().uuid, group.uuid());
        assert_eq!(group.get_part_by_local_id(101).unwrap().uuid, child_uuid);
        assert!(group.get_part_by_local_id(999).is_none());
    }
}
