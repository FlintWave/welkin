// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::groups_io::{
    GroupsCtx, active_group, agent_data_update_for, alert_message,
    name_bytes, refresh_viewer_group_state,
};
use glam::Vec3;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use welkin_caps::event_queue::EventQueue;
use welkin_msg::msgs;
use welkin_msg::Msg;
use welkin_net::server::ClientSession;
use welkin_services::remote::groups::InviteResult;

#[derive(Debug)]
pub enum InviteOut {
    Alert(msgs::AlertMessage),
    Im {
        to: Uuid,
        im: msgs::ImprovedInstantMessage,
    },
    AgentData(msgs::AgentDataUpdate),
}

pub fn is_invite_packet(id: welkin_msg::MsgId) -> bool {
    id == msgs::InviteGroupRequest::ID
}

pub fn is_invite_reply_dialog(dialog: u8) -> bool {
    dialog == 35 || dialog == 36
}

pub async fn process_invite_packet(
    ctx: &GroupsCtx,
    agent: Uuid,
    inviter_name: &str,
    region_id: Uuid,
    msg_id: welkin_msg::MsgId,
    body: &[u8],
) -> Option<Vec<InviteOut>> {
    if !is_invite_packet(msg_id) {
        return None;
    }
    let req = msgs::InviteGroupRequest::decode(body).ok()?;

    let group_id = req.group_data.group_id;
    let group = match ctx.connector().get_group(agent, Some(group_id), None).await {
        Ok(Some(group)) => group,
        _ => return Some(Vec::new()),
    };
    let group_name = group.group_name;

    // A failed role lookup yields no roles, so only the Everyone role can be offered.
    let held_roles = ctx
        .connector()
        .get_agent_roles(agent, agent, group_id)
        .await
        .unwrap_or_default();

    let mut outs = Vec::new();
    for entry in req.invite_data.into_iter().take(32) {
        if entry.invitee_id.is_nil() || entry.invitee_id == agent {
            continue;
        }
        if entry.role_id != Uuid::nil()
            && !held_roles.iter().any(|role| role.role_id == entry.role_id)
        {
            outs.push(InviteOut::Alert(alert_message(
                "You can only invite members into roles you hold.",
            )));
            continue;
        }

        let invite_id = Uuid::new_v4();
        match ctx
            .connector()
            .invite(
                agent,
                "ADD",
                invite_id,
                Some(group_id),
                Some(entry.role_id),
                Some(entry.invitee_id),
            )
            .await
        {
            Ok(InviteResult::Added(true)) => {
                let im = msgs::ImprovedInstantMessage {
                    agent_data: msgs::improved_instant_message::AgentData {
                        agent_id: group_id,
                        session_id: Uuid::nil(),
                    },
                    message_block: msgs::improved_instant_message::MessageBlock {
                        from_group: true,
                        to_agent_id: entry.invitee_id,
                        parent_estate_id: 0,
                        region_id,
                        position: Vec3::ZERO,
                        offline: 0,
                        dialog: 3,
                        id: invite_id,
                        timestamp: 0,
                        from_agent_name: name_bytes(inviter_name),
                        message: name_bytes(&format!(
                            "{inviter_name} has invited you to join a group called {group_name}. There is no cost to join this group."
                        )),
                        binary_bucket: vec![0u8; 20],
                    },
                    estate_block: msgs::improved_instant_message::EstateBlock::default(),
                    meta_data: Vec::new(),
                };
                outs.push(InviteOut::Im {
                    to: entry.invitee_id,
                    im,
                });
            }
            Ok(InviteResult::Added(false)) => {
                outs.push(InviteOut::Alert(alert_message(
                    "Unable to invite that agent (no permission, or already a member).",
                )));
            }
            Ok(_) => {
                outs.push(InviteOut::Alert(alert_message(
                    "Unable to invite that agent (no permission, or already a member).",
                )));
            }
            Err(_) => {
                outs.push(InviteOut::Alert(alert_message(
                    "The groups service is unavailable.",
                )));
                break;
            }
        }
    }

    Some(outs)
}

