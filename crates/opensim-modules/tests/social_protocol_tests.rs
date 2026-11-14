// SPDX-License-Identifier: GPL-3.0-or-later

//! Protocol replay tests for social service handlers.
//!
//! Verifies that the Groups service HTTP handlers produce XML responses
//! matching the C# `GroupsServiceRobustConnector` wire format.
//! Each test sends a form-encoded POST request through an in-process axum
//! router backed by an in-memory SQLite database.

use std::sync::Arc;

use http_body_util::BodyExt;
use opensim_data::groups::GroupsStore;
use opensim_services::groups::{GroupsServiceState, router as groups_router};
use tower::ServiceExt;

// ===========================================================================
// SQLite schema for groups tables (compatible with GroupsStore queries)
// ===========================================================================

const GROUPS_SCHEMA: &str = "\
CREATE TABLE os_groups_groups (
    GroupID TEXT NOT NULL DEFAULT '' PRIMARY KEY,
    Location TEXT NOT NULL DEFAULT '',
    Name TEXT NOT NULL DEFAULT '' UNIQUE,
    Charter TEXT NOT NULL DEFAULT '',
    InsigniaID TEXT NOT NULL DEFAULT '',
    FounderID TEXT NOT NULL DEFAULT '',
    MembershipFee INTEGER NOT NULL DEFAULT 0,
    OpenEnrollment TEXT NOT NULL DEFAULT '',
    ShowInList TEXT NOT NULL DEFAULT '0',
    AllowPublish TEXT NOT NULL DEFAULT '0',
    MaturePublish TEXT NOT NULL DEFAULT '0',
    OwnerRoleID TEXT NOT NULL DEFAULT ''
);

CREATE TABLE os_groups_membership (
    GroupID TEXT NOT NULL DEFAULT '',
    PrincipalID TEXT NOT NULL DEFAULT '',
    SelectedRoleID TEXT NOT NULL DEFAULT '',
    Contribution TEXT NOT NULL DEFAULT '0',
    ListInProfile TEXT NOT NULL DEFAULT '1',
    AcceptNotices TEXT NOT NULL DEFAULT '1',
    AccessToken TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (GroupID, PrincipalID)
);

CREATE TABLE os_groups_roles (
    GroupID TEXT NOT NULL DEFAULT '',
    RoleID TEXT NOT NULL DEFAULT '',
    Name TEXT NOT NULL DEFAULT '',
    Description TEXT NOT NULL DEFAULT '',
    Title TEXT NOT NULL DEFAULT '',
    Powers TEXT NOT NULL DEFAULT '0',
    PRIMARY KEY (GroupID, RoleID)
);

CREATE TABLE os_groups_rolemembership (
    GroupID TEXT NOT NULL DEFAULT '',
    RoleID TEXT NOT NULL DEFAULT '',
    PrincipalID TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (GroupID, RoleID, PrincipalID)
);

CREATE TABLE os_groups_invites (
    InviteID TEXT NOT NULL DEFAULT '' PRIMARY KEY,
    GroupID TEXT NOT NULL DEFAULT '',
    RoleID TEXT NOT NULL DEFAULT '',
    PrincipalID TEXT NOT NULL DEFAULT '',
    TMStamp INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE os_groups_notices (
    GroupID TEXT NOT NULL DEFAULT '',
    NoticeID TEXT NOT NULL DEFAULT '' PRIMARY KEY,
    TMStamp TEXT NOT NULL DEFAULT '0',
    FromName TEXT NOT NULL DEFAULT '',
    Subject TEXT NOT NULL DEFAULT '',
    Message TEXT NOT NULL DEFAULT '',
    HasAttachment TEXT NOT NULL DEFAULT '0',
    AttachmentType TEXT NOT NULL DEFAULT '0',
    AttachmentName TEXT NOT NULL DEFAULT '',
    AttachmentItemID TEXT NOT NULL DEFAULT '',
    AttachmentOwnerID TEXT NOT NULL DEFAULT ''
);

CREATE TABLE os_groups_principals (
    PrincipalID TEXT NOT NULL DEFAULT '' PRIMARY KEY,
    ActiveGroupID TEXT NOT NULL DEFAULT ''
);
";

// ===========================================================================
// Constants
// ===========================================================================

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const AGENT_UUID: &str = "d47f3c77-9e2a-4b58-b6d1-8e3f2a1c5b09";
const AGENT2_UUID: &str = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";

// ===========================================================================
// Test helpers
// ===========================================================================

async fn setup_db() -> GroupsStore {
    sqlx::any::install_default_drivers();
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory SQLite pool");

    for stmt in GROUPS_SCHEMA.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(trimmed)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("schema setup failed on:\n{trimmed}\nerror: {e}"));
    }

    GroupsStore::new(pool)
}

