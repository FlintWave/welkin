// SPDX-License-Identifier: LGPL-3.0-or-later

//! Worn attachments: wearing an inventory object, attaching an in-world object, and
//! taking attachments off (to inventory or to the ground). Rules follow OpenSim's
//! `AttachmentsModule`; the scene model lives in `welkin_scene::scene`.

use std::net::SocketAddr;
use std::sync::Arc;

use glam::Vec3;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use welkin_data::assets::AssetRow;
use welkin_data::inventory::InventoryItemRow;
use welkin_modules::permissions::DefaultPermissionsModule;
use welkin_msg::msgs::{KillObject, kill_object};
use welkin_net::server::ClientSession;
use welkin_scene::scene::{MAX_ATTACHMENT_POINT, Scene};
use welkin_scene::xml2::serialize_group;
use welkin_services::remote::asset::RemoteAssetConnector;
use welkin_services::remote::avatar::RemoteAvatarConnector;
use welkin_services::remote::inventory::RemoteInventoryConnector;

use crate::inventory_rez::{self, DeRezAction};

type ClientMap = Arc<RwLock<std::collections::HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>;

/// Most attachments one avatar may wear (OpenSim `Constants.MaxAgentAttachments`).
pub const MAX_AGENT_ATTACHMENTS: usize = 38;
/// Attachment point used when nothing else is known (left hand).
const DEFAULT_ATTACH_POINT: u8 = 5;
/// Inventory item flags keep the worn attachment point in the low byte.
const ATTACH_POINT_FLAG_MASK: i64 = 0xff;
/// Set in a viewer's attachment point to add to, rather than replace, what is worn there.
const APPEND_FLAG: u8 = 0x80;

/// Everything the attachment code needs from the region.
pub struct AttachCtx<'a> {
    pub scene: &'a Scene,
    pub perms: &'a DefaultPermissionsModule,
    pub inventory: &'a RemoteInventoryConnector,
    pub assets: &'a RemoteAssetConnector,
    pub avatars: &'a RemoteAvatarConnector,
    pub socket: &'a UdpSocket,
    pub clients: &'a ClientMap,
}

/// Split a viewer's attachment-point byte into (point, append).
pub fn split_point(raw: u8) -> (u8, bool) {
    (raw & !APPEND_FLAG, raw & APPEND_FLAG != 0)
}

/// OpenSim's `AttachObjectInternal` choice of attachment point and offset.
/// `requested` is the viewer's point (0 = "wear", pick for me); `group_point` the point the
/// object already carries (0 if none); `last_point` / `attached_pos` what it remembers from
/// its last time worn; `world_pos` where it lies now. Returns `None` for an invalid point.
pub fn resolve_attach_point(
    requested: u8,
    group_point: u8,
    last_point: u8,
    attached_pos: Vec3,
    world_pos: Vec3,
) -> Option<(u8, Vec3)> {
    let mut pos = world_pos;
    // Moving to a different point than before: start at the point itself.
    if requested != 0 && requested != group_point {
        pos = Vec3::ZERO;
    }
    // Same point as last time: restore the saved offset.
    if requested != 0 && requested == last_point {
        pos = attached_pos;
    }
    let mut point = requested;
    if point == 0 {
        point = group_point;
    }
    if point == 0 {
        point = last_point;
        pos = attached_pos;
    }
    if point == 0 {
        point = DEFAULT_ATTACH_POINT;
        pos = Vec3::ZERO;
    }
    if point > MAX_ATTACHMENT_POINT {
        return None;
    }
    Some((point, pos))
}

fn now_secs() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0)
}

async fn send_to_all(ctx: &AttachCtx<'_>, msg: &impl welkin_msg::Msg) {
    let all = ctx.clients.read().await;
    for (addr, c) in all.iter() {
        let mut sess = c.lock().await;
        let (pkt, _) = sess.circuit.send_msg(msg, true);
        let _ = ctx.socket.send_to(&pkt, *addr).await;
    }
}

async fn kill(ctx: &AttachCtx<'_>, local_ids: Vec<u32>) {
    if local_ids.is_empty() {
        return;
    }
    let msg = KillObject {
        object_data: local_ids.into_iter().map(|id| kill_object::ObjectData { id }).collect(),
    };
    send_to_all(ctx, &msg).await;
}

fn local_ids(group: &welkin_scene::group::SceneObjectGroup) -> Vec<u32> {
    group.parts().map(|p| p.local_id).filter(|&id| id != 0).collect()
}

/// Tell the owner's viewer about a changed inventory item.
async fn notify_item(ctx: &AttachCtx<'_>, agent: Uuid, item: &InventoryItemRow) {
    let msg = crate::inventory_item_update(agent, item);
    let all = ctx.clients.read().await;
    for (addr, c) in all.iter() {
        let mut sess = c.lock().await;
        if sess.agent_id == agent {
            let (pkt, _) = sess.circuit.send_msg(&msg, true);
            let _ = ctx.socket.send_to(&pkt, *addr).await;
        }
    }
}

