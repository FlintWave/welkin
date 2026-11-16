// SPDX-License-Identifier: LGPL-3.0-or-later

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use uuid::Uuid;

use welkin_caps::event_queue::EventQueue;
use welkin_msg::{Msg, msgs, MsgId};
use welkin_net::server::ClientSession;

use crate::groups_io::*;
use crate::groups_queries;

pub fn is_group_packet(id: MsgId) -> bool {
    matches!(
        id,
        msgs::CreateGroupRequest::ID
            | msgs::JoinGroupRequest::ID
            | msgs::LeaveGroupRequest::ID
            | msgs::UpdateGroupInfo::ID
            | msgs::ActivateGroup::ID
            | msgs::GroupProfileRequest::ID
            | msgs::GroupMembersRequest::ID
            | msgs::GroupRoleDataRequest::ID
            | msgs::GroupTitlesRequest::ID
            | msgs::GroupRoleMembersRequest::ID
    )
}

pub fn spawn_group_packet(
    ctx: GroupsCtx,
    agent: Uuid,
    msg_id: MsgId,
    body: Vec<u8>,
    first: String,
    last: String,
    eq: Option<Arc<EventQueue>>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Some(outs) =
            process_group_packet(&ctx, agent, msg_id, &body, &first, &last, eq.as_deref()).await
        {
            let mut session = client.lock().await;
            for out in outs {
                send_group_out(&mut session, &socket, addr, out).await;
            }
        } else if let Some(outs) =
            groups_queries::process_query_packet(&ctx, agent, msg_id, &body).await
        {
            let mut session = client.lock().await;
            for out in outs {
                groups_queries::send_query_out(&mut session, &socket, addr, out).await;
            }
        }
    })
}

pub fn spawn_group_name_reply(
    ctx: GroupsCtx,
    ids: Vec<Uuid>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let names = group_names(&ctx, &ids).await;
        let blocks = names
            .into_iter()
            .map(|(id, name)| msgs::uuid_group_name_reply::UUIDNameBlock {
                id,
                group_name: name_bytes(&name),
            })
            .collect();
        let msg = msgs::UUIDGroupNameReply {
            uuid_name_block: blocks,
        };
        let mut session = client.lock().await;
        let (bytes, _) = session.circuit.send_msg(&msg, true);
        let _ = socket.send_to(&bytes, addr).await;
    })
}

pub fn spawn_agent_data_update(
    ctx: GroupsCtx,
    agent: Uuid,
    first: String,
    last: String,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let active = active_group(&ctx, agent).await;
        let msg = agent_data_update_for(agent, &first, &last, &active);
        let mut session = client.lock().await;
        let (bytes, _) = session.circuit.send_msg(&msg, true);
        let _ = socket.send_to(&bytes, addr).await;
    })
}