fn build_groups_app(store: GroupsStore) -> axum::Router {
    let state = GroupsServiceState {
        store: Arc::new(store),
    };
    groups_router(state)
}

fn form_body(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

async fn post_groups(
    app: &axum::Router,
    body: &str,
) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/groups")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes).to_string();
    (status, body_str)
}

/// Verify that a response is well-formed XML with a ServerResponse root.
fn assert_valid_xml_response(body: &str, test_name: &str) {
    assert!(
        body.starts_with("<?xml version=\"1.0\"?>"),
        "{test_name}: response should start with XML declaration, got: {body}"
    );
    assert!(
        body.contains("<ServerResponse>"),
        "{test_name}: response should contain <ServerResponse>, got: {body}"
    );
    assert!(
        body.contains("</ServerResponse>"),
        "{test_name}: response should contain </ServerResponse>, got: {body}"
    );
}

/// Extract the text content of a specific XML element from the response.
fn extract_xml_value(body: &str, element: &str) -> Option<String> {
    let open = format!("<{element}>");
    let close = format!("</{element}>");
    if let Some(start) = body.find(&open) {
        let start = start + open.len();
        if let Some(end) = body[start..].find(&close) {
            return Some(body[start..start + end].to_string());
        }
    }
    None
}

// ===========================================================================
// 1. PUTGROUP ADD: create a new group
// ===========================================================================

#[tokio::test]
async fn groups_protocol_putgroup_add() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    let body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Protocol Test Group"),
        ("Charter", "We test protocols"),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "True"),
        ("MaturePublish", "False"),
    ]);

    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "PUTGROUP ADD");

    // Should contain the group data.
    assert!(
        response.contains("<RESULT type=\"List\">"),
        "PUTGROUP ADD should return a List result: {response}"
    );
    assert!(
        response.contains("<GroupName>Protocol Test Group</GroupName>"),
        "Response should contain the group name: {response}"
    );
    assert!(
        response.contains("<Charter>We test protocols</Charter>"),
        "Response should contain the charter: {response}"
    );
    assert!(
        response.contains("<FounderID>"),
        "Response should contain FounderID: {response}"
    );
    assert!(
        response.contains("<OwnerRoleID>"),
        "Response should contain OwnerRoleID: {response}"
    );
    // MemberCount should be at least 1 (the founder).
    assert!(
        response.contains("<MemberCount>"),
        "Response should contain MemberCount: {response}"
    );
}

// ===========================================================================
// 2. GETGROUP by ID
// ===========================================================================