/// Mark an inventory item worn at `point` (0 = not worn) and tell the viewer.
async fn set_item_worn_point(ctx: &AttachCtx<'_>, agent: Uuid, item_id: Uuid, point: u8) {
    let Ok(Some(mut item)) = ctx.inventory.get_item(&item_id.to_string()).await else { return };
    item.flags = (item.flags & !ATTACH_POINT_FLAG_MASK) | i64::from(point);
    if matches!(ctx.inventory.update_item(&item).await, Ok(true)) {
        notify_item(ctx, agent, &item).await;
    }
}

/// Write an attachment's current state back to the inventory item it came from.
async fn save_group_into_item(
    ctx: &AttachCtx<'_>,
    agent: Uuid,
    item_id: Uuid,
    group: &welkin_scene::group::SceneObjectGroup,
) -> Result<(), String> {
    let mut item = ctx
        .inventory
        .get_item(&item_id.to_string())
        .await
        .map_err(|e| format!("inventory service: {e}"))?
        .ok_or_else(|| "the attachment's inventory item no longer exists".to_string())?;
    let xml = serialize_group(group).map_err(|e| format!("serialising the attachment: {e}"))?;
    let asset_id = Uuid::new_v4();
    let now = now_secs();
    ctx.assets
        .store_asset(&AssetRow {
            name: item.inventory_name.clone(),
            description: item.inventory_description.clone(),
            asset_type: 6,
            local: 0,
            temporary: 0,
            data: xml.into_bytes(),
            id: asset_id.to_string(),
            create_time: now,
            access_time: now,
            asset_flags: 0,
            creator_id: group.root_part().creator_id.to_string(),
        })
        .await
        .map_err(|e| format!("asset service: {e}"))?;
    item.asset_id = asset_id.to_string();
    item.flags &= !ATTACH_POINT_FLAG_MASK;
    if !ctx.inventory.update_item(&item).await.map_err(|e| format!("inventory service: {e}"))? {
        return Err("the inventory service refused the update".to_string());
    }
    notify_item(ctx, agent, &item).await;
    Ok(())
}

/// Prefix of the avatar-service keys that remember what is worn where (`_ap_<point>`).
const WORN_KEY_PREFIX: &str = "_ap_";

/// The avatar-service items for a set of worn attachments: one `_ap_<point>` per point whose
/// value lists the inventory item ids there (comma separated when several are worn).
pub fn worn_items(worn: &[(u8, Uuid)], item_of: impl Fn(&Uuid) -> Uuid) -> Vec<(String, String)> {
    let mut by_point: std::collections::BTreeMap<u8, Vec<Uuid>> = std::collections::BTreeMap::new();
    for (point, group) in worn {
        let item = item_of(group);
        if !item.is_nil() {
            by_point.entry(*point).or_default().push(item);
        }
    }
    by_point
        .into_iter()
        .map(|(point, items)| {
            let value = items.iter().map(Uuid::to_string).collect::<Vec<_>>().join(",");
            (format!("{WORN_KEY_PREFIX}{point}"), value)
        })
        .collect()
}

/// Remember what the avatar wears so the next login can wear it again.
async fn save_worn_list(ctx: &AttachCtx<'_>, agent: Uuid) {
    let worn = ctx.scene.attachments_of(&agent);
    let items = worn_items(&worn, |g| ctx.scene.get_object(g).map(|g| g.attachment_item_id).unwrap_or(Uuid::nil()));
    let stale: Vec<String> = match ctx.avatars.get_avatar(&agent.to_string()).await {
        Ok(existing) => existing
            .into_iter()
            .map(|(k, _)| k)
            .filter(|k| k.starts_with(WORN_KEY_PREFIX) && !items.iter().any(|(n, _)| n == k))
            .collect(),
        Err(e) => {
            tracing::warn!(agent = %agent, error = %e, "could not read the saved attachment list");
            return;
        }
    };
    if let Err(e) = ctx.avatars.remove_items(&agent.to_string(), &stale).await {
        tracing::warn!(agent = %agent, error = %e, "could not clear old saved attachments");
    }
    if let Err(e) = ctx.avatars.set_items(&agent.to_string(), &items).await {
        tracing::warn!(agent = %agent, error = %e, "could not save the attachment list");
    }
}

/// Parse the saved list back into (point, item id) pairs, in point order.
pub fn parse_worn_items(items: &[(String, String)]) -> Vec<(u8, Uuid)> {
    let mut out = Vec::new();
    for (key, value) in items {
        let Some(point) = key.strip_prefix(WORN_KEY_PREFIX).and_then(|p| p.parse::<u8>().ok()) else { continue };
        for id in value.split(',') {
            // Older data may carry "<item>:<asset>" or similar; the item id comes first.
            let id = id.split([':', ';']).next().unwrap_or("").trim();
            if let Ok(id) = Uuid::parse_str(id) {
                out.push((point, id));
            }
        }
    }
    out.sort();
    out
}

