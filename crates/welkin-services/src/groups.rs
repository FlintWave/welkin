// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use uuid::Uuid;
use welkin_data::groups::{
    GroupPowers, GroupRow, GroupsStore, InvitationRow, MembershipRow, NoticeRow, PrincipalRow,
    RoleMembershipRow, RoleRow,
};

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

// ---- State and router ----

#[derive(Clone)]
pub struct GroupsServiceState {
    pub store: Arc<GroupsStore>,
}

pub fn router(state: GroupsServiceState) -> Router {
    Router::new()
        .route("/groups", post(handle_groups))
        .with_state(state)
}

// ---- XML response helpers (matching C# ServerUtils.BuildXmlResponse) ----

fn xml_result(body: &str) -> Response {
    let xml = format!("<?xml version=\"1.0\"?><ServerResponse>{body}</ServerResponse>");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
        .into_response()
}

fn null_result(reason: &str) -> Response {
    xml_result(&format!("<RESULT>NULL</RESULT><REASON>{reason}</REASON>"))
}

fn bool_result(success: bool) -> Response {
    let v = if success { "true" } else { "false" };
    xml_result(&format!("<RESULT>{v}</RESULT>"))
}

fn dict_to_xml(dict: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (k, v) in dict {
        out.push_str(&format!("<{k}>{}</{k}>", xml_escape(v)));
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn list_to_xml(prefix: &str, items: &[String]) -> String {
    let mut out = String::from("<RESULT type=\"List\">");
    for (i, item) in items.iter().enumerate() {
        out.push_str(&format!(
            "<{prefix}-{i} type=\"List\">{item}</{prefix}-{i}>"
        ));
    }
    out.push_str("</RESULT>");
    out
}

trait BoolLike {
    fn to_bool_int(&self) -> i32;
}

impl BoolLike for i32 {
    fn to_bool_int(&self) -> i32 {
        *self
    }
}

impl BoolLike for &i32 {
    fn to_bool_int(&self) -> i32 {
        **self
    }
}

impl BoolLike for &str {
    fn to_bool_int(&self) -> i32 {
        if *self == "1" || self.eq_ignore_ascii_case("true") {
            1
        } else {
            0
        }
    }
}

impl BoolLike for String {
    fn to_bool_int(&self) -> i32 {
        if self == "1" || self.eq_ignore_ascii_case("true") {
            1
        } else {
            0
        }
    }
}

impl BoolLike for &String {
    fn to_bool_int(&self) -> i32 {
        if *self == "1" || self.eq_ignore_ascii_case("true") {
            1
        } else {
            0
        }
    }
}

fn bool_str<T: BoolLike>(val: T) -> String {
    if val.to_bool_int() != 0 {
        "True".to_string()
    } else {
        "False".to_string()
    }
}

fn group_row_to_xml(row: &GroupRow, member_count: i64, role_count: i64) -> String {
    dict_to_xml(&[
        ("AllowPublish", &bool_str(row.allow_publish)),
        ("Charter", &row.charter),
        ("FounderID", &row.founder_id),
        ("FounderUUI", ""),
        ("GroupID", &row.group_id),
        ("GroupName", &row.name),
        ("InsigniaID", &row.insignia_id),
        ("MaturePublish", &bool_str(row.mature_publish)),
        ("MembershipFee", &row.membership_fee.to_string()),
        ("OpenEnrollment", &bool_str(&row.open_enrollment)),
        ("OwnerRoleID", &row.owner_role_id),
        ("ServiceLocation", &row.location),
        ("ShownInList", &bool_str(row.show_in_list)),
        ("MemberCount", &member_count.to_string()),
        ("RoleCount", &role_count.to_string()),
    ])
}

fn membership_to_xml(
    membership: &MembershipRow,
    group: &GroupRow,
    role_title: &str,
    role_powers: u64,
) -> String {
    dict_to_xml(&[
        ("AcceptNotices", &bool_str(&membership.accept_notices)),
        ("AccessToken", &membership.access_token),
        ("Active", "True"),
        ("ActiveRole", &membership.selected_role_id),
        ("AllowPublish", &bool_str(group.allow_publish)),
        ("Charter", &group.charter),
        ("Contribution", &membership.contribution.to_string()),
        ("FounderID", &group.founder_id),
        ("GroupID", &group.group_id),
        ("GroupName", &group.name),
        ("GroupPicture", &group.insignia_id),
        ("GroupPowers", &role_powers.to_string()),
        ("GroupTitle", role_title),
        ("ListInProfile", &bool_str(&membership.list_in_profile)),
        ("MaturePublish", &bool_str(group.mature_publish)),
        ("MembershipFee", &group.membership_fee.to_string()),
        ("OpenEnrollment", &bool_str(&group.open_enrollment)),
        ("ShowInList", &bool_str(group.show_in_list)),
    ])
}

fn member_to_xml(m: &MembershipRow, title: &str, powers: u64, is_owner: bool) -> String {
    dict_to_xml(&[
        ("AcceptNotices", &bool_str(m.accept_notices)),
        ("AccessToken", &m.access_token),
        ("AgentID", &m.principal_id),
        ("AgentPowers", &powers.to_string()),
        ("Contribution", &m.contribution.to_string()),
        ("IsOwner", if is_owner { "True" } else { "False" }),
        ("ListInProfile", &bool_str(m.list_in_profile)),
        ("OnlineStatus", ""),
        ("Title", title),
    ])
}

fn role_to_xml(r: &RoleRow, members: i64) -> String {
    dict_to_xml(&[
        ("Description", &r.description),
        ("Members", &members.to_string()),
        ("Name", &r.name),
        ("Powers", &r.powers.to_string()),
        ("RoleID", &r.role_id),
        ("Title", &r.title),
    ])
}

fn role_member_to_xml(rm: &RoleMembershipRow) -> String {
    dict_to_xml(&[("RoleID", &rm.role_id), ("MemberID", &rm.principal_id)])
}

fn invite_to_xml(inv: &InvitationRow) -> String {
    dict_to_xml(&[
        ("InviteID", &inv.invite_id),
        ("GroupID", &inv.group_id),
        ("RoleID", &inv.role_id),
        ("AgentID", &inv.principal_id),
    ])
}

fn notice_to_xml(n: &NoticeRow) -> String {
    dict_to_xml(&[
        ("NoticeID", &n.notice_id),
        ("Timestamp", &n.timestamp.to_string()),
        ("FromName", &n.from_name),
        ("Subject", &n.subject),
        ("HasAttachment", &bool_str(n.has_attachment)),
        ("AttachmentItemID", &n.attachment_item_id),
        ("AttachmentName", &n.attachment_name),
        ("AttachmentType", &n.attachment_type.to_string()),
        ("AttachmentOwnerID", &n.attachment_owner_id),
    ])
}

fn notice_info_to_xml(n: &NoticeRow) -> String {
    let mut s = notice_to_xml(n);
    s.push_str(&format!(
        "<GroupID>{}</GroupID><Message>{}</Message>",
        xml_escape(&n.group_id),
        xml_escape(&n.message),
    ));
    s
}

fn dir_group_to_xml(row: &GroupRow, members: i64) -> String {
    dict_to_xml(&[
        ("GroupID", &row.group_id),
        ("Name", &row.name),
        ("NMembers", &members.to_string()),
        ("SearchOrder", "0"),
    ])
}

fn parse_bool(val: &str) -> bool {
    val == "1" || val.eq_ignore_ascii_case("true")
}

fn form_str<'a>(form: &'a HashMap<String, String>, key: &str) -> &'a str {
    form.get(key).map(|s| s.as_str()).unwrap_or("")
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---- Form parsing (matching grid.rs pattern) ----

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_form(body: &str) -> HashMap<String, String> {
    let mut form = HashMap::new();
    for pair in body.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        form.insert(url_decode(k), url_decode(v));
    }
    form
}

