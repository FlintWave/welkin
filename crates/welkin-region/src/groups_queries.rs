// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;
use welkin_msg::Msg;

use tracing::warn;
use uuid::Uuid;

use crate::groups_io::{GroupsCtx, name_bytes};
use welkin_msg::msgs;

#[derive(Debug)]
pub enum QueryOut {
    Profile(msgs::GroupProfileReply),
    Members(msgs::GroupMembersReply),
    RoleData(msgs::GroupRoleDataReply),
    Titles(msgs::GroupTitlesReply),
    RoleMembers(msgs::GroupRoleMembersReply),
}

pub async fn process_query_packet(
    ctx: &GroupsCtx,
    agent: Uuid,
    msg_id: welkin_msg::MsgId,
    body: &[u8],
) -> Option<Vec<QueryOut>> {
    if msg_id == msgs::GroupProfileRequest::ID {
        let req = msgs::GroupProfileRequest::decode(body).ok()?;
        let group_id = req.group_data.group_id;
        if group_id == Uuid::nil() {
            return Some(vec![]);
        }

        let group = match ctx.connector().get_group(agent, Some(group_id), None).await {
            Ok(Some(group)) => group,
            _ => return Some(vec![]),
        };

        let memberships = match ctx.connector().get_membership(agent, Some(group_id)).await {
            Ok(records) => records,
            Err(err) => {
                warn!(error = %err, "failed to get group membership");
                return Some(vec![]);
            }
        };

        let (member_title, powers_mask) = memberships
            .first()
            .map(|record| (name_bytes(&record.group_title), record.group_powers))
            .unwrap_or_else(|| (name_bytes(""), 0));

        let reply = msgs::GroupProfileReply {
            agent_data: msgs::group_profile_reply::AgentData {
                agent_id: agent,
            },
            group_data: msgs::group_profile_reply::GroupData {
                group_id,
                name: name_bytes(&group.group_name),
                charter: bytes_nul(&group.charter, 1023),
                show_in_list: group.show_in_list,
                member_title,
                powers_mask,
                insignia_id: group.insignia_id,
                founder_id: group.founder_id,
                membership_fee: group.membership_fee,
                open_enrollment: group.open_enrollment,
                money: 0,
                group_membership_count: group.member_count as i32,
                group_roles_count: group.role_count as i32,
                allow_publish: group.allow_publish,
                mature_publish: group.mature_publish,
                owner_role: group.owner_role_id,
            },
        };

        Some(vec![QueryOut::Profile(reply)])
    } else if msg_id == msgs::GroupMembersRequest::ID {
        let req = msgs::GroupMembersRequest::decode(body).ok()?;
        let group_id = req.group_data.group_id;
        let request_id = req.group_data.request_id;

        if !is_member(ctx, agent, group_id).await {
            return Some(vec![]);
        }

        let members = match ctx.connector().get_group_members(agent, group_id).await {
            Ok(members) => members,
            Err(err) => {
                warn!(error = %err, "failed to get group members");
                return Some(vec![]);
            }
        };

        if members.is_empty() {
            return Some(vec![]);
        }

        let member_count = members.len() as i32;
        let mut out = Vec::new();
        for chunk in members.chunks(25) {
            let reply = msgs::GroupMembersReply {
                agent_data: msgs::group_members_reply::AgentData {
                    agent_id: agent,
                },
                group_data: msgs::group_members_reply::GroupData {
                    group_id,
                    request_id,
                    member_count,
                },
                member_data: chunk
                    .iter()
                    .map(|member| msgs::group_members_reply::MemberData {
                        agent_id: member.agent_id,
                        contribution: member.contribution,
                        online_status: name_bytes(""),
                        agent_powers: member.agent_powers,
                        title: name_bytes(&member.title),
                        is_owner: member.is_owner,
                    })
                    .collect(),
            };
            out.push(QueryOut::Members(reply));
        }

        Some(out)
    } else if msg_id == msgs::GroupRoleDataRequest::ID {
        let req = msgs::GroupRoleDataRequest::decode(body).ok()?;
        let group_id = req.group_data.group_id;
        let request_id = req.group_data.request_id;

        if !is_member(ctx, agent, group_id).await {
            return Some(vec![]);
        }

        let roles = match ctx.connector().get_group_roles(agent, group_id).await {
            Ok(roles) => roles,
            Err(err) => {
                warn!(error = %err, "failed to get group roles");
                return Some(vec![]);
            }
        };

        let reply = msgs::GroupRoleDataReply {
            agent_data: msgs::group_role_data_reply::AgentData {
                agent_id: agent,
            },
            group_data: msgs::group_role_data_reply::GroupData {
                group_id,
                request_id,
                role_count: roles.len() as i32,
            },
            role_data: roles
                .iter()
                .map(|role| msgs::group_role_data_reply::RoleData {
                    role_id: role.role_id,
                    name: name_bytes(&role.name),
                    title: name_bytes(&role.title),
                    description: bytes_nul(&role.description, 1023),
                    powers: role.powers,
                    members: role.members as u32,
                })
                .collect(),
        };

        Some(vec![QueryOut::RoleData(reply)])
    } else if msg_id == msgs::GroupTitlesRequest::ID {
        let req = msgs::GroupTitlesRequest::decode(body).ok()?;
        let group_id = req.agent_data.group_id;
        let request_id = req.agent_data.request_id;

        if !is_member(ctx, agent, group_id).await {
            return Some(vec![]);
        }

        let roles = match ctx.connector().get_agent_roles(agent, agent, group_id).await {
            Ok(roles) => roles,
            Err(err) => {
                warn!(error = %err, "failed to get agent roles");
                return Some(vec![]);
            }
        };

        let memberships = match ctx.connector().get_membership(agent, Some(group_id)).await {
            Ok(records) => records,
            Err(err) => {
                warn!(error = %err, "failed to get group membership");
                return Some(vec![]);
            }
        };
        let active_role = memberships
            .first()
            .map(|record| record.active_role)
            .unwrap_or_else(Uuid::nil);

        let reply = msgs::GroupTitlesReply {
            agent_data: msgs::group_titles_reply::AgentData {
                agent_id: agent,
                group_id,
                request_id,
            },
            group_data: roles
                .iter()
                .map(|role| msgs::group_titles_reply::GroupData {
                    title: name_bytes(&role.name),
                    role_id: role.role_id,
                    selected: role.role_id == active_role,
                })
                .collect(),
        };

        Some(vec![QueryOut::Titles(reply)])
    } else if msg_id == msgs::GroupRoleMembersRequest::ID {
        let req = msgs::GroupRoleMembersRequest::decode(body).ok()?;
        let group_id = req.group_data.group_id;
        let request_id = req.group_data.request_id;

        if !is_member(ctx, agent, group_id).await {
            return Some(vec![]);
        }

        let pairs = match ctx.connector().get_role_members(agent, group_id).await {
            Ok(pairs) => pairs,
            Err(err) => {
                warn!(error = %err, "failed to get role members");
                return Some(vec![]);
            }
        };

        if pairs.is_empty() {
            return Some(vec![]);
        }

        let total_pairs = pairs.len() as u32;
        let mut out = Vec::new();
        for chunk in pairs.chunks(32) {
            let reply = msgs::GroupRoleMembersReply {
                agent_data: msgs::group_role_members_reply::AgentData {
                    agent_id: agent,
                    group_id,
                    request_id,
                    total_pairs,
                },
                member_data: chunk
                    .iter()
                    .map(|pair| msgs::group_role_members_reply::MemberData {
                        role_id: pair.role_id,
                        member_id: pair.member_id,
                    })
                    .collect(),
            };
            out.push(QueryOut::RoleMembers(reply));
        }

        Some(out)
    } else {
        None
    }
}