#[tokio::test]
async fn groups_protocol_getgroup_by_id() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // First create a group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Getable Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "False"),
        ("ShownInList", "False"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID in create response");

    // Now GET it.
    let get_body = form_body(&[
        ("METHOD", "GETGROUP"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
    ]);
    let (status, response) = post_groups(&app, &get_body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "GETGROUP");

    assert!(
        response.contains("<GroupName>Getable Group</GroupName>"),
        "GETGROUP should return the group: {response}"
    );
    assert!(
        response.contains(&format!("<GroupID>{group_id}</GroupID>")),
        "GETGROUP should return the correct GroupID: {response}"
    );
}

// ===========================================================================
// 3. GETGROUP not found
// ===========================================================================

#[tokio::test]
async fn groups_protocol_getgroup_not_found() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    let body = form_body(&[
        ("METHOD", "GETGROUP"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", "ffffffff-ffff-ffff-ffff-ffffffffffff"),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "GETGROUP not found");

    assert!(
        response.contains("<RESULT>NULL</RESULT>"),
        "Missing group should return NULL: {response}"
    );
    assert!(
        response.contains("<REASON>"),
        "NULL result should include REASON: {response}"
    );
}

// ===========================================================================
// 4. ADDAGENTTOGROUP
// ===========================================================================

#[tokio::test]
async fn groups_protocol_addagent_to_group() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create a group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Agent Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Add a second agent.
    let add_body = form_body(&[
        ("METHOD", "ADDAGENTTOGROUP"),
        ("RequestingAgentID", AGENT_UUID),
        ("AgentID", AGENT2_UUID),
        ("GroupID", &group_id),
        ("RoleID", ZERO_UUID),
        ("AccessToken", ""),
    ]);
    let (status, response) = post_groups(&app, &add_body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "ADDAGENTTOGROUP");

    // Should return membership data.
    assert!(
        response.contains("<RESULT type=\"List\">"),
        "ADDAGENTTOGROUP should return List: {response}"
    );
    assert!(
        response.contains("<GroupID>"),
        "Membership response should contain GroupID: {response}"
    );
    assert!(
        response.contains("<GroupName>Agent Group</GroupName>"),
        "Membership response should contain GroupName: {response}"
    );
}

// ===========================================================================
// 5. REMOVEAGENTFROMGROUP
// ===========================================================================

#[tokio::test]
async fn groups_protocol_removeagent_from_group() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group and add agent.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Remove Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Add agent2.
    let add_body = form_body(&[
        ("METHOD", "ADDAGENTTOGROUP"),
        ("RequestingAgentID", AGENT_UUID),
        ("AgentID", AGENT2_UUID),
        ("GroupID", &group_id),
        ("RoleID", ZERO_UUID),
        ("AccessToken", ""),
    ]);
    post_groups(&app, &add_body).await;

    // Remove agent2 (self-leave).
    let remove_body = form_body(&[
        ("METHOD", "REMOVEAGENTFROMGROUP"),
        ("RequestingAgentID", AGENT2_UUID),
        ("AgentID", AGENT2_UUID),
        ("GroupID", &group_id),
    ]);
    let (status, response) = post_groups(&app, &remove_body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "REMOVEAGENTFROMGROUP");

    assert!(
        response.contains("<RESULT>true</RESULT>"),
        "REMOVEAGENTFROMGROUP should return true: {response}"
    );
}

// ===========================================================================
// 6. GETGROUPMEMBERS
// ===========================================================================

#[tokio::test]
async fn groups_protocol_get_group_members() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Members Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Get members.
    let body = form_body(&[
        ("METHOD", "GETGROUPMEMBERS"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "GETGROUPMEMBERS");

    // Should be a list with the founder.
    assert!(
        response.contains("<RESULT type=\"List\">"),
        "GETGROUPMEMBERS should return a List: {response}"
    );
    assert!(
        response.contains("<AgentID>"),
        "Members list should include AgentID: {response}"
    );
}

// ===========================================================================
// 7. PUTROLE
// ===========================================================================

#[tokio::test]
async fn groups_protocol_putrole() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Role Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Add a new role.
    let role_id = "11111111-2222-3333-4444-555555555555";
    let body = form_body(&[
        ("METHOD", "PUTROLE"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
        ("RoleID", role_id),
        ("Name", "Builders"),
        ("Description", "People who build"),
        ("Title", "Builder"),
        ("Powers", "16384"),
        ("OP", "ADD"),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "PUTROLE");

    assert!(
        response.contains("<RESULT>true</RESULT>"),
        "PUTROLE should return true: {response}"
    );
}

// ===========================================================================
// 8. GETGROUPROLES
// ===========================================================================

#[tokio::test]
async fn groups_protocol_get_group_roles() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group (which auto-creates 3 roles).
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Roles Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Get roles.
    let body = form_body(&[
        ("METHOD", "GETGROUPROLES"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "GETGROUPROLES");

    // Should return a list of roles.
    assert!(
        response.contains("<RESULT type=\"List\">"),
        "GETGROUPROLES should return a List: {response}"
    );
    // Should have at least 3 roles: Everyone, Officers, Owners.
    assert!(
        response.contains("<Name>Everyone</Name>"),
        "Should contain Everyone role: {response}"
    );
    assert!(
        response.contains("<Name>Officers</Name>"),
        "Should contain Officers role: {response}"
    );
    assert!(
        response.contains("<Name>Owners</Name>"),
        "Should contain Owners role: {response}"
    );
}

// ===========================================================================
// 9. ADDNOTICE
// ===========================================================================

#[tokio::test]
async fn groups_protocol_addnotice() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Notice Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    let notice_id = "aabbccdd-1111-2222-3333-444455556666";
    let body = form_body(&[
        ("METHOD", "ADDNOTICE"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
        ("NoticeID", notice_id),
        ("FromName", "Test User"),
        ("Subject", "Meeting Tonight"),
        ("Message", "Don't miss it!"),
        ("HasAttachment", "False"),
        ("AttachmentType", "0"),
        ("AttachmentName", ""),
        ("AttachmentItemID", ZERO_UUID),
        ("AttachmentOwnerID", ZERO_UUID),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "ADDNOTICE");

    assert!(
        response.contains("<RESULT>true</RESULT>"),
        "ADDNOTICE should return true: {response}"
    );
}

// ===========================================================================
// 10. GETNOTICES
// ===========================================================================

#[tokio::test]
async fn groups_protocol_getnotices() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Notices Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Add a notice.
    let notice_id = "11223344-5566-7788-99aa-bbccddeeff00";
    let add_body = form_body(&[
        ("METHOD", "ADDNOTICE"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
        ("NoticeID", notice_id),
        ("FromName", "Admin"),
        ("Subject", "Hello"),
        ("Message", "World"),
        ("HasAttachment", "False"),
        ("AttachmentType", "0"),
        ("AttachmentName", ""),
        ("AttachmentItemID", ZERO_UUID),
        ("AttachmentOwnerID", ZERO_UUID),
    ]);
    post_groups(&app, &add_body).await;

    // Retrieve notices.
    let body = form_body(&[
        ("METHOD", "GETNOTICES"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "GETNOTICES");

    assert!(
        response.contains("<RESULT type=\"List\">"),
        "GETNOTICES should return a List: {response}"
    );
    assert!(
        response.contains(&format!("<NoticeID>{notice_id}</NoticeID>")),
        "GETNOTICES should include the notice: {response}"
    );
    assert!(
        response.contains("<Subject>Hello</Subject>"),
        "Notice should have subject: {response}"
    );
}

// ===========================================================================
// 11. FINDGROUPS
// ===========================================================================

#[tokio::test]
async fn groups_protocol_findgroups() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create two groups.
    for name in ["Searchable Alpha", "Searchable Beta"] {
        let body = form_body(&[
            ("METHOD", "PUTGROUP"),
            ("OP", "ADD"),
            ("RequestingAgentID", AGENT_UUID),
            ("GroupName", name),
            ("Charter", ""),
            ("FounderID", AGENT_UUID),
            ("InsigniaID", ZERO_UUID),
            ("MembershipFee", "0"),
            ("OpenEnrollment", "True"),
            ("ShownInList", "True"),
            ("AllowPublish", "False"),
            ("MaturePublish", "False"),
        ]);
        post_groups(&app, &body).await;
    }

    // Search for "Searchable".
    let body = form_body(&[
        ("METHOD", "FINDGROUPS"),
        ("RequestingAgentID", AGENT_UUID),
        ("Query", "Searchable"),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "FINDGROUPS");

    assert!(
        response.contains("<RESULT type=\"List\">"),
        "FINDGROUPS should return a List: {response}"
    );
    // Both groups should be found.
    assert!(
        response.contains("<Name>Searchable Alpha</Name>"),
        "Should find Alpha: {response}"
    );
    assert!(
        response.contains("<Name>Searchable Beta</Name>"),
        "Should find Beta: {response}"
    );
}

// ===========================================================================
// 12. Unknown METHOD returns NULL/REASON
// ===========================================================================

#[tokio::test]
async fn groups_protocol_unknown_method() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    let body = form_body(&[("METHOD", "DOESNOTEXIST")]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "unknown method");

    assert!(
        response.contains("<RESULT>NULL</RESULT>"),
        "Unknown method should return NULL: {response}"
    );
}

// ===========================================================================
// 13. Missing METHOD returns NULL/REASON
// ===========================================================================

#[tokio::test]
async fn groups_protocol_missing_method() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // No METHOD field in body.
    let body = form_body(&[("SomeKey", "SomeValue")]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "missing method");

    assert!(
        response.contains("<RESULT>NULL</RESULT>"),
        "Missing METHOD should return NULL: {response}"
    );
}

// ===========================================================================
// 14. GETMEMBERSHIP
// ===========================================================================

#[tokio::test]
async fn groups_protocol_getmembership() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group (founder auto-joins).
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Membership Group"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Get membership for the founder.
    let body = form_body(&[
        ("METHOD", "GETMEMBERSHIP"),
        ("RequestingAgentID", AGENT_UUID),
        ("AgentID", AGENT_UUID),
        ("GroupID", &group_id),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "GETMEMBERSHIP");

    assert!(
        response.contains("<RESULT type=\"List\">"),
        "GETMEMBERSHIP should return List: {response}"
    );
    assert!(
        response.contains("<GroupName>Membership Group</GroupName>"),
        "Should show group name: {response}"
    );
}

// ===========================================================================
// 15. PUTGROUP UPDATE
// ===========================================================================

#[tokio::test]
async fn groups_protocol_putgroup_update() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Create group.
    let create_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Updatable Group"),
        ("Charter", "Original charter"),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);
    let (_, create_resp) = post_groups(&app, &create_body).await;
    let group_id = extract_xml_value(&create_resp, "GroupID").expect("GroupID");

    // Update the group's charter.
    let update_body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "UPDATE"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupID", &group_id),
        ("Charter", "Updated charter"),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "100"),
        ("OpenEnrollment", "False"),
        ("ShownInList", "True"),
        ("AllowPublish", "True"),
        ("MaturePublish", "False"),
    ]);
    let (status, response) = post_groups(&app, &update_body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "PUTGROUP UPDATE");

    assert!(
        response.contains("<Charter>Updated charter</Charter>"),
        "Updated charter should appear: {response}"
    );
}