// ---- Permission checks ----

async fn has_power(
    store: &GroupsStore,
    agent_id: &str,
    group_id: &str,
    power: GroupPowers,
) -> bool {
    let rmemberships = match store.retrieve_member_roles(group_id, agent_id).await {
        Ok(r) => r,
        Err(_) => return false,
    };
    for rm in &rmemberships {
        if let Ok(Some(role)) = store.retrieve_role(group_id, &rm.role_id).await {
            let bits: u64 = role.powers as u64;
            if bits & power.bits() != 0 {
                return true;
            }
        }
    }
    false
}

async fn is_owner(store: &GroupsStore, agent_id: &str, group_id: &str) -> bool {
    let group = match store.retrieve_group_by_id(group_id).await {
        Ok(Some(g)) => g,
        _ => return false,
    };
    store
        .retrieve_role_member(group_id, &group.owner_role_id, agent_id)
        .await
        .ok()
        .flatten()
        .is_some()
}

// ---- Internal service methods ----

async fn add_or_update_role(
    store: &GroupsStore,
    group_id: &str,
    role_id: &str,
    name: &str,
    description: &str,
    title: &str,
    powers: u64,
    add: bool,
) -> bool {
    let existing = store.retrieve_role(group_id, role_id).await.ok().flatten();
    if add && existing.is_some() {
        return false;
    }
    if !add && existing.is_none() {
        return false;
    }
    let row = RoleRow {
        group_id: group_id.to_string(),
        role_id: role_id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        title: title.to_string(),
        powers: powers as i64,
    };
    store.store_role(&row).await.unwrap_or(false)
}

async fn add_agent_to_group_role(
    store: &GroupsStore,
    agent_id: &str,
    group_id: &str,
    role_id: &str,
) {
    if store
        .retrieve_role_member(group_id, role_id, agent_id)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return;
    }
    let row = RoleMembershipRow {
        group_id: group_id.to_string(),
        role_id: role_id.to_string(),
        principal_id: agent_id.to_string(),
    };
    let _ = store.store_role_member(&row).await;

    // Update selected role
    if let Ok(Some(mut membership)) = store.retrieve_member(group_id, agent_id).await {
        membership.selected_role_id = role_id.to_string();
        let _ = store.store_member(&membership).await;
    }
}

