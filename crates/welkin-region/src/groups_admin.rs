// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use welkin_caps::event_queue::EventQueue;
use welkin_msg::{self, Msg, msgs};
use welkin_net::server::ClientSession;
use welkin_services::remote::RemoteError;

use crate::groups_io;

#[derive(Debug)]
pub enum AdminOut {
    Alert(msgs::AlertMessage),
    EjectReply(msgs::EjectGroupMemberReply),
    AgentData(msgs::AgentDataUpdate),
    ToOther {
        agent: Uuid,
        drop: msgs::AgentDropGroup,
    },
}

pub fn is_admin_packet(id: welkin_msg::MsgId) -> bool {
    id == msgs::GroupRoleUpdate::ID
        || id == msgs::GroupRoleChanges::ID
        || id == msgs::GroupTitleUpdate::ID
        || id == msgs::SetGroupAcceptNotices::ID
        || id == msgs::EjectGroupMemberRequest::ID
}

async fn refresh_out(
    ctx: &groups_io::GroupsCtx,
    agent: Uuid,
    first: &str,
    last: &str,
    eq: Option<&EventQueue>,
    out: &mut Vec<AdminOut>,
) {
    if let Some(eq) = eq {
        out.push(AdminOut::AgentData(
            groups_io::refresh_viewer_group_state(ctx, agent, first, last, eq).await,
        ));
    } else {
        out.push(AdminOut::AgentData(groups_io::agent_data_update_for(
            agent,
            first,
            last,
            &groups_io::active_group(ctx, agent).await,
        )));
    }
}