pub async fn process_invite_reply(
    ctx: &GroupsCtx,
    agent: Uuid,
    first: &str,
    last: &str,
    im: &msgs::ImprovedInstantMessage,
    eq: Option<&EventQueue>,
) -> Vec<InviteOut> {
    if !is_invite_reply_dialog(im.message_block.dialog) {
        return Vec::new();
    }

    let invite_id = im.message_block.id;
    let invite = match ctx
        .connector()
        .invite(agent, "GET", invite_id, None, None, None)
        .await
    {
        Ok(InviteResult::Invite(Some(invite))) => invite,
        other => {
            tracing::warn!(%invite_id, result = ?other, "group invite reply: invite lookup failed");
            return Vec::new();
        }
    };

    if invite.agent_id != agent {
        tracing::warn!(
            "group invite {} belongs to {}, not authenticated {}",
            invite_id,
            invite.agent_id,
            agent
        );
        return Vec::new();
    }

    match im.message_block.dialog {
        35 => match ctx
            .connector()
            .add_agent_to_group(agent, agent, invite.group_id, invite.role_id, "")
            .await
        {
            Ok(_) => {
                let _ = ctx
                    .connector()
                    .invite(agent, "DELETE", invite_id, None, None, None)
                    .await;
                let message_box = msgs::ImprovedInstantMessage {
                    agent_data: msgs::improved_instant_message::AgentData {
                        agent_id: Uuid::nil(),
                        session_id: Uuid::nil(),
                    },
                    message_block: msgs::improved_instant_message::MessageBlock {
                        from_group: false,
                        to_agent_id: agent,
                        parent_estate_id: 0,
                        region_id: Uuid::nil(),
                        position: Vec3::ZERO,
                        offline: 0,
                        dialog: 1,
                        id: Uuid::nil(),
                        timestamp: 0,
                        from_agent_name: name_bytes("Groups"),
                        message: name_bytes("You have been added to the group."),
                        binary_bucket: Vec::new(),
                    },
                    estate_block: msgs::improved_instant_message::EstateBlock::default(),
                    meta_data: Vec::new(),
                };
                let mut outs = vec![InviteOut::Im {
                    to: agent,
                    im: message_box,
                }];
                if let Some(event_queue) = eq {
                    outs.push(InviteOut::AgentData(
                        refresh_viewer_group_state(ctx, agent, first, last, event_queue).await,
                    ));
                } else {
                    outs.push(InviteOut::AgentData(agent_data_update_for(
                        agent,
                        first,
                        last,
                        &active_group(ctx, agent).await,
                    )));
                }
                outs
            }
            Err(_) => vec![InviteOut::Alert(alert_message(
                "Unable to add you to the group.",
            ))],
        },
        36 => {
            let _ = ctx
                .connector()
                .invite(agent, "DELETE", invite_id, None, None, None)
                .await;
            Vec::new()
        }
        _ => Vec::new(),
    }
}

pub fn spawn_invite_packet(
    ctx: GroupsCtx,
    agent: Uuid,
    inviter_name: String,
    region_id: Uuid,
    msg_id: welkin_msg::MsgId,
    body: Vec<u8>,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    let queue = ctx.queue().clone();
    let _ = queue.submit(agent, async move {
        if let Some(outs) =
            process_invite_packet(&ctx, agent, &inviter_name, region_id, msg_id, &body).await
        {
            deliver_outs(outs, agent, clients, client, socket, addr).await;
        }
    });
}

pub fn spawn_invite_reply(
    ctx: GroupsCtx,
    agent: Uuid,
    first: String,
    last: String,
    im: msgs::ImprovedInstantMessage,
    event_queues: Arc<RwLock<HashMap<Uuid, Arc<EventQueue>>>>,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    let queue = ctx.queue().clone();
    let _ = queue.submit(agent, async move {
        let event_queue = {
            let guard = event_queues.read().await;
            guard.get(&agent).cloned()
        };
        let event_queue_ref = event_queue.as_ref().map(|queue| queue.as_ref());
        let outs = process_invite_reply(&ctx, agent, &first, &last, &im, event_queue_ref).await;
        deliver_outs(outs, agent, clients, client, socket, addr).await;
    });
}