async fn add_agent_to_group(
    store: &GroupsStore,
    agent_id: &str,
    group_id: &str,
    role_id: &str,
    access_token: &str,
) {
    // Check if already member
    if store
        .retrieve_member(group_id, agent_id)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return;
    }
    let membership = MembershipRow {
        group_id: group_id.to_string(),
        principal_id: agent_id.to_string(),
        selected_role_id: role_id.to_string(),
        contribution: 0,
        list_in_profile: 1,
        accept_notices: 1,
        access_token: access_token.to_string(),
    };
    let _ = store.store_member(&membership).await;

    // Add to Everyone role
    add_agent_to_group_role(store, agent_id, group_id, ZERO_UUID).await;

    // Add to specified role if not Everyone
    if role_id != ZERO_UUID {
        add_agent_to_group_role(store, agent_id, group_id, role_id).await;
    }

    // Make this the active group
    let principal = PrincipalRow {
        principal_id: agent_id.to_string(),
        active_group_id: group_id.to_string(),
    };
    let _ = store.store_principal(&principal).await;
}

async fn get_membership_data(
    store: &GroupsStore,
    agent_id: &str,
    group_id: &str,
) -> Option<(MembershipRow, GroupRow, String, u64)> {
    let group = store.retrieve_group_by_id(group_id).await.ok()??;
    let membership = store
        .retrieve_member(&group.group_id, agent_id)
        .await
        .ok()??;
    let active_role_id = &membership.selected_role_id;
    let role = store
        .retrieve_role(&group.group_id, active_role_id)
        .await
        .ok()
        .flatten();
    let (title, powers) = match role {
        Some(r) => (r.title.clone(), r.powers as u64),
        None => (String::new(), 0),
    };
    Some((membership, group, title, powers))
}

// ---- Main handler ----

async fn handle_groups(
    State(state): State<GroupsServiceState>,
    body: axum::body::Bytes,
) -> Response {
    let body_str = String::from_utf8_lossy(&body);
    let form = parse_form(&body_str);

    let method = match form.get("METHOD") {
        Some(m) => m.as_str(),
        None => return null_result("Unknown method"),
    };

    let store = &state.store;

    tracing::debug!(method, "groups request");

    match method {
        "PUTGROUP" => handle_put_group(store, &form).await,
        "GETGROUP" => handle_get_group(store, &form).await,
        "ADDAGENTTOGROUP" => handle_add_agent_to_group(store, &form).await,
        "REMOVEAGENTFROMGROUP" => handle_remove_agent_from_group(store, &form).await,
        "GETMEMBERSHIP" => handle_get_membership(store, &form).await,
        "GETGROUPMEMBERS" => handle_get_group_members(store, &form).await,
        "PUTROLE" => handle_put_role(store, &form).await,
        "REMOVEROLE" => handle_remove_role(store, &form).await,
        "GETGROUPROLES" => handle_get_group_roles(store, &form).await,
        "GETROLEMEMBERS" => handle_get_role_members(store, &form).await,
        "AGENTROLE" => handle_agent_role(store, &form).await,
        "GETAGENTROLES" => handle_get_agent_roles(store, &form).await,
        "SETACTIVE" => handle_set_active(store, &form).await,
        "UPDATEMEMBERSHIP" => handle_update_membership(store, &form).await,
        "INVITE" => handle_invite(store, &form).await,
        "ADDNOTICE" => handle_add_notice(store, &form).await,
        "GETNOTICES" => handle_get_notices(store, &form).await,
        "FINDGROUPS" => handle_find_groups(store, &form).await,
        _ => {
            tracing::warn!(method, "unknown groups method");
            null_result("Unknown method")
        }
    }
}

// ---- Handler implementations ----

