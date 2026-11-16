// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;

use uuid::Uuid;
use welkin_caps::event_queue::EventQueue;
use welkin_llsd::{Llsd, Map};
use welkin_services::remote::groups::{RemoteGroupsConnector, MembershipRecord};

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
}
