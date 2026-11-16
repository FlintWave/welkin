// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex as ParkingMutex;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use welkin_msg::Msg;
use welkin_msg::msgs;

use crate::groups_io as gio;

#[derive(Clone)]
pub struct GroupChat {
    pub roster: Arc<ParkingMutex<HashMap<(Uuid, Uuid), Instant>>>,
    pub ttl: Duration,
}

impl GroupChat {
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(30))
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            roster: Arc::new(ParkingMutex::new(HashMap::new())),
            ttl,
        }
    }

    pub fn mark(&self, group: Uuid, agent: Uuid) {
        self.roster.lock().insert((group, agent), Instant::now());
    }

    pub fn remove(&self, group: Uuid, agent: Uuid) {
        self.roster.lock().remove(&(group, agent));
    }

    pub fn is_fresh(&self, group: Uuid, agent: Uuid) -> bool {
        let guard = self.roster.lock();
        match guard.get(&(group, agent)) {
            Some(instant) => instant.elapsed() < self.ttl,
            None => false,
        }
    }

    pub fn fresh_members(&self, group: Uuid) -> Vec<Uuid> {
        let guard = self.roster.lock();
        guard
            .iter()
            .filter_map(|((g, agent), instant)| {
                if *g == group && instant.elapsed() < self.ttl {
                    Some(*agent)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum ChatOut {
    Im {
        to: Uuid,
        im: msgs::ImprovedInstantMessage,
    },
    Alert(msgs::AlertMessage),
    NoticesList(msgs::GroupNoticesListReply),
    Event { to: Uuid, name: &'static str, body: welkin_llsd::Llsd },
}

pub mod dialog {
    pub const SESSION_ADD: u8 = 13;
    #[allow(dead_code)] // protocol value, pinned by test; not handled
    pub const SESSION_OFFLINE_ADD: u8 = 14;
    pub const SESSION_GROUP_START: u8 = 15;
    pub const SESSION_SEND: u8 = 17;
    pub const SESSION_DROP: u8 = 18;
    pub const GROUP_NOTICE: u8 = 32;
    pub const GROUP_NOTICE_REQUESTED: u8 = 37;
}

pub fn is_chat_dialog(dialog: u8, _from_group: bool) -> bool {
    matches!(
        dialog,
        dialog::SESSION_ADD
            | dialog::SESSION_GROUP_START
            | dialog::SESSION_SEND
            | dialog::SESSION_DROP
            | dialog::GROUP_NOTICE
            | dialog::GROUP_NOTICE_REQUESTED
    )
}

pub fn is_notices_list_packet(id: welkin_msg::MsgId) -> bool {
    id == msgs::GroupNoticesListRequest::ID
}

async fn is_member(ctx: &gio::GroupsCtx, agent: Uuid, group: Uuid) -> bool {
    match ctx.connector().get_membership(agent, Some(group)).await {
        Ok(memberships) => !memberships.is_empty(),
        Err(err) => {
            tracing::warn!("get_membership failed: {:?}", err);
            false
        }
    }
}

fn now_u32() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

fn make_notice_im(
    group: Uuid,
    notice_id: Uuid,
    recipient: Uuid,
    from_name: &str,
    subject: &str,
    body: &str,
    timestamp: u32,
) -> msgs::ImprovedInstantMessage {
    let mut bucket = vec![0u8; 19];
    bucket[2..18].copy_from_slice(group.as_bytes());

    msgs::ImprovedInstantMessage {
        agent_data: msgs::improved_instant_message::AgentData {
            agent_id: group,
            session_id: Uuid::nil(),
        },
        message_block: msgs::improved_instant_message::MessageBlock {
            from_group: true,
            to_agent_id: recipient,
            parent_estate_id: 0,
            region_id: Uuid::nil(),
            position: glam::Vec3::ZERO,
            offline: 1,
            dialog: dialog::GROUP_NOTICE,
            id: notice_id,
            timestamp,
            from_agent_name: gio::name_bytes(from_name),
            message: gio::name_bytes(&format!("{}|{}", subject, body)),
            binary_bucket: bucket,
        },
        estate_block: msgs::improved_instant_message::EstateBlock::default(),
        meta_data: Vec::new(),
    }
}

pub fn agent_list_update_event(group: Uuid, updates: &[(Uuid, bool)]) -> welkin_llsd::Llsd {
    let mut agent_updates = welkin_llsd::Map::new();
    for (uuid, enter) in updates {
        let transition = if *enter { "ENTER" } else { "LEAVE" };
        let mut info = welkin_llsd::Map::new();
        info.insert("can_voice_chat".to_string(), welkin_llsd::Llsd::Boolean(false));
        info.insert("is_moderator".to_string(), welkin_llsd::Llsd::Boolean(false));
        let mut mutes = welkin_llsd::Map::new();
        mutes.insert("text".to_string(), welkin_llsd::Llsd::Boolean(false));
        info.insert("mutes".to_string(), welkin_llsd::Llsd::Map(mutes));
        let mut entry = welkin_llsd::Map::new();
        entry.insert("info".to_string(), welkin_llsd::Llsd::Map(info));
        entry.insert("transition".to_string(), welkin_llsd::Llsd::String(transition.to_string()));
        agent_updates.insert(uuid.to_string(), welkin_llsd::Llsd::Map(entry));
    }
    let mut top = welkin_llsd::Map::new();
    top.insert("agent_updates".to_string(), welkin_llsd::Llsd::Map(agent_updates));
    top.insert("updates".to_string(), welkin_llsd::Llsd::Map(welkin_llsd::Map::new()));
    top.insert("session_id".to_string(), welkin_llsd::Llsd::Uuid(group));
    welkin_llsd::Llsd::Map(top)
}

pub fn session_start_reply_event(group: Uuid, group_name: &str) -> welkin_llsd::Llsd {
    let mut moderated_mode = welkin_llsd::Map::new();
    moderated_mode.insert("voice".into(), welkin_llsd::Llsd::Boolean(false));

    let mut session_info = welkin_llsd::Map::new();
    session_info.insert(
        "moderated_mode".into(),
        welkin_llsd::Llsd::Map(moderated_mode),
    );
    session_info.insert(
        "session_name".into(),
        welkin_llsd::Llsd::String(group_name.to_string()),
    );
    session_info.insert("type".into(), welkin_llsd::Llsd::Integer(0));
    session_info.insert("voice_enabled".into(), welkin_llsd::Llsd::Boolean(false));

    let mut map = welkin_llsd::Map::new();
    map.insert("session_id".into(), welkin_llsd::Llsd::Uuid(group));
    map.insert("temp_session_id".into(), welkin_llsd::Llsd::Uuid(group));
    map.insert("success".into(), welkin_llsd::Llsd::Boolean(true));
    map.insert("session_info".into(), welkin_llsd::Llsd::Map(session_info));

    welkin_llsd::Llsd::Map(map)
}

pub async fn process_chat_im(
    ctx: &gio::GroupsCtx,
    chat: &GroupChat,
    agent: Uuid,
    first: &str,
    last: &str,
    region_id: Uuid,
    im: &msgs::ImprovedInstantMessage,
    eq: Option<&welkin_caps::event_queue::EventQueue>,
) -> Vec<ChatOut> {
    let mut out = Vec::new();
    let dialog = im.message_block.dialog;

    match (dialog, im.message_block.from_group) {
        (dialog::SESSION_GROUP_START, _) => {
            let group = im.message_block.id;
            if is_member(ctx, agent, group).await {
                chat.mark(group, agent);
                let group_name = ctx
                    .connector()
                    .get_group(agent, Some(group), None)
                    .await
                    .ok()
                    .flatten()
                    .map(|g| g.group_name.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                if let Some(eq) = eq {
                    eq.enqueue(
                        "ChatterBoxSessionStartReply",
                        session_start_reply_event(group, &group_name),
                    )
                    .await;
                }
                out.extend(roster_events(chat, group, agent));
            } else {
                out.push(ChatOut::Alert(gio::alert_message(
                    "You are not a member of that group.",
                )));
            }
        }
        (dialog::SESSION_SEND, _) => {
            let group = im.message_block.id;
            let allowed = if chat.is_fresh(group, agent) {
                true
            } else {
                let ok = is_member(ctx, agent, group).await;
                if ok {
                    chat.mark(group, agent);
                }
                ok
            };

            if !allowed {
                out.push(ChatOut::Alert(gio::alert_message(
                    "You are not a member of that group.",
                )));
            } else {
                let now = now_u32();
                let sender_name = gio::name_bytes(&format!("{} {}", first, last));
                let fresh = chat.fresh_members(group);
                for other in fresh {
                    if other == agent {
                        continue;
                    }
                    let mut msg = im.clone();
                    msg.agent_data.agent_id = agent;
                    msg.agent_data.session_id = Uuid::nil();
                    msg.message_block.to_agent_id = group;
                    msg.message_block.region_id = region_id;
                    msg.message_block.offline = 0;
                    msg.message_block.timestamp = now;
                    msg.message_block.from_agent_name = sender_name.clone();
                    msg.message_block.dialog = dialog::SESSION_SEND;
                    msg.message_block.from_group = true;
                    out.push(ChatOut::Im { to: other, im: msg });
                }
            }
        }
        (dialog::SESSION_DROP, _) => {
            let group = im.message_block.id;
            if chat.is_fresh(group, agent) {
                let others: Vec<Uuid> = chat
                    .fresh_members(group)
                    .into_iter()
                    .filter(|a| *a != agent)
                    .collect();
                chat.remove(group, agent);
                for other in others {
                    out.push(ChatOut::Event {
                        to: other,
                        name: "ChatterBoxSessionAgentListUpdates",
                        body: agent_list_update_event(group, &[(agent, false)]),
                    });
                }
            }
        }
        (dialog::SESSION_ADD, _) => {
            let group = im.message_block.id;
            if is_member(ctx, agent, group).await {
                chat.mark(group, agent);
                out.extend(roster_events(chat, group, agent));
            }
        }
        (32, _) => {
            let group = im.message_block.to_agent_id;

            if !is_member(ctx, agent, group).await {
                out.push(ChatOut::Alert(gio::alert_message(
                    "Group membership not found",
                )));
                return out;
            }

            let powers = match ctx.connector().get_membership(agent, Some(group)).await {
                Ok(memberships) => memberships
                    .first()
                    .map(|m| m.group_powers)
                    .unwrap_or(0),
                Err(err) => {
                    tracing::warn!("get_membership failed: {:?}", err);
                    0
                }
            };
            if powers & (1u64 << 42) == 0 {
                out.push(ChatOut::Alert(gio::alert_message(
                    "No permission to send notice to group",
                )));
                return out;
            }

            let text = gio::bytes_to_string(&im.message_block.message);
            let split_at = match text.find('|') {
                Some(pos) => pos,
                None => return out,
            };
            let subject = &text[..split_at];
            let body = &text[split_at + 1..];

            if im.message_block.binary_bucket.first().copied().unwrap_or(0) > 0 {
                out.push(ChatOut::Alert(gio::alert_message(
                    "Group notice attachments are not supported yet.",
                )));
                return out;
            }

            let notice_id = Uuid::new_v4();
            let sender_name = format!("{} {}", first, last);

            match ctx
                .connector()
                .add_notice(
                    agent,
                    group,
                    notice_id,
                    &sender_name,
                    subject,
                    body,
                    false,
                    0,
                    "",
                    Uuid::nil(),
                    Uuid::nil(),
                )
                .await
            {
                Ok(true) => {
                    let timestamp = now_u32();
                    match ctx.connector().get_group_members(agent, group).await {
                        Ok(members) => {
                            for member in members {
                                if !member.accept_notices {
                                    continue;
                                }
                                out.push(ChatOut::Im {
                                    to: member.agent_id,
                                    im: make_notice_im(
                                        group,
                                        notice_id,
                                        member.agent_id,
                                        &sender_name,
                                        subject,
                                        body,
                                        timestamp,
                                    ),
                                });
                            }
                        }
                        Err(err) => {
                            tracing::warn!("get_group_members failed: {:?}", err);
                            out.push(ChatOut::Alert(gio::alert_message(
                                "Unable to send the group notice.",
                            )));
                        }
                    }
                }
                Ok(false) | Err(_) => {
                    out.push(ChatOut::Alert(gio::alert_message(
                        "Unable to send the group notice.",
                    )));
                }
            }
        }
        (37, _) => {
            let notice_id = im.message_block.id;
            if let Ok(Some(notice)) = ctx.connector().get_notice(agent, notice_id).await {
                if is_member(ctx, agent, notice.group_id).await {
                    let timestamp = notice.timestamp as u32;
                    out.push(ChatOut::Im {
                        to: agent,
                        im: make_notice_im(
                            notice.group_id,
                            notice.notice_id,
                            agent,
                            &notice.from_name,
                            &notice.subject,
                            &notice.message,
                            timestamp,
                        ),
                    });
                }
            }
        }
        _ => {}
    }

    out
}

pub async fn process_notices_list(
    ctx: &gio::GroupsCtx,
    agent: Uuid,
    msg_id: welkin_msg::MsgId,
    body: &[u8],
) -> Option<Vec<ChatOut>> {
    if msg_id != msgs::GroupNoticesListRequest::ID {
        return None;
    }

    let group = msgs::GroupNoticesListRequest::decode(body).ok()?.data.group_id;

    if !is_member(ctx, agent, group).await {
        return Some(Vec::new());
    }

    let notices = match ctx.connector().get_notices(agent, group).await {
        Ok(notices) => notices,
        Err(err) => {
            tracing::warn!("get_notices failed: {:?}", err);
            Vec::new()
        }
    };

    let reply = msgs::GroupNoticesListReply {
        agent_data: msgs::group_notices_list_reply::AgentData {
            agent_id: agent,
            group_id: group,
        },
        data: notices
            .into_iter()
            .map(|notice| msgs::group_notices_list_reply::Data {
                notice_id: notice.notice_id,
                timestamp: notice.timestamp as u32,
                from_name: gio::name_bytes(&notice.from_name),
                subject: gio::name_bytes(&notice.subject),
                has_attachment: notice.has_attachment,
                asset_type: notice.attachment_type,
            })
            .collect(),
    };

    Some(vec![ChatOut::NoticesList(reply)])
}

pub fn spawn_chat_im(
    ctx: gio::GroupsCtx,
    chat: GroupChat,
    agent: Uuid,
    first: String,
    last: String,
    region_id: Uuid,
    im: msgs::ImprovedInstantMessage,
    event_queues: Arc<RwLock<HashMap<Uuid, Arc<welkin_caps::event_queue::EventQueue>>>>,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<welkin_net::server::ClientSession>>>>>,
    client: Arc<Mutex<welkin_net::server::ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    let queue = ctx.queue().clone();
    let _ = queue.submit(agent, async move {
        let eq = { let guard = event_queues.read().await; guard.get(&agent).cloned() };
        let outputs = process_chat_im(&ctx, &chat, agent, &first, &last, region_id, &im, eq.as_deref()).await;
        deliver(outputs, agent, clients, event_queues.clone(), client, socket, addr).await;
    });
}

pub fn spawn_notices_list(
    ctx: gio::GroupsCtx,
    agent: Uuid,
    msg_id: welkin_msg::MsgId,
    body: Vec<u8>,
    client: Arc<Mutex<welkin_net::server::ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    tokio::spawn(async move {
        if let Some(outputs) = process_notices_list(&ctx, agent, msg_id, &body).await {
            let clients = Arc::new(RwLock::new(HashMap::new()));
            deliver(outputs, agent, clients, Arc::new(RwLock::new(HashMap::new())), client, socket, addr).await;
        }
    });
}

fn roster_events(chat: &GroupChat, group: Uuid, agent: Uuid) -> Vec<ChatOut> {
    let members = chat.fresh_members(group);
    let all: Vec<(Uuid, bool)> = members.iter().map(|u| (*u, true)).collect();
    let mut outs = Vec::new();
    outs.push(ChatOut::Event {
        to: agent,
        name: "ChatterBoxSessionAgentListUpdates",
        body: agent_list_update_event(group, &all),
    });
    for &other in &members {
        if other != agent {
            outs.push(ChatOut::Event {
                to: other,
                name: "ChatterBoxSessionAgentListUpdates",
                body: agent_list_update_event(group, &[(agent, true)]),
            });
        }
    }
    outs
}

async fn deliver(
    outs: Vec<ChatOut>,
    agent: Uuid,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<welkin_net::server::ClientSession>>>>>,
    event_queues: Arc<RwLock<HashMap<Uuid, Arc<welkin_caps::event_queue::EventQueue>>>>,
    client: Arc<Mutex<welkin_net::server::ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    for out in outs {
        match out {
            ChatOut::Event { to, name, body } => {
                let eq = { let guard = event_queues.read().await; guard.get(&to).cloned() };
                if let Some(eq) = eq {
                    eq.enqueue(name, body).await;
                }
            }
            ChatOut::Alert(message) => {
                let bytes = {
                    let mut guard = client.lock().await;
                    let (data, _) = guard.circuit.send_msg(&message, true);
                    data
                };
                let _ = socket.send_to(&bytes, addr).await;
            }
            ChatOut::NoticesList(message) => {
                let bytes = {
                    let mut guard = client.lock().await;
                    let (data, _) = guard.circuit.send_msg(&message, true);
                    data
                };
                let _ = socket.send_to(&bytes, addr).await;
            }
            ChatOut::Im { to, im } => {
                if to == agent {
                    let bytes = {
                        let mut guard = client.lock().await;
                        let (data, _) = guard.circuit.send_msg(&im, true);
                        data
                    };
                    let _ = socket.send_to(&bytes, addr).await;
                } else {
                    let found = {
                        let guard = clients.read().await;
                        let mut target = None;
                        let mut target_addr = None;
                        for (peer, session) in guard.iter() {
                            let session_agent = {
                                let locked = session.lock().await;
                                locked.agent_id
                            };
                            if session_agent == to {
                                target = Some(session.clone());
                                target_addr = Some(*peer);
                                break;
                            }
                        }
                        drop(guard);
                        (target, target_addr)
                    };

                    if let (Some(session), Some(peer)) = found {
                        let bytes = {
                            let mut guard = session.lock().await;
                            let (data, _) = guard.circuit.send_msg(&im, true);
                            data
                        };
                        let _ = socket.send_to(&bytes, peer).await;
                    } else {
                        tracing::debug!("no client session found for agent {}", to);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::sync::RwLock;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    const DDLS: &[&str] = &[
        "CREATE TABLE IF NOT EXISTS os_groups_groups (GroupID CHAR(36) PRIMARY KEY, Location VARCHAR(255) NOT NULL DEFAULT '', Name VARCHAR(255) NOT NULL DEFAULT '', Charter TEXT NOT NULL DEFAULT '', InsigniaID CHAR(36) NOT NULL DEFAULT '', FounderID CHAR(36) NOT NULL DEFAULT '', MembershipFee INTEGER NOT NULL DEFAULT 0, OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', ShowInList INTEGER NOT NULL DEFAULT 0, AllowPublish INTEGER NOT NULL DEFAULT 0, MaturePublish INTEGER NOT NULL DEFAULT 0, OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_membership (GroupID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, SelectedRoleID CHAR(36) NOT NULL DEFAULT '', Contribution INTEGER NOT NULL DEFAULT 0, ListInProfile INTEGER NOT NULL DEFAULT 1, AcceptNotices INTEGER NOT NULL DEFAULT 1, AccessToken CHAR(36) NOT NULL DEFAULT '', PRIMARY KEY (GroupID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_roles (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, Name VARCHAR(255) NOT NULL DEFAULT '', Description VARCHAR(255) NOT NULL DEFAULT '', Title VARCHAR(255) NOT NULL DEFAULT '', Powers BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (GroupID, RoleID))",
        "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, PRIMARY KEY (GroupID, RoleID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_invites (InviteID CHAR(36) PRIMARY KEY, GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, TMStamp INTEGER NOT NULL DEFAULT 0)",
        "CREATE TABLE IF NOT EXISTS os_groups_notices (GroupID CHAR(36) NOT NULL, NoticeID CHAR(36) PRIMARY KEY, TMStamp INTEGER NOT NULL DEFAULT 0, FromName VARCHAR(255) NOT NULL DEFAULT '', Subject VARCHAR(255) NOT NULL DEFAULT '', Message TEXT NOT NULL DEFAULT '', HasAttachment INTEGER NOT NULL DEFAULT 0, AttachmentType INTEGER NOT NULL DEFAULT 0, AttachmentName VARCHAR(128) NOT NULL DEFAULT '', AttachmentItemID CHAR(36) NOT NULL DEFAULT '', AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_principals (PrincipalID VARCHAR(255) PRIMARY KEY, ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
    ];

    struct DbGuard {
        path: std::path::PathBuf,
    }

    impl Drop for DbGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    fn temp_db_path() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "welkin-region-groups-chat-{}-{}.sqlite",
            std::process::id(),
            n
        ))
    }

    async fn make_ctx() -> (gio::GroupsCtx, DbGuard) {
        let path = temp_db_path();
        let guard = DbGuard {
            path: path.clone(),
        };
        let _ = std::fs::remove_file(&path);

        sqlx::any::install_default_drivers();
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .unwrap();

        use sqlx::Executor;
        for ddl in DDLS {
            pool.execute(*ddl).await.unwrap();
        }

        let state = welkin_services::groups::GroupsServiceState {
            store: Arc::new(welkin_data::groups::GroupsStore::new(pool)),
        };
        let app = welkin_services::groups::router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let url = format!("http://{}", addr);
        let ctx = gio::GroupsCtx::new(&url);
        (ctx, guard)
    }

    fn mk_im(dialog: u8, from_group: bool, id: Uuid, to: Uuid, message: &str, bucket: Vec<u8>) -> msgs::ImprovedInstantMessage {
        msgs::ImprovedInstantMessage {
            agent_data: msgs::improved_instant_message::AgentData {
                agent_id: Uuid::nil(),
                session_id: Uuid::nil(),
            },
            message_block: msgs::improved_instant_message::MessageBlock {
                from_group,
                to_agent_id: to,
                parent_estate_id: 0,
                region_id: Uuid::nil(),
                position: glam::Vec3::ZERO,
                offline: 0,
                dialog,
                id,
                timestamp: 0,
                from_agent_name: Vec::new(),
                message: message.as_bytes().to_vec(),
                binary_bucket: bucket,
            },
            estate_block: msgs::improved_instant_message::EstateBlock::default(),
            meta_data: Vec::new(),
        }
    }

    async fn create_group(ctx: &gio::GroupsCtx, founder: Uuid) -> Uuid {
        let group = ctx
            .connector()
            .create_group(
                founder,
                &welkin_services::remote::groups::GroupRecord {
                    group_id: Uuid::nil(),
                    group_name: "Test Group".to_string(),
                    charter: String::new(),
                    insignia_id: Uuid::nil(),
                    founder_id: founder,
                    membership_fee: 0,
                    open_enrollment: true,
                    show_in_list: true,
                    allow_publish: false,
                    mature_publish: false,
                    owner_role_id: Uuid::nil(),
                    member_count: 0,
                    role_count: 0,
                },
            )
            .await
            .unwrap();
        group.group_id
    }

    fn has_im(outs: &[ChatOut]) -> bool {
        outs.iter().any(|o| matches!(o, ChatOut::Im { .. }))
    }

    #[tokio::test]
    async fn chat_non_member_alerts() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let non_member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;

        let chat = GroupChat::new();

        let im_start = mk_im(15, false, group, group, "", Vec::new());
        let out = process_chat_im(&ctx, &chat, non_member, "Non", "Member", Uuid::nil(), &im_start, None).await;
        assert!(matches!(out.as_slice(), [ChatOut::Alert(_)]));
        assert!(!chat.is_fresh(group, non_member));

        let im_send = mk_im(17, true, group, group, "hello", Vec::new());
        let out = process_chat_im(&ctx, &chat, non_member, "Non", "Member", Uuid::nil(), &im_send, None).await;
        assert!(matches!(out.as_slice(), [ChatOut::Alert(_)]));
        assert!(!has_im(&out));
        assert!(!chat.is_fresh(group, non_member));
    }

    #[tokio::test]
    async fn chat_members_broadcast() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let non_member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, member, group, Uuid::nil(), "")
            .await
            .unwrap();

        let chat = GroupChat::new();

        let im_start = mk_im(15, true, group, group, "", Vec::new());
        let _ = process_chat_im(&ctx, &chat, founder, "Found", "Er", Uuid::nil(), &im_start, None).await;
        let _ = process_chat_im(&ctx, &chat, member, "Mem", "Ber", Uuid::nil(), &im_start, None).await;

        let im_send = mk_im(17, true, group, group, "packet name", Vec::new());
        let out = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im_send, None).await;

        assert_eq!(out.len(), 1);
        match &out[0] {
            ChatOut::Im { to, im } => {
                assert_eq!(*to, member);
                assert_eq!(im.message_block.dialog, 17);
                assert!(im.message_block.from_group);
                assert_eq!(im.agent_data.agent_id, founder);
                assert_eq!(
                    im.message_block.from_agent_name,
                    gio::name_bytes("First Last")
                );
            }
            _ => panic!("expected one Im"),
        }
        assert!(!has_im_for(&out, non_member));
    }

    fn has_im_for(outs: &[ChatOut], agent: Uuid) -> bool {
        outs.iter().any(|o| match o {
            ChatOut::Im { to, .. } => *to == agent,
            _ => false,
        })
    }

    #[tokio::test]
    async fn chat_ejected_member() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, member, group, Uuid::nil(), "")
            .await
            .unwrap();

        let chat = GroupChat::with_ttl(Duration::ZERO);

        let im_start = mk_im(15, true, group, group, "", Vec::new());
        let _ = process_chat_im(&ctx, &chat, member, "Mem", "Ber", Uuid::nil(), &im_start, None).await;

        ctx.connector()
            .remove_agent_from_group(founder, member, group)
            .await
            .unwrap();

        let im_send = mk_im(17, true, group, group, "msg", Vec::new());
        let out = process_chat_im(&ctx, &chat, member, "Mem", "Ber", Uuid::nil(), &im_send, None).await;
        assert!(matches!(out.as_slice(), [ChatOut::Alert(_)]));
        assert!(!has_im(&out));

        chat.mark(group, founder);
        let im_drop = mk_im(18, true, group, group, "", Vec::new());
        let _ = process_chat_im(&ctx, &chat, founder, "Found", "Er", Uuid::nil(), &im_drop, None).await;
        assert!(!chat.is_fresh(group, founder));
    }

    #[tokio::test]
    async fn notice_send_and_delivery() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member1 = Uuid::new_v4();
        let member2 = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, member1, group, Uuid::nil(), "")
            .await
            .unwrap();
        ctx.connector()
            .add_agent_to_group(founder, member2, group, Uuid::nil(), "")
            .await
            .unwrap();
        ctx.connector()
            .update_membership(member2, member2, group, false, true)
            .await
            .unwrap();

        let chat = GroupChat::new();
        let im = mk_im(32, true, Uuid::nil(), group, "Subj|Body text", Vec::new());
        let out = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im, None).await;

        let accepting = [founder, member1];
        assert_eq!(out.len(), accepting.len());
        for recipient in accepting {
            let found = out.iter().any(|o| match o {
                ChatOut::Im { to, im } => {
                    *to == recipient
                        && im.message_block.dialog == 32
                        && im.message_block.from_group
                        && im.agent_data.agent_id == group
                        && gio::bytes_to_string(&im.message_block.message) == "Subj|Body text"
                        && gio::bytes_to_string(&im.message_block.from_agent_name) == "First Last"
                        && im.message_block.binary_bucket.len() == 19
                        && im.message_block.binary_bucket[2..18] == group.as_bytes()[..]
                        && im.message_block.offline == 1
                }
                _ => false,
            });
            assert!(found, "missing notice for {}", recipient);
        }
        assert!(!out.iter().any(|o| match o {
            ChatOut::Im { to, .. } => *to == member2,
            _ => false,
        }));

        let notices = ctx
            .connector()
            .get_notices(founder, group)
            .await
            .unwrap();
        assert!(!notices.is_empty());
        assert_eq!(notices[0].subject, "Subj");
        // The list carries no body; the single-notice lookup does.
        let full = ctx
            .connector()
            .get_notice(founder, notices[0].notice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(full.message, "Body text");
    }

    #[tokio::test]
    async fn notice_permissions_and_attachments() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let plain_member = Uuid::new_v4();
        let non_member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, plain_member, group, Uuid::nil(), "")
            .await
            .unwrap();

        let chat = GroupChat::new();

        let im_plain = mk_im(32, true, Uuid::nil(), group, "Subj|Body", Vec::new());
        let out = process_chat_im(&ctx, &chat, plain_member, "Plain", "Member", Uuid::nil(), &im_plain, None).await;
        assert!(matches!(out.as_slice(), [ChatOut::Alert(msg)] if *msg == gio::alert_message("No permission to send notice to group")));
        let before = ctx.connector().get_notices(founder, group).await.unwrap().len();

        let im_attach = mk_im(32, true, Uuid::nil(), group, "Subj|Body", vec![1u8]);
        let out = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im_attach, None).await;
        assert!(matches!(out.as_slice(), [ChatOut::Alert(msg)] if *msg == gio::alert_message("Group notice attachments are not supported yet.")));
        let after = ctx.connector().get_notices(founder, group).await.unwrap().len();
        assert_eq!(before, after);

        let im_non = mk_im(32, true, Uuid::nil(), group, "Subj|Body", Vec::new());
        let out = process_chat_im(&ctx, &chat, non_member, "Non", "Mem", Uuid::nil(), &im_non, None).await;
        assert!(matches!(out.as_slice(), [ChatOut::Alert(msg)] if *msg == gio::alert_message("Group membership not found")));

        let im_nopipe = mk_im(32, true, Uuid::nil(), group, "NoPipe", Vec::new());
        let out = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im_nopipe, None).await;
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn notice_request_37() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let non_member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, member, group, Uuid::nil(), "")
            .await
            .unwrap();

        let chat = GroupChat::new();
        let im = mk_im(32, true, Uuid::nil(), group, "Subj|Body", Vec::new());
        let _ = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im, None).await;

        let notices = ctx.connector().get_notices(founder, group).await.unwrap();
        let notice_id = notices[0].notice_id;

        let im37 = mk_im(37, false, notice_id, Uuid::nil(), "", Vec::new());
        let out = process_chat_im(&ctx, &chat, member, "Mem", "Ber", Uuid::nil(), &im37, None).await;
        assert_eq!(out.len(), 1);
        match &out[0] {
            ChatOut::Im { to, im } => {
                assert_eq!(*to, member);
                assert_eq!(im.message_block.dialog, 32);
                assert_eq!(im.agent_data.agent_id, group);
                assert_eq!(gio::bytes_to_string(&im.message_block.message), "Subj|Body");
            }
            _ => panic!("expected notice Im"),
        }

        let out = process_chat_im(&ctx, &chat, non_member, "Non", "Mem", Uuid::nil(), &im37, None).await;
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn notices_list() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let non_member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, member, group, Uuid::nil(), "")
            .await
            .unwrap();

        let chat = GroupChat::new();
        let im = mk_im(32, true, Uuid::nil(), group, "Subj|Body", Vec::new());
        let _ = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im, None).await;

        let req = msgs::GroupNoticesListRequest {
            agent_data: msgs::group_notices_list_request::AgentData {
                agent_id: member,
                session_id: Uuid::nil(),
            },
            data: msgs::group_notices_list_request::Data { group_id: group },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let body = w.buf;

        let out = process_notices_list(&ctx, member, msgs::GroupNoticesListRequest::ID, &body).await;
        match out {
            Some(v) => match &v[..] {
                [ChatOut::NoticesList(reply)] => {
                    assert_eq!(reply.agent_data.agent_id, member);
                    assert_eq!(reply.agent_data.group_id, group);
                    assert!(!reply.data.is_empty());
                }
                _ => panic!("expected NoticesList"),
            },
            None => panic!("expected Some"),
        }

        let out = process_notices_list(&ctx, non_member, msgs::GroupNoticesListRequest::ID, &body).await;
        assert!(matches!(out, Some(v) if v.is_empty()));

        assert!(process_notices_list(&ctx, member, msgs::UUIDGroupNameRequest::ID, &body).await.is_none());

        let truncated = &body[..body.len() / 2];
        // A truncated body decodes with a zero-filled group id: a non-member, so nothing is sent.
        assert!(process_notices_list(&ctx, member, msgs::GroupNoticesListRequest::ID, truncated)
            .await
            .map_or(true, |out| out.is_empty()));
    }

    #[tokio::test]
    async fn spawn_chat_im_end_to_end() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector()
            .add_agent_to_group(founder, member, group, Uuid::nil(), "")
            .await
            .unwrap();

        let founder_socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let member_socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let founder_addr = founder_socket.local_addr().unwrap();
        let member_addr = member_socket.local_addr().unwrap();

        let region_socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());

        let founder_session = Arc::new(Mutex::new(welkin_net::server::ClientSession::new(
            founder_addr,
            1,
            founder,
            Uuid::new_v4(),
        )));
        let member_session = Arc::new(Mutex::new(welkin_net::server::ClientSession::new(
            member_addr,
            2,
            member,
            Uuid::new_v4(),
        )));

        let mut clients_map = HashMap::new();
        clients_map.insert(founder_addr, founder_session.clone());
        clients_map.insert(member_addr, member_session.clone());
        let clients = Arc::new(RwLock::new(clients_map));

        let chat = GroupChat::new();
        let im = mk_im(32, true, Uuid::nil(), group, "Subj|Body", Vec::new());

        spawn_chat_im(
            ctx,
            chat,
            founder,
            "First".to_string(),
            "Last".to_string(),
            Uuid::nil(),
            im,
            Arc::new(RwLock::new(HashMap::new())),
            clients,
            founder_session,
            region_socket,
            founder_addr,
        );

        let mut buf = vec![0u8; 65536];
        let received = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            member_socket.recv(&mut buf),
        )
        .await
        .unwrap()
        .unwrap();

        let packet = welkin_msg::parse_packet(&buf[..received]).unwrap();
        assert_eq!(packet.id, msgs::ImprovedInstantMessage::ID);
    }

    #[tokio::test]
    async fn chat_dialog_constants_match_protocol() {
        assert_eq!(crate::groups_chat::dialog::SESSION_ADD, 13);
        assert_eq!(crate::groups_chat::dialog::SESSION_OFFLINE_ADD, 14);
        assert_eq!(crate::groups_chat::dialog::SESSION_GROUP_START, 15);
        assert_eq!(crate::groups_chat::dialog::SESSION_SEND, 17);
        assert_eq!(crate::groups_chat::dialog::SESSION_DROP, 18);
        assert_eq!(crate::groups_chat::dialog::GROUP_NOTICE, 32);
        assert_eq!(crate::groups_chat::dialog::GROUP_NOTICE_REQUESTED, 37);

        for dialog in [13u8, 15, 17, 18, 32, 37] {
            assert!(crate::groups_chat::is_chat_dialog(dialog, false));
            assert!(crate::groups_chat::is_chat_dialog(dialog, true));
        }
        for dialog in [14u8, 16, 19, 20] {
            assert!(!crate::groups_chat::is_chat_dialog(dialog, false));
            assert!(!crate::groups_chat::is_chat_dialog(dialog, true));
        }
    }

    #[tokio::test]
    async fn session_start_replies_through_event_queue() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let chat = crate::groups_chat::GroupChat::new();
        let agent = founder;
        let im = mk_im(crate::groups_chat::dialog::SESSION_GROUP_START, false, group, group, "hello", Vec::new());

        let eq = welkin_caps::event_queue::EventQueue::new();
        let outputs = process_chat_im(&ctx, &chat, agent, "First", "Last", Uuid::nil(), &im, Some(&eq)).await;
        assert!(!has_im(&outputs));
        assert!(chat.is_fresh(group, agent));

        let response = eq.poll(std::time::Duration::from_millis(500)).await;
        let welkin_caps::event_queue::EventQueueResponse::Events { events, .. } = response else {
            panic!("expected events");
        };
        let text = String::from_utf8_lossy(&events).to_string();
        assert!(text.contains("ChatterBoxSessionStartReply"));
        assert!(text.contains("Test Group"));

        let outsider = Uuid::new_v4();
        let eq2 = welkin_caps::event_queue::EventQueue::new();
        let im2 = mk_im(crate::groups_chat::dialog::SESSION_GROUP_START, false, group, group, "hello", Vec::new());
        let outputs2 = process_chat_im(&ctx, &chat, outsider, "Third", "Fourth", Uuid::nil(), &im2, Some(&eq2)).await;
        assert!(matches!(
            outputs2.first(),
            Some(crate::groups_chat::ChatOut::Alert(_))
        ));
        let response2 = eq2.poll(std::time::Duration::from_millis(500)).await;
        assert!(matches!(
            response2,
            welkin_caps::event_queue::EventQueueResponse::Timeout
        ));
    }

    #[tokio::test]
    async fn session_drop_stops_delivery() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let chat = crate::groups_chat::GroupChat::new();
        let agent = founder;

        let im_start =
            mk_im(crate::groups_chat::dialog::SESSION_GROUP_START, false, group, group, "hello", Vec::new());
        process_chat_im(&ctx, &chat, agent, "First", "Last", Uuid::nil(), &im_start, None).await;
        assert!(chat.is_fresh(group, agent));

        let im_drop = mk_im(crate::groups_chat::dialog::SESSION_DROP, false, group, group, "bye", Vec::new());
        process_chat_im(&ctx, &chat, agent, "First", "Last", Uuid::nil(), &im_drop, None).await;
        assert!(!chat.is_fresh(group, agent));
    }

    #[tokio::test]
    async fn agent_list_update_event_shape() {
        let group = Uuid::new_v4();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let ev = agent_list_update_event(group, &[(a, true), (b, false)]);
        let welkin_llsd::Llsd::Map(top) = ev else { panic!("top"); };
        let agent_updates = top.get("agent_updates").cloned().expect("agent_updates");
        let welkin_llsd::Llsd::Map(au) = agent_updates else { panic!("au"); };
        assert_eq!(au.len(), 2);
        let ae = au.get(&a.to_string()).cloned().expect("a");
        let welkin_llsd::Llsd::Map(aem) = ae else { panic!("aem"); };
        let welkin_llsd::Llsd::String(at) = aem.get("transition").cloned().expect("at") else { panic!(); };
        assert_eq!(at, "ENTER");
        let welkin_llsd::Llsd::Map(info) = aem.get("info").cloned().expect("info") else { panic!(); };
        assert!(matches!(info.get("can_voice_chat"), Some(welkin_llsd::Llsd::Boolean(false))));
        assert!(matches!(info.get("is_moderator"), Some(welkin_llsd::Llsd::Boolean(false))));
        let welkin_llsd::Llsd::Map(mutes) = info.get("mutes").cloned().expect("mutes") else { panic!(); };
        assert!(matches!(mutes.get("text"), Some(welkin_llsd::Llsd::Boolean(false))));
        let be = au.get(&b.to_string()).cloned().expect("b");
        let welkin_llsd::Llsd::Map(bem) = be else { panic!(); };
        let welkin_llsd::Llsd::String(bt) = bem.get("transition").cloned().expect("bt") else { panic!(); };
        assert_eq!(bt, "LEAVE");
        let updates = top.get("updates").cloned().expect("updates");
        let welkin_llsd::Llsd::Map(um) = updates else { panic!(); };
        assert!(um.is_empty());
        let session_id = top.get("session_id").cloned().expect("sid");
        assert!(matches!(session_id, welkin_llsd::Llsd::Uuid(x) if x == group));
    }

    #[tokio::test]
    async fn start_and_drop_emit_roster_events() {
        let (ctx, _guard) = make_ctx().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        ctx.connector().add_agent_to_group(founder, member, group, Uuid::nil(), "").await.unwrap();

        let chat = GroupChat::new();
        let im = mk_im(15, false, group, Uuid::nil(), "", Vec::new());
        let outs = process_chat_im(&ctx, &chat, founder, "First", "Last", Uuid::nil(), &im, None).await;
        assert_eq!(outs.len(), 1);
        let ChatOut::Event { to, name, body } = &outs[0] else { panic!("not event"); };
        assert_eq!(*to, founder);
        assert_eq!(*name, "ChatterBoxSessionAgentListUpdates");
        let welkin_llsd::Llsd::Map(top) = body else { panic!(); };
        let welkin_llsd::Llsd::Map(au) = top.get("agent_updates").cloned().unwrap() else { panic!(); };
        assert_eq!(au.len(), 1);
        assert!(au.contains_key(&founder.to_string()));

        let im = mk_im(15, false, group, Uuid::nil(), "", Vec::new());
        let outs = process_chat_im(&ctx, &chat, member, "First", "Last", Uuid::nil(), &im, None).await;
        assert_eq!(outs.len(), 2);
        let ChatOut::Event { to: t0, body: b0, .. } = &outs[0] else { panic!(); };
        assert_eq!(*t0, member);
        let welkin_llsd::Llsd::Map(top0) = b0 else { panic!(); };
        let welkin_llsd::Llsd::Map(au0) = top0.get("agent_updates").cloned().unwrap() else { panic!(); };
        assert_eq!(au0.len(), 2);
        let ChatOut::Event { to: t1, body: b1, .. } = &outs[1] else { panic!(); };
        assert_eq!(*t1, founder);
        let welkin_llsd::Llsd::Map(top1) = b1 else { panic!(); };
        let welkin_llsd::Llsd::Map(au1) = top1.get("agent_updates").cloned().unwrap() else { panic!(); };
        assert_eq!(au1.len(), 1);
        assert!(au1.contains_key(&member.to_string()));
        let welkin_llsd::Llsd::Map(e) = au1.get(&member.to_string()).cloned().unwrap() else { panic!(); };
        let welkin_llsd::Llsd::String(tr) = e.get("transition").cloned().unwrap() else { panic!(); };
        assert_eq!(tr, "ENTER");

        let im = mk_im(18, false, group, Uuid::nil(), "", Vec::new());
        let outs = process_chat_im(&ctx, &chat, member, "First", "Last", Uuid::nil(), &im, None).await;
        assert_eq!(outs.len(), 1);
        let ChatOut::Event { to, name, body } = &outs[0] else { panic!(); };
        assert_eq!(*to, founder);
        assert_eq!(*name, "ChatterBoxSessionAgentListUpdates");
        let welkin_llsd::Llsd::Map(top) = body else { panic!(); };
        let welkin_llsd::Llsd::Map(au) = top.get("agent_updates").cloned().unwrap() else { panic!(); };
        assert_eq!(au.len(), 1);
        let welkin_llsd::Llsd::Map(e) = au.get(&member.to_string()).cloned().unwrap() else { panic!(); };
        let welkin_llsd::Llsd::String(tr) = e.get("transition").cloned().unwrap() else { panic!(); };
        assert_eq!(tr, "LEAVE");
        assert!(outs.iter().all(|o| !matches!(o, ChatOut::Event { to: x, .. } if *x == member)));

        let stranger = Uuid::new_v4();
        let im = mk_im(15, false, group, Uuid::nil(), "", Vec::new());
        let outs = process_chat_im(&ctx, &chat, stranger, "First", "Last", Uuid::nil(), &im, None).await;
        assert!(outs.iter().all(|o| !matches!(o, ChatOut::Event { .. })));
    }
}