pub async fn send_query_out(
    session: &mut welkin_net::server::ClientSession,
    socket: &tokio::net::UdpSocket,
    addr: std::net::SocketAddr,
    out: QueryOut,
) {
    match out {
        QueryOut::Profile(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        QueryOut::Members(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        QueryOut::RoleData(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        QueryOut::Titles(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
        QueryOut::RoleMembers(msg) => {
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            let _ = socket.send_to(&bytes, addr).await;
        }
    }
}

async fn is_member(ctx: &GroupsCtx, agent: Uuid, group_id: Uuid) -> bool {
    match ctx.connector().get_membership(agent, Some(group_id)).await {
        Ok(records) => !records.is_empty(),
        Err(err) => {
            warn!(error = %err, "get_membership failed");
            false
        }
    }
}

fn bytes_nul(s: &str, max: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for ch in s.chars() {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        if bytes.len() + encoded.len() > max {
            break;
        }
        bytes.extend_from_slice(encoded.as_bytes());
    }
    bytes.push(0);
    bytes
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
    use welkin_llsd::{Llsd, Map};
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
            "welkin-region-groups-queries-{}-{}.sqlite",
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
    async fn profile_nonmember_and_member() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let founder = Uuid::new_v4();
        let group = create_test_group(&ctx, founder, "ProfileGroup").await;

        let outsider = Uuid::new_v4();
        for requester in [outsider, founder] {
            let req = msgs::GroupProfileRequest {
                agent_data: msgs::group_profile_request::AgentData {
                    agent_id: requester,
                    session_id: Uuid::nil(),
                },
                group_data: msgs::group_profile_request::GroupData {
                    group_id: group.group_id,
                },
            };
            let mut w = welkin_msg::Writer::new();
            req.encode_body(&mut w);
            let out = process_query_packet(&ctx, requester, msgs::GroupProfileRequest::ID, &w.buf).await;
            assert!(out.is_some());
            let out = out.unwrap();
            assert_eq!(out.len(), 1);
            if requester == outsider {
                if let QueryOut::Profile(profile) = &out[0] {
                    assert_eq!(profile.group_data.member_title, name_bytes(""));
                    assert_eq!(profile.group_data.powers_mask, 0);
                } else {
                    panic!("expected profile");
                }
            }
        }

        let unknown = Uuid::new_v4();
        let req = msgs::GroupProfileRequest {
            agent_data: msgs::group_profile_request::AgentData {
                agent_id: founder,
                session_id: Uuid::nil(),
            },
            group_data: msgs::group_profile_request::GroupData {
                group_id: unknown,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, founder, msgs::GroupProfileRequest::ID, &w.buf).await;
        match out {
            Some(v) => assert!(v.is_empty()),
            _ => panic!("expected empty profile reply"),
        }
    }

    #[tokio::test]
    async fn members_chunking_and_security() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let founder = Uuid::new_v4();
        let group = create_test_group(&ctx, founder, "MembersGroup").await;

        for _ in 0..30 {
            let agent = Uuid::new_v4();
            let _ = ctx
                .connector()
                .add_agent_to_group(founder, agent, group.group_id, Uuid::nil(), "")
                .await;
        }

        let request_id = Uuid::new_v4();
        let req = msgs::GroupMembersRequest {
            agent_data: msgs::group_members_request::AgentData {
                agent_id: founder,
                session_id: Uuid::nil(),
            },
            group_data: msgs::group_members_request::GroupData {
                group_id: group.group_id,
                request_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, founder, msgs::GroupMembersRequest::ID, &w.buf)
            .await
            .expect("members result");
        assert!(!out.is_empty());
        let mut total = 0i32;
        let mut sizes = Vec::new();
        for query_out in &out {
            if let QueryOut::Members(reply) = query_out {
                sizes.push(reply.member_data.len());
                if total == 0 {
                    total = reply.group_data.member_count;
                }
                assert_eq!(reply.group_data.member_count, total);
                assert_eq!(reply.group_data.request_id, request_id);
            } else {
                panic!("expected members reply");
            }
        }
        let summed: usize = sizes.iter().sum();
        assert_eq!(summed, total as usize);
        assert!(sizes.len() >= 2);

        let outsider = Uuid::new_v4();
        let req = msgs::GroupMembersRequest {
            agent_data: msgs::group_members_request::AgentData {
                agent_id: outsider,
                session_id: Uuid::nil(),
            },
            group_data: msgs::group_members_request::GroupData {
                group_id: group.group_id,
                request_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, outsider, msgs::GroupMembersRequest::ID, &w.buf).await;
        match out {
            Some(v) => assert!(v.is_empty()),
            _ => panic!("expected empty members reply"),
        }
    }

    #[tokio::test]
    async fn role_data_lists_default_roles() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let founder = Uuid::new_v4();
        let group = create_test_group(&ctx, founder, "RoleDataGroup").await;

        let request_id = Uuid::new_v4();
        let req = msgs::GroupRoleDataRequest {
            agent_data: msgs::group_role_data_request::AgentData {
                agent_id: founder,
                session_id: Uuid::nil(),
            },
            group_data: msgs::group_role_data_request::GroupData {
                group_id: group.group_id,
                request_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, founder, msgs::GroupRoleDataRequest::ID, &w.buf)
            .await
            .expect("role data result");
        assert_eq!(out.len(), 1);
        if let QueryOut::RoleData(reply) = &out[0] {
            assert!(reply.group_data.role_count >= 3);
            assert_eq!(reply.group_data.request_id, request_id);
            let names: Vec<String> = reply
                .role_data
                .iter()
                .map(|role| String::from_utf8_lossy(&role.name).to_string())
                .collect();
            for expected in ["Everyone", "Officers", "Owners"] {
                assert!(names.iter().any(|name| name.contains(expected)));
            }
        } else {
            panic!("expected role data reply");
        }
    }

    #[tokio::test]
    async fn titles_selected_role() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let founder = Uuid::new_v4();
        let group = create_test_group(&ctx, founder, "TitlesGroup").await;

        let request_id = Uuid::new_v4();
        let req = msgs::GroupTitlesRequest {
            agent_data: msgs::group_titles_request::AgentData {
                agent_id: founder,
                session_id: Uuid::nil(),
                group_id: group.group_id,
                request_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, founder, msgs::GroupTitlesRequest::ID, &w.buf)
            .await
            .expect("titles result");
        assert_eq!(out.len(), 1);
        if let QueryOut::Titles(reply) = &out[0] {
            assert_eq!(reply.agent_data.agent_id, founder);
            assert_eq!(reply.agent_data.group_id, group.group_id);
            assert_eq!(reply.agent_data.request_id, request_id);
            assert!(reply.group_data.iter().filter(|data| data.selected).count() >= 1);
        } else {
            panic!("expected titles reply");
        }
    }

    #[tokio::test]
    async fn role_members_single_chunk() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let founder = Uuid::new_v4();
        let group = create_test_group(&ctx, founder, "RoleMembersGroup").await;

        for _ in 0..30 {
            let agent = Uuid::new_v4();
            let _ = ctx
                .connector()
                .add_agent_to_group(founder, agent, group.group_id, Uuid::nil(), "")
                .await;
        }

        let request_id = Uuid::new_v4();
        let req = msgs::GroupRoleMembersRequest {
            agent_data: msgs::group_role_members_request::AgentData {
                agent_id: founder,
                session_id: Uuid::nil(),
            },
            group_data: msgs::group_role_members_request::GroupData {
                group_id: group.group_id,
                request_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, founder, msgs::GroupRoleMembersRequest::ID, &w.buf)
            .await
            .expect("role members result");
        assert!(!out.is_empty());
        let mut total_seen = 0usize;
        for query_out in &out {
            if let QueryOut::RoleMembers(reply) = query_out {
                assert!(reply.member_data.len() <= 32);
                assert_eq!(reply.agent_data.request_id, request_id);
                total_seen += reply.member_data.len();
            } else {
                panic!("expected role members reply");
            }
        }
        if let QueryOut::RoleMembers(first) = &out[0] {
            assert_eq!(first.agent_data.total_pairs as usize, total_seen);
        }
    }

    #[tokio::test]
    async fn authenticated_agent_overrides_packet_agent() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let founder = Uuid::new_v4();
        let group = create_test_group(&ctx, founder, "AuthGroup").await;

        let impostor = Uuid::new_v4();
        let req = msgs::GroupProfileRequest {
            agent_data: msgs::group_profile_request::AgentData {
                agent_id: impostor,
                session_id: Uuid::nil(),
            },
            group_data: msgs::group_profile_request::GroupData {
                group_id: group.group_id,
            },
        };
        let mut w = welkin_msg::Writer::new();
        req.encode_body(&mut w);
        let out = process_query_packet(&ctx, founder, msgs::GroupProfileRequest::ID, &w.buf).await
            .expect("authenticated profile result");
        assert_eq!(out.len(), 1);
        if let QueryOut::Profile(profile) = &out[0] {
            assert_eq!(profile.agent_data.agent_id, founder);
        } else {
            panic!("expected profile");
        }
    }

    #[tokio::test]
    async fn non_query_or_truncated_returns_none() {
        let (url, _guard) = start_service().await;
        let ctx = GroupsCtx::new(&url);
        let agent = Uuid::new_v4();

        let none = process_query_packet(&ctx, agent, msgs::UUIDGroupNameRequest::ID, &[]).await;
        assert!(none.is_none());

        let truncated = process_query_packet(&ctx, agent, msgs::GroupProfileRequest::ID, &[]).await;
        // An empty body decodes as a nil group id, which yields no reply.
        assert!(truncated.map_or(true, |out| out.is_empty()));
    }

    #[allow(dead_code)]
    fn _keep_imports(_q: EventQueue, _r: EventQueueResponse, _m: Map, _l: Llsd) {}
    #[allow(dead_code)]
    fn _keep_time(_d: Duration) {}
}