pub async fn process_admin_packet(
    ctx: &groups_io::GroupsCtx,
    agent: Uuid,
    msg_id: welkin_msg::MsgId,
    body: &[u8],
    first: &str,
    last: &str,
    eq: Option<&EventQueue>,
) -> Option<Vec<AdminOut>> {
    if !is_admin_packet(msg_id) {
        return None;
    }

    match msg_id {
        msgs::GroupRoleUpdate::ID => {
            let m = msgs::GroupRoleUpdate::decode(body).ok()?;
            let group_id = m.agent_data.group_id;
            let mut out = Vec::new();

            for rd in m.role_data {
                match rd.update_type {
                    4 => {
                        let name = groups_io::bytes_to_string(&rd.name);
                        let description = groups_io::bytes_to_string(&rd.description);
                        let title = groups_io::bytes_to_string(&rd.title);
                        let role_id = Uuid::new_v4();
                        match ctx
                            .connector()
                            .put_role(
                                agent,
                                group_id,
                                role_id,
                                &name,
                                &description,
                                &title,
                                rd.powers,
                                "ADD",
                            )
                            .await
                        {
                            Ok(true) => {}
                            Ok(false) => out.push(AdminOut::Alert(groups_io::alert_message(
                                "Unable to create role: permission denied",
                            ))),
                            Err(e) => out.push(AdminOut::Alert(groups_io::alert_message(&format!(
                                "Unable to create role: {}",
                                e
                            )))),
                        }
                    }
                    5 => {
                        let _ = ctx
                            .connector()
                            .remove_role(agent, group_id, rd.role_id)
                            .await;
                    }
                    1 | 2 | 3 => {
                        let name = groups_io::bytes_to_string(&rd.name);
                        let description = groups_io::bytes_to_string(&rd.description);
                        let title = groups_io::bytes_to_string(&rd.title);
                        let ok = ctx
                            .connector()
                            .put_role(
                                agent,
                                group_id,
                                rd.role_id,
                                &name,
                                &description,
                                &title,
                                rd.powers,
                                "UPDATE",
                            )
                            .await;
                        if !matches!(ok, Ok(true)) {
                            out.push(AdminOut::Alert(groups_io::alert_message(
                                "Unable to update role.",
                            )));
                        }
                    }
                    0 => {}
                    _ => {}
                }
            }

            refresh_out(ctx, agent, first, last, eq, &mut out).await;
            Some(out)
        }
        msgs::GroupRoleChanges::ID => {
            let m = msgs::GroupRoleChanges::decode(body).ok()?;
            let group_id = m.agent_data.group_id;
            let mut out = Vec::new();

            for rc in m.role_change {
                match rc.change {
                    0 => {
                        let ok = ctx
                            .connector()
                            .agent_role(agent, rc.member_id, group_id, rc.role_id, "ADD")
                            .await;
                        if !matches!(ok, Ok(true)) {
                            out.push(AdminOut::Alert(groups_io::alert_message(
                                "Unable to change role membership.",
                            )));
                        }
                    }
                    1 => {
                        let ok = ctx
                            .connector()
                            .agent_role(agent, rc.member_id, group_id, rc.role_id, "DELETE")
                            .await;
                        if !matches!(ok, Ok(true)) {
                            out.push(AdminOut::Alert(groups_io::alert_message(
                                "Unable to change role membership.",
                            )));
                        }
                    }
                    _ => {
                        tracing::warn!("ignored group role change value {}", rc.change);
                    }
                }
            }

            refresh_out(ctx, agent, first, last, eq, &mut out).await;
            Some(out)
        }
        msgs::GroupTitleUpdate::ID => {
            let m = msgs::GroupTitleUpdate::decode(body).ok()?;
            let group_id = m.agent_data.group_id;
            let title_role_id = m.agent_data.title_role_id;
            let mut out = Vec::new();

            if ctx
                .connector()
                .set_active(agent, agent, group_id, "ROLE", Some(title_role_id))
                .await
                .is_err()
            {
                out.push(AdminOut::Alert(groups_io::alert_message(
                    "Unable to update group title.",
                )));
            }

            refresh_out(ctx, agent, first, last, eq, &mut out).await;
            Some(out)
        }
        msgs::SetGroupAcceptNotices::ID => {
            let m = msgs::SetGroupAcceptNotices::decode(body).ok()?;
            let group_id = m.data.group_id;
            let mut out = Vec::new();

            let _ = ctx
                .connector()
                .update_membership(agent, agent, group_id, m.data.accept_notices, m.new_data.list_in_profile)
                .await;

            refresh_out(ctx, agent, first, last, eq, &mut out).await;
            Some(out)
        }
        msgs::EjectGroupMemberRequest::ID => {
            let m = msgs::EjectGroupMemberRequest::decode(body).ok()?;
            let group_id = m.group_data.group_id;
            let mut out = Vec::new();

            for ed in m.eject_data {
                let ejectee = ed.ejectee_id;
                match ctx
                    .connector()
                    .remove_agent_from_group(agent, ejectee, group_id)
                    .await
                {
                    Ok(()) => {
                        out.push(AdminOut::EjectReply(msgs::EjectGroupMemberReply {
                            agent_data: msgs::eject_group_member_reply::AgentData { agent_id: agent },
                            group_data: msgs::eject_group_member_reply::GroupData { group_id },
                            eject_data: msgs::eject_group_member_reply::EjectData { success: true },
                        }));
                        out.push(AdminOut::ToOther {
                            agent: ejectee,
                            drop: msgs::AgentDropGroup {
                                agent_data: msgs::agent_drop_group::AgentData {
                                    agent_id: ejectee,
                                    group_id,
                                },
                            },
                        });
                    }
                    Err(RemoteError::Server(reason)) => {
                        out.push(AdminOut::EjectReply(msgs::EjectGroupMemberReply {
                            agent_data: msgs::eject_group_member_reply::AgentData { agent_id: agent },
                            group_data: msgs::eject_group_member_reply::GroupData { group_id },
                            eject_data: msgs::eject_group_member_reply::EjectData { success: false },
                        }));
                        out.push(AdminOut::Alert(groups_io::alert_message(&format!(
                            "Unable to eject: {}",
                            reason
                        ))));
                    }
                    Err(_) => {
                        out.push(AdminOut::EjectReply(msgs::EjectGroupMemberReply {
                            agent_data: msgs::eject_group_member_reply::AgentData { agent_id: agent },
                            group_data: msgs::eject_group_member_reply::GroupData { group_id },
                            eject_data: msgs::eject_group_member_reply::EjectData { success: false },
                        }));
                        out.push(AdminOut::Alert(groups_io::alert_message(
                            "The groups service is unavailable.",
                        )));
                    }
                }
            }

            refresh_out(ctx, agent, first, last, eq, &mut out).await;
            Some(out)
        }
        _ => None,
    }
}