/// At login: wear again what the avatar wore last time. Returns how many were worn.
pub async fn restore_saved_attachments(ctx: &AttachCtx<'_>, agent: Uuid) -> usize {
    let items = match ctx.avatars.get_avatar(&agent.to_string()).await {
        Ok(items) => items,
        Err(e) => {
            tracing::warn!(agent = %agent, error = %e, "could not read the saved attachment list");
            return 0;
        }
    };
    let mut worn = 0;
    let mut seen_points: std::collections::HashSet<u8> = std::collections::HashSet::new();
    for (point, item_id) in parse_worn_items(&items) {
        // Several items on one point: after the first, add rather than replace.
        let raw = if seen_points.insert(point) { point } else { point | APPEND_FLAG };
        match rez_attachment_from_inventory_inner(ctx, agent, item_id, raw, false).await {
            Ok(_) => worn += 1,
            Err(e) => tracing::warn!(agent = %agent, item = %item_id, error = %e, "saved attachment not restored"),
        }
    }
    worn
}

struct AttachPlan {
    point: u8,
    offset: Vec3,
    replaced: Vec<Uuid>,
}

async fn plan_attach(
    ctx: &AttachCtx<'_>,
    agent: Uuid,
    group_id: Uuid,
    requested: u8,
    append: bool,
) -> Result<AttachPlan, String> {
    let group = ctx.scene.get_object(&group_id).ok_or("the object is not in the scene")?;
    let root = group.root_part();
    let (point, offset) = resolve_attach_point(
        requested,
        group.attachment_point,
        root.shape.last_attach_point,
        root.attached_pos,
        group.position(),
    )
    .ok_or_else(|| format!("invalid attachment point {requested}"))?;
    drop(group);

    let worn = ctx.scene.attachments_of(&agent);
    if worn.iter().any(|(_, id)| *id == group_id) {
        return Err("already attached".to_string());
    }
    let replaced: Vec<Uuid> = if append {
        Vec::new()
    } else {
        worn.iter().filter(|(p, _)| *p == point).map(|(_, id)| *id).collect()
    };
    if worn.len() - replaced.len() >= MAX_AGENT_ATTACHMENTS {
        return Err("too many attachments".to_string());
    }

    Ok(AttachPlan {
        point,
        offset,
        replaced,
    })
}

/// Attach an object that is already in the scene. Replaces what is worn at the same point
/// unless `append`. Returns the attachment point used.
pub async fn attach_scene_object(
    ctx: &AttachCtx<'_>,
    agent: Uuid,
    group_id: Uuid,
    requested: u8,
    append: bool,
    item_id: Uuid,
) -> Result<u8, String> {
    let plan = plan_attach(ctx, agent, group_id, requested, append).await?;
    let AttachPlan {
        point,
        offset,
        replaced,
    } = plan;

    for old in replaced {
        // Only attachments that came from inventory can be put back there.
        if ctx.scene.get_object(&old).is_some_and(|g| !g.attachment_item_id.is_nil()) {
            detach_to_inventory(ctx, agent, old).await?;
        }
    }

    ctx.scene.with_object_mut(&group_id, |g| g.root_part_mut().attached_pos = offset);
    ctx.scene.attach_object(&group_id, &agent, point, item_id).map_err(|e| e.to_string())?;
    if let Some(group) = ctx.scene.get_object(&group_id) {
        crate::broadcast_full_object_update(&group, ctx.scene.region_info.region_handle, ctx.socket, ctx.clients).await;
    }
    if !item_id.is_nil() {
        set_item_worn_point(ctx, agent, item_id, point).await;
    }
    Ok(point)
}

/// `RezSingleAttachmentFromInv`: wear an inventory object.
pub async fn rez_attachment_from_inventory(
    ctx: &AttachCtx<'_>,
    agent: Uuid,
    item_id: Uuid,
    raw_point: u8,
) -> Result<u8, String> {
    rez_attachment_from_inventory_inner(ctx, agent, item_id, raw_point, true).await
}

async fn rez_attachment_from_inventory_inner(
    ctx: &AttachCtx<'_>,
    agent: Uuid,
    item_id: Uuid,
    raw_point: u8,
    save_list: bool,
) -> Result<u8, String> {
    // Viewers ask again for what is already worn at login; ignore it.
    for (_, id) in ctx.scene.attachments_of(&agent) {
        if ctx.scene.get_object(&id).is_some_and(|g| g.attachment_item_id == item_id) {
            return Err("already worn".to_string());
        }
    }
    let (point, append) = split_point(raw_point);
    let position = ctx.scene.get_presence(&agent).map(|p| p.position).ok_or("the avatar is not in the region")?;
    let (item, group) = inventory_rez::rez_from_inventory(ctx.inventory, ctx.assets, agent, item_id, position).await?;
    let group_id = ctx.scene.add_object(group);
    let item_uuid = Uuid::parse_str(&item.inventory_id).unwrap_or(item_id);
    match attach_scene_object(ctx, agent, group_id, point, append, item_uuid).await {
        Ok(point) => {
            if save_list {
                save_worn_list(ctx, agent).await;
            }
            Ok(point)
        }
        Err(e) => {
            ctx.scene.remove_object(&group_id);
            Err(e)
        }
    }
}