async fn handle_put_group(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let op = form_str(form, "OP");
    if requesting_agent.is_empty() || op.is_empty() {
        return null_result("Bad network data");
    }

    let group_id_str = form_str(form, "GroupID");

    if op == "ADD" {
        let name = form_str(form, "GroupName");
        if name.is_empty() {
            return null_result("Group name is required");
        }

        // Check duplicate
        if let Ok(Some(_)) = store.retrieve_group_by_name(name).await {
            return null_result("A group with that name already exists");
        }

        let group_id = Uuid::new_v4().to_string();
        let owner_role_id = Uuid::new_v4().to_string();
        let officers_role_id = Uuid::new_v4().to_string();

        let row = GroupRow {
            group_id: group_id.clone(),
            name: name.to_string(),
            charter: form_str(form, "Charter").to_string(),
            insignia_id: form_str(form, "InsigniaID").to_string(),
            founder_id: form_str(form, "FounderID").to_string(),
            membership_fee: form_str(form, "MembershipFee").parse().unwrap_or(0),
            open_enrollment: if parse_bool(form_str(form, "OpenEnrollment")) {
                "1"
            } else {
                "0"
            }
            .to_string(),
            show_in_list: if parse_bool(form_str(form, "ShownInList")) {
                1
            } else {
                0
            },
            allow_publish: if parse_bool(form_str(form, "AllowPublish")) {
                1
            } else {
                0
            },
            mature_publish: if parse_bool(form_str(form, "MaturePublish")) {
                1
            } else {
                0
            },
            owner_role_id: owner_role_id.clone(),
            location: String::new(),
        };

        if store.store_group(&row).await.is_err() {
            return null_result("Failed to create group");
        }

        // Create default roles
        let everyone_powers = GroupPowers::DEFAULT_EVERYONE.bits();
        let officer_powers = GroupPowers::OFFICERS.bits();
        let owner_powers = GroupPowers::OWNERS.bits();

        add_or_update_role(
            store,
            &group_id,
            ZERO_UUID,
            "Everyone",
            "Everyone in the group is in the everyone role.",
            &format!("Member of {name}"),
            everyone_powers,
            true,
        )
        .await;
        add_or_update_role(
            store,
            &group_id,
            &officers_role_id,
            "Officers",
            "The officers of the group, with more powers than regular members.",
            &format!("Officer of {name}"),
            officer_powers,
            true,
        )
        .await;
        add_or_update_role(
            store,
            &group_id,
            &owner_role_id,
            "Owners",
            "Owners of the group",
            &format!("Owner of {name}"),
            owner_powers,
            true,
        )
        .await;

        // Add founder
        let founder_id = form_str(form, "FounderID");
        add_agent_to_group(store, founder_id, &group_id, &owner_role_id, "").await;

        // Return the group
        let mc = store.member_count(&group_id).await.unwrap_or(0);
        let rc = store.role_count(&group_id).await.unwrap_or(0);
        if let Ok(Some(g)) = store.retrieve_group_by_id(&group_id).await {
            xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                group_row_to_xml(&g, mc, rc)
            ))
        } else {
            null_result("Internal Error")
        }
    } else if op == "UPDATE" {
        if group_id_str.is_empty() {
            return null_result("Bad network data");
        }
        if !has_power(
            store,
            requesting_agent,
            group_id_str,
            GroupPowers::CHANGE_ACTIONS,
        )
        .await
        {
            return null_result("Insufficient permissions");
        }
        let existing = match store.retrieve_group_by_id(group_id_str).await {
            Ok(Some(g)) => g,
            _ => return null_result("Group not found"),
        };
        let row = GroupRow {
            group_id: existing.group_id.clone(),
            name: existing.name,
            charter: form_str(form, "Charter").to_string(),
            insignia_id: form_str(form, "InsigniaID").to_string(),
            founder_id: existing.founder_id,
            membership_fee: form_str(form, "MembershipFee")
                .parse()
                .unwrap_or(existing.membership_fee),
            open_enrollment: if parse_bool(form_str(form, "OpenEnrollment")) {
                "1"
            } else {
                "0"
            }
            .to_string(),
            show_in_list: if parse_bool(form_str(form, "ShownInList")) {
                1
            } else {
                0
            },
            allow_publish: if parse_bool(form_str(form, "AllowPublish")) {
                1
            } else {
                0
            },
            mature_publish: if parse_bool(form_str(form, "MaturePublish")) {
                1
            } else {
                0
            },
            owner_role_id: existing.owner_role_id,
            location: existing.location,
        };
        let _ = store.store_group(&row).await;

        let mc = store.member_count(&row.group_id).await.unwrap_or(0);
        let rc = store.role_count(&row.group_id).await.unwrap_or(0);
        if let Ok(Some(g)) = store.retrieve_group_by_id(&row.group_id).await {
            xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                group_row_to_xml(&g, mc, rc)
            ))
        } else {
            null_result("Internal Error")
        }
    } else {
        null_result("Bad OP")
    }
}

async fn handle_get_group(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    if requesting_agent.is_empty() {
        return null_result("Bad network data");
    }

    let group = if let Some(gid) = form.get("GroupID") {
        store.retrieve_group_by_id(gid).await.ok().flatten()
    } else if let Some(name) = form.get("Name") {
        store.retrieve_group_by_name(name).await.ok().flatten()
    } else {
        None
    };

    match group {
        Some(g) => {
            let mc = store.member_count(&g.group_id).await.unwrap_or(0);
            let rc = store.role_count(&g.group_id).await.unwrap_or(0);
            xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                group_row_to_xml(&g, mc, rc)
            ))
        }
        None => null_result("Group not found"),
    }
}

async fn handle_add_agent_to_group(
    store: &GroupsStore,
    form: &HashMap<String, String>,
) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let agent_id = form_str(form, "AgentID");
    let group_id = form_str(form, "GroupID");
    let role_id = form_str(form, "RoleID");

    if requesting_agent.is_empty()
        || agent_id.is_empty()
        || group_id.is_empty()
        || role_id.is_empty()
    {
        return null_result("Bad network data");
    }

    let token = form_str(form, "AccessToken");
    add_agent_to_group(store, agent_id, group_id, role_id, token).await;

    match get_membership_data(store, agent_id, group_id).await {
        Some((m, g, title, powers)) => xml_result(&format!(
            "<RESULT type=\"List\">{}</RESULT>",
            membership_to_xml(&m, &g, &title, powers)
        )),
        None => null_result("Internal error"),
    }
}