// ===========================================================================
// 16. PUTGROUP bad data
// ===========================================================================

#[tokio::test]
async fn groups_protocol_putgroup_bad_data() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    // Missing OP.
    let body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("RequestingAgentID", AGENT_UUID),
    ]);
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert_valid_xml_response(&response, "PUTGROUP bad data");
    assert!(
        response.contains("<RESULT>NULL</RESULT>"),
        "Bad data should return NULL: {response}"
    );
}

// ===========================================================================
// 17. Duplicate group name rejected
// ===========================================================================

#[tokio::test]
async fn groups_protocol_duplicate_group_name() {
    let store = setup_db().await;
    let app = build_groups_app(store);

    let body = form_body(&[
        ("METHOD", "PUTGROUP"),
        ("OP", "ADD"),
        ("RequestingAgentID", AGENT_UUID),
        ("GroupName", "Unique Name"),
        ("Charter", ""),
        ("FounderID", AGENT_UUID),
        ("InsigniaID", ZERO_UUID),
        ("MembershipFee", "0"),
        ("OpenEnrollment", "True"),
        ("ShownInList", "True"),
        ("AllowPublish", "False"),
        ("MaturePublish", "False"),
    ]);

    // First creation succeeds.
    let (status, _) = post_groups(&app, &body).await;
    assert_eq!(status, 200);

    // Second creation with same name should fail.
    let (status, response) = post_groups(&app, &body).await;
    assert_eq!(status, 200);
    assert!(
        response.contains("<RESULT>NULL</RESULT>"),
        "Duplicate name should return NULL: {response}"
    );
    assert!(
        response.contains("already exists"),
        "Should mention already exists: {response}"
    );
}
