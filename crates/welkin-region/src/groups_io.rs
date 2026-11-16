// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;

use uuid::Uuid;
use welkin_caps::event_queue::EventQueue;
use welkin_llsd::{Llsd, Map};
use welkin_msg::Msg;
use welkin_msg::msgs::{
    agent_data_update, agent_drop_group, create_group_reply, create_group_request, join_group_reply,
    leave_group_reply, update_group_info,
};
use welkin_msg::msgs;
use welkin_msg::msgs::alert_message as alert_message_msg;
use welkin_services::remote::RemoteError;
use welkin_services::remote::groups::{RemoteGroupsConnector, MembershipRecord, GroupRecord};

#[derive(Clone)]
pub struct GroupsCtx {
    connector: Arc<RemoteGroupsConnector>,
}

impl GroupsCtx {
    pub fn new(server_uri: &str) -> Self {
        Self {
            connector: Arc::new(RemoteGroupsConnector::new(server_uri)),
        }
    }

    pub fn connector(&self) -> &Arc<RemoteGroupsConnector> {
        &self.connector
    }
}

pub fn groups_server_uri(
    opensim_groups: Option<&str>,
    groups_service: Option<&str>,
    grid_server: Option<&str>,
) -> Option<String> {
    for candidate in [opensim_groups, groups_service, grid_server] {
        if let Some(value) = candidate {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActiveGroup {
    pub group_id: Uuid,
    pub title: String,
    pub powers: u64,
    pub name: String,
}

#[derive(Debug)]
pub enum GroupOut {
    CreateReply(msgs::CreateGroupReply),
    JoinReply(msgs::JoinGroupReply),
    LeaveReply(msgs::LeaveGroupReply),
    Drop(msgs::AgentDropGroup),
    Alert(msgs::AlertMessage),
    AgentData(msgs::AgentDataUpdate),
}

pub async fn active_group(ctx: &GroupsCtx, agent: Uuid) -> ActiveGroup {
    match ctx.connector.get_membership(agent, None).await {
        Ok(records) => {
            if let Some(record) = records.first() {
                ActiveGroup {
                    group_id: record.group_id,
                    title: record.group_title.clone(),
                    powers: record.group_powers,
                    name: record.group_name.clone(),
                }
            } else {
                ActiveGroup::default()
            }
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to fetch active group");
            ActiveGroup::default()
        }
    }
}

pub async fn agent_memberships(ctx: &GroupsCtx, agent: Uuid) -> Vec<MembershipRecord> {
    match ctx.connector.get_all_memberships(agent).await {
        Ok(records) => records,
        Err(err) => {
            tracing::warn!(error = %err, "failed to fetch agent memberships");
            Vec::new()
        }
    }
}

pub fn agent_group_data_event(agent: Uuid, memberships: &[MembershipRecord]) -> Llsd {
    let mut root = Map::new();

    let mut agent_data_map = Map::new();
    agent_data_map.insert("AgentID".to_string(), Llsd::Uuid(agent));
    root.insert(
        "AgentData".to_string(),
        Llsd::Array(vec![Llsd::Map(agent_data_map)]),
    );

    if memberships.is_empty() {
        root.insert("GroupData".to_string(), Llsd::Array(Vec::new()));
        root.insert("NewGroupData".to_string(), Llsd::Array(Vec::new()));
    } else {
        let mut group_data = Vec::with_capacity(memberships.len());
        let mut new_group_data = Vec::with_capacity(memberships.len());

        for membership in memberships {
            let mut group_map = Map::new();
            group_map.insert("GroupID".to_string(), Llsd::Uuid(membership.group_id));
            group_map.insert(
                "GroupPowers".to_string(),
                Llsd::Binary(membership.group_powers.to_be_bytes().to_vec()),
            );
            group_map.insert(
                "AcceptNotices".to_string(),
                Llsd::Boolean(membership.accept_notices),
            );
            group_map.insert(
                "GroupInsigniaID".to_string(),
                Llsd::Uuid(membership.group_picture),
            );
            group_map.insert(
                "Contribution".to_string(),
                Llsd::Integer(membership.contribution),
            );
            group_map.insert(
                "GroupName".to_string(),
                Llsd::String(membership.group_name.clone()),
            );
            group_data.push(Llsd::Map(group_map));

            let mut new_group_map = Map::new();
            new_group_map.insert(
                "ListInProfile".to_string(),
                Llsd::Boolean(membership.list_in_profile),
            );
            new_group_data.push(Llsd::Map(new_group_map));
        }

        root.insert("GroupData".to_string(), Llsd::Array(group_data));
        root.insert("NewGroupData".to_string(), Llsd::Array(new_group_data));
    }

    Llsd::Map(root)
}

pub async fn send_agent_group_data(ctx: &GroupsCtx, agent: Uuid, eq: &EventQueue) {
    let memberships = agent_memberships(ctx, agent).await;
    eq.enqueue(
        "AgentGroupDataUpdate",
        agent_group_data_event(agent, &memberships),
    )
    .await;
}

pub fn name_bytes(name: &str) -> Vec<u8> {
    let bytes = name.as_bytes();
    let mut end = bytes.len().min(254);
    while end > 0 && !name.is_char_boundary(end) {
        end -= 1;
    }

    let mut result = bytes[..end].to_vec();
    result.push(0);
    result
}

pub fn bytes_to_string(b: &[u8]) -> String {
    let end = b
        .iter()
        .rposition(|&byte| byte != 0)
        .map(|index| index + 1)
        .unwrap_or(0);
    String::from_utf8_lossy(&b[..end]).into_owned()
}

pub fn alert_message(text: &str) -> msgs::AlertMessage {
    msgs::AlertMessage {
        alert_data: alert_message_msg::AlertData {
            message: name_bytes(text),
        },
        alert_info: Vec::new(),
        agent_info: Vec::new(),
    }
}

pub fn agent_data_update_for(
    agent: Uuid,
    first: &str,
    last: &str,
    active: &ActiveGroup,
) -> msgs::AgentDataUpdate {
    msgs::AgentDataUpdate {
        agent_data: agent_data_update::AgentData {
            agent_id: agent,
            first_name: first.as_bytes().to_vec(),
            last_name: last.as_bytes().to_vec(),
            group_title: name_bytes(&active.title),
            active_group_id: active.group_id,
            group_powers: active.powers,
            group_name: name_bytes(&active.name),
        },
    }
}

pub async fn refresh_viewer_group_state(
    ctx: &GroupsCtx,
    agent: Uuid,
    first: &str,
    last: &str,
    eq: &EventQueue,
) -> msgs::AgentDataUpdate {
    send_agent_group_data(ctx, agent, eq).await;
    let active = active_group(ctx, agent).await;
    agent_data_update_for(agent, first, last, &active)
}

pub async fn process_group_packet(
    ctx: &GroupsCtx,
    agent: Uuid,
    msg_id: welkin_msg::MsgId,
    body: &[u8],
    first: &str,
    last: &str,
    eq: Option<&EventQueue>,
) -> Option<Vec<GroupOut>> {
    if msg_id == msgs::CreateGroupRequest::ID {
        let req = match msgs::CreateGroupRequest::decode(body) {
            Ok(req) => req,
            Err(err) => {
                tracing::warn!(error = %err, "failed to decode CreateGroupRequest");
                return None;
            }
        };
        let reply = handle_create_group(ctx, agent, &req.group_data).await;
        let success = reply.reply_data.success;
        let mut outs = vec![GroupOut::CreateReply(reply)];
        if success {
            if let Some(eq) = eq {
                let update = refresh_viewer_group_state(ctx, agent, first, last, eq).await;
                outs.push(GroupOut::AgentData(update));
            } else {
                let active = active_group(ctx, agent).await;
                outs.push(GroupOut::AgentData(agent_data_update_for(
                    agent, first, last, &active,
                )));
            }
        }
        Some(outs)
    } else if msg_id == msgs::JoinGroupRequest::ID {
        let req = match msgs::JoinGroupRequest::decode(body) {
            Ok(req) => req,
            Err(err) => {
                tracing::warn!(error = %err, "failed to decode JoinGroupRequest");
                return None;
            }
        };
        let (reply, alert) = handle_join_group(ctx, agent, req.group_data.group_id).await;
        let success = reply.group_data.success;
        let mut outs = vec![GroupOut::JoinReply(reply)];
        if let Some(text) = alert {
            outs.push(GroupOut::Alert(alert_message(&text)));
        }
        if success {
            if let Some(eq) = eq {
                let update = refresh_viewer_group_state(ctx, agent, first, last, eq).await;
                outs.push(GroupOut::AgentData(update));
            } else {
                let active = active_group(ctx, agent).await;
                outs.push(GroupOut::AgentData(agent_data_update_for(
                    agent, first, last, &active,
                )));
            }
        }
        Some(outs)
    } else if msg_id == msgs::LeaveGroupRequest::ID {
        let req = match msgs::LeaveGroupRequest::decode(body) {
            Ok(req) => req,
            Err(err) => {
                tracing::warn!(error = %err, "failed to decode LeaveGroupRequest");
                return None;
            }
        };
        let (reply, drop) = handle_leave_group(ctx, agent, req.group_data.group_id).await;
        let success = reply.group_data.success;
        let mut outs = vec![GroupOut::LeaveReply(reply)];
        if let Some(drop) = drop {
            outs.push(GroupOut::Drop(drop));
        }
        if success {
            if let Some(eq) = eq {
                let update = refresh_viewer_group_state(ctx, agent, first, last, eq).await;
                outs.push(GroupOut::AgentData(update));
            } else {
                let active = active_group(ctx, agent).await;
                outs.push(GroupOut::AgentData(agent_data_update_for(
                    agent, first, last, &active,
                )));
            }
        }
        Some(outs)
    } else if msg_id == msgs::UpdateGroupInfo::ID {
        let req = match msgs::UpdateGroupInfo::decode(body) {
            Ok(req) => req,
            Err(err) => {
                tracing::warn!(error = %err, "failed to decode UpdateGroupInfo");
                return None;
            }
        };
        let alert = handle_update_group_info(ctx, agent, &req.group_data).await;
        if let Some(text) = alert {
            Some(vec![GroupOut::Alert(alert_message(&text))])
        } else {
            let mut outs = Vec::new();
            if let Some(eq) = eq {
                let update = refresh_viewer_group_state(ctx, agent, first, last, eq).await;
                outs.push(GroupOut::AgentData(update));
            } else {
                let active = active_group(ctx, agent).await;
                outs.push(GroupOut::AgentData(agent_data_update_for(
                    agent, first, last, &active,
                )));
            }
            Some(outs)
        }
    } else if msg_id == msgs::ActivateGroup::ID {
        let req = match msgs::ActivateGroup::decode(body) {
            Ok(req) => req,
            Err(err) => {
                tracing::warn!(error = %err, "failed to decode ActivateGroup");
                return None;
            }
        };
        let activated = handle_activate_group(ctx, agent, req.agent_data.group_id).await;
        if activated {
            let mut outs = Vec::new();
            if let Some(eq) = eq {
                let update = refresh_viewer_group_state(ctx, agent, first, last, eq).await;
                outs.push(GroupOut::AgentData(update));
            } else {
                let active = active_group(ctx, agent).await;
                outs.push(GroupOut::AgentData(agent_data_update_for(
                    agent, first, last, &active,
                )));
            }
            Some(outs)
        } else {
            Some(vec![GroupOut::Alert(alert_message(
                "Unable to activate the group.",
            ))])
        }
    } else {
        None
    }
}

pub async fn send_group_out(
    session: &mut welkin_net::server::ClientSession,
    socket: &tokio::net::UdpSocket,
    addr: std::net::SocketAddr,
    out: GroupOut,
) {
    match out {
        GroupOut::CreateReply(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        GroupOut::JoinReply(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        GroupOut::LeaveReply(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        GroupOut::Drop(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        GroupOut::Alert(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        GroupOut::AgentData(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
    }
}

pub async fn handle_create_group(
    ctx: &GroupsCtx,
    agent: Uuid,
    data: &create_group_request::GroupData,
) -> msgs::CreateGroupReply {
    let name = bytes_to_string(&data.name);
    let charter = bytes_to_string(&data.charter);

    if name.is_empty() {
        return msgs::CreateGroupReply {
            agent_data: create_group_reply::AgentData { agent_id: agent },
            reply_data: create_group_reply::ReplyData {
                group_id: Uuid::nil(),
                success: false,
                message: name_bytes("Group name is required."),
            },
        };
    }

    match ctx.connector().get_group(agent, None, Some(&name)).await {
        Ok(Some(_)) => {
            return msgs::CreateGroupReply {
                agent_data: create_group_reply::AgentData { agent_id: agent },
                reply_data: create_group_reply::ReplyData {
                    group_id: Uuid::nil(),
                    success: false,
                    message: name_bytes("A group with the same name already exists."),
                },
            };
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to look up group by name");
            return msgs::CreateGroupReply {
                agent_data: create_group_reply::AgentData { agent_id: agent },
                reply_data: create_group_reply::ReplyData {
                    group_id: Uuid::nil(),
                    success: false,
                    message: name_bytes("The groups service is unavailable."),
                },
            };
        }
        Ok(None) => {}
    }

    let record = GroupRecord {
        group_id: Uuid::nil(),
        group_name: name,
        charter,
        insignia_id: data.insignia_id,
        founder_id: agent,
        membership_fee: data.membership_fee,
        open_enrollment: data.open_enrollment,
        show_in_list: data.show_in_list,
        allow_publish: data.allow_publish,
        mature_publish: data.mature_publish,
        owner_role_id: Uuid::nil(),
        member_count: 0,
        role_count: 0,
    };

    match ctx.connector().create_group(agent, &record).await {
        Ok(created) => msgs::CreateGroupReply {
            agent_data: create_group_reply::AgentData { agent_id: agent },
            reply_data: create_group_reply::ReplyData {
                group_id: created.group_id,
                success: true,
                message: name_bytes("Group created successfully"),
            },
        },
        Err(RemoteError::Server(reason)) => msgs::CreateGroupReply {
            agent_data: create_group_reply::AgentData { agent_id: agent },
            reply_data: create_group_reply::ReplyData {
                group_id: Uuid::nil(),
                success: false,
                message: name_bytes(&reason),
            },
        },
        Err(err) => {
            tracing::warn!(error = %err, "failed to create group");
            msgs::CreateGroupReply {
                agent_data: create_group_reply::AgentData { agent_id: agent },
                reply_data: create_group_reply::ReplyData {
                    group_id: Uuid::nil(),
                    success: false,
                    message: name_bytes("The groups service is unavailable."),
                },
            }
        }
    }
}

pub async fn handle_join_group(
    ctx: &GroupsCtx,
    agent: Uuid,
    group_id: Uuid,
) -> (msgs::JoinGroupReply, Option<String>) {
    let make_reply = |success: bool| msgs::JoinGroupReply {
        agent_data: join_group_reply::AgentData { agent_id: agent },
        group_data: join_group_reply::GroupData {
            group_id,
            success,
        },
    };

    match ctx.connector().get_group(agent, Some(group_id), None).await {
        Err(err) => {
            tracing::warn!(error = %err, "failed to fetch group for join");
            (make_reply(false), Some("The groups service is unavailable.".to_string()))
        }
        Ok(None) => (
            make_reply(false),
            Some("The group could not be found.".to_string()),
        ),
        Ok(Some(group)) => {
            if !group.open_enrollment {
                return (
                    make_reply(false),
                    Some("This group is not open for enrollment.".to_string()),
                );
            }

            match ctx
                .connector()
                .add_agent_to_group(agent, agent, group_id, Uuid::nil(), "")
                .await
            {
                Ok(_) => (make_reply(true), None),
                Err(RemoteError::Server(reason)) => (make_reply(false), Some(reason)),
                Err(err) => {
                    tracing::warn!(error = %err, "failed to join group");
                    (
                        make_reply(false),
                        Some("The groups service is unavailable.".to_string()),
                    )
                }
            }
        }
    }
}

pub async fn handle_leave_group(
    ctx: &GroupsCtx,
    agent: Uuid,
    group_id: Uuid,
) -> (msgs::LeaveGroupReply, Option<msgs::AgentDropGroup>) {
    match ctx
        .connector()
        .remove_agent_from_group(agent, agent, group_id)
        .await
    {
        Ok(()) => {
            let reply = msgs::LeaveGroupReply {
                agent_data: leave_group_reply::AgentData { agent_id: agent },
                group_data: leave_group_reply::GroupData {
                    group_id,
                    success: true,
                },
            };
            let drop = msgs::AgentDropGroup {
                agent_data: agent_drop_group::AgentData {
                    agent_id: agent,
                    group_id,
                },
            };
            (reply, Some(drop))
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to leave group");
            (
                msgs::LeaveGroupReply {
                    agent_data: leave_group_reply::AgentData { agent_id: agent },
                    group_data: leave_group_reply::GroupData {
                        group_id,
                        success: false,
                    },
                },
                None,
            )
        }
    }
}

pub async fn handle_update_group_info(
    ctx: &GroupsCtx,
    agent: Uuid,
    data: &update_group_info::GroupData,
) -> Option<String> {
    let record = GroupRecord {
        group_id: data.group_id,
        group_name: String::new(),
        charter: bytes_to_string(&data.charter),
        insignia_id: data.insignia_id,
        founder_id: Uuid::nil(),
        membership_fee: data.membership_fee,
        open_enrollment: data.open_enrollment,
        show_in_list: data.show_in_list,
        allow_publish: data.allow_publish,
        mature_publish: data.mature_publish,
        owner_role_id: Uuid::nil(),
        member_count: 0,
        role_count: 0,
    };

    match ctx.connector().update_group(agent, &record).await {
        Ok(Some(_)) => None,
        Ok(None) => Some("Unable to update the group (insufficient permissions or group not found).".to_string()),
        Err(RemoteError::Server(reason)) => Some(reason),
        Err(err) => {
            tracing::warn!(error = %err, "failed to update group");
            Some("The groups service is unavailable.".to_string())
        }
    }
}

pub async fn handle_activate_group(ctx: &GroupsCtx, agent: Uuid, group_id: Uuid) -> bool {
    match ctx
        .connector()
        .set_active(agent, agent, group_id, "GROUP", None)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) if group_id.is_nil() => true,
        Ok(None) => false,
        Err(err) => {
            tracing::warn!(error = %err, "failed to activate group");
            false
        }
    }
}

pub async fn group_names(ctx: &GroupsCtx, ids: &[Uuid]) -> Vec<(Uuid, String)> {
    let mut result = Vec::with_capacity(ids.len());

    for id in ids {
        if id.is_nil() {
            result.push((*id, "Unknown".to_string()));
            continue;
        }

        let name = match ctx.connector.get_group(Uuid::nil(), Some(*id), None).await {
            Ok(Some(group)) => group.group_name,
            Ok(None) => "Unknown".to_string(),
            Err(err) => {
                tracing::warn!(error = %err, "failed to fetch group name");
                "Unknown".to_string()
            }
        };
        result.push((*id, name));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::msgs::{join_group_request, leave_group_request};

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use tokio::time::Duration;
    use uuid::Uuid;
    use welkin_caps::event_queue::{EventQueue, EventQueueResponse};
    use welkin_data::groups::GroupsStore;
    use welkin_llsd::Llsd;
    use welkin_services::groups::{GroupsServiceState, router};
    use welkin_services::remote::groups::{GroupRecord, MembershipRecord};

    static SERVICE_COUNTER: AtomicU64 = AtomicU64::new(0);

    const DDL: &[&str] = &[
        "CREATE TABLE IF NOT EXISTS os_groups_groups (GroupID CHAR(36) PRIMARY KEY, Location VARCHAR(255) NOT NULL DEFAULT '', Name VARCHAR(255) NOT NULL DEFAULT '', Charter TEXT NOT NULL DEFAULT '', InsigniaID CHAR(36) NOT NULL DEFAULT '', FounderID CHAR(36) NOT NULL DEFAULT '', MembershipFee INTEGER NOT NULL DEFAULT 0, OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', ShowInList INTEGER NOT NULL DEFAULT 0, AllowPublish INTEGER NOT NULL DEFAULT 0, MaturePublish INTEGER NOT NULL DEFAULT 0, OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_membership (GroupID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, SelectedRoleID CHAR(36) NOT NULL DEFAULT '', Contribution INTEGER NOT NULL DEFAULT 0, ListInProfile INTEGER NOT NULL DEFAULT 1, AcceptNotices INTEGER NOT NULL DEFAULT 1, AccessToken CHAR(36) NOT NULL DEFAULT '', PRIMARY KEY (GroupID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_roles (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, Name VARCHAR(255) NOT NULL DEFAULT '', Description VARCHAR(255) NOT NULL DEFAULT '', Title VARCHAR(255) NOT NULL DEFAULT '', Powers BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (GroupID, RoleID))",
        "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, PRIMARY KEY (GroupID, RoleID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_invites (InviteID CHAR(36) PRIMARY KEY, GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, TMStamp INTEGER NOT NULL DEFAULT 0)",
        "CREATE TABLE IF NOT EXISTS os_groups_notices (GroupID CHAR(36) NOT NULL, NoticeID CHAR(36) PRIMARY KEY, TMStamp INTEGER NOT NULL DEFAULT 0, FromName VARCHAR(255) NOT NULL DEFAULT '', Subject VARCHAR(255) NOT NULL DEFAULT '', Message TEXT NOT NULL DEFAULT '', HasAttachment INTEGER NOT NULL DEFAULT 0, AttachmentType INTEGER NOT NULL DEFAULT 0, AttachmentName VARCHAR(128) NOT NULL DEFAULT '', AttachmentItemID CHAR(36) NOT NULL DEFAULT '', AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_principals (PrincipalID VARCHAR(255) PRIMARY KEY, ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
    ];

    struct ServiceGuard {
        path: std::path::PathBuf,
        handle: tokio::task::JoinHandle<()>,
    }

    impl Drop for ServiceGuard {
        fn drop(&mut self) {
            self.handle.abort();
            let _ = std::fs::remove_file(&self.path);
        }
    }

    async fn start_service() -> (String, ServiceGuard) {
        sqlx::any::install_default_drivers();
        let counter = SERVICE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "welkin-region-groups-{}-{}.sqlite",
            std::process::id(),
            counter
        ));

        let database_url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&database_url)
            .await
            .expect("connect sqlite pool");

        for ddl in DDL {
            sqlx::query(ddl)
                .execute(&pool)
                .await
                .expect("create table");
        }

        let store = Arc::new(GroupsStore::new(pool));
        let state = GroupsServiceState { store };
        let app = router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind groups service");
        let address = listener.local_addr().expect("local addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve groups service");
        });

        (
            format!("http://{}", address),
            ServiceGuard { path, handle },
        )
    }

    async fn create_test_group(ctx: &GroupsCtx, founder: Uuid, name: &str) -> GroupRecord {
        let group = GroupRecord {
            group_id: Uuid::new_v4(),
            group_name: name.to_string(),
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
        };

        ctx.connector()
            .create_group(founder, &group)
            .await
            .expect("create group")
    }

    #[tokio::test]
    async fn groups_server_uri_precedence_cases() {
        assert_eq!(
            groups_server_uri(None, Some(" http://groups "), Some("http://grid")),
            Some("http://groups".to_string())
        );
        assert_eq!(
            groups_server_uri(None, Some("   "), Some("http://grid")),
            Some("http://grid".to_string())
        );
        assert_eq!(groups_server_uri(None, None, Some("")), None);
        assert_eq!(groups_server_uri(None, Some(""), None), None);
        assert_eq!(
            groups_server_uri(Some("http://os"), Some("http://groups"), Some("http://grid")),
            Some("http://os".to_string())
        );
    }

    #[tokio::test]
    async fn agent_memberships_returns_founders_group() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();

        let created = create_test_group(&ctx, founder, "Founder Group").await;
        let memberships = agent_memberships(&ctx, founder).await;

        assert!(!memberships.is_empty());
        assert_eq!(memberships[0].group_id, created.group_id);
        assert_eq!(memberships[0].group_name, created.group_name);
    }

    #[tokio::test]
    async fn active_group_default_then_set_active() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();

        assert_eq!(active_group(&ctx, agent).await, ActiveGroup::default());

        let created = create_test_group(&ctx, agent, "Active Group").await;
        let _ = ctx
            .connector()
            .set_active(agent, agent, created.group_id, "GROUP", None)
            .await
            .expect("set active group");

        let active = active_group(&ctx, agent).await;
        assert_eq!(active.group_id, created.group_id);
        assert_eq!(active.name, created.group_name);
    }

    #[tokio::test]
    async fn agent_group_data_event_structure() {
        let agent = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let picture = Uuid::new_v4();
        let membership = MembershipRecord {
            group_id,
            group_name: "Test Membership".to_string(),
            group_picture: picture,
            group_title: "Title".to_string(),
            group_powers: 99,
            accept_notices: true,
            list_in_profile: false,
            contribution: 7,
            active_role: Uuid::new_v4(),
            active: true,
            open_enrollment: false,
            membership_fee: 0,
            charter: String::new(),
            allow_publish: false,
            mature_publish: false,
            show_in_list: true,
            founder_id: Uuid::new_v4(),
        };

        let event = agent_group_data_event(agent, &[membership]);
        match event {
            Llsd::Map(root) => {
                match root.get("AgentData") {
                    Some(Llsd::Array(agent_data)) => {
                        assert_eq!(agent_data.len(), 1);
                        match &agent_data[0] {
                            Llsd::Map(map) => {
                                assert_eq!(map.get("AgentID"), Some(&Llsd::Uuid(agent)));
                            }
                            _ => panic!("expected AgentData map"),
                        }
                    }
                    _ => panic!("expected AgentData array"),
                }

                match root.get("GroupData") {
                    Some(Llsd::Array(group_data)) => {
                        assert_eq!(group_data.len(), 1);
                        match &group_data[0] {
                            Llsd::Map(map) => {
                                assert_eq!(map.get("GroupID"), Some(&Llsd::Uuid(group_id)));
                                assert_eq!(
                                    map.get("GroupPowers"),
                                    Some(&Llsd::Binary(99u64.to_be_bytes().to_vec()))
                                );
                                assert_eq!(map.get("AcceptNotices"), Some(&Llsd::Boolean(true)));
                                assert_eq!(map.get("GroupInsigniaID"), Some(&Llsd::Uuid(picture)));
                                assert_eq!(map.get("Contribution"), Some(&Llsd::Integer(7)));
                                assert_eq!(
                                    map.get("GroupName"),
                                    Some(&Llsd::String("Test Membership".to_string()))
                                );
                            }
                            _ => panic!("expected GroupData map"),
                        }
                    }
                    _ => panic!("expected GroupData array"),
                }

                match root.get("NewGroupData") {
                    Some(Llsd::Array(new_group_data)) => {
                        assert_eq!(new_group_data.len(), 1);
                        match &new_group_data[0] {
                            Llsd::Map(map) => {
                                assert_eq!(map.get("ListInProfile"), Some(&Llsd::Boolean(false)));
                            }
                            _ => panic!("expected NewGroupData map"),
                        }
                    }
                    _ => panic!("expected NewGroupData array"),
                }
            }
            _ => panic!("expected root map"),
        }

        let empty_event = agent_group_data_event(agent, &[]);
        match empty_event {
            Llsd::Map(root) => {
                match root.get("GroupData") {
                    Some(Llsd::Array(items)) => assert!(items.is_empty()),
                    _ => panic!("expected empty GroupData array"),
                }
                match root.get("NewGroupData") {
                    Some(Llsd::Array(items)) => assert!(items.is_empty()),
                    _ => panic!("expected empty NewGroupData array"),
                }
            }
            _ => panic!("expected root map"),
        }
    }

    #[tokio::test]
    async fn send_agent_group_data_enqueues_event() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let created = create_test_group(&ctx, founder, "Queued Group").await;

        let queue = EventQueue::new();
        send_agent_group_data(&ctx, founder, &queue).await;

        match queue.poll(Duration::from_millis(200)).await {
            EventQueueResponse::Events { events, .. } => {
                let text = String::from_utf8_lossy(&events);
                assert!(text.contains("AgentGroupDataUpdate"));
                assert!(text.contains(&created.group_name));
            }
            _ => panic!("expected queued events"),
        }
    }

    #[tokio::test]
    async fn group_names_real_unknown_and_nil() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let created = create_test_group(&ctx, founder, "Real Named Group").await;

        let unknown = Uuid::new_v4();
        let pairs = group_names(&ctx, &[created.group_id, unknown, Uuid::nil()]).await;

        assert_eq!(pairs[0], (created.group_id, created.group_name));
        assert_eq!(pairs[1], (unknown, "Unknown".to_string()));
        assert_eq!(pairs[2], (Uuid::nil(), "Unknown".to_string()));
    }

    #[tokio::test]
    async fn name_bytes_truncates_and_terminates() {
        assert_eq!(name_bytes("abc"), b"abc\0".to_vec());

        let long = "a".repeat(300);
        let output = name_bytes(&long);
        assert_eq!(output.len(), 255);
        assert!(output[..254].iter().all(|byte| *byte == b'a'));
        assert_eq!(output[254], 0);

        let multibyte = "é".repeat(128);
        let output = name_bytes(&multibyte);
        assert_eq!(output.len(), 255);
        assert!(output[254] == 0);
        assert!(std::str::from_utf8(&output[..254]).is_ok());
    }

    #[tokio::test]
    async fn create_group_success_then_duplicate_fails() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();

        let data = create_group_request::GroupData {
            name: b"Alpha".to_vec(),
            charter: b"Alpha charter".to_vec(),
            open_enrollment: true,
            ..Default::default()
        };

        let reply = handle_create_group(&ctx, agent, &data).await;
        assert!(reply.reply_data.success);
        assert!(!reply.reply_data.group_id.is_nil());
        let group_id = reply.reply_data.group_id;

        let memberships = agent_memberships(&ctx, agent).await;
        assert!(memberships.iter().any(|m| m.group_id == group_id));

        let duplicate = handle_create_group(&ctx, agent, &data).await;
        assert!(!duplicate.reply_data.success);
        assert!(bytes_to_string(&duplicate.reply_data.message).contains("already exists"));

        let empty_data = create_group_request::GroupData::default();
        let empty_reply = handle_create_group(&ctx, agent, &empty_data).await;
        assert!(!empty_reply.reply_data.success);
    }

    #[tokio::test]
    async fn join_open_group_then_closed_group() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let second = Uuid::new_v4();

        let open_data = create_group_request::GroupData {
            name: b"Open Group".to_vec(),
            open_enrollment: true,
            ..Default::default()
        };
        let open_reply = handle_create_group(&ctx, founder, &open_data).await;
        assert!(open_reply.reply_data.success);
        let open_id = open_reply.reply_data.group_id;

        let (join_reply, alert) = handle_join_group(&ctx, second, open_id).await;
        assert!(join_reply.group_data.success);
        assert!(alert.is_none());
        let memberships = agent_memberships(&ctx, second).await;
        assert!(memberships.iter().any(|m| m.group_id == open_id));

        let closed_data = create_group_request::GroupData {
            name: b"Closed Group".to_vec(),
            open_enrollment: false,
            ..Default::default()
        };
        let closed_reply = handle_create_group(&ctx, founder, &closed_data).await;
        assert!(closed_reply.reply_data.success);
        let closed_id = closed_reply.reply_data.group_id;

        let (closed_join_reply, closed_alert) = handle_join_group(&ctx, second, closed_id).await;
        assert!(!closed_join_reply.group_data.success);
        assert!(closed_alert.as_deref().unwrap_or_default().contains("not open"));
    }

    #[tokio::test]
    async fn leave_group_returns_drop_packet() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let second = Uuid::new_v4();

        let open_data = create_group_request::GroupData {
            name: b"Leave Group".to_vec(),
            open_enrollment: true,
            ..Default::default()
        };
        let create_reply = handle_create_group(&ctx, founder, &open_data).await;
        assert!(create_reply.reply_data.success);
        let group_id = create_reply.reply_data.group_id;

        let (join_reply, join_alert) = handle_join_group(&ctx, second, group_id).await;
        assert!(join_reply.group_data.success);
        assert!(join_alert.is_none());

        let (leave_reply, drop) = handle_leave_group(&ctx, second, group_id).await;
        assert!(leave_reply.group_data.success);
        let drop = drop.expect("drop packet");
        assert_eq!(drop.agent_data.agent_id, second);
        assert_eq!(drop.agent_data.group_id, group_id);

        let memberships = agent_memberships(&ctx, second).await;
        assert!(!memberships.iter().any(|m| m.group_id == group_id));
    }

    #[tokio::test]
    async fn update_group_info_permissions() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let random = Uuid::new_v4();

        let create_data = create_group_request::GroupData {
            name: b"Update Group".to_vec(),
            ..Default::default()
        };
        let create_reply = handle_create_group(&ctx, founder, &create_data).await;
        assert!(create_reply.reply_data.success);
        let group_id = create_reply.reply_data.group_id;

        let update = update_group_info::GroupData {
            group_id,
            charter: b"Updated charter".to_vec(),
            ..Default::default()
        };
        let result = handle_update_group_info(&ctx, founder, &update).await;
        assert!(result.is_none());

        let group = ctx
            .connector()
            .get_group(founder, Some(group_id), None)
            .await
            .expect("get group")
            .expect("group exists");
        assert_eq!(group.charter, "Updated charter");

        let denied_update = update_group_info::GroupData {
            group_id,
            charter: b"Should not apply".to_vec(),
            ..Default::default()
        };
        let denied = handle_update_group_info(&ctx, random, &denied_update).await;
        assert!(denied.is_some());

        let group_after = ctx
            .connector()
            .get_group(founder, Some(group_id), None)
            .await
            .expect("get group")
            .expect("group exists");
        assert_eq!(group_after.charter, "Updated charter");
    }

    #[tokio::test]
    async fn activate_group_sets_active() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();

        let create_data = create_group_request::GroupData {
            name: b"Active Group".to_vec(),
            ..Default::default()
        };
        let create_reply = handle_create_group(&ctx, agent, &create_data).await;
        assert!(create_reply.reply_data.success);
        let group_id = create_reply.reply_data.group_id;

        let activated = handle_activate_group(&ctx, agent, group_id).await;
        assert!(activated);

        let active = active_group(&ctx, agent).await;
        assert_eq!(active.group_id, group_id);
    }

    #[tokio::test]
    async fn alert_message_and_agent_data_update_for() {
        let agent = Uuid::new_v4();

        let alert = alert_message("hello");
        assert_eq!(alert.alert_data.message, name_bytes("hello"));

        let active = ActiveGroup {
            group_id: Uuid::new_v4(),
            title: "Title".to_string(),
            powers: 42,
            name: "Group Name".to_string(),
        };
        let update = agent_data_update_for(agent, "First", "Last", &active);

        assert_eq!(update.agent_data.agent_id, agent);
        assert_eq!(update.agent_data.first_name, b"First".to_vec());
        assert_eq!(update.agent_data.last_name, b"Last".to_vec());
        assert_eq!(update.agent_data.group_name, name_bytes("Group Name"));
    }

    #[tokio::test]
    async fn refresh_viewer_group_state_enqueues_and_returns_active() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();

        let create_data = create_group_request::GroupData {
            name: b"Refresh Group".to_vec(),
            ..Default::default()
        };
        let create_reply = handle_create_group(&ctx, agent, &create_data).await;
        assert!(create_reply.reply_data.success);
        let group_id = create_reply.reply_data.group_id;

        assert!(handle_activate_group(&ctx, agent, group_id).await);

        let queue = EventQueue::new();
        let update = refresh_viewer_group_state(&ctx, agent, "A", "B", &queue).await;
        assert_eq!(update.agent_data.active_group_id, group_id);

        match queue.poll(Duration::from_millis(200)).await {
            EventQueueResponse::Events { events, .. } => {
                let text = String::from_utf8_lossy(&events);
                assert!(text.contains("AgentGroupDataUpdate"));
                assert!(text.contains("Refresh Group"));
            }
            _ => panic!("expected queued events"),
        }
    }

    #[tokio::test]
    async fn process_group_packet_create_success_with_event_queue() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();
        let queue = EventQueue::new();

        let msg = msgs::CreateGroupRequest {
            agent_data: create_group_request::AgentData { agent_id: agent, session_id: Uuid::nil() },
            group_data: create_group_request::GroupData {
                name: b"Packet Create Group".to_vec(),
                ..Default::default()
            },
        };
        let mut w = welkin_msg::Writer::new();
        welkin_msg::Msg::encode_body(&msg, &mut w);

        let outs = process_group_packet(
            &ctx,
            agent,
            msgs::CreateGroupRequest::ID,
            &w.buf,
            "First",
            "Last",
            Some(&queue),
        )
        .await
        .expect("process create packet");

        assert!(outs
            .iter()
            .any(|out| matches!(out, GroupOut::CreateReply(r) if r.reply_data.success)));

        let active = active_group(&ctx, agent).await;
        let agent_data = outs
            .iter()
            .find_map(|out| match out {
                GroupOut::AgentData(update) => Some(update),
                _ => None,
            })
            .expect("agent data output");

        assert_eq!(agent_data.agent_data.agent_id, agent);
        assert_eq!(agent_data.agent_data.first_name, b"First".to_vec());
        assert_eq!(agent_data.agent_data.last_name, b"Last".to_vec());
        assert_eq!(agent_data.agent_data.active_group_id, active.group_id);
        assert_eq!(agent_data.agent_data.group_powers, active.powers);
        assert_eq!(agent_data.agent_data.group_title, name_bytes(&active.title));
        assert_eq!(agent_data.agent_data.group_name, name_bytes(&active.name));
    }

    #[tokio::test]
    async fn process_group_packet_join_closed_group() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let joiner = Uuid::new_v4();

        let created = handle_create_group(
            &ctx,
            founder,
            &create_group_request::GroupData {
                name: b"Closed Packet Group".to_vec(),
                open_enrollment: false,
                ..Default::default()
            },
        )
        .await;
        assert!(created.reply_data.success);

        let msg = msgs::JoinGroupRequest {
            agent_data: join_group_request::AgentData { agent_id: joiner, session_id: Uuid::nil() },
            group_data: join_group_request::GroupData {
                group_id: created.reply_data.group_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        welkin_msg::Msg::encode_body(&msg, &mut w);

        let outs = process_group_packet(
            &ctx,
            joiner,
            msgs::JoinGroupRequest::ID,
            &w.buf,
            "First",
            "Last",
            None,
        )
        .await
        .expect("process join packet");

        assert!(outs
            .iter()
            .any(|out| matches!(out, GroupOut::JoinReply(r) if !r.group_data.success)));
        assert!(outs.iter().any(|out| matches!(out, GroupOut::Alert(_))));
    }

    #[tokio::test]
    async fn process_group_packet_leave_open_group() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();

        let created = handle_create_group(
            &ctx,
            founder,
            &create_group_request::GroupData {
                name: b"Leave Packet Group".to_vec(),
                open_enrollment: true,
                ..Default::default()
            },
        )
        .await;
        assert!(created.reply_data.success);
        let group_id = created.reply_data.group_id;

        let (joined, join_alert) = handle_join_group(&ctx, member, group_id).await;
        assert!(joined.group_data.success);
        assert!(join_alert.is_none());

        let msg = msgs::LeaveGroupRequest {
            agent_data: leave_group_request::AgentData { agent_id: member, session_id: Uuid::nil() },
            group_data: leave_group_request::GroupData { group_id },
        };
        let mut w = welkin_msg::Writer::new();
        welkin_msg::Msg::encode_body(&msg, &mut w);

        let outs = process_group_packet(
            &ctx,
            member,
            msgs::LeaveGroupRequest::ID,
            &w.buf,
            "First",
            "Last",
            None,
        )
        .await
        .expect("process leave packet");

        assert!(outs
            .iter()
            .any(|out| matches!(out, GroupOut::LeaveReply(r) if r.group_data.success)));
        assert!(outs.iter().any(|out| matches!(out, GroupOut::Drop(_))));
    }

    #[tokio::test]
    async fn process_group_packet_create_spoofed_agent() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let authenticated = Uuid::new_v4();
        let spoofed = Uuid::new_v4();

        let msg = msgs::CreateGroupRequest {
            agent_data: create_group_request::AgentData { agent_id: spoofed, session_id: Uuid::nil() },
            group_data: create_group_request::GroupData {
                name: b"Spoof Packet Group".to_vec(),
                ..Default::default()
            },
        };
        let mut w = welkin_msg::Writer::new();
        welkin_msg::Msg::encode_body(&msg, &mut w);

        let result = process_group_packet(
            &ctx,
            authenticated,
            msgs::CreateGroupRequest::ID,
            &w.buf,
            "First",
            "Last",
            None,
        )
        .await;
        assert!(result.is_some());

        let memberships = agent_memberships(&ctx, authenticated).await;
        assert!(memberships.iter().any(|m| m.group_name == "Spoof Packet Group"));
        assert!(agent_memberships(&ctx, spoofed).await.is_empty());
    }

    #[tokio::test]
    async fn process_group_packet_non_group_message() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();

        let result = process_group_packet(
            &ctx,
            agent,
            msgs::UUIDGroupNameRequest::ID,
            &[],
            "First",
            "Last",
            None,
        )
        .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn process_group_packet_truncated_body() {
        let (base, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&base);
        let agent = Uuid::new_v4();

        let msg = msgs::CreateGroupRequest {
            agent_data: create_group_request::AgentData { agent_id: agent, session_id: Uuid::nil() },
            group_data: create_group_request::GroupData {
                name: b"Truncated Packet Group".to_vec(),
                ..Default::default()
            },
        };
        let mut w = welkin_msg::Writer::new();
        welkin_msg::Msg::encode_body(&msg, &mut w);
        assert!(w.buf.len() > 5);

        let result = process_group_packet(
            &ctx,
            agent,
            msgs::CreateGroupRequest::ID,
            &w.buf[..5],
            "First",
            "Last",
            None,
        )
        .await;

        assert!(result.is_none());
    }
}