async fn deliver_outs(
    outs: Vec<InviteOut>,
    agent: Uuid,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    for out in outs {
        match out {
            InviteOut::Alert(msg) => {
                let mut session = client.lock().await;
                let (bytes, _) = session.circuit.send_msg(&msg, true);
                drop(session);
                let _ = socket.send_to(&bytes, addr).await;
            }
            InviteOut::AgentData(msg) => {
                let mut session = client.lock().await;
                let (bytes, _) = session.circuit.send_msg(&msg, true);
                drop(session);
                let _ = socket.send_to(&bytes, addr).await;
            }
            InviteOut::Im { to, im } => {
                if to == agent {
                    let mut session = client.lock().await;
                    let (bytes, _) = session.circuit.send_msg(&im, true);
                    drop(session);
                    let _ = socket.send_to(&bytes, addr).await;
                } else {
                    let target = {
                        let guard = clients.read().await;
                        let mut found = None;
                        for (peer, session) in guard.iter() {
                            let session_agent = {
                                let locked = session.lock().await;
                                locked.agent_id
                            };
                            if session_agent == to {
                                found = Some((*peer, session.clone()));
                                break;
                            }
                        }
                        found
                    };
                    if let Some((peer, session)) = target {
                        let mut locked = session.lock().await;
                        let (bytes, _) = locked.circuit.send_msg(&im, true);
                        drop(locked);
                        let _ = socket.send_to(&bytes, peer).await;
                    } else {
                        tracing::warn!("group invite target is not in this region; the invite stays stored");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::Writer;
    use crate::groups_io::bytes_to_string;
    use welkin_services::remote::groups::GroupRecord;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::time::{Duration, timeout};

    const DDL: &[&str] = &[
        "CREATE TABLE IF NOT EXISTS os_groups_groups (GroupID CHAR(36) PRIMARY KEY, Location VARCHAR(255) NOT NULL DEFAULT '', Name VARCHAR(255) NOT NULL DEFAULT '', Charter TEXT NOT NULL DEFAULT '', InsigniaID CHAR(36) NOT NULL DEFAULT '', FounderID CHAR(36) NOT NULL DEFAULT '', MembershipFee INTEGER NOT NULL DEFAULT 0, OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', ShowInList INTEGER NOT NULL DEFAULT 0, AllowPublish INTEGER NOT NULL DEFAULT 0, MaturePublish INTEGER NOT NULL DEFAULT 0, OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_membership (GroupID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, SelectedRoleID CHAR(36) NOT NULL DEFAULT '', Contribution INTEGER NOT NULL DEFAULT 0, ListInProfile INTEGER NOT NULL DEFAULT 1, AcceptNotices INTEGER NOT NULL DEFAULT 1, AccessToken CHAR(36) NOT NULL DEFAULT '', PRIMARY KEY (GroupID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_roles (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, Name VARCHAR(255) NOT NULL DEFAULT '', Description VARCHAR(255) NOT NULL DEFAULT '', Title VARCHAR(255) NOT NULL DEFAULT '', Powers BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (GroupID, RoleID))",
        "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, PRIMARY KEY (GroupID, RoleID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_invites (InviteID CHAR(36) PRIMARY KEY, GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, TMStamp INTEGER NOT NULL DEFAULT 0)",
        "CREATE TABLE IF NOT EXISTS os_groups_notices (GroupID CHAR(36) NOT NULL, NoticeID CHAR(36) PRIMARY KEY, TMStamp INTEGER NOT NULL DEFAULT 0, FromName VARCHAR(255) NOT NULL DEFAULT '', Subject VARCHAR(255) NOT NULL DEFAULT '', Message TEXT NOT NULL DEFAULT '', HasAttachment INTEGER NOT NULL DEFAULT 0, AttachmentType INTEGER NOT NULL DEFAULT 0, AttachmentName VARCHAR(128) NOT NULL DEFAULT '', AttachmentItemID CHAR(36) NOT NULL DEFAULT '', AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_principals (PrincipalID VARCHAR(255) PRIMARY KEY, ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
    ];

    struct TestDbGuard {
        path: std::path::PathBuf,
    }

    impl Drop for TestDbGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    async fn start_service() -> (GroupsCtx, TestDbGuard) {
        sqlx::any::install_default_drivers();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "welkin-region-groups-invites-{}-{}.sqlite",
            std::process::id(),
            counter
        ));
        let _ = std::fs::remove_file(&path);
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .unwrap();
        for ddl in DDL {
            sqlx::query(*ddl).execute(&pool).await.unwrap();
        }
        let state = welkin_services::groups::GroupsServiceState {
            store: Arc::new(welkin_data::groups::GroupsStore::new(pool.clone())),
        };
        let app = welkin_services::groups::router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let probe_founder = Uuid::new_v4();
        let mut ctx = None;
        for base in [
            format!("http://{}", addr),
            format!("http://{}/", addr),
            format!("http://{}/groups", addr),
            format!("http://{}/Groups", addr),
        ] {
            let candidate = GroupsCtx::new(&base);
            if candidate
                .connector()
                .create_group(
                    probe_founder,
                    &GroupRecord {
                        group_id: Uuid::nil(),
                        group_name: "probe".into(),
                        charter: String::new(),
                        insignia_id: Uuid::nil(),
                        founder_id: probe_founder,
                        membership_fee: 0,
                        open_enrollment: false,
                        show_in_list: true,
                        allow_publish: false,
                        mature_publish: false,
                        owner_role_id: Uuid::nil(),
                        member_count: 0,
                        role_count: 0,
                    },
                )
                .await
                .is_ok()
            {
                ctx = Some(candidate);
                break;
            }
        }
        let ctx = ctx.unwrap_or_else(|| GroupsCtx::new(&format!("http://{}", addr)));
        (ctx, TestDbGuard { path })
    }

    async fn create_group(ctx: &GroupsCtx, founder: Uuid) -> GroupRecord {
        ctx.connector()
            .create_group(
                founder,
                &GroupRecord {
                    group_id: Uuid::nil(),
                    group_name: "Test Group".into(),
                    charter: String::new(),
                    insignia_id: Uuid::nil(),
                    founder_id: founder,
                    membership_fee: 0,
                    open_enrollment: false,
                    show_in_list: true,
                    allow_publish: false,
                    mature_publish: false,
                    owner_role_id: Uuid::nil(),
                    member_count: 0,
                    role_count: 0,
                },
            )
            .await
            .unwrap()
    }

    fn invite_packet_body(entries: Vec<(Uuid, Uuid)>, group_id: Uuid) -> Vec<u8> {
        let req = msgs::InviteGroupRequest {
            agent_data: msgs::invite_group_request::AgentData {
                agent_id: Uuid::nil(),
                session_id: Uuid::nil(),
            },
            group_data: msgs::invite_group_request::GroupData { group_id },
            invite_data: entries
                .into_iter()
                .map(|(invitee_id, role_id)| msgs::invite_group_request::InviteData {
                    invitee_id,
                    role_id,
                })
                .collect(),
        };
        let mut writer = Writer::new();
        req.encode_body(&mut writer);
        writer.buf
    }

    fn make_im(
        to: Uuid,
        dialog: u8,
        id: Uuid,
        from_group: bool,
        from_name: &str,
        message: Vec<u8>,
    ) -> msgs::ImprovedInstantMessage {
        msgs::ImprovedInstantMessage {
            agent_data: msgs::improved_instant_message::AgentData {
                agent_id: if from_group { Uuid::nil() } else { to },
                session_id: Uuid::nil(),
            },
            message_block: msgs::improved_instant_message::MessageBlock {
                from_group,
                to_agent_id: to,
                parent_estate_id: 0,
                region_id: Uuid::nil(),
                position: Vec3::ZERO,
                offline: 0,
                dialog,
                id,
                timestamp: 0,
                from_agent_name: name_bytes(from_name),
                message,
                binary_bucket: Vec::new(),
            },
            estate_block: msgs::improved_instant_message::EstateBlock::default(),
            meta_data: Vec::new(),
        }
    }

    #[tokio::test]
    async fn invite_stored_and_im_sent() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let invitee = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let body = invite_packet_body(vec![(invitee, Uuid::nil())], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        assert_eq!(outs.len(), 1);
        let im = match &outs[0] {
            InviteOut::Im { im, .. } => im,
            _ => panic!("expected im"),
        };
        assert_eq!(im.message_block.dialog, 3);
        assert!(im.message_block.from_group);
        assert_eq!(im.agent_data.agent_id, group.group_id);
        assert!(bytes_to_string(&im.message_block.message).contains("Test Group"));

        let invite_id = im.message_block.id;
        match ctx
            .connector()
            .invite(founder, "GET", invite_id, None, None, None)
            .await
            .unwrap()
        {
            InviteResult::Invite(Some(record)) => assert_eq!(record.agent_id, invitee),
            _ => panic!("expected stored invite"),
        }
    }

    #[tokio::test]
    async fn accept_adds_member_and_refreshes() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let invitee = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let body = invite_packet_body(vec![(invitee, Uuid::nil())], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        let invite_id = match &outs[0] {
            InviteOut::Im { im, .. } => im.message_block.id,
            _ => panic!("expected im"),
        };

        let reply = make_im(invitee, 35, invite_id, false, "Invitee", Vec::new());
        let outs = process_invite_reply(&ctx, invitee, "Invitee", "User", &reply, None).await;
        assert_eq!(outs.len(), 2);
        match &outs[0] {
            InviteOut::Im { to, im } => {
                assert_eq!(*to, invitee);
                assert_eq!(im.message_block.dialog, 1);
                assert_eq!(im.message_block.id, Uuid::nil());
                assert_eq!(
                    bytes_to_string(&im.message_block.message),
                    "You have been added to the group."
                );
            }
            _ => panic!("expected message box"),
        }
        assert!(matches!(outs[1], InviteOut::AgentData(_)));

        let memberships = ctx
            .connector()
            .get_membership(invitee, Some(group.group_id))
            .await
            .unwrap();
        assert_eq!(memberships.len(), 1);

        match ctx
            .connector()
            .invite(founder, "GET", invite_id, None, None, None)
            .await
            .unwrap()
        {
            InviteResult::Invite(None) => {}
            _ => panic!("expected deleted invite"),
        }
    }

    #[tokio::test]
    async fn decline_removes_invite_without_membership() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let invitee = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let body = invite_packet_body(vec![(invitee, Uuid::nil())], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        let invite_id = match &outs[0] {
            InviteOut::Im { im, .. } => im.message_block.id,
            _ => panic!("expected im"),
        };

        let reply = make_im(invitee, 36, invite_id, false, "Invitee", Vec::new());
        let outs = process_invite_reply(&ctx, invitee, "Invitee", "User", &reply, None).await;
        assert!(outs.is_empty());

        let memberships = ctx
            .connector()
            .get_membership(invitee, Some(group.group_id))
            .await
            .unwrap();
        assert!(memberships.is_empty());

        match ctx
            .connector()
            .invite(founder, "GET", invite_id, None, None, None)
            .await
            .unwrap()
        {
            InviteResult::Invite(None) => {}
            _ => panic!("expected deleted invite"),
        }
    }

    #[tokio::test]
    async fn spoofed_accept_does_not_add_or_delete() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let invitee = Uuid::new_v4();
        let third = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let body = invite_packet_body(vec![(invitee, Uuid::nil())], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        let invite_id = match &outs[0] {
            InviteOut::Im { im, .. } => im.message_block.id,
            _ => panic!("expected im"),
        };

        let reply = make_im(third, 35, invite_id, false, "Third", Vec::new());
        let outs = process_invite_reply(&ctx, third, "Third", "User", &reply, None).await;
        assert!(outs.is_empty());

        for principal in [third, invitee] {
            let memberships = ctx
                .connector()
                .get_membership(principal, Some(group.group_id))
                .await
                .unwrap();
            assert!(memberships.is_empty());
        }

        match ctx
            .connector()
            .invite(founder, "GET", invite_id, None, None, None)
            .await
            .unwrap()
        {
            InviteResult::Invite(Some(_)) => {}
            _ => panic!("expected invite to remain"),
        }
    }

    #[tokio::test]
    async fn role_restrictions_and_power_checks() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let outsider = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;

        ctx.connector()
            .add_agent_to_group(founder, member, group.group_id, Uuid::nil(), "")
            .await
            .unwrap();

        let body = invite_packet_body(vec![(outsider, Uuid::nil())], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            member,
            "Member",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        assert!(outs.iter().any(|out| matches!(out, InviteOut::Alert(_))));
        assert!(!outs.iter().any(|out| matches!(out, InviteOut::Im { .. })));

        let unheld_role = Uuid::new_v4();
        let body = invite_packet_body(vec![(outsider, unheld_role)], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        assert!(outs.iter().any(|out| matches!(out, InviteOut::Alert(_))));
        assert!(!outs.iter().any(|out| matches!(out, InviteOut::Im { .. })));

        let body = invite_packet_body(vec![(outsider, group.owner_role_id)], group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        assert!(outs.iter().any(|out| matches!(out, InviteOut::Im { .. })));
    }

    #[tokio::test]
    async fn caps_invite_entries_at_thirty_two() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let entries: Vec<(Uuid, Uuid)> = (0..40)
            .map(|_| (Uuid::new_v4(), Uuid::nil()))
            .collect();
        let body = invite_packet_body(entries, group.group_id);
        let outs = process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &body,
        )
        .await
        .unwrap();
        let im_count = outs
            .iter()
            .filter(|out| matches!(out, InviteOut::Im { .. }))
            .count();
        assert!(im_count <= 32);
        assert_eq!(im_count, 32);
    }

    #[tokio::test]
    async fn non_invite_or_truncated_returns_none() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;
        let body = invite_packet_body(vec![(Uuid::new_v4(), Uuid::nil())], group.group_id);
        assert!(process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::ImprovedInstantMessage::ID,
            &body,
        )
        .await
        .is_none());

        assert!(process_invite_packet(
            &ctx,
            founder,
            "Founder",
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            &[1, 2, 3],
        )
        .await
        .is_none());
    }

    #[tokio::test]
    async fn spawn_invite_packet_end_to_end() {
        let (ctx, _db) = start_service().await;
        let founder = Uuid::new_v4();
        let invitee = Uuid::new_v4();
        let group = create_group(&ctx, founder).await;

        let socket_founder = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr_founder = socket_founder.local_addr().unwrap();
        let socket_invitee = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr_invitee = socket_invitee.local_addr().unwrap();
        let server_socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());

        let session_founder = Arc::new(Mutex::new(ClientSession::new(
            addr_founder,
            1,
            founder,
            Uuid::new_v4(),
        )));
        let session_invitee = Arc::new(Mutex::new(ClientSession::new(
            addr_invitee,
            2,
            invitee,
            Uuid::new_v4(),
        )));

        let clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        clients
            .write()
            .await
            .insert(addr_founder, session_founder.clone());
        clients
            .write()
            .await
            .insert(addr_invitee, session_invitee.clone());

        let body = invite_packet_body(vec![(invitee, Uuid::nil())], group.group_id);
        spawn_invite_packet(
            ctx.clone(),
            founder,
            "Founder".into(),
            Uuid::nil(),
            msgs::InviteGroupRequest::ID,
            body,
            clients.clone(),
            session_founder.clone(),
            server_socket.clone(),
            addr_founder,
        );

        let mut buffer = vec![0u8; 65535];
        let (size, _) = timeout(Duration::from_secs(5), socket_invitee.recv_from(&mut buffer))
            .await
            .unwrap()
            .unwrap();
        let packet = welkin_msg::parse_packet(&buffer[..size]).unwrap();
        assert_eq!(packet.id, msgs::ImprovedInstantMessage::ID);

        let im = msgs::ImprovedInstantMessage {
            agent_data: msgs::improved_instant_message::AgentData {
                agent_id: Uuid::nil(),
                session_id: Uuid::nil(),
            },
            message_block: msgs::improved_instant_message::MessageBlock {
                from_group: false,
                to_agent_id: invitee,
                parent_estate_id: 0,
                region_id: Uuid::nil(),
                position: Vec3::ZERO,
                offline: 0,
                dialog: 35,
                id: {
                    // Decode is not required for the test; use a placeholder recovered below is impossible,
                    // so instead rely on the service by fetching? Reuse the first packet id cannot because
                    // packet body was already received. Keep a fresh parse via Msg trait.
                    use welkin_msg::Msg as _;
                    let parsed: msgs::ImprovedInstantMessage =
                        msgs::ImprovedInstantMessage::decode(&packet.body).unwrap();
                    parsed.message_block.id
                },
                timestamp: 0,
                from_agent_name: name_bytes("Invitee"),
                message: Vec::new(),
                binary_bucket: Vec::new(),
            },
            estate_block: msgs::improved_instant_message::EstateBlock::default(),
            meta_data: Vec::new(),
        };

        let event_queues: Arc<RwLock<HashMap<Uuid, Arc<EventQueue>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        spawn_invite_reply(
            ctx.clone(),
            invitee,
            "Invitee".into(),
            "User".into(),
            im,
            event_queues,
            clients.clone(),
            session_invitee.clone(),
            server_socket.clone(),
            addr_invitee,
        );

        let (size, _) = timeout(Duration::from_secs(5), socket_invitee.recv_from(&mut buffer))
            .await
            .unwrap()
            .unwrap();
        let reply_packet = welkin_msg::parse_packet(&buffer[..size]).unwrap();
        assert_eq!(reply_packet.id, msgs::ImprovedInstantMessage::ID);
    }
}