pub fn spawn_admin_packet(
    ctx: groups_io::GroupsCtx,
    agent: Uuid,
    msg_id: welkin_msg::MsgId,
    body: Vec<u8>,
    first: String,
    last: String,
    event_queues: Arc<RwLock<HashMap<Uuid, Arc<EventQueue>>>>,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<tokio::net::UdpSocket>,
    addr: SocketAddr,
) {
    tokio::spawn(async move {
        let eq_opt = {
            let guard = event_queues.read().await;
            guard.get(&agent).cloned()
        };

        let outs = process_admin_packet(&ctx, agent, msg_id, &body, &first, &last, eq_opt.as_deref()).await;
        let Some(outs) = outs else {
            return;
        };

        for out in outs {
            match out {
                AdminOut::Alert(msg) => {
                    let bytes = {
                        let mut guard = client.lock().await;
                        let (bytes, _) = guard.circuit.send_msg(&msg, true);
                        bytes
                    };
                    let _ = socket.send_to(&bytes, addr).await;
                }
                AdminOut::EjectReply(msg) => {
                    let bytes = {
                        let mut guard = client.lock().await;
                        let (bytes, _) = guard.circuit.send_msg(&msg, true);
                        bytes
                    };
                    let _ = socket.send_to(&bytes, addr).await;
                }
                AdminOut::AgentData(msg) => {
                    let bytes = {
                        let mut guard = client.lock().await;
                        let (bytes, _) = guard.circuit.send_msg(&msg, true);
                        bytes
                    };
                    let _ = socket.send_to(&bytes, addr).await;
                }
                AdminOut::ToOther { agent: target, drop: drop_msg } => {
                    let found = {
                        let guard = clients.read().await;
                        let mut found = None;
                        for (peer, session) in guard.iter() {
                            let locked = session.lock().await;
                            if locked.agent_id == target {
                                found = Some((*peer, session.clone()));
                            }
                            drop(locked);
                            if found.is_some() {
                                break;
                            }
                        }
                        found
                    };

                    if let Some((peer, session)) = found {
                        let bytes = {
                            let mut guard = session.lock().await;
                            let (bytes, _) = guard.circuit.send_msg(&drop_msg, true);
                            bytes
                        };
                        let _ = socket.send_to(&bytes, peer).await;
                    }

                    let target_eq = {
                        let guard = event_queues.read().await;
                        guard.get(&target).cloned()
                    };
                    if let Some(eq) = target_eq {
                        groups_io::send_agent_group_data(&ctx, target, &eq).await;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use tokio::net::{TcpListener, UdpSocket};
    use uuid::Uuid;

    use welkin_services::remote::groups::GroupRecord;
    use welkin_msg::Writer;
    use welkin_services::groups::{router, GroupsServiceState};
    use welkin_data::groups::GroupsStore;

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

    struct TestDb {
        _pool: sqlx::Pool<sqlx::Any>,
        path: String,
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    async fn setup_service() -> (groups_io::GroupsCtx, TestDb) {
        sqlx::any::install_default_drivers();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = format!("welkin-region-groups-admin-{}-{}.sqlite", std::process::id(), n);
        let url = format!("sqlite:{}?mode=rwc", path);
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&url)
            .await
            .unwrap();
        for ddl in DDLS {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = GroupsServiceState {
            store: Arc::new(GroupsStore::new(pool.clone())),
        };
        let app = router(state);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let ctx = groups_io::GroupsCtx::new(&format!("http://{}", addr));
        (ctx, TestDb { _pool: pool, path })
    }

    fn make_group_record(founder: Uuid) -> GroupRecord {
        GroupRecord {
            group_id: Uuid::nil(),
            group_name: "TestGroup".to_string(),
            charter: "".to_string(),
            insignia_id: Uuid::nil(),
            founder_id: founder,
            membership_fee: 0,
            open_enrollment: true,
            show_in_list: false,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        }
    }

    fn encode_msg<M: Msg>(msg: &M) -> Vec<u8> {
        let mut w = Writer::new();
        msg.encode_body(&mut w);
        w.buf
    }

    #[tokio::test]
    async fn role_create_update_delete() {
        let (ctx, _db) = setup_service().await;
        let founder = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(founder, &make_group_record(founder))
            .await
            .unwrap();

        // create
        let msg = msgs::GroupRoleUpdate {
            agent_data: msgs::group_role_update::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_data: vec![msgs::group_role_update::RoleData {
                role_id: Uuid::nil(),
                name: b"TestRole".to_vec(),
                description: b"".to_vec(),
                title: b"Tester".to_vec(),
                powers: 0,
                update_type: 4,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, founder, msgs::GroupRoleUpdate::ID, &body, "F", "L", None)
            .await
            .unwrap();
        assert!(!outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));

        let roles = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap();
        let created = roles.iter().find(|r| r.name == "TestRole").unwrap().clone();
        let role_id = created.role_id;

        // update
        let msg = msgs::GroupRoleUpdate {
            agent_data: msgs::group_role_update::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_data: vec![msgs::group_role_update::RoleData {
                role_id,
                name: b"Renamed".to_vec(),
                description: b"".to_vec(),
                title: b"Tester".to_vec(),
                powers: 0,
                update_type: 3,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, founder, msgs::GroupRoleUpdate::ID, &body, "F", "L", None)
            .await
            .unwrap();
        assert!(!outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));
        let roles = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap();
        assert!(roles.iter().any(|r| r.role_id == role_id && r.name == "Renamed"));

        // delete
        let msg = msgs::GroupRoleUpdate {
            agent_data: msgs::group_role_update::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_data: vec![msgs::group_role_update::RoleData {
                role_id,
                name: b"Renamed".to_vec(),
                description: b"".to_vec(),
                title: b"Tester".to_vec(),
                powers: 0,
                update_type: 5,
            }],
        };
        let body = encode_msg(&msg);
        let _ = process_admin_packet(&ctx, founder, msgs::GroupRoleUpdate::ID, &body, "F", "L", None)
            .await
            .unwrap();
        let roles = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap();
        assert!(!roles.iter().any(|r| r.role_id == role_id));

        // non-member create -> alert, no role
        let non_member = Uuid::new_v4();
        let before = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap()
            .len();
        let msg = msgs::GroupRoleUpdate {
            agent_data: msgs::group_role_update::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_data: vec![msgs::group_role_update::RoleData {
                role_id: Uuid::nil(),
                name: b"BadRole".to_vec(),
                description: b"".to_vec(),
                title: b"Bad".to_vec(),
                powers: 0,
                update_type: 4,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, non_member, msgs::GroupRoleUpdate::ID, &body, "N", "M", None)
            .await
            .unwrap();
        assert!(outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));
        let after = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap()
            .len();
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn role_changes_add_remove() {
        let (ctx, _db) = setup_service().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(founder, &make_group_record(founder))
            .await
            .unwrap();
        ctx.connector()
            .add_agent_to_group(founder, member, group.group_id, group.owner_role_id, "")
            .await
            .unwrap();

        // create a role
        let msg = msgs::GroupRoleUpdate {
            agent_data: msgs::group_role_update::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_data: vec![msgs::group_role_update::RoleData {
                role_id: Uuid::nil(),
                name: b"XRole".to_vec(),
                description: b"".to_vec(),
                title: b"X".to_vec(),
                powers: 0,
                update_type: 4,
            }],
        };
        let body = encode_msg(&msg);
        process_admin_packet(&ctx, founder, msgs::GroupRoleUpdate::ID, &body, "F", "L", None)
            .await
            .unwrap();
        let roles = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap();
        let rid = roles.iter().find(|r| r.name == "XRole").unwrap().role_id;

        // add member to role
        let msg = msgs::GroupRoleChanges {
            agent_data: msgs::group_role_changes::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_change: vec![msgs::group_role_changes::RoleChange {
                role_id: rid,
                member_id: member,
                change: 0,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, founder, msgs::GroupRoleChanges::ID, &body, "F", "L", None)
            .await
            .unwrap();
        assert!(!outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));
        let rms = ctx
            .connector()
            .get_role_members(founder, group.group_id)
            .await
            .unwrap();
        assert!(rms
            .iter()
            .any(|rm| rm.role_id == rid && rm.member_id == member));

        // remove
        let msg = msgs::GroupRoleChanges {
            agent_data: msgs::group_role_changes::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_change: vec![msgs::group_role_changes::RoleChange {
                role_id: rid,
                member_id: member,
                change: 1,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, founder, msgs::GroupRoleChanges::ID, &body, "F", "L", None)
            .await
            .unwrap();
        assert!(!outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));
        let rms = ctx
            .connector()
            .get_role_members(founder, group.group_id)
            .await
            .unwrap();
        assert!(!rms
            .iter()
            .any(|rm| rm.role_id == rid && rm.member_id == member));
    }

    #[tokio::test]
    async fn title_update_sets_active_role() {
        let (ctx, _db) = setup_service().await;
        let agent = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(agent, &make_group_record(agent))
            .await
            .unwrap();

        let msg = msgs::GroupTitleUpdate {
            agent_data: msgs::group_title_update::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
                title_role_id: group.owner_role_id,
            },
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, agent, msgs::GroupTitleUpdate::ID, &body, "F", "L", None)
            .await
            .unwrap();
        assert!(!outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));

        let memberships = ctx
            .connector()
            .get_membership(agent, Some(group.group_id))
            .await
            .unwrap();
        let m = memberships
            .iter()
            .find(|x| x.group_id == group.group_id)
            .unwrap();
        assert_eq!(m.active_role, group.owner_role_id);
    }

    #[tokio::test]
    async fn accept_notices_reflected() {
        let (ctx, _db) = setup_service().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(founder, &make_group_record(founder))
            .await
            .unwrap();
        ctx.connector()
            .add_agent_to_group(founder, member, group.group_id, group.owner_role_id, "")
            .await
            .unwrap();

        let msg = msgs::SetGroupAcceptNotices {
            agent_data: msgs::set_group_accept_notices::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
            },
            data: msgs::set_group_accept_notices::Data {
                group_id: group.group_id,
                accept_notices: false,
            },
            new_data: msgs::set_group_accept_notices::NewData {
                list_in_profile: false,
            },
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, member, msgs::SetGroupAcceptNotices::ID, &body, "M", "B", None)
            .await
            .unwrap();
        let _ = outs;

        let members = ctx
            .connector()
            .get_group_members(founder, group.group_id)
            .await
            .unwrap();
        let mr = members.iter().find(|m| m.agent_id == member).unwrap();
        assert!(!mr.accept_notices);
        assert!(!mr.list_in_profile);
    }

    #[tokio::test]
    async fn eject_behavior() {
        let (ctx, _db) = setup_service().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(founder, &make_group_record(founder))
            .await
            .unwrap();
        ctx.connector()
            .add_agent_to_group(founder, member, group.group_id, group.owner_role_id, "")
            .await
            .unwrap();

        // founder ejects
        let msg = msgs::EjectGroupMemberRequest {
            agent_data: msgs::eject_group_member_request::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
            },
            group_data: msgs::eject_group_member_request::GroupData {
                group_id: group.group_id,
            },
            eject_data: vec![msgs::eject_group_member_request::EjectData {
                ejectee_id: member,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, founder, msgs::EjectGroupMemberRequest::ID, &body, "F", "L", None)
            .await
            .unwrap();
        assert!(outs
            .iter()
            .any(|o| matches!(o, AdminOut::EjectReply(r) if r.eject_data.success)));
        assert!(outs
            .iter()
            .any(|o| matches!(o, AdminOut::ToOther { agent, .. } if *agent == member)));

        let members = ctx
            .connector()
            .get_group_members(founder, group.group_id)
            .await
            .unwrap();
        assert!(!members.iter().any(|m| m.agent_id == member));

        // member (no power) tries to eject founder
        let msg = msgs::EjectGroupMemberRequest {
            agent_data: msgs::eject_group_member_request::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
            },
            group_data: msgs::eject_group_member_request::GroupData {
                group_id: group.group_id,
            },
            eject_data: vec![msgs::eject_group_member_request::EjectData {
                ejectee_id: founder,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, member, msgs::EjectGroupMemberRequest::ID, &body, "M", "B", None)
            .await
            .unwrap();
        assert!(outs
            .iter()
            .any(|o| matches!(o, AdminOut::EjectReply(r) if !r.eject_data.success)));
        assert!(outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));
    }

    #[tokio::test]
    async fn spoof_uses_authenticated_agent() {
        let (ctx, _db) = setup_service().await;
        let founder = Uuid::new_v4();
        let non_member = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(founder, &make_group_record(founder))
            .await
            .unwrap();

        // spoof founder id in GroupRoleUpdate
        let before = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap()
            .len();
        let msg = msgs::GroupRoleUpdate {
            agent_data: msgs::group_role_update::AgentData {
                agent_id: founder, // spoofed
                session_id: Uuid::new_v4(),
                group_id: group.group_id,
            },
            role_data: vec![msgs::group_role_update::RoleData {
                role_id: Uuid::nil(),
                name: b"SpoofRole".to_vec(),
                description: b"".to_vec(),
                title: b"Spoof".to_vec(),
                powers: 0,
                update_type: 4,
            }],
        };
        let body = encode_msg(&msg);
        let outs = process_admin_packet(&ctx, non_member, msgs::GroupRoleUpdate::ID, &body, "N", "M", None)
            .await
            .unwrap();
        assert!(outs.iter().any(|o| matches!(o, AdminOut::Alert(_))));
        let after = ctx
            .connector()
            .get_group_roles(founder, group.group_id)
            .await
            .unwrap()
            .len();
        assert_eq!(before, after);

        // spoof founder id in SetGroupAcceptNotices
        let msg = msgs::SetGroupAcceptNotices {
            agent_data: msgs::set_group_accept_notices::AgentData {
                agent_id: founder, // spoofed
                session_id: Uuid::new_v4(),
            },
            data: msgs::set_group_accept_notices::Data {
                group_id: group.group_id,
                accept_notices: false,
            },
            new_data: msgs::set_group_accept_notices::NewData {
                list_in_profile: false,
            },
        };
        let body = encode_msg(&msg);
        let _ = process_admin_packet(&ctx, non_member, msgs::SetGroupAcceptNotices::ID, &body, "N", "M", None)
            .await;
    }

    #[tokio::test]
    async fn non_admin_or_truncated() {
        let (ctx, _db) = setup_service().await;
        let agent = Uuid::new_v4();
        // non-admin id
        let out = process_admin_packet(&ctx, agent, msgs::AlertMessage::ID, &[], "F", "L", None).await;
        assert!(out.is_none());
        // truncated body for an admin id
        let out = process_admin_packet(&ctx, agent, msgs::GroupRoleUpdate::ID, &[], "F", "L", None).await;
        // An empty body decodes as an empty request: nothing to apply, nothing but a refresh.
        assert!(out.map_or(true, |outs| outs.iter().all(|o| matches!(o, AdminOut::AgentData(_)))));
    }

    #[tokio::test]
    async fn spawn_admin_packet_end_to_end() {
        let (ctx, _db) = setup_service().await;
        let founder = Uuid::new_v4();
        let member = Uuid::new_v4();
        let group = ctx
            .connector()
            .create_group(founder, &make_group_record(founder))
            .await
            .unwrap();
        ctx.connector()
            .add_agent_to_group(founder, member, group.group_id, group.owner_role_id, "")
            .await
            .unwrap();

        let viewer_founder = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let viewer_member = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let addr_founder = viewer_founder.local_addr().unwrap();
        let addr_member = viewer_member.local_addr().unwrap();

        let server_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());

        let mut eqs = HashMap::new();
        eqs.insert(founder, Arc::new(EventQueue::new()));
        eqs.insert(member, Arc::new(EventQueue::new()));
        let event_queues = Arc::new(RwLock::new(eqs));

        let session_founder = Arc::new(Mutex::new(ClientSession::new(
            addr_founder,
            0,
            founder,
            Uuid::new_v4(),
        )));
        let session_member = Arc::new(Mutex::new(ClientSession::new(
            addr_member,
            0,
            member,
            Uuid::new_v4(),
        )));
        let mut clients = HashMap::new();
        clients.insert(addr_founder, session_founder.clone());
        clients.insert(addr_member, session_member.clone());
        let clients = Arc::new(RwLock::new(clients));

        let msg = msgs::EjectGroupMemberRequest {
            agent_data: msgs::eject_group_member_request::AgentData {
                agent_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
            },
            group_data: msgs::eject_group_member_request::GroupData {
                group_id: group.group_id,
            },
            eject_data: vec![msgs::eject_group_member_request::EjectData {
                ejectee_id: member,
            }],
        };
        let body = encode_msg(&msg);

        spawn_admin_packet(
            ctx,
            founder,
            msgs::EjectGroupMemberRequest::ID,
            body,
            "Founder".to_string(),
            "Resident".to_string(),
            event_queues,
            clients,
            session_founder,
            server_socket,
            addr_founder,
        );

        // member receives AgentDropGroup
        let mut buf = vec![0u8; 65535];
        let received_member = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            viewer_member.recv_from(&mut buf),
        )
        .await;
        assert!(received_member.is_ok());
        let (n, _) = received_member.unwrap().unwrap();
        let pkt = welkin_msg::parse_packet(&buf[..n]).unwrap();
        assert_eq!(pkt.id, msgs::AgentDropGroup::ID);

        // founder receives EjectGroupMemberReply
        let mut buf2 = vec![0u8; 65535];
        let received_founder = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            viewer_founder.recv_from(&mut buf2),
        )
        .await;
        assert!(received_founder.is_ok());
        let (n2, _) = received_founder.unwrap().unwrap();
        let pkt2 = welkin_msg::parse_packet(&buf2[..n2]).unwrap();
        assert_eq!(pkt2.id, msgs::EjectGroupMemberReply::ID);
    }
}
