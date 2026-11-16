// SPDX-License-Identifier: LGPL-3.0-or-later

//! groups are grid-only for now: Hypergrid agent ids of the form `uuid;http://home;First Last` are not parsed or supported by this connector.

use quick_xml::{Reader, events::Event};
use std::time::Duration;
use uuid::Uuid;

use super::RemoteError;

pub struct RemoteGroupsConnector {
    client: reqwest::Client,
    groups_uri: String,
}

impl RemoteGroupsConnector {
    pub fn new(server_uri: &str) -> Self {
        let base = server_uri.trim_end_matches('/');
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            groups_uri: format!("{base}/groups"),
        }
    }

    pub async fn create_group(
        &self,
        requesting_agent: Uuid,
        group: &GroupRecord,
    ) -> Result<GroupRecord, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "PUTGROUP".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("OP", "ADD".to_string()),
            ("GroupName", group.group_name.clone()),
            ("Charter", group.charter.clone()),
            ("InsigniaID", group.insignia_id.to_string()),
            ("FounderID", group.founder_id.to_string()),
            ("MembershipFee", group.membership_fee.to_string()),
            ("OpenEnrollment", group.open_enrollment.to_string()),
            ("ShownInList", group.show_in_list.to_string()),
            ("AllowPublish", group.allow_publish.to_string()),
            ("MaturePublish", group.mature_publish.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Err(RemoteError::Server(reason_text(&nodes)));
        }

        let result = result_node(&nodes).ok_or_else(|| RemoteError::Parse("missing RESULT".to_string()))?;
        Ok(parse_group(result)?)
    }

    pub async fn update_group(
        &self,
        requesting_agent: Uuid,
        group: &GroupRecord,
    ) -> Result<Option<GroupRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "PUTGROUP".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group.group_id.to_string()),
            ("OP", "UPDATE".to_string()),
            ("Charter", group.charter.clone()),
            ("InsigniaID", group.insignia_id.to_string()),
            ("MembershipFee", group.membership_fee.to_string()),
            ("OpenEnrollment", group.open_enrollment.to_string()),
            ("ShownInList", group.show_in_list.to_string()),
            ("AllowPublish", group.allow_publish.to_string()),
            ("MaturePublish", group.mature_publish.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(None);
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(None),
        };
        Ok(Some(parse_group(result)?))
    }

    pub async fn get_group(
        &self,
        requesting_agent: Uuid,
        group_id: Option<Uuid>,
        name: Option<&str>,
    ) -> Result<Option<GroupRecord>, RemoteError> {
        let mut params: Vec<(&str, String)> = vec![
            ("METHOD", "GETGROUP".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
        ];

        if let Some(id) = group_id {
            if !id.is_nil() {
                params.push(("GroupID", id.to_string()));
            }
        }
        if let Some(n) = name {
            if !n.is_empty() {
                params.push(("Name", n.to_string()));
            }
        }

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(None);
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(None),
        };
        Ok(Some(parse_group(result)?))
    }

    pub async fn add_agent_to_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        access_token: &str,
    ) -> Result<MembershipRecord, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "ADDAGENTTOGROUP".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("AgentID", agent_id.to_string()),
            ("GroupID", group_id.to_string()),
            ("RoleID", role_id.to_string()),
            ("AccessToken", access_token.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Err(RemoteError::Server(reason_text(&nodes)));
        }

        let result = result_node(&nodes).ok_or_else(|| RemoteError::Parse("missing RESULT".to_string()))?;
        Ok(parse_membership(result)?)
    }

    pub async fn remove_agent_from_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "REMOVEAGENTFROMGROUP".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("AgentID", agent_id.to_string()),
            ("GroupID", group_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Err(RemoteError::Server(reason_text(&nodes)));
        }
        Ok(())
    }

    pub async fn get_membership(
        &self,
        agent: Uuid,
        group_id: Option<Uuid>,
    ) -> Result<Vec<MembershipRecord>, RemoteError> {
        let mut params: Vec<(&str, String)> = vec![
            ("METHOD", "GETMEMBERSHIP".to_string()),
            ("RequestingAgentID", agent.to_string()),
            ("AgentID", agent.to_string()),
        ];

        if let Some(id) = group_id {
            if !id.is_nil() {
                params.push(("GroupID", id.to_string()));
            }
        }

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let mut out = Vec::new();
        let items = prefixed_children(result, "m");
        if items.is_empty() {
            if field(result, "GroupID") != "" || field(result, "GroupName") != "" {
                out.push(parse_membership(result)?);
            }
        } else {
            for item in items {
                out.push(parse_membership(item)?);
            }
        }
        Ok(out)
    }

    pub async fn get_all_memberships(&self, agent: Uuid) -> Result<Vec<MembershipRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETMEMBERSHIP".to_string()),
            ("RequestingAgentID", agent.to_string()),
            ("AgentID", agent.to_string()),
            ("ALL", "true".to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let mut out = Vec::new();
        let items = prefixed_children(result, "m");
        if items.is_empty() {
            if field(result, "GroupID") != "" || field(result, "GroupName") != "" {
                out.push(parse_membership(result)?);
            }
        } else {
            for item in items {
                out.push(parse_membership(item)?);
            }
        }
        Ok(out)
    }

    pub async fn get_group_members(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<MemberRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETGROUPMEMBERS".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let items = prefixed_children(result, "m");
        if items.is_empty() {
            Ok(Vec::new())
        } else {
            let mut out = Vec::new();
            for item in items {
                out.push(parse_member(item)?);
            }
            Ok(out)
        }
    }

    pub async fn put_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        name: &str,
        description: &str,
        title: &str,
        powers: u64,
        op: &str,
    ) -> Result<bool, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "PUTROLE".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
            ("RoleID", role_id.to_string()),
            ("Name", name.to_string()),
            ("Description", description.to_string()),
            ("Title", title.to_string()),
            ("Powers", powers.to_string()),
            ("OP", op.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(false);
        }
        Ok(parse_bool(&result_text(&nodes))?)
    }

    pub async fn remove_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    ) -> Result<bool, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "REMOVEROLE".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
            ("RoleID", role_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(false);
        }
        Ok(parse_bool(&result_text(&nodes))?)
    }

    pub async fn get_group_roles(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<RoleRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETGROUPROLES".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let items = prefixed_children(result, "r");
        let mut out = Vec::new();
        for item in items {
            out.push(parse_role(item)?);
        }
        Ok(out)
    }

    pub async fn get_role_members(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<RoleMemberRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETROLEMEMBERS".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let items = prefixed_children(result, "rm");
        let mut out = Vec::new();
        for item in items {
            out.push(parse_role_member(item)?);
        }
        Ok(out)
    }

    pub async fn agent_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        op: &str,
    ) -> Result<bool, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "AGENTROLE".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("AgentID", agent_id.to_string()),
            ("GroupID", group_id.to_string()),
            ("RoleID", role_id.to_string()),
            ("OP", op.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(false);
        }
        Ok(parse_bool(&result_text(&nodes))?)
    }

    pub async fn get_agent_roles(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<RoleRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETAGENTROLES".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("AgentID", agent_id.to_string()),
            ("GroupID", group_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let items = prefixed_children(result, "r");
        let mut out = Vec::new();
        for item in items {
            out.push(parse_role(item)?);
        }
        Ok(out)
    }

    pub async fn set_active(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        op: &str,
        role_id: Option<Uuid>,
    ) -> Result<Option<MembershipRecord>, RemoteError> {
        let mut params: Vec<(&str, String)> = vec![
            ("METHOD", "SETACTIVE".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("AgentID", agent_id.to_string()),
            ("GroupID", group_id.to_string()),
            ("OP", op.to_string()),
        ];

        if op.eq_ignore_ascii_case("ROLE") {
            params.push((
                "RoleID",
                role_id.unwrap_or_else(Uuid::nil).to_string(),
            ));
        }

        let nodes = self.call(&params).await?;
        if op.eq_ignore_ascii_case("GROUP") {
            if result_text(&nodes).eq_ignore_ascii_case("NULL") {
                return Ok(None);
            }
            let result =
                result_node(&nodes).ok_or_else(|| RemoteError::Parse("missing RESULT".to_string()))?;
            Ok(Some(parse_membership(result)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        accept_notices: bool,
        list_in_profile: bool,
    ) -> Result<bool, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "UPDATEMEMBERSHIP".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("AgentID", agent_id.to_string()),
            ("GroupID", group_id.to_string()),
            ("AcceptNotices", accept_notices.to_string()),
            ("ListInProfile", list_in_profile.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(false);
        }
        Ok(parse_bool(&result_text(&nodes))?)
    }

    pub async fn invite(
        &self,
        requesting_agent: Uuid,
        op: &str,
        invite_id: Uuid,
        group_id: Option<Uuid>,
        role_id: Option<Uuid>,
        agent_id: Option<Uuid>,
    ) -> Result<InviteResult, RemoteError> {
        let mut params: Vec<(&str, String)> = vec![
            ("METHOD", "INVITE".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("InviteID", invite_id.to_string()),
            ("OP", op.to_string()),
        ];

        if op.eq_ignore_ascii_case("ADD") {
            if let Some(id) = group_id {
                params.push(("GroupID", id.to_string()));
            }
            if let Some(id) = role_id {
                params.push(("RoleID", id.to_string()));
            }
            if let Some(id) = agent_id {
                params.push(("AgentID", id.to_string()));
            }
        }

        let nodes = self.call(&params).await?;

        if op.eq_ignore_ascii_case("ADD") {
            if result_text(&nodes).eq_ignore_ascii_case("NULL") {
                Ok(InviteResult::Added(false))
            } else {
                Ok(InviteResult::Added(parse_bool(&result_text(&nodes))?))
            }
        } else if op.eq_ignore_ascii_case("GET") {
            if result_text(&nodes).eq_ignore_ascii_case("NULL") {
                Ok(InviteResult::Invite(None))
            } else {
                let result = match result_node(&nodes) {
                    Some(node) => node,
                    None => return Ok(InviteResult::Invite(None)),
                };
                Ok(InviteResult::Invite(Some(parse_invite(result)?)))
            }
        } else {
            Ok(InviteResult::Deleted)
        }
    }

    pub async fn add_notice(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        notice_id: Uuid,
        from_name: &str,
        subject: &str,
        message: &str,
        has_attachment: bool,
        attachment_type: u8,
        attachment_name: &str,
        attachment_item_id: Uuid,
        attachment_owner_id: Uuid,
    ) -> Result<bool, RemoteError> {
        let mut params: Vec<(&str, String)> = vec![
            ("METHOD", "ADDNOTICE".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
            ("NoticeID", notice_id.to_string()),
            ("FromName", from_name.to_string()),
            ("Subject", subject.to_string()),
            ("Message", message.to_string()),
            ("HasAttachment", has_attachment.to_string()),
        ];

        if has_attachment {
            params.push(("AttachmentType", attachment_type.to_string()));
            params.push(("AttachmentName", attachment_name.to_string()));
            params.push(("AttachmentItemID", attachment_item_id.to_string()));
            params.push(("AttachmentOwnerID", attachment_owner_id.to_string()));
        }

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(false);
        }
        Ok(parse_bool(&result_text(&nodes))?)
    }

    pub async fn get_notices(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<NoticeRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETNOTICES".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("GroupID", group_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let items = prefixed_children(result, "n");
        let mut out = Vec::new();
        for item in items {
            out.push(parse_notice(item)?);
        }
        Ok(out)
    }

    pub async fn get_notice(
        &self,
        requesting_agent: Uuid,
        notice_id: Uuid,
    ) -> Result<Option<NoticeRecord>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "GETNOTICES".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("NoticeID", notice_id.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(None);
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(None),
        };

        Ok(Some(parse_notice(result)?))
    }

    pub async fn find_groups(
        &self,
        requesting_agent: Uuid,
        query: &str,
    ) -> Result<Vec<DirGroup>, RemoteError> {
        let params: Vec<(&str, String)> = vec![
            ("METHOD", "FINDGROUPS".to_string()),
            ("RequestingAgentID", requesting_agent.to_string()),
            ("Query", query.to_string()),
        ];

        let nodes = self.call(&params).await?;
        if result_text(&nodes).eq_ignore_ascii_case("NULL") {
            return Ok(Vec::new());
        }

        let result = match result_node(&nodes) {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        let items = prefixed_children(result, "n");
        let mut out = Vec::new();
        for item in items {
            out.push(parse_dir_group(item)?);
        }
        Ok(out)
    }

    async fn call(&self, params: &[(&str, String)]) -> Result<Vec<Node>, RemoteError> {
        let response = self
            .client
            .post(&self.groups_uri)
            .form(params)
            .send()
            .await
            .map_err(RemoteError::Http)?;
        let response = response.error_for_status().map_err(RemoteError::Http)?;
        let body = response.text().await.map_err(RemoteError::Http)?;
        let nodes = parse_xml(&body)?;
        if result_node(&nodes).is_none() {
            return Err(RemoteError::Parse("missing RESULT".to_string()));
        }
        Ok(nodes)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum InviteResult {
    Added(bool),
    Invite(Option<InviteRecord>),
    Deleted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupRecord {
    pub group_id: Uuid,
    pub group_name: String,
    pub charter: String,
    pub insignia_id: Uuid,
    pub founder_id: Uuid,
    pub membership_fee: i32,
    pub open_enrollment: bool,
    pub show_in_list: bool,
    pub allow_publish: bool,
    pub mature_publish: bool,
    pub owner_role_id: Uuid,
    pub member_count: i64,
    pub role_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MembershipRecord {
    pub group_id: Uuid,
    pub group_name: String,
    pub group_picture: Uuid,
    pub group_title: String,
    pub group_powers: u64,
    pub accept_notices: bool,
    pub list_in_profile: bool,
    pub contribution: i32,
    pub active_role: Uuid,
    pub active: bool,
    pub open_enrollment: bool,
    pub membership_fee: i32,
    pub charter: String,
    pub allow_publish: bool,
    pub mature_publish: bool,
    pub show_in_list: bool,
    pub founder_id: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberRecord {
    pub agent_id: Uuid,
    pub is_owner: bool,
    pub title: String,
    pub agent_powers: u64,
    pub accept_notices: bool,
    pub list_in_profile: bool,
    pub contribution: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoleRecord {
    pub role_id: Uuid,
    pub name: String,
    pub title: String,
    pub description: String,
    pub powers: u64,
    pub members: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoleMemberRecord {
    pub role_id: Uuid,
    pub member_id: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InviteRecord {
    pub invite_id: Uuid,
    pub group_id: Uuid,
    pub role_id: Uuid,
    pub agent_id: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoticeRecord {
    pub notice_id: Uuid,
    pub timestamp: i64,
    pub from_name: String,
    pub subject: String,
    pub has_attachment: bool,
    pub attachment_item_id: Uuid,
    pub attachment_name: String,
    pub attachment_type: u8,
    pub attachment_owner_id: Uuid,
    pub group_id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirGroup {
    pub group_id: Uuid,
    pub name: String,
    pub member_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Element {
        name: String,
        attrs: Vec<(String, String)>,
        children: Vec<Node>,
    },
    Text(String),
}

impl Node {
    fn name(&self) -> &str {
        match self {
            Node::Element { name, .. } => name.as_str(),
            Node::Text(_) => "",
        }
    }

    fn children(&self) -> &[Node] {
        match self {
            Node::Element { children, .. } => children.as_slice(),
            Node::Text(_) => &[],
        }
    }

    #[allow(dead_code)]
    fn attr(&self, key: &str) -> Option<String> {
        match self {
            Node::Element { attrs, .. } => attrs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone()),
            Node::Text(_) => None,
        }
    }

    fn text_content(&self) -> String {
        match self {
            Node::Text(s) => s.clone(),
            Node::Element { children, .. } => {
                children.iter().map(Node::text_content).collect::<Vec<_>>().join("")
            }
        }
    }
}

fn parse_xml(input: &str) -> Result<Vec<Node>, RemoteError> {
    if input.trim().is_empty() {
        return Err(RemoteError::Parse("empty body".to_string()));
    }

    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<(String, Vec<(String, String)>, Vec<Node>)> = Vec::new();
    let mut top: Vec<Node> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let mut attrs = Vec::new();
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = attr.unescape_value().map(|v| v.into_owned()).unwrap_or_default();
                        attrs.push((key, value));
                    }
                }
                stack.push((name, attrs, Vec::new()));
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let mut attrs = Vec::new();
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = attr.unescape_value().map(|v| v.into_owned()).unwrap_or_default();
                        attrs.push((key, value));
                    }
                }
                let node = Node::Element {
                    name,
                    attrs,
                    children: Vec::new(),
                };
                if let Some((_, _, children)) = stack.last_mut() {
                    children.push(node);
                } else {
                    top.push(node);
                }
            }
            Ok(Event::Text(t)) => {
                let text = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                if !text.is_empty() {
                    if let Some((_, _, children)) = stack.last_mut() {
                        children.push(Node::Text(text));
                    } else {
                        top.push(Node::Text(text));
                    }
                }
            }
            Ok(Event::CData(t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).to_string();
                if !text.is_empty() {
                    if let Some((_, _, children)) = stack.last_mut() {
                        children.push(Node::Text(text));
                    } else {
                        top.push(Node::Text(text));
                    }
                }
            }
            Ok(Event::End(e)) => {
                let end_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if let Some((name, attrs, children)) = stack.pop() {
                    if name != end_name {
                        return Err(RemoteError::Parse(format!(
                            "mismatched end tag: expected </{name}>, found </{end_name}>"
                        )));
                    }
                    let node = Node::Element {
                        name,
                        attrs,
                        children,
                    };
                    if let Some((_, _, parent_children)) = stack.last_mut() {
                        parent_children.push(node);
                    } else {
                        top.push(node);
                    }
                } else {
                    return Err(RemoteError::Parse(format!(
                        "unexpected closing tag </{end_name}>"
                    )));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(RemoteError::Parse(e.to_string()));
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        return Err(RemoteError::Parse("unexpected EOF while parsing XML".to_string()));
    }

    if top.len() == 1 {
        if let Node::Element { name, children, .. } = &top[0] {
            if name == "ServerResponse" {
                return Ok(children.clone());
            }
        }
    }
    Ok(top)
}

fn result_node(nodes: &[Node]) -> Option<&Node> {
    nodes.iter().find(|n| n.name() == "RESULT")
}

fn result_text(nodes: &[Node]) -> String {
    result_node(nodes).map(|n| n.text_content()).unwrap_or_default()
}

fn reason_text(nodes: &[Node]) -> String {
    nodes
        .iter()
        .find(|n| n.name() == "REASON")
        .map(|n| n.text_content())
        .unwrap_or_default()
}

fn field(node: &Node, name: &str) -> String {
    node.children()
        .iter()
        .find(|c| c.name() == name)
        .map(|c| c.text_content())
        .unwrap_or_default()
}

fn parse_bool(value: &str) -> Result<bool, RemoteError> {
    if value.is_empty() {
        Ok(false)
    } else if value == "1" || value.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if value == "0" || value.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err(RemoteError::Parse(format!("invalid bool value: {value}")))
    }
}

fn parse_uuid(value: &str) -> Uuid {
    if value.is_empty() {
        Uuid::nil()
    } else {
        Uuid::parse_str(value).unwrap_or_else(|_| Uuid::nil())
    }
}

fn parse_required_uuid(value: &str, field: &str) -> Result<Uuid, RemoteError> {
    if value.is_empty() {
        return Err(RemoteError::Parse(format!("missing {field}")));
    }
    Uuid::parse_str(value).map_err(|_| RemoteError::Parse(format!("invalid {field}")))
}

fn parse_i32(value: &str, field: &str) -> Result<i32, RemoteError> {
    if value.is_empty() {
        Ok(0)
    } else {
        value.parse().map_err(|_| RemoteError::Parse(format!("invalid {field}")))
    }
}

fn parse_i64(value: &str, field: &str) -> Result<i64, RemoteError> {
    if value.is_empty() {
        Ok(0)
    } else {
        value.parse().map_err(|_| RemoteError::Parse(format!("invalid {field}")))
    }
}

fn parse_u64(value: &str, field: &str) -> Result<u64, RemoteError> {
    if value.is_empty() {
        Ok(0)
    } else {
        value.parse().map_err(|_| RemoteError::Parse(format!("invalid {field}")))
    }
}

fn parse_u8(value: &str, field: &str) -> Result<u8, RemoteError> {
    if value.is_empty() {
        Ok(0)
    } else {
        value.parse().map_err(|_| RemoteError::Parse(format!("invalid {field}")))
    }
}

fn prefixed_children<'a>(node: &'a Node, prefix: &str) -> Vec<&'a Node> {
    let tag_prefix = format!("{prefix}-");
    node.children()
        .iter()
        .filter(|child| {
            match child.name().strip_prefix(&tag_prefix) {
                Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
                None => false,
            }
        })
        .collect()
}

fn parse_group(node: &Node) -> Result<GroupRecord, RemoteError> {
    Ok(GroupRecord {
        group_id: parse_required_uuid(&field(node, "GroupID"), "GroupID")?,
        group_name: field(node, "GroupName"),
        charter: field(node, "Charter"),
        insignia_id: parse_uuid(&field(node, "InsigniaID")),
        founder_id: parse_required_uuid(&field(node, "FounderID"), "FounderID")?,
        membership_fee: parse_i32(&field(node, "MembershipFee"), "MembershipFee")?,
        open_enrollment: parse_bool(&field(node, "OpenEnrollment"))?,
        show_in_list: parse_bool(&field(node, "ShownInList"))?,
        allow_publish: parse_bool(&field(node, "AllowPublish"))?,
        mature_publish: parse_bool(&field(node, "MaturePublish"))?,
        owner_role_id: parse_required_uuid(&field(node, "OwnerRoleID"), "OwnerRoleID")?,
        member_count: parse_i64(&field(node, "MemberCount"), "MemberCount")?,
        role_count: parse_i64(&field(node, "RoleCount"), "RoleCount")?,
    })
}

fn parse_membership(node: &Node) -> Result<MembershipRecord, RemoteError> {
    Ok(MembershipRecord {
        group_id: parse_required_uuid(&field(node, "GroupID"), "GroupID")?,
        group_name: field(node, "GroupName"),
        group_picture: parse_uuid(&field(node, "GroupPicture")),
        group_title: field(node, "GroupTitle"),
        group_powers: parse_u64(&field(node, "GroupPowers"), "GroupPowers")?,
        accept_notices: parse_bool(&field(node, "AcceptNotices"))?,
        list_in_profile: parse_bool(&field(node, "ListInProfile"))?,
        contribution: parse_i32(&field(node, "Contribution"), "Contribution")?,
        active_role: parse_uuid(&field(node, "ActiveRole")),
        active: parse_bool(&field(node, "Active"))?,
        open_enrollment: parse_bool(&field(node, "OpenEnrollment"))?,
        membership_fee: parse_i32(&field(node, "MembershipFee"), "MembershipFee")?,
        charter: field(node, "Charter"),
        allow_publish: parse_bool(&field(node, "AllowPublish"))?,
        mature_publish: parse_bool(&field(node, "MaturePublish"))?,
        show_in_list: parse_bool(&field(node, "ShowInList"))?,
        founder_id: parse_uuid(&field(node, "FounderID")),
    })
}

fn parse_member(node: &Node) -> Result<MemberRecord, RemoteError> {
    Ok(MemberRecord {
        agent_id: parse_required_uuid(&field(node, "AgentID"), "AgentID")?,
        is_owner: parse_bool(&field(node, "IsOwner"))?,
        title: field(node, "Title"),
        agent_powers: parse_u64(&field(node, "AgentPowers"), "AgentPowers")?,
        accept_notices: parse_bool(&field(node, "AcceptNotices"))?,
        list_in_profile: parse_bool(&field(node, "ListInProfile"))?,
        contribution: parse_i32(&field(node, "Contribution"), "Contribution")?,
    })
}

fn parse_role(node: &Node) -> Result<RoleRecord, RemoteError> {
    Ok(RoleRecord {
        role_id: parse_required_uuid(&field(node, "RoleID"), "RoleID")?,
        name: field(node, "Name"),
        title: field(node, "Title"),
        description: field(node, "Description"),
        powers: parse_u64(&field(node, "Powers"), "Powers")?,
        members: parse_i64(&field(node, "Members"), "Members")?,
    })
}

fn parse_role_member(node: &Node) -> Result<RoleMemberRecord, RemoteError> {
    Ok(RoleMemberRecord {
        role_id: parse_required_uuid(&field(node, "RoleID"), "RoleID")?,
        member_id: parse_required_uuid(&field(node, "MemberID"), "MemberID")?,
    })
}

fn parse_invite(node: &Node) -> Result<InviteRecord, RemoteError> {
    Ok(InviteRecord {
        invite_id: parse_required_uuid(&field(node, "InviteID"), "InviteID")?,
        group_id: parse_required_uuid(&field(node, "GroupID"), "GroupID")?,
        role_id: parse_uuid(&field(node, "RoleID")),
        agent_id: parse_required_uuid(&field(node, "AgentID"), "AgentID")?,
    })
}

fn parse_notice(node: &Node) -> Result<NoticeRecord, RemoteError> {
    Ok(NoticeRecord {
        notice_id: parse_required_uuid(&field(node, "NoticeID"), "NoticeID")?,
        timestamp: parse_i64(&field(node, "Timestamp"), "Timestamp")?,
        from_name: field(node, "FromName"),
        subject: field(node, "Subject"),
        has_attachment: parse_bool(&field(node, "HasAttachment"))?,
        attachment_item_id: parse_uuid(&field(node, "AttachmentItemID")),
        attachment_name: field(node, "AttachmentName"),
        attachment_type: parse_u8(&field(node, "AttachmentType"), "AttachmentType")?,
        attachment_owner_id: parse_uuid(&field(node, "AttachmentOwnerID")),
        group_id: parse_uuid(&field(node, "GroupID")),
        message: field(node, "Message"),
    })
}

fn parse_dir_group(node: &Node) -> Result<DirGroup, RemoteError> {
    Ok(DirGroup {
        group_id: parse_required_uuid(&field(node, "GroupID"), "GroupID")?,
        name: field(node, "Name"),
        member_count: parse_i64(&field(node, "NMembers"), "NMembers")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::NamedTempFile;
    use welkin_data::groups::GroupsStore;

    struct DbGuard {
        path: PathBuf,
    }

    impl Drop for DbGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    const DDL: &[&str] = &[
        "CREATE TABLE IF NOT EXISTS os_groups_groups (GroupID CHAR(36) PRIMARY KEY, Location VARCHAR(255) NOT NULL DEFAULT '', Name VARCHAR(255) NOT NULL DEFAULT '', Charter TEXT NOT NULL DEFAULT '', InsigniaID CHAR(36) NOT NULL DEFAULT '', FounderID CHAR(36) NOT NULL DEFAULT '', MembershipFee INTEGER NOT NULL DEFAULT 0, OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', ShowInList INTEGER NOT NULL DEFAULT 0, AllowPublish INTEGER NOT NULL DEFAULT 0, MaturePublish INTEGER NOT NULL DEFAULT 0, OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_membership (GroupID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, SelectedRoleID CHAR(36) NOT NULL DEFAULT '', Contribution INTEGER NOT NULL DEFAULT 0, ListInProfile INTEGER NOT NULL DEFAULT 1, AcceptNotices INTEGER NOT NULL DEFAULT 1, AccessToken CHAR(36) NOT NULL DEFAULT '', PRIMARY KEY (GroupID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_roles (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, Name VARCHAR(255) NOT NULL DEFAULT '', Description VARCHAR(255) NOT NULL DEFAULT '', Title VARCHAR(255) NOT NULL DEFAULT '', Powers BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (GroupID, RoleID))",
        "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, PRIMARY KEY (GroupID, RoleID, PrincipalID))",
        "CREATE TABLE IF NOT EXISTS os_groups_invites (InviteID CHAR(36) PRIMARY KEY, GroupID CHAR(36) NOT NULL, RoleID CHAR(36) NOT NULL, PrincipalID VARCHAR(255) NOT NULL, TMStamp INTEGER NOT NULL DEFAULT 0)",
        "CREATE TABLE IF NOT EXISTS os_groups_notices (GroupID CHAR(36) NOT NULL, NoticeID CHAR(36) PRIMARY KEY, TMStamp INTEGER NOT NULL DEFAULT 0, FromName VARCHAR(255) NOT NULL DEFAULT '', Subject VARCHAR(255) NOT NULL DEFAULT '', Message TEXT NOT NULL DEFAULT '', HasAttachment INTEGER NOT NULL DEFAULT 0, AttachmentType INTEGER NOT NULL DEFAULT 0, AttachmentName VARCHAR(128) NOT NULL DEFAULT '', AttachmentItemID CHAR(36) NOT NULL DEFAULT '', AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
        "CREATE TABLE IF NOT EXISTS os_groups_principals (PrincipalID VARCHAR(255) PRIMARY KEY, ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
    ];

    async fn setup() -> (RemoteGroupsConnector, DbGuard) {
        sqlx::any::install_default_drivers();
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);

        let guard = DbGuard { path: path.clone() };
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .unwrap();

        for ddl in DDL {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }

        let state = crate::groups::GroupsServiceState {
            store: Arc::new(GroupsStore::new(pool)),
        };
        let app = crate::groups::router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let conn = RemoteGroupsConnector::new(&format!("http://{}", addr));
        (conn, guard)
    }

    #[tokio::test]
    async fn create_and_get_group_roundtrip() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Test Group".to_string(),
            charter: "charter".to_string(),
            insignia_id: Uuid::new_v4(),
            founder_id: agent,
            membership_fee: 42,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: false,
            mature_publish: true,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        };

        let created = conn.create_group(agent, &group).await.unwrap();
        assert_eq!(created.group_name, group.group_name);
        assert_eq!(created.charter, group.charter);
        assert_eq!(created.membership_fee, group.membership_fee);
        assert_eq!(created.open_enrollment, group.open_enrollment);
        assert_eq!(created.show_in_list, group.show_in_list);
        assert_eq!(created.allow_publish, group.allow_publish);
        assert_eq!(created.mature_publish, group.mature_publish);
        assert_ne!(created.group_id, Uuid::nil());

        let by_id = conn
            .get_group(agent, Some(created.group_id), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_id, created);

        let by_name = conn
            .get_group(agent, None, Some(&group.group_name))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_name, created);

        let duplicate = conn.create_group(agent, &group).await;
        assert!(matches!(duplicate, Err(RemoteError::Server(_))));
    }

    #[tokio::test]
    async fn founder_membership_after_create() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Founder Group".to_string(),
            charter: "charter".to_string(),
            insignia_id: Uuid::nil(),
            founder_id: agent,
            membership_fee: 7,
            open_enrollment: false,
            show_in_list: true,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        };

        let created = conn.create_group(agent, &group).await.unwrap();
        let memberships = conn.get_membership(agent, Some(created.group_id)).await.unwrap();
        assert_eq!(memberships.len(), 1);
        assert_ne!(memberships[0].group_powers, 0);
    }

    #[tokio::test]
    async fn add_remove_group_members() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Members Group".to_string(),
            charter: "charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();

        let second = Uuid::new_v4();
        let _membership = conn
            .add_agent_to_group(agent, second, created.group_id, Uuid::nil(), "token")
            .await
            .unwrap();

        let members = conn
            .get_group_members(agent, created.group_id)
            .await
            .unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members.iter().filter(|m| m.is_owner).count(), 1);

        conn.remove_agent_from_group(agent, second, created.group_id)
            .await
            .unwrap();
        let members = conn
            .get_group_members(agent, created.group_id)
            .await
            .unwrap();
        assert_eq!(members.len(), 1);
    }

    #[tokio::test]
    async fn put_role_and_assignment() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Roles Group".to_string(),
            charter: "charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();
        let new_role = Uuid::new_v4();

        assert!(conn
            .put_role(
                agent,
                created.group_id,
                new_role,
                "NewRole",
                "description",
                "title",
                123,
                "ADD",
            )
            .await
            .unwrap());

        let roles = conn.get_group_roles(agent, created.group_id).await.unwrap();
        for expected in ["Everyone", "Officers", "Owners", "NewRole"] {
            assert!(roles.iter().any(|r| r.name == expected), "missing {expected}");
        }

        assert!(conn
            .agent_role(agent, agent, created.group_id, new_role, "ADD")
            .await
            .unwrap());
        let role_members = conn
            .get_role_members(agent, created.group_id)
            .await
            .unwrap();
        assert!(role_members
            .iter()
            .any(|rm| rm.role_id == new_role && rm.member_id == agent));
    }

    #[tokio::test]
    async fn invite_add_get_delete() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Invite Group".to_string(),
            charter: "charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();
        let invite_id = Uuid::new_v4();
        let invitee = Uuid::new_v4();

        match conn
            .invite(
                agent,
                "ADD",
                invite_id,
                Some(created.group_id),
                Some(Uuid::nil()),
                Some(invitee),
            )
            .await
            .unwrap()
        {
            InviteResult::Added(true) => {}
            other => panic!("unexpected invite add result: {:?}", other),
        }

        match conn
            .invite(agent, "GET", invite_id, None, None, None)
            .await
            .unwrap()
        {
            InviteResult::Invite(Some(inv)) => assert_eq!(inv.invite_id, invite_id),
            other => panic!("unexpected invite get result: {:?}", other),
        }

        conn.invite(agent, "DELETE", invite_id, None, None, None)
            .await
            .unwrap();

        match conn
            .invite(agent, "GET", invite_id, None, None, None)
            .await
            .unwrap()
        {
            InviteResult::Invite(None) => {}
            other => panic!("unexpected invite after delete: {:?}", other),
        }
    }

    #[tokio::test]
    async fn add_notice_and_list() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Notice Group".to_string(),
            charter: "charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();
        let notice_id = Uuid::new_v4();

        assert!(conn
            .add_notice(
                agent,
                created.group_id,
                notice_id,
                "From",
                "Subject",
                "Message",
                false,
                0,
                "",
                Uuid::nil(),
                Uuid::nil(),
            )
            .await
            .unwrap());

        let notices = conn.get_notices(agent, created.group_id).await.unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].notice_id, notice_id);
        assert_eq!(notices[0].subject, "Subject");

        let single = conn.get_notice(agent, notice_id).await.unwrap().unwrap();
        assert_eq!(single.message, "Message");
        assert_eq!(single.group_id, created.group_id);
    }

    #[tokio::test]
    async fn find_groups_by_prefix() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Prefix Group".to_string(),
            charter: "charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();

        let hits = conn.find_groups(agent, "Prefix").await.unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|g| g.group_id == created.group_id));
    }

    #[tokio::test]
    async fn set_active_and_update_membership() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Active Group".to_string(),
            charter: "charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();
        let role_id = Uuid::new_v4();
        assert!(conn
            .put_role(
                agent,
                created.group_id,
                role_id,
                "Role",
                "description",
                "title",
                1,
                "ADD",
            )
            .await
            .unwrap());

        let membership = conn
            .set_active(agent, agent, created.group_id, "GROUP", None)
            .await
            .unwrap();
        assert!(membership.is_some());

        conn.set_active(agent, agent, created.group_id, "ROLE", Some(role_id))
            .await
            .unwrap();

        assert!(conn
            .update_membership(agent, agent, created.group_id, false, false)
            .await
            .unwrap());
        let memberships = conn.get_membership(agent, Some(created.group_id)).await.unwrap();
        assert!(!memberships[0].accept_notices);
        assert!(!memberships[0].list_in_profile);
    }

    #[tokio::test]
    async fn update_group_changes_charter() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Update Group".to_string(),
            charter: "old charter".to_string(),
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
        let created = conn.create_group(agent, &group).await.unwrap();

        let mut updated = created.clone();
        updated.charter = "new charter".to_string();
        let result = conn.update_group(agent, &updated).await.unwrap().unwrap();
        assert_eq!(result.charter, "new charter");

        let fetched = conn
            .get_group(agent, Some(created.group_id), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.charter, "new charter");
    }

    #[test]
    fn parse_xml_literal() {
        let xml = "<?xml version=\"1.0\"?><ServerResponse><RESULT type=\"List\"><r-0 type=\"List\"><RoleID>12345678-1234-1234-1234-123456789abc</RoleID></r-0></RESULT></ServerResponse>";
        let nodes = parse_xml(xml).unwrap();
        let result = nodes.iter().find(|n| n.name() == "RESULT").unwrap();
        let items: Vec<&Node> = result
            .children()
            .iter()
            .filter(|n| n.name().starts_with("r-"))
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(field(items[0], "RoleID"), "12345678-1234-1234-1234-123456789abc");
    }

    async fn fixed_body_server(body: &'static str) -> RemoteGroupsConnector {
        let app = axum::Router::new().route("/groups", axum::routing::post(move || async move { body }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        RemoteGroupsConnector::new(&format!("http://{}", addr))
    }

    #[tokio::test]
    async fn malformed_bodies_are_errors() {
        let bodies = [
            "<html><body>502 Bad Gateway</body></html>",
            "",
            "<?xml version=\"1.0\"?><ServerResponse><RESULT type=\"List\"><m-0 type=\"List\"><GroupID>",
        ];

        for body in bodies {
            let conn = fixed_body_server(body).await;
            assert!(conn.get_group(Uuid::new_v4(), Some(Uuid::new_v4()), None).await.is_err());
            assert!(conn.get_membership(Uuid::new_v4(), None).await.is_err());
            assert!(conn.get_group_members(Uuid::new_v4(), Uuid::new_v4()).await.is_err());
            assert!(conn.find_groups(Uuid::new_v4(), "x").await.is_err());
        }
    }

    #[tokio::test]
    async fn null_result_is_none_and_empty() {
        let body = "<?xml version=\"1.0\"?><ServerResponse><RESULT>NULL</RESULT><REASON>x</REASON></ServerResponse>";
        let conn = fixed_body_server(body).await;
        assert!(conn.get_group(Uuid::new_v4(), Some(Uuid::new_v4()), None).await.unwrap().is_none());
        assert!(conn.get_group_members(Uuid::new_v4(), Uuid::new_v4()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn membership_without_group_id_is_error() {
        let body = "<ServerResponse><RESULT type=\"List\"><GroupName>x</GroupName></RESULT></ServerResponse>";
        let conn = fixed_body_server(body).await;
        assert!(conn.get_membership(Uuid::new_v4(), None).await.is_err());
    }

    #[tokio::test]
    async fn membership_prefix_returns_one_record() {
        let body = "<ServerResponse><RESULT type=\"List\"><members>x</members><m-0 type=\"List\"><GroupID>11111111-1111-1111-1111-111111111111</GroupID><GroupName>A</GroupName></m-0></RESULT></ServerResponse>";
        let conn = fixed_body_server(body).await;
        let records = conn.get_membership(Uuid::new_v4(), None).await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].group_id, Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap());
        assert_eq!(records[0].group_name, "A");
    }

    #[tokio::test]
    async fn get_all_memberships_lists_every_group() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();

        let group_one = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Group One".to_string(),
            charter: "charter".to_string(),
            insignia_id: Uuid::new_v4(),
            founder_id: agent,
            membership_fee: 42,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: false,
            mature_publish: true,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        };
        conn.create_group(agent, &group_one).await.unwrap();

        let group_two = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Group Two".to_string(),
            charter: "charter".to_string(),
            insignia_id: Uuid::new_v4(),
            founder_id: agent,
            membership_fee: 42,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: false,
            mature_publish: true,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        };
        conn.create_group(agent, &group_two).await.unwrap();

        let records = conn.get_all_memberships(agent).await.unwrap();
        assert_eq!(records.len(), 2);
        let names: Vec<String> = records.iter().map(|record| record.group_name.clone()).collect();
        assert!(names.contains(&"Group One".to_string()));
        assert!(names.contains(&"Group Two".to_string()));

        let unknown = Uuid::new_v4();
        let empty = conn.get_all_memberships(unknown).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn founder_is_owner_not_officer() {
        let (conn, _guard) = setup().await;
        let agent = Uuid::new_v4();
        let group = GroupRecord {
            group_id: Uuid::nil(),
            group_name: "Test Group".to_string(),
            charter: "charter".to_string(),
            insignia_id: Uuid::new_v4(),
            founder_id: agent,
            membership_fee: 42,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: false,
            mature_publish: true,
            owner_role_id: Uuid::nil(),
            member_count: 0,
            role_count: 0,
        };

        let created = conn.create_group(agent, &group).await.unwrap();

        let memberships = conn.get_membership(agent, Some(created.group_id)).await.unwrap();
        assert_eq!(memberships.len(), 1);
        let membership = &memberships[0];
        assert_eq!(membership.active_role, created.owner_role_id);
        assert_eq!(membership.group_title, format!("Owner of {}", created.group_name));
        assert_eq!(membership.group_powers, welkin_data::groups::GroupPowers::OWNERS.bits());

        let roles = conn.get_agent_roles(agent, agent, created.group_id).await.unwrap();
        let mut names: Vec<&str> = roles.iter().map(|role| role.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["Everyone", "Owners"]);
        assert!(!roles.iter().any(|role| role.name == "Officers"));
    }
}