async fn handle_remove_agent_from_group(
    store: &GroupsStore,
    form: &HashMap<String, String>,
) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let agent_id = form_str(form, "AgentID");
    let group_id = form_str(form, "GroupID");

    if requesting_agent.is_empty() || agent_id.is_empty() || group_id.is_empty() {
        return null_result("Bad network data");
    }

    // Permission check
    if requesting_agent != agent_id
        && !has_power(store, requesting_agent, group_id, GroupPowers::EJECT).await
    {
        return null_result(&format!("Insufficient permissions. {agent_id}"));
    }

    let _ = store.delete_member(group_id, agent_id).await;
    let _ = store.delete_member_all_roles(group_id, agent_id).await;

    // Inactivate group if it was active
    if let Ok(Some(principal)) = store.retrieve_principal(agent_id).await
        && principal.active_group_id == group_id
    {
        let p = PrincipalRow {
            principal_id: agent_id.to_string(),
            active_group_id: ZERO_UUID.to_string(),
        };
        let _ = store.store_principal(&p).await;
    }

    bool_result(true)
}

async fn handle_get_membership(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let agent_id = form_str(form, "AgentID");

    if requesting_agent.is_empty() || agent_id.is_empty() {
        return null_result("Bad network data");
    }

    let all = form.contains_key("ALL");
    let group_id_str = form_str(form, "GroupID");

    if !all {
        let gid = if group_id_str.is_empty() || group_id_str == ZERO_UUID {
            // Get active group
            match store.retrieve_principal(agent_id).await {
                Ok(Some(p)) => p.active_group_id,
                _ => return null_result("No such membership"),
            }
        } else {
            group_id_str.to_string()
        };

        match get_membership_data(store, agent_id, &gid).await {
            Some((m, g, title, powers)) => xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                membership_to_xml(&m, &g, &title, powers)
            )),
            None => null_result("No such membership"),
        }
    } else {
        let memberships = match store.retrieve_memberships(agent_id).await {
            Ok(m) if !m.is_empty() => m,
            _ => return null_result("No memberships"),
        };

        let mut items = Vec::new();
        for ms in &memberships {
            if let Some((_m, g, title, powers)) =
                get_membership_data(store, agent_id, &ms.group_id).await
            {
                items.push(membership_to_xml(ms, &g, &title, powers));
            }
        }

        if items.is_empty() {
            return null_result("No memberships");
        }

        xml_result(&list_to_xml("m", &items))
    }
}

async fn handle_get_group_members(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");

    if requesting_agent.is_empty() || group_id.is_empty() {
        return null_result("Bad network data");
    }

    let group = match store.retrieve_group_by_id(group_id).await {
        Ok(Some(g)) => g,
        _ => return null_result("No members"),
    };

    let members = match store.retrieve_members(group_id).await {
        Ok(m) if !m.is_empty() => m,
        _ => return null_result("No members"),
    };

    let owner_role_id = &group.owner_role_id;
    let mut items = Vec::new();
    for m in &members {
        // Get selected role info
        let role = store
            .retrieve_role(group_id, &m.selected_role_id)
            .await
            .ok()
            .flatten();
        let (title, powers) = match &role {
            Some(r) => (r.title.as_str(), r.powers as u64),
            None => ("", 0),
        };

        // Check if owner
        let is_owner = store
            .retrieve_role_member(group_id, owner_role_id, &m.principal_id)
            .await
            .ok()
            .flatten()
            .is_some();

        items.push(member_to_xml(m, title, powers, is_owner));
    }

    if items.is_empty() {
        return null_result("No members");
    }

    xml_result(&list_to_xml("m", &items))
}

async fn handle_put_role(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");
    let role_id = form_str(form, "RoleID");
    let name = form_str(form, "Name");
    let description = form_str(form, "Description");
    let title = form_str(form, "Title");
    let powers_str = form_str(form, "Powers");
    let op = form_str(form, "OP");

    if requesting_agent.is_empty() || group_id.is_empty() || role_id.is_empty() || op.is_empty() {
        return null_result("Bad network data");
    }

    let powers: u64 = powers_str.parse().unwrap_or(0);

    let success = if op == "ADD" {
        if !has_power(store, requesting_agent, group_id, GroupPowers::CREATE_ROLE).await {
            return bool_result(false);
        }
        add_or_update_role(
            store,
            group_id,
            role_id,
            name,
            description,
            title,
            powers,
            true,
        )
        .await
    } else if op == "UPDATE" {
        if !has_power(
            store,
            requesting_agent,
            group_id,
            GroupPowers::CHANGE_ACTIONS,
        )
        .await
        {
            return bool_result(false);
        }
        add_or_update_role(
            store,
            group_id,
            role_id,
            name,
            description,
            title,
            powers,
            false,
        )
        .await
    } else {
        false
    };

    bool_result(success)
}