/// `ObjectAttach`: attach an object lying in the world (a copy goes into inventory first, as
/// in OpenSim, so the attachment can be taken off again).
pub async fn attach_from_world(
    ctx: &AttachCtx<'_>,
    agent: Uuid,
    local_id: u32,
    raw_point: u8,
) -> Result<u8, String> {
    let group_id = ctx.scene.get_object_by_local_id(local_id).ok_or("unknown object")?;
    let group = ctx.scene.get_object(&group_id).ok_or("unknown object")?;
    let root = group.root_part();
    if !ctx.perms.can_take_object(agent, root.owner_id, root.group_id) {
        return Err("no permission to attach this object".to_string());
    }
    if group.is_attachment {
        return Err("already attached".to_string());
    }
    let (point, append) = split_point(raw_point);
    plan_attach(ctx, agent, group_id, point, append).await?;
    let item = inventory_rez::save_group_to_inventory(ctx.inventory, ctx.assets, DeRezAction::Take, agent, Uuid::nil(), &group)
        .await?;
    drop(group);
    let item_id = Uuid::parse_str(&item.inventory_id).map_err(|_| "bad item id".to_string())?;
    notify_item(ctx, agent, &item).await;
    let point = attach_scene_object(ctx, agent, group_id, point, append, item_id).await?;
    save_worn_list(ctx, agent).await;
    Ok(point)
}

/// Take an attachment off and put it back into its inventory item.
pub async fn detach_to_inventory(ctx: &AttachCtx<'_>, agent: Uuid, group_id: Uuid) -> Result<(), String> {
    let group = ctx.scene.get_object(&group_id).ok_or("unknown object")?;
    if !group.is_attachment || group.attached_avatar != agent {
        return Err("not one of this avatar's attachments".to_string());
    }
    let item_id = group.attachment_item_id;
    if item_id.is_nil() {
        return Err("the attachment has no inventory item".to_string());
    }
    // Save first; only then remove, so a service failure never loses the object.
    save_group_into_item(ctx, agent, item_id, &group).await?;
    let ids = local_ids(&group);
    drop(group);
    ctx.scene.remove_attachment(&group_id);
    kill(ctx, ids).await;
    save_worn_list(ctx, agent).await;
    Ok(())
}

/// `ObjectDrop`: put an attachment on the ground where the avatar stands.
pub async fn detach_to_ground(ctx: &AttachCtx<'_>, agent: Uuid, local_id: u32) -> Result<(), String> {
    let group_id = ctx.scene.get_object_by_local_id(local_id).ok_or("unknown object")?;
    let group = ctx.scene.get_object(&group_id).ok_or("unknown object")?;
    if !group.is_attachment || group.attached_avatar != agent {
        return Err("not one of this avatar's attachments".to_string());
    }
    let item_id = group.attachment_item_id;
    let ids = local_ids(&group);
    drop(group);
    let position = ctx.scene.get_presence(&agent).map(|p| p.position).ok_or("the avatar is not in the region")?;
    // The viewer re-creates the object as a world object.
    kill(ctx, ids).await;
    if !ctx.scene.detach_object(&group_id, position) {
        return Err("not attached".to_string());
    }
    if let Some(group) = ctx.scene.get_object(&group_id) {
        crate::broadcast_full_object_update(&group, ctx.scene.region_info.region_handle, ctx.socket, ctx.clients).await;
    }
    if !item_id.is_nil() {
        set_item_worn_point(ctx, agent, item_id, 0).await;
    }
    save_worn_list(ctx, agent).await;
    Ok(())
}

/// `DetachAttachmentIntoInv` / `ObjectDetach`: the attachment that came from `item_id`.
pub async fn detach_item_to_inventory(ctx: &AttachCtx<'_>, agent: Uuid, item_id: Uuid) -> Result<(), String> {
    let group_id = ctx
        .scene
        .attachments_of(&agent)
        .into_iter()
        .map(|(_, id)| id)
        .find(|id| ctx.scene.get_object(id).is_some_and(|g| g.attachment_item_id == item_id))
        .ok_or("that item is not worn")?;
    detach_to_inventory(ctx, agent, group_id).await
}