pub fn spawn_login_group_data(
    ctx: GroupsCtx,
    agent: Uuid,
    first: String,
    last: String,
    eq: Arc<EventQueue>,
    client: Arc<Mutex<ClientSession>>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let ticket = ctx.sequencer().ticket(agent);
        let (active, memberships) = tokio::join!(
            active_group(&ctx, agent),
            agent_memberships(&ctx, agent)
        );
        if !ctx.sequencer().try_publish(agent, ticket) {
            return;
        }
        eq.enqueue(
            "AgentGroupDataUpdate",
            agent_group_data_event(agent, &memberships),
        )
        .await;
        let msg = agent_data_update_for(agent, &first, &last, &active);
        let mut session = client.lock().await;
        let (bytes, _) = session.circuit.send_msg(&msg, true);
        let _ = socket.send_to(&bytes, addr).await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use tokio::net::{TcpListener, UdpSocket};
    use tokio::time::timeout;
    use uuid::Uuid;

    #[tokio::test]
    async fn is_group_packet_truth_table() {
        assert!(is_group_packet(msgs::CreateGroupRequest::ID));
        assert!(is_group_packet(msgs::JoinGroupRequest::ID));
        assert!(is_group_packet(msgs::LeaveGroupRequest::ID));
        assert!(is_group_packet(msgs::UpdateGroupInfo::ID));
        assert!(is_group_packet(msgs::ActivateGroup::ID));
        assert!(is_group_packet(msgs::GroupProfileRequest::ID));
        assert!(is_group_packet(msgs::GroupMembersRequest::ID));
        assert!(is_group_packet(msgs::GroupRoleDataRequest::ID));
        assert!(is_group_packet(msgs::GroupTitlesRequest::ID));
        assert!(is_group_packet(msgs::GroupRoleMembersRequest::ID));

        assert!(!is_group_packet(msgs::UUIDGroupNameRequest::ID));
        assert!(!is_group_packet(msgs::AgentDataUpdateRequest::ID));
    }

    async fn hung_service() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let mut streams = Vec::new();
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                streams.push(stream);
            }
        });
        (format!("http://{}", addr), handle)
    }

    #[tokio::test]
    async fn hung_service_does_not_block_packet_loop() {
        let (uri, _hung) = hung_service().await;
        let ctx = GroupsCtx::new(&uri);

        let viewer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let region_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let viewer_addr = viewer.local_addr().unwrap();
        let agent = Uuid::new_v4();
        let client = Arc::new(Mutex::new(ClientSession::new(
            viewer_addr,
            1,
            agent,
            Uuid::new_v4(),
        )));

        let eq = Arc::new(EventQueue::new());

        // spawn_group_packet
        {
            let create = msgs::CreateGroupRequest {
                agent_data: msgs::create_group_request::AgentData {
                    agent_id: agent,
                    session_id: Uuid::nil(),
                },
                group_data: msgs::create_group_request::GroupData {
                    name: b"HungGroup".to_vec(),
                    ..Default::default()
                },
            };
            let mut w = welkin_msg::Writer::new();
            create.encode_body(&mut w);
            let start = Instant::now();
            spawn_group_packet(
                ctx.clone(),
                agent,
                msgs::CreateGroupRequest::ID,
                w.buf,
                "First".into(),
                "Last".into(),
                None,
                client.clone(),
                region_socket.clone(),
                viewer_addr,
            );
            assert!(start.elapsed() < Duration::from_millis(100));

            let lock_start = Instant::now();
            let guard = client.lock().await;
            assert!(lock_start.elapsed() < Duration::from_millis(100));
            drop(guard);

            let mut buf = [0u8; 65536];
            let recv = timeout(Duration::from_millis(200), viewer.recv_from(&mut buf)).await;
            assert!(recv.is_err());
        }

        // spawn_group_name_reply
        {
            let start = Instant::now();
            spawn_group_name_reply(
                ctx.clone(),
                vec![Uuid::new_v4()],
                client.clone(),
                region_socket.clone(),
                viewer_addr,
            );
            assert!(start.elapsed() < Duration::from_millis(100));

            let lock_start = Instant::now();
            let guard = client.lock().await;
            assert!(lock_start.elapsed() < Duration::from_millis(100));
            drop(guard);

            let mut buf = [0u8; 65536];
            let recv = timeout(Duration::from_millis(200), viewer.recv_from(&mut buf)).await;
            assert!(recv.is_err());
        }

        // spawn_agent_data_update
        {
            let start = Instant::now();
            spawn_agent_data_update(
                ctx.clone(),
                agent,
                "First".into(),
                "Last".into(),
                client.clone(),
                region_socket.clone(),
                viewer_addr,
            );
            assert!(start.elapsed() < Duration::from_millis(100));

            let lock_start = Instant::now();
            let guard = client.lock().await;
            assert!(lock_start.elapsed() < Duration::from_millis(100));
            drop(guard);

            let mut buf = [0u8; 65536];
            let recv = timeout(Duration::from_millis(200), viewer.recv_from(&mut buf)).await;
            assert!(recv.is_err());
        }

        // spawn_login_group_data
        {
            let start = Instant::now();
            spawn_login_group_data(
                ctx.clone(),
                agent,
                "First".into(),
                "Last".into(),
                eq.clone(),
                client.clone(),
                region_socket.clone(),
                viewer_addr,
            );
            assert!(start.elapsed() < Duration::from_millis(100));

            let lock_start = Instant::now();
            let guard = client.lock().await;
            assert!(lock_start.elapsed() < Duration::from_millis(100));
            drop(guard);

            let mut buf = [0u8; 65536];
            let recv = timeout(Duration::from_millis(200), viewer.recv_from(&mut buf)).await;
            assert!(recv.is_err());
        }
    }

    #[tokio::test]
    async fn real_service_delivery() {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let db_path = format!(
            "welkin-region-groups-tasks-{}-{}.sqlite",
            std::process::id(),
            n
        );

        struct Guard(String);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _guard = Guard(db_path.clone());

        sqlx::any::install_default_drivers();
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&format!("sqlite:{}?mode=rwc", db_path))
            .await
            .unwrap();

        let ddls = [
            "CREATE TABLE IF NOT EXISTS os_groups_groups (GroupID CHAR(36) PRIMARY KEY, Location VARCHAR(255) NOT NULL DEFAULT '', Name VARCHAR(255) NOT NULL DEFAULT '', Charter TEXT NOT NULL DEFAULT '', InsigniaID CHAR(36) NOT NULL DEFAULT '', FounderID CHAR(36) NOT NULL DEFAULT '', MembershipFee INTEGER NOT NULL DEFAULT 0, OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', ShowInList INTEGER NOT NULL DEFAULT 0, AllowPublish INTEGER NOT NULL DEFAULT 0, MaturePublish INTEGER NOT NULL DEFAULT 0, OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS os_groups_membership (GroupID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, SelectedRoleID CHAR(36) NOT NULL DEFAULT '', Contribution INTEGER NOT NULL DEFAULT 0, ListInProfile INTEGER NOT NULL DEFAULT 1, AcceptNotices INTEGER NOT NULL DEFAULT 1, AccessToken CHAR(36) NOT NULL DEFAULT '', PRIMARY KEY (GroupID, PrincipalID))",
            "CREATE TABLE IF NOT EXISTS os_groups_roles (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, Name VARCHAR(255) NOT NULL DEFAULT '', Description VARCHAR(255) NOT NULL DEFAULT '', Title VARCHAR(255) NOT NULL DEFAULT '', Powers BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (GroupID, RoleID))",
            "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, PRIMARY KEY (GroupID, RoleID, PrincipalID))",
            "CREATE TABLE IF NOT EXISTS os_groups_invites (InviteID CHAR(36) PRIMARY KEY, GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, TMStamp INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS os_groups_notices (GroupID CHAR(36) NOT NULL, NoticeID CHAR(36) PRIMARY KEY, TMStamp INTEGER NOT NULL DEFAULT 0, FromName VARCHAR(255) NOT NULL DEFAULT '', Subject VARCHAR(255) NOT NULL DEFAULT '', Message TEXT NOT NULL DEFAULT '', HasAttachment INTEGER NOT NULL DEFAULT 0, AttachmentType INTEGER NOT NULL DEFAULT 0, AttachmentName VARCHAR(128) NOT NULL DEFAULT '', AttachmentItemID CHAR(36) NOT NULL DEFAULT '', AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS os_groups_principals (PrincipalID VARCHAR(255) PRIMARY KEY, ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
        ];

        for ddl in ddls {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }

        let state = welkin_services::groups::GroupsServiceState {
            store: Arc::new(welkin_data::groups::GroupsStore::new(pool)),
        };
        let app = welkin_services::groups::router(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let router_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let uri = format!("http://{}", router_addr);
        let ctx = GroupsCtx::new(&uri);

        let viewer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let region_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let viewer_addr = viewer.local_addr().unwrap();
        let agent = Uuid::new_v4();
        let client = Arc::new(Mutex::new(ClientSession::new(
            viewer_addr,
            1,
            agent,
            Uuid::new_v4(),
        )));

        let record = welkin_services::remote::groups::GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Tasks Group".into(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: agent,
            membership_fee: 0,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        };

        let group_id = ctx
            .connector()
            .create_group(agent, &record)
            .await
            .expect("create group")
            .group_id;

        let _ = ctx
            .connector()
            .set_active(agent, agent, group_id, "GROUP", None)
            .await;

        spawn_group_name_reply(
            ctx.clone(),
            vec![group_id],
            client.clone(),
            region_socket.clone(),
            viewer_addr,
        );

        let mut buf = [0u8; 65536];
        let (n, _) = timeout(Duration::from_secs(2), viewer.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
        let pkt = welkin_msg::parse_packet(&buf[..n]).unwrap();
        assert_eq!(pkt.id, msgs::UUIDGroupNameReply::ID);

        let eq = Arc::new(EventQueue::new());

        spawn_login_group_data(
            ctx.clone(),
            agent,
            "First".into(),
            "Last".into(),
            eq.clone(),
            client.clone(),
            region_socket.clone(),
            viewer_addr,
        );

        let (n, _) = timeout(Duration::from_secs(2), viewer.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
        let pkt = welkin_msg::parse_packet(&buf[..n]).unwrap();
        assert_eq!(pkt.id, msgs::AgentDataUpdate::ID);

        let poll = eq
            .poll(Duration::from_millis(500))
            .await;
        match poll {
            welkin_caps::event_queue::EventQueueResponse::Events { events, .. } => {
                assert!(String::from_utf8_lossy(&events).contains("AgentGroupDataUpdate"));
            }
            _ => panic!("expected events"),
        }
    }
}