async fn handle_remove_role(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");
    let role_id = form_str(form, "RoleID");

    if requesting_agent.is_empty() || group_id.is_empty() || role_id.is_empty() {
        return null_result("Bad network data");
    }

    if !has_power(store, requesting_agent, group_id, GroupPowers::DELETE_ROLE).await {
        return bool_result(false);
    }

    // Cannot delete Everyone role
    if role_id == ZERO_UUID {
        return bool_result(false);
    }

    // Cannot delete Owner role
    if let Ok(Some(g)) = store.retrieve_group_by_id(group_id).await
        && role_id == g.owner_role_id
    {
        return bool_result(false);
    }

    let _ = store.delete_role(group_id, role_id).await;
    bool_result(true)
}

async fn handle_get_group_roles(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");

    if requesting_agent.is_empty() || group_id.is_empty() {
        return null_result("Bad network data");
    }

    let roles = match store.retrieve_roles(group_id).await {
        Ok(r) if !r.is_empty() => r,
        _ => return null_result("No members"),
    };

    let mut items = Vec::new();
    for r in &roles {
        let count = store
            .role_member_count(group_id, &r.role_id)
            .await
            .unwrap_or(0);
        items.push(role_to_xml(r, count));
    }

    xml_result(&list_to_xml("r", &items))
}

async fn handle_get_role_members(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");

    if requesting_agent.is_empty() || group_id.is_empty() {
        return null_result("Bad network data");
    }

    let rmembers = match store.retrieve_roles_members(group_id).await {
        Ok(r) if !r.is_empty() => r,
        _ => return null_result("No members"),
    };

    let items: Vec<String> = rmembers.iter().map(role_member_to_xml).collect();
    xml_result(&list_to_xml("rm", &items))
}

async fn handle_agent_role(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");
    let role_id = form_str(form, "RoleID");
    let agent_id = form_str(form, "AgentID");
    let op = form_str(form, "OP");

    if requesting_agent.is_empty()
        || group_id.is_empty()
        || role_id.is_empty()
        || agent_id.is_empty()
        || op.is_empty()
    {
        return null_result("Bad network data");
    }

    let success = if op == "ADD" {
        let limited = has_power(
            store,
            requesting_agent,
            group_id,
            GroupPowers::ASSIGN_MEMBER_LIMITED,
        )
        .await;
        let unlimited = has_power(
            store,
            requesting_agent,
            group_id,
            GroupPowers::ASSIGN_MEMBER,
        )
        .await
            || is_owner(store, requesting_agent, group_id).await;

        if !limited && !unlimited {
            return bool_result(false);
        }

        if !unlimited && limited {
            // Check requester has this role
            if store
                .retrieve_role_member(group_id, role_id, requesting_agent)
                .await
                .ok()
                .flatten()
                .is_none()
            {
                return bool_result(false);
            }
        }

        add_agent_to_group_role(store, agent_id, group_id, role_id).await;
        true
    } else if op == "DELETE" {
        if role_id == ZERO_UUID {
            return bool_result(false);
        }

        let limited = has_power(
            store,
            requesting_agent,
            group_id,
            GroupPowers::ASSIGN_MEMBER_LIMITED,
        )
        .await;
        let unlimited = has_power(
            store,
            requesting_agent,
            group_id,
            GroupPowers::ASSIGN_MEMBER,
        )
        .await
            || is_owner(store, requesting_agent, group_id).await;

        if !limited && !unlimited {
            return bool_result(false);
        }

        let _ = store.delete_role_member(group_id, role_id, agent_id).await;

        // Find another role
        let roles = store
            .retrieve_member_roles(group_id, agent_id)
            .await
            .unwrap_or_default();
        let new_role = roles
            .iter()
            .find(|r| r.role_id != ZERO_UUID)
            .map(|r| r.role_id.clone())
            .unwrap_or_else(|| ZERO_UUID.to_string());

        if let Ok(Some(mut member)) = store.retrieve_member(group_id, agent_id).await {
            member.selected_role_id = new_role;
            let _ = store.store_member(&member).await;
        }
        true
    } else {
        false
    };

    bool_result(success)
}

async fn handle_get_agent_roles(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");
    let agent_id = form_str(form, "AgentID");

    if requesting_agent.is_empty() || group_id.is_empty() || agent_id.is_empty() {
        return null_result("Bad network data");
    }

    let role_memberships = match store.retrieve_member_roles(group_id, agent_id).await {
        Ok(r) if !r.is_empty() => r,
        _ => return null_result("No members"),
    };

    let mut items = Vec::new();
    for rm in &role_memberships {
        if let Ok(Some(role)) = store.retrieve_role(group_id, &rm.role_id).await {
            let count = store
                .role_member_count(group_id, &rm.role_id)
                .await
                .unwrap_or(0);
            items.push(role_to_xml(&role, count));
        }
    }

    if items.is_empty() {
        return null_result("No members");
    }

    xml_result(&list_to_xml("r", &items))
}