/// An avatar left: save each attachment into its inventory item and remove it from the scene
/// and from every viewer.
pub async fn remove_all_for_avatar(ctx: &AttachCtx<'_>, agent: Uuid) {
    for group in ctx.scene.take_attachments_of(&agent) {
        let item_id = group.attachment_item_id;
        if !item_id.is_nil()
            && let Err(e) = save_group_into_item(ctx, agent, item_id, &group).await
        {
            tracing::warn!(agent = %agent, item = %item_id, error = %e, "could not save an attachment on logout");
        }
        kill(ctx, local_ids(&group)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wear_picks_the_remembered_point_or_the_left_hand() {
        // "Wear" (0) with nothing remembered: left hand, no offset.
        assert_eq!(resolve_attach_point(0, 0, 0, Vec3::ONE, Vec3::new(9.0, 9.0, 9.0)), Some((5, Vec3::ZERO)));
        // "Wear" with a remembered point: that point and its offset.
        assert_eq!(resolve_attach_point(0, 0, 2, Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO), Some((2, Vec3::new(1.0, 2.0, 3.0))));
        // Explicit point equal to the remembered one keeps the saved offset...
        assert_eq!(resolve_attach_point(2, 0, 2, Vec3::new(1.0, 2.0, 3.0), Vec3::new(9.0, 9.0, 9.0)), Some((2, Vec3::new(1.0, 2.0, 3.0))));
        // ...a different point starts at the point itself.
        assert_eq!(resolve_attach_point(6, 0, 2, Vec3::new(1.0, 2.0, 3.0), Vec3::new(9.0, 9.0, 9.0)), Some((6, Vec3::ZERO)));
        // Out of range is refused.
        assert_eq!(resolve_attach_point(56, 0, 0, Vec3::ZERO, Vec3::ZERO), None);
        assert_eq!(resolve_attach_point(55, 0, 0, Vec3::ZERO, Vec3::ZERO), Some((55, Vec3::ZERO)));
    }

    #[test]
    fn the_append_bit_is_split_off_the_point() {
        assert_eq!(split_point(2), (2, false));
        assert_eq!(split_point(0x82), (2, true));
        assert_eq!(split_point(0x80), (0, true));
    }

    // ---- end to end: real services, a connected viewer socket ----

    use crate::test_support::{add_object_item, spawn_services};
    use welkin_msg::Msg;
    use welkin_msg::msgs::{KillObject, ObjectUpdate, UpdateCreateInventoryItem};
    use welkin_scene::presence::{PresenceType, ScenePresence};
    use welkin_scene::scene::{RegionInfo, SceneConfig};

    struct Harness {
        scene: Scene,
        agent: Uuid,
        avatar_local: u32,
        viewer: UdpSocket,
        server: UdpSocket,
        clients: ClientMap,
        inventory: RemoteInventoryConnector,
        assets: RemoteAssetConnector,
        avatars: RemoteAvatarConnector,
        perms: DefaultPermissionsModule,
        svc: crate::test_support::Services,
    }

    async fn harness(tag: &str) -> Harness {
        let svc = spawn_services(tag).await;
        let scene = Scene::new(RegionInfo::new("T", 1000, 1000), SceneConfig::default());
        let agent = Uuid::new_v4();
        let mut presence = ScenePresence::new(agent, "Test", "Avatar", PresenceType::User);
        presence.position = Vec3::new(50.0, 60.0, 25.0);
        scene.add_presence(presence);
        let avatar_local = scene.get_presence(&agent).unwrap().local_id;
        let viewer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = viewer.local_addr().unwrap();
        let session = ClientSession::new(addr, 1, agent, Uuid::new_v4());
        let clients: ClientMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
        clients.write().await.insert(addr, Arc::new(Mutex::new(session)));
        let inventory = RemoteInventoryConnector::new(&svc.base);
        let assets = RemoteAssetConnector::new(&svc.base);
        let avatars = RemoteAvatarConnector::new(&svc.base);
        Harness { scene, agent, avatar_local, viewer, server, clients, inventory, assets, avatars, perms: DefaultPermissionsModule::new(), svc }
    }

    impl Harness {
        fn ctx(&self) -> AttachCtx<'_> {
            AttachCtx {
                scene: &self.scene,
                perms: &self.perms,
                inventory: &self.inventory,
                assets: &self.assets,
                avatars: &self.avatars,
                socket: &self.server,
                clients: &self.clients,
            }
        }

        /// Every datagram the viewer got, as (message id, body).
        async fn received(&self) -> Vec<welkin_msg::IncomingPacket> {
            let mut out = Vec::new();
            let mut buf = vec![0u8; 2048];
            while let Ok(Ok((n, _))) =
                tokio::time::timeout(std::time::Duration::from_millis(200), self.viewer.recv_from(&mut buf)).await
            {
                out.push(welkin_msg::parse_packet(&buf[..n]).expect("parse"));
            }
            out
        }

        async fn item(&self, id: &str) -> InventoryItemRow {
            self.inventory.get_item(id).await.unwrap().expect("item")
        }
    }

    use welkin_data::inventory::InventoryItemRow;

    #[tokio::test]
    async fn wearing_an_inventory_object_attaches_it_and_taking_it_off_saves_it_back() {
        let h = harness("wear").await;
        let item = add_object_item(&h.svc, h.agent, "Hat").await;
        let item_id = Uuid::parse_str(&item.inventory_id).unwrap();

        // Wear at the skull (point 2).
        let point = rez_attachment_from_inventory(&h.ctx(), h.agent, item_id, 2).await.expect("wear");
        assert_eq!(point, 2);
        let worn = h.scene.attachments_of(&h.agent);
        assert_eq!(worn.len(), 1);
        let group = h.scene.get_object(&worn[0].1).unwrap();
        assert_eq!(group.attachment_item_id, item_id);
        assert_eq!(group.root_part().parent_id, h.avatar_local);
        assert_eq!(group.part_count(), 2);

        // The viewer was told: an update parented to the avatar with the swapped point and the item id.
        let msgs = h.received().await;
        let updates: Vec<_> = msgs
            .iter()
            .filter(|p| p.id == ObjectUpdate::ID)
            .flat_map(|p| ObjectUpdate::decode(&p.body).unwrap().object_data)
            .collect();
        let root = updates.iter().find(|o| o.parent_id == h.avatar_local).expect("root update");
        assert_eq!(root.state, 0x20);
        assert!(String::from_utf8_lossy(&root.name_value).contains(&item_id.to_string()));
        assert!(msgs.iter().any(|p| p.id == UpdateCreateInventoryItem::ID), "inventory item update sent");
        // The item records where it is worn.
        assert_eq!(h.item(&item.inventory_id).await.flags & 0xff, 2);

        // Asking again for something already worn does nothing.
        assert!(rez_attachment_from_inventory(&h.ctx(), h.agent, item_id, 2).await.is_err());
        assert_eq!(h.scene.attachments_of(&h.agent).len(), 1);

        // Take it off into inventory: gone from the scene, killed on the viewer, asset replaced.
        let old_asset = h.item(&item.inventory_id).await.asset_id;
        detach_item_to_inventory(&h.ctx(), h.agent, item_id).await.expect("detach");
        assert!(h.scene.attachments_of(&h.agent).is_empty());
        assert!(h.scene.get_object(&worn[0].1).is_none());
        let msgs = h.received().await;
        assert!(msgs.iter().any(|p| p.id == KillObject::ID), "kill sent");
        let after = h.item(&item.inventory_id).await;
        assert_eq!(after.flags & 0xff, 0, "no longer worn");
        assert_ne!(after.asset_id, old_asset, "the saved state is a new asset");
    }

    #[tokio::test]
    async fn wearing_a_second_item_at_the_same_point_replaces_the_first_unless_appending() {
        let h = harness("replace").await;
        let a = add_object_item(&h.svc, h.agent, "A").await;
        let b = add_object_item(&h.svc, h.agent, "B").await;
        let c = add_object_item(&h.svc, h.agent, "C").await;
        let id = |i: &InventoryItemRow| Uuid::parse_str(&i.inventory_id).unwrap();

        rez_attachment_from_inventory(&h.ctx(), h.agent, id(&a), 2).await.unwrap();
        rez_attachment_from_inventory(&h.ctx(), h.agent, id(&b), 2).await.unwrap();
        let worn: Vec<_> = h
            .scene
            .attachments_of(&h.agent)
            .iter()
            .map(|(_, g)| h.scene.get_object(g).unwrap().attachment_item_id)
            .collect();
        assert_eq!(worn, vec![id(&b)], "B replaced A");
        assert_eq!(h.item(&a.inventory_id).await.flags & 0xff, 0, "A is back in inventory");

        rez_attachment_from_inventory(&h.ctx(), h.agent, id(&c), 2 | 0x80).await.unwrap();
        assert_eq!(h.scene.attachments_of(&h.agent).len(), 2, "append keeps B");
    }

    #[tokio::test]
    async fn dropping_an_attachment_puts_it_in_the_world_and_logout_saves_and_removes_the_rest() {
        let h = harness("drop").await;
        let a = add_object_item(&h.svc, h.agent, "A").await;
        let b = add_object_item(&h.svc, h.agent, "B").await;
        let id = |i: &InventoryItemRow| Uuid::parse_str(&i.inventory_id).unwrap();
        rez_attachment_from_inventory(&h.ctx(), h.agent, id(&a), 2).await.unwrap();
        rez_attachment_from_inventory(&h.ctx(), h.agent, id(&b), 6).await.unwrap();
        let (_, a_group) = h.scene.attachments_of(&h.agent).into_iter().find(|(p, _)| *p == 2).unwrap();
        let a_local = h.scene.get_object(&a_group).unwrap().root_part().local_id;
        h.received().await;

        detach_to_ground(&h.ctx(), h.agent, a_local).await.expect("drop");
        let dropped = h.scene.get_object(&a_group).expect("still in the scene");
        assert!(!dropped.is_attachment);
        assert_eq!(dropped.position(), Vec3::new(50.0, 60.0, 25.0));
        assert_eq!(h.item(&a.inventory_id).await.flags & 0xff, 0);
        assert_eq!(h.scene.attachments_of(&h.agent).len(), 1);

        // Strangers cannot drop someone else's attachment.
        let (_, b_group) = h.scene.attachments_of(&h.agent)[0];
        let b_local = h.scene.get_object(&b_group).unwrap().root_part().local_id;
        assert!(detach_to_ground(&h.ctx(), Uuid::new_v4(), b_local).await.is_err());

        // Logout: B is saved into its item and leaves the scene.
        let old_asset = h.item(&b.inventory_id).await.asset_id;
        remove_all_for_avatar(&h.ctx(), h.agent).await;
        assert!(h.scene.attachments_of(&h.agent).is_empty());
        assert!(h.scene.get_object(&b_group).is_none());
        assert!(h.scene.get_object(&a_group).is_some(), "the dropped object stays in the world");
        assert_ne!(h.item(&b.inventory_id).await.asset_id, old_asset);
        assert!(h.received().await.iter().any(|p| p.id == KillObject::ID));
    }

    #[tokio::test]
    async fn an_in_world_object_can_be_attached_after_a_copy_goes_to_inventory() {
        let h = harness("world").await;
        // A trash folder is not needed for Take; the Objects folder is looked up.
        sqlx::query("INSERT INTO inventoryfolders (folderName,type,version,folderID,agentID,parentFolderID) VALUES ('Objects',6,1,$1,$2,$3)")
            .bind(Uuid::new_v4().to_string())
            .bind(h.agent.to_string())
            .bind(Uuid::nil().to_string())
            .execute(h.svc.pool.inner())
            .await
            .unwrap();
        let root = welkin_scene::part::SceneObjectPart::new("Sword", h.agent);
        let group = h.scene.add_object(welkin_scene::group::SceneObjectGroup::new(root));
        let local = h.scene.get_object(&group).unwrap().root_part().local_id;

        let point = attach_from_world(&h.ctx(), h.agent, local, 5).await.expect("attach");
        assert_eq!(point, 5);
        let g = h.scene.get_object(&group).unwrap();
        assert!(g.is_attachment && !g.attachment_item_id.is_nil());
        let item = h.item(&g.attachment_item_id.to_string()).await;
        assert_eq!(item.inventory_name, "Sword");
        assert_eq!(item.flags & 0xff, 5);

        // Someone who may not take it cannot attach it.
        let stranger = Uuid::new_v4();
        let other = h.scene.add_object(welkin_scene::group::SceneObjectGroup::new(
            welkin_scene::part::SceneObjectPart::new("Not yours", h.agent),
        ));
        let other_local = h.scene.get_object(&other).unwrap().root_part().local_id;
        let mut perms = DefaultPermissionsModule::new();
        perms.config.bypass_permissions = false;
        let ctx = AttachCtx { perms: &perms, ..h.ctx() };
        assert!(attach_from_world(&ctx, stranger, other_local, 5).await.unwrap_err().contains("permission"));
        let _ = &h.perms;
    }

#[test]
fn worn_items_groups_items_by_point() {
    let g_a = Uuid::new_v4();
    let g_b = Uuid::new_v4();
    let g_c = Uuid::new_v4();
    let item_a = Uuid::new_v4();
    let item_b = Uuid::new_v4();
    let item_c = Uuid::new_v4();

    let item_for = |g: &Uuid| -> Uuid {
        if *g == g_a {
            item_a
        } else if *g == g_b {
            item_b
        } else if *g == g_c {
            item_c
        } else {
            Uuid::nil()
        }
    };

    let out = worn_items(&[(2, g_a), (2, g_b), (6, g_c)], |g| item_for(g));
    assert_eq!(
        out,
        vec![
            ("_ap_2".to_string(), format!("{},{}", item_a, item_b)),
            ("_ap_6".to_string(), item_c.to_string()),
        ]
    );

    // A group whose item id is nil is skipped.
    let g_nil = Uuid::new_v4();
    let out_nil = worn_items(&[(2, g_nil)], |g| {
        if *g == g_nil {
            Uuid::nil()
        } else {
            Uuid::nil()
        }
    });
    assert!(out_nil.is_empty());

    // Empty input gives an empty vec.
    let out_empty = worn_items(&[], |g| item_for(g));
    assert!(out_empty.is_empty());
}

#[test]
fn parse_worn_items_reads_ids_and_ignores_junk() {
    let u = Uuid::new_v4();
    let u2 = Uuid::new_v4();
    let asset = Uuid::new_v4();

    let items = vec![
        ("_ap_2".to_string(), format!("{},{}", u, u2)),
        ("_ap_6".to_string(), format!("{}:{}", u, asset)),
        ("Height".to_string(), "1.7".to_string()),
        ("_ap_x".to_string(), u.to_string()),
        ("_ap_5".to_string(), "not-a-uuid".to_string()),
    ];

    let parsed = parse_worn_items(&items);
    let mut expected = vec![(2u8, u), (2u8, u2), (6u8, u)];
    expected.sort();
    assert_eq!(parsed, expected);
}

#[tokio::test]
async fn the_avatar_service_remembers_what_is_worn_and_forgets_what_is_taken_off() {
    let h = harness("avatar_service_remembers").await;
    let item_a = add_object_item(&h.svc, h.agent, "A").await;
    let item_b = add_object_item(&h.svc, h.agent, "B").await;
    let id_a = Uuid::parse_str(&item_a.inventory_id).unwrap();
    let id_b = Uuid::parse_str(&item_b.inventory_id).unwrap();

    rez_attachment_from_inventory(&h.ctx(), h.agent, id_a, 2)
        .await
        .expect("wear a");
    rez_attachment_from_inventory(&h.ctx(), h.agent, id_b, 6)
        .await
        .expect("wear b");

    let avatar = h
        .avatars
        .get_avatar(&h.agent.to_string())
        .await
        .expect("get avatar");
    assert!(avatar
        .iter()
        .any(|(k, v)| k == "_ap_2" && *v == id_a.to_string()));
    assert!(avatar
        .iter()
        .any(|(k, v)| k == "_ap_6" && *v == id_b.to_string()));

    detach_item_to_inventory(&h.ctx(), h.agent, id_a)
        .await
        .expect("detach a");

    let avatar = h
        .avatars
        .get_avatar(&h.agent.to_string())
        .await
        .expect("get avatar");
    assert!(!avatar.iter().any(|(k, _)| k == "_ap_2"));
    assert!(avatar
        .iter()
        .any(|(k, v)| k == "_ap_6" && *v == id_b.to_string()));
}

#[tokio::test]
async fn wearing_again_at_login_restores_the_saved_attachments() {
    let h = harness("restore_saved").await;
    let item_a = add_object_item(&h.svc, h.agent, "A").await;
    let item_b = add_object_item(&h.svc, h.agent, "B").await;
    let id_a = Uuid::parse_str(&item_a.inventory_id).unwrap();
    let id_b = Uuid::parse_str(&item_b.inventory_id).unwrap();

    rez_attachment_from_inventory(&h.ctx(), h.agent, id_a, 2)
        .await
        .expect("wear a");
    rez_attachment_from_inventory(&h.ctx(), h.agent, id_b, 6)
        .await
        .expect("wear b");

    remove_all_for_avatar(&h.ctx(), h.agent).await;

    assert!(h.scene.attachments_of(&h.agent).is_empty());
    let avatar = h
        .avatars
        .get_avatar(&h.agent.to_string())
        .await
        .expect("get avatar");
    assert!(avatar
        .iter()
        .any(|(k, v)| k == "_ap_2" && *v == id_a.to_string()));
    assert!(avatar
        .iter()
        .any(|(k, v)| k == "_ap_6" && *v == id_b.to_string()));

    let restored = restore_saved_attachments(&h.ctx(), h.agent).await;
    assert_eq!(restored, 2);

    let worn = h.scene.attachments_of(&h.agent);
    let points: Vec<u8> = worn.iter().map(|w| w.0).collect();
    assert_eq!(points, vec![2, 6]);
    for (point, group_id) in &worn {
        let group = h.scene.get_object(group_id).unwrap();
        let expected = if *point == 2 { id_a } else { id_b };
        assert_eq!(group.attachment_item_id, expected);
    }

    let restored_again = restore_saved_attachments(&h.ctx(), h.agent).await;
    assert_eq!(restored_again, 0);
}

#[tokio::test]
async fn a_failed_world_attach_leaves_inventory_untouched() {
    let h = harness("failed-world-attach").await;
    sqlx::query("INSERT INTO inventoryfolders (folderName,type,version,folderID,agentID,parentFolderID) VALUES ('Objects',6,1,$1,$2,$3)")
        .bind(Uuid::new_v4().to_string())
        .bind(h.agent.to_string())
        .bind(Uuid::nil().to_string())
        .execute(h.svc.pool.inner())
        .await
        .unwrap();

    let group = h.scene.add_object(welkin_scene::group::SceneObjectGroup::new(welkin_scene::part::SceneObjectPart::new("Sword", h.agent)));
    let local = h.scene.get_object(&group).unwrap().root_part().local_id;

    let err = attach_from_world(&h.ctx(), h.agent, local, 56).await.unwrap_err();
    assert!(err.contains("invalid attachment point"));

    let count: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM inventoryitems")
        .fetch_one(h.svc.pool.inner())
        .await
        .unwrap();
    assert_eq!(count, 0);

    assert!(!h.scene.get_object(&group).unwrap().is_attachment);
}
}