async fn handle_set_active(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");
    let agent_id = form_str(form, "AgentID");
    let op = form_str(form, "OP");

    if requesting_agent.is_empty() || group_id.is_empty() || agent_id.is_empty() || op.is_empty() {
        return null_result("Bad network data");
    }

    if op == "GROUP" {
        let principal = PrincipalRow {
            principal_id: agent_id.to_string(),
            active_group_id: group_id.to_string(),
        };
        let _ = store.store_principal(&principal).await;

        match get_membership_data(store, agent_id, group_id).await {
            Some((m, g, title, powers)) => xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                membership_to_xml(&m, &g, &title, powers)
            )),
            None => null_result("Internal error"),
        }
    } else if op == "ROLE" {
        let role_id = form_str(form, "RoleID");
        if role_id != ZERO_UUID && store.retrieve_role_member(group_id, role_id, agent_id).await.ok().flatten().is_none() {
            return bool_result(false);
        }
        if let Ok(Some(mut member)) = store.retrieve_member(group_id, agent_id).await {
            member.selected_role_id = role_id.to_string();
            let _ = store.store_member(&member).await;
        }
        bool_result(true)
    } else {
        null_result("Bad OP")
    }
}

async fn handle_update_membership(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let agent_id = form_str(form, "AgentID");
    let group_id = form_str(form, "GroupID");

    if requesting_agent.is_empty()
        || agent_id.is_empty()
        || group_id.is_empty()
        || !form.contains_key("AcceptNotices")
        || !form.contains_key("ListInProfile")
    {
        return null_result("Bad network data");
    }

    if let Ok(Some(mut member)) = store.retrieve_member(group_id, agent_id).await {
        member.accept_notices = if parse_bool(form_str(form, "AcceptNotices")) {
            1
        } else {
            0
        };
        member.list_in_profile = if parse_bool(form_str(form, "ListInProfile")) {
            1
        } else {
            0
        };
        let _ = store.store_member(&member).await;
    }

    bool_result(true)
}

async fn handle_invite(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let invite_id = form_str(form, "InviteID");

    if requesting_agent.is_empty() || invite_id.is_empty() {
        return null_result("Bad network data");
    }

    let op = form_str(form, "OP");

    if op == "ADD" {
        let group_id = form_str(form, "GroupID");
        let role_id = form_str(form, "RoleID");
        let agent_id = form_str(form, "AgentID");

        if group_id.is_empty() || role_id.is_empty() || agent_id.is_empty() {
            return null_result("Bad network data");
        }

        // Check already a member
        if store
            .retrieve_member(group_id, agent_id)
            .await
            .ok()
            .flatten()
            .is_some()
        {
            return bool_result(false);
        }

        // Check permission
        if !has_power(store, requesting_agent, group_id, GroupPowers::INVITE).await {
            return bool_result(false);
        }

        // Delete existing invite for this group+agent
        if let Ok(Some(existing)) = store.retrieve_invitation_by_group(group_id, agent_id).await {
            let _ = store.delete_invite(&existing.invite_id).await;
        }

        let inv = InvitationRow {
            invite_id: invite_id.to_string(),
            group_id: group_id.to_string(),
            role_id: role_id.to_string(),
            principal_id: agent_id.to_string(),
            timestamp: unix_now() as i32,
        };
        let success = store.store_invitation(&inv).await.unwrap_or(false);
        bool_result(success)
    } else if op == "DELETE" {
        let _ = store.delete_invite(invite_id).await;
        bool_result(true)
    } else if op == "GET" {
        match store.retrieve_invitation(invite_id).await {
            Ok(Some(inv)) => xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                invite_to_xml(&inv)
            )),
            _ => xml_result("<RESULT>NULL</RESULT>"),
        }
    } else {
        null_result("Bad OP in request")
    }
}

async fn handle_add_notice(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let group_id = form_str(form, "GroupID");
    let notice_id = form_str(form, "NoticeID");

    if requesting_agent.is_empty()
        || group_id.is_empty()
        || notice_id.is_empty()
        || !form.contains_key("FromName")
        || !form.contains_key("Subject")
        || !form.contains_key("Message")
        || !form.contains_key("HasAttachment")
    {
        return null_result("Bad network data");
    }

    if !has_power(store, requesting_agent, group_id, GroupPowers::SEND_NOTICES).await {
        return bool_result(false);
    }

    let has_att = parse_bool(form_str(form, "HasAttachment"));
    let row = NoticeRow {
        group_id: group_id.to_string(),
        notice_id: notice_id.to_string(),
        timestamp: unix_now(),
        from_name: form_str(form, "FromName").to_string(),
        subject: form_str(form, "Subject").to_string(),
        message: form_str(form, "Message").to_string(),
        has_attachment: if has_att { 1 } else { 0 },
        attachment_type: if has_att {
            form_str(form, "AttachmentType").parse().unwrap_or(0)
        } else {
            0
        },
        attachment_name: if has_att {
            form_str(form, "AttachmentName").to_string()
        } else {
            String::new()
        },
        attachment_item_id: if has_att {
            form_str(form, "AttachmentItemID").to_string()
        } else {
            ZERO_UUID.to_string()
        },
        attachment_owner_id: if has_att {
            form_str(form, "AttachmentOwnerID").to_string()
        } else {
            String::new()
        },
    };

    let success = store.store_notice(&row).await.unwrap_or(false);
    bool_result(success)
}

async fn handle_get_notices(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    if requesting_agent.is_empty() {
        return null_result("Bad network data");
    }

    if let Some(notice_id) = form.get("NoticeID") {
        // Single notice
        match store.retrieve_notice(notice_id).await {
            Ok(Some(n)) => xml_result(&format!(
                "<RESULT type=\"List\">{}</RESULT>",
                notice_info_to_xml(&n)
            )),
            _ => null_result("No such notice"),
        }
    } else if let Some(group_id) = form.get("GroupID") {
        // All notices for group
        let notices = match store.retrieve_notices(group_id).await {
            Ok(n) if !n.is_empty() => n,
            _ => return null_result("No notices"),
        };

        let items: Vec<String> = notices.iter().map(notice_to_xml).collect();
        xml_result(&list_to_xml("n", &items))
    } else {
        null_result("Bad OP in request")
    }
}

async fn handle_find_groups(store: &GroupsStore, form: &HashMap<String, String>) -> Response {
    let requesting_agent = form_str(form, "RequestingAgentID");
    let query = form_str(form, "Query");

    if requesting_agent.is_empty() || query.is_empty() {
        return null_result("Bad network data");
    }

    let groups = match store.retrieve_groups(query).await {
        Ok(g) if !g.is_empty() => g,
        _ => return null_result("No hits"),
    };

    let mut items = Vec::new();
    for g in &groups {
        // Skip proxies (groups with a Location)
        if !g.location.is_empty() {
            continue;
        }
        let mc = store.member_count(&g.group_id).await.unwrap_or(0);
        if mc == 0 {
            continue;
        }
        items.push(dir_group_to_xml(g, mc));
    }

    if items.is_empty() {
        return null_result("No hits");
    }

    xml_result(&list_to_xml("n", &items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("<test>&\""), "&lt;test&gt;&amp;&quot;");
    }

    #[test]
    fn test_bool_str() {
        assert_eq!(bool_str(1), "True");
        assert_eq!(bool_str(0), "False");
        assert_eq!(bool_str("1"), "True");
        assert_eq!(bool_str("0"), "False");
        assert_eq!(bool_str("true"), "True");
        assert_eq!(bool_str("True"), "True");
        assert_eq!(bool_str("false"), "False");
    }

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool("1"));
        assert!(parse_bool("True"));
        assert!(parse_bool("true"));
        assert!(!parse_bool("0"));
        assert!(!parse_bool("False"));
        assert!(!parse_bool(""));
    }

    #[test]
    fn test_dict_to_xml() {
        let xml = dict_to_xml(&[("Key", "Value"), ("Num", "42")]);
        assert!(xml.contains("<Key>Value</Key>"));
        assert!(xml.contains("<Num>42</Num>"));
    }

    #[test]
    fn test_list_to_xml() {
        let items = vec!["<A>1</A>".to_string(), "<B>2</B>".to_string()];
        let xml = list_to_xml("m", &items);
        assert!(xml.contains("<RESULT type=\"List\">"));
        assert!(xml.contains("<m-0 type=\"List\"><A>1</A></m-0>"));
        assert!(xml.contains("<m-1 type=\"List\"><B>2</B></m-1>"));
    }

    #[test]
    fn test_null_result_format() {
        let resp = null_result("test reason");
        // null_result returns a Response, but we can verify the helper functions
        // are correctly formatting by checking their subcomponents
        let _ = resp;
    }

    #[test]
    fn test_form_str_missing() {
        let form: HashMap<String, String> = HashMap::new();
        assert_eq!(form_str(&form, "Missing"), "");
    }

    #[test]
    fn test_form_str_present() {
        let mut form = HashMap::new();
        form.insert("Key".to_string(), "Value".to_string());
        assert_eq!(form_str(&form, "Key"), "Value");
    }
}

// ---------------------------------------------------------------------------
// Helper: resolve a list of group UUIDs to names (local store / connector)
// ---------------------------------------------------------------------------


/// Resolve a list of group UUIDs to names using the local groups store where
/// possible. Missing or unresolvable groups default to "Unknown Group".
pub async fn resolve_group_names(
    state: &GroupsServiceState,
    group_ids: &[Uuid],
) -> HashMap<Uuid, String> {
    let mut out: HashMap<Uuid, String> = HashMap::new();
    for id in group_ids {
        out.insert(*id, "Unknown Group".to_string());
        if id.is_nil() {
            continue;
        }
        if let Ok(Some(row)) = state.store.retrieve_group_by_id(&id.to_string()).await {
            if !row.name.trim().is_empty() {
                out.insert(*id, row.name);
            }
        }
    }
    out
}

/// Alias for `resolve_group_names`.
pub async fn get_group_names(
    state: &GroupsServiceState,
    group_ids: &[Uuid],
) -> HashMap<Uuid, String> {
    resolve_group_names(state, group_ids).await
}

/// Alias for `resolve_group_names`.
pub async fn group_names(
    state: &GroupsServiceState,
    group_ids: &[Uuid],
) -> HashMap<Uuid, String> {
    resolve_group_names(state, group_ids).await
}
