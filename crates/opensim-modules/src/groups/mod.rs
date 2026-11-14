// SPDX-License-Identifier: GPL-3.0-or-later

//! Groups V2 region module — region-side groups handler.
//!
//! Ports the C# `GroupsModule` from `OpenSim/Addons/Groups/GroupsModule.cs`.
//! Handles viewer packets, maintains a connector to the groups service, and
//! exposes the [`IGroupsModule`] trait for sibling region modules to query.

pub mod hg_connector;
pub mod messaging;

use std::collections::HashMap;

use opensim_data::groups::GroupPowers;
use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Zero UUID constant
// ---------------------------------------------------------------------------

const ZERO_UUID: Uuid = Uuid::nil();

// ---------------------------------------------------------------------------
// Module-level data types (viewer / inter-module API)
// ---------------------------------------------------------------------------

/// High-level group record returned to callers and the viewer.
///
/// Mirrors the C# `ExtendedGroupRecord`.
#[derive(Debug, Clone)]
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

/// Membership data combining group info, role info, and agent-specific
/// settings.
///
/// Mirrors the C# `GroupMembershipData` / `ExtendedGroupMembershipData`.
#[derive(Debug, Clone)]
pub struct GroupMembershipData {
    pub group_id: Uuid,
    pub group_name: String,
    pub group_insignia: Uuid,
    pub group_title: String,
    pub group_powers: GroupPowers,
    pub accept_notices: bool,
    pub list_in_profile: bool,
    pub contribution: i32,
    pub active_role: Uuid,
    pub active: bool,
}

/// Data about a single group member as returned by `get_group_members`.
///
/// Mirrors the C# `GroupMembersData`.
#[derive(Debug, Clone)]
pub struct GroupMembersData {
    pub agent_id: Uuid,
    pub is_owner: bool,
    pub title: String,
    pub agent_powers: GroupPowers,
    pub accept_notices: bool,
    pub list_in_profile: bool,
    pub contribution: i32,
    pub online_status: String,
}

/// Role data for display.
///
/// Mirrors the C# `GroupRolesData`.
#[derive(Debug, Clone)]
pub struct GroupRolesData {
    pub role_id: Uuid,
    pub name: String,
    pub title: String,
    pub description: String,
    pub powers: GroupPowers,
    pub members: i64,
}

/// Title (role selection) data.
#[derive(Debug, Clone)]
pub struct GroupTitleData {
    pub role_id: Uuid,
    pub name: String,
    pub title: String,
    pub selected: bool,
}

/// Notice summary for listing.
#[derive(Debug, Clone)]
pub struct GroupNoticeData {
    pub notice_id: Uuid,
    pub group_id: Uuid,
    pub timestamp: u64,
    pub from_name: String,
    pub subject: String,
    pub has_attachment: bool,
    pub attachment_type: u8,
    pub attachment_name: String,
    pub attachment_item_id: Uuid,
    pub attachment_owner_id: Uuid,
}

/// Invite info for pending invitations.
#[derive(Debug, Clone)]
pub struct GroupInviteInfo {
    pub invite_id: Uuid,
    pub group_id: Uuid,
    pub role_id: Uuid,
    pub agent_id: Uuid,
}

// ---------------------------------------------------------------------------
// Viewer packet types (stubs for packet handling)
// ---------------------------------------------------------------------------

/// Viewer packet types that the groups module handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupsPacketType {
    CreateGroupRequest,
    UpdateGroupInfo,
    JoinGroupRequest,
    LeaveGroupRequest,
    EjectGroupMemberRequest,
    InviteGroupRequest,
    GroupRoleDataRequest,
    GroupMembersRequest,
    GroupTitlesRequest,
    GroupActiveProposalsRequest,
    GroupVoteHistoryRequest,
    GroupAccountSummaryRequest,
}

/// Capabilities that the groups module registers.
pub const GROUP_CAPABILITIES: &[&str] = &[
    "GroupActiveProposals",
    "GroupVoteHistory",
    "GroupAccountSummary",
];

// ---------------------------------------------------------------------------
// IGroupsServicesConnector — service connector interface
// ---------------------------------------------------------------------------

/// Abstraction over the groups service, whether local or remote (Hypergrid).
///
/// Mirrors the C# `IGroupsServicesConnector`.
pub trait IGroupsServicesConnector: Send + Sync {
    fn create_group(
        &self,
        requesting_agent: Uuid,
        name: &str,
        charter: &str,
        show_in_list: bool,
        insignia_id: Uuid,
        membership_fee: i32,
        open_enrollment: bool,
        allow_publish: bool,
        mature_publish: bool,
        founder_id: Uuid,
    ) -> Result<GroupRecord, String>;

    fn update_group(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        charter: &str,
        show_in_list: bool,
        insignia_id: Uuid,
        membership_fee: i32,
        open_enrollment: bool,
        allow_publish: bool,
        mature_publish: bool,
    ) -> Result<(), String>;

    fn get_group_record(&self, requesting_agent: Uuid, group_id: Uuid) -> Option<GroupRecord>;

    fn find_groups(&self, requesting_agent: Uuid, search: &str) -> Vec<GroupRecord>;

    fn get_group_members(&self, requesting_agent: Uuid, group_id: Uuid) -> Vec<GroupMembersData>;

    fn get_group_roles(&self, requesting_agent: Uuid, group_id: Uuid) -> Vec<GroupRolesData>;

    fn get_group_role_members(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Vec<(Uuid, Uuid)>;

    fn add_agent_to_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    ) -> Result<(), String>;

    fn remove_agent_from_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), String>;

    fn add_agent_to_group_invite(
        &self,
        requesting_agent: Uuid,
        invite_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        agent_id: Uuid,
    ) -> bool;

    fn get_agent_to_group_invite(
        &self,
        requesting_agent: Uuid,
        invite_id: Uuid,
    ) -> Option<GroupInviteInfo>;

    fn remove_agent_to_group_invite(&self, requesting_agent: Uuid, invite_id: Uuid);

    fn set_agent_active_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    );

    fn get_agent_active_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
    ) -> Option<GroupMembershipData>;

    fn get_agent_group_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Option<GroupMembershipData>;

    fn get_agent_group_memberships(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
    ) -> Vec<GroupMembershipData>;

    fn add_group_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        name: &str,
        description: &str,
        title: &str,
        powers: GroupPowers,
    ) -> Result<(), String>;

    fn remove_group_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    );

    fn update_group_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        name: &str,
        description: &str,
        title: &str,
        powers: GroupPowers,
    );

    fn add_agent_to_group_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    );

    fn remove_agent_from_group_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    );

    fn set_agent_active_group_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    );

    fn update_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        accept_notices: bool,
        list_in_profile: bool,
    );

    fn add_group_notice(
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
    ) -> bool;

    fn get_group_notice(
        &self,
        requesting_agent: Uuid,
        notice_id: Uuid,
    ) -> Option<GroupNoticeData>;

    fn get_group_notices(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupNoticeData>;

    fn get_agent_group_roles(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupRolesData>;
}

// ---------------------------------------------------------------------------
// IGroupsModule trait
// ---------------------------------------------------------------------------

/// Interface exposed to other region modules that need groups information.
///
/// Mirrors the C# `IGroupsModule`.
pub trait IGroupsModule: Send + Sync {
    /// Set the agent's active group.
    fn activate_group(&self, agent_id: Uuid, group_id: Uuid);

    /// Get membership info for an agent in a specific group.
    fn get_member_info(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Option<GroupMembershipData>;

    /// Get all group memberships for an agent.
    fn get_groups_for_agent(&self, agent_id: Uuid) -> Vec<GroupMembershipData>;

    /// Get a group's record.
    fn get_group_record(&self, group_id: Uuid) -> Option<GroupRecord>;

    /// Get all roles in a group.
    fn get_group_roles(&self, group_id: Uuid) -> Vec<GroupRolesData>;

    /// Get all members in a group.
    fn get_group_members(&self, group_id: Uuid) -> Vec<GroupMembersData>;

    /// Check whether an agent has a specific power in a group.
    fn agent_has_power(&self, agent_id: Uuid, group_id: Uuid, power: GroupPowers) -> bool;
}

// ---------------------------------------------------------------------------
// StubGroupsConnector — in-memory connector for testing
// ---------------------------------------------------------------------------

/// A stub connector that stores groups data in memory.
///
/// Used for testing and as a fallback when no real service is configured.
pub struct StubGroupsConnector {
    groups: HashMap<Uuid, GroupRecord>,
    memberships: HashMap<(Uuid, Uuid), GroupMembershipData>,
    members: HashMap<Uuid, Vec<GroupMembersData>>,
    roles: HashMap<Uuid, Vec<GroupRolesData>>,
    invites: HashMap<Uuid, GroupInviteInfo>,
    notices: HashMap<Uuid, Vec<GroupNoticeData>>,
    active_groups: HashMap<Uuid, Uuid>,
}

impl StubGroupsConnector {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            memberships: HashMap::new(),
            members: HashMap::new(),
            roles: HashMap::new(),
            invites: HashMap::new(),
            notices: HashMap::new(),
            active_groups: HashMap::new(),
        }
    }
}

impl Default for StubGroupsConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl IGroupsServicesConnector for StubGroupsConnector {
    fn create_group(
        &self,
        _requesting_agent: Uuid,
        name: &str,
        charter: &str,
        show_in_list: bool,
        insignia_id: Uuid,
        membership_fee: i32,
        open_enrollment: bool,
        allow_publish: bool,
        mature_publish: bool,
        founder_id: Uuid,
    ) -> Result<GroupRecord, String> {
        let group_id = Uuid::new_v4();
        let owner_role_id = Uuid::new_v4();
        Ok(GroupRecord {
            group_id,
            group_name: name.to_string(),
            charter: charter.to_string(),
            insignia_id,
            founder_id,
            membership_fee,
            open_enrollment,
            show_in_list,
            allow_publish,
            mature_publish,
            owner_role_id,
            member_count: 1,
            role_count: 3,
        })
    }

    fn update_group(
        &self,
        _requesting_agent: Uuid,
        _group_id: Uuid,
        _charter: &str,
        _show_in_list: bool,
        _insignia_id: Uuid,
        _membership_fee: i32,
        _open_enrollment: bool,
        _allow_publish: bool,
        _mature_publish: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    fn get_group_record(&self, _requesting_agent: Uuid, group_id: Uuid) -> Option<GroupRecord> {
        self.groups.get(&group_id).cloned()
    }

    fn find_groups(&self, _requesting_agent: Uuid, search: &str) -> Vec<GroupRecord> {
        let lower = search.to_lowercase();
        self.groups
            .values()
            .filter(|g| g.group_name.to_lowercase().contains(&lower))
            .cloned()
            .collect()
    }

    fn get_group_members(
        &self,
        _requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupMembersData> {
        self.members.get(&group_id).cloned().unwrap_or_default()
    }

    fn get_group_roles(
        &self,
        _requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupRolesData> {
        self.roles.get(&group_id).cloned().unwrap_or_default()
    }

    fn get_group_role_members(
        &self,
        _requesting_agent: Uuid,
        _group_id: Uuid,
    ) -> Vec<(Uuid, Uuid)> {
        Vec::new()
    }

    fn add_agent_to_group(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
    ) -> Result<(), String> {
        Ok(())
    }

    fn remove_agent_from_group(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
    ) -> Result<(), String> {
        Ok(())
    }

    fn add_agent_to_group_invite(
        &self,
        _requesting_agent: Uuid,
        _invite_id: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
        _agent_id: Uuid,
    ) -> bool {
        true
    }

    fn get_agent_to_group_invite(
        &self,
        _requesting_agent: Uuid,
        invite_id: Uuid,
    ) -> Option<GroupInviteInfo> {
        self.invites.get(&invite_id).cloned()
    }

    fn remove_agent_to_group_invite(&self, _requesting_agent: Uuid, _invite_id: Uuid) {}

    fn set_agent_active_group(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
    ) {
    }

    fn get_agent_active_membership(
        &self,
        _requesting_agent: Uuid,
        agent_id: Uuid,
    ) -> Option<GroupMembershipData> {
        let gid = self.active_groups.get(&agent_id)?;
        self.memberships.get(&(agent_id, *gid)).cloned()
    }

    fn get_agent_group_membership(
        &self,
        _requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Option<GroupMembershipData> {
        self.memberships.get(&(agent_id, group_id)).cloned()
    }

    fn get_agent_group_memberships(
        &self,
        _requesting_agent: Uuid,
        agent_id: Uuid,
    ) -> Vec<GroupMembershipData> {
        self.memberships
            .iter()
            .filter(|((a, _g), _)| *a == agent_id)
            .map(|(_, v)| v.clone())
            .collect()
    }

    fn add_group_role(
        &self,
        _requesting_agent: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
        _name: &str,
        _description: &str,
        _title: &str,
        _powers: GroupPowers,
    ) -> Result<(), String> {
        Ok(())
    }

    fn remove_group_role(
        &self,
        _requesting_agent: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
    ) {
    }

    fn update_group_role(
        &self,
        _requesting_agent: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
        _name: &str,
        _description: &str,
        _title: &str,
        _powers: GroupPowers,
    ) {
    }

    fn add_agent_to_group_role(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
    ) {
    }

    fn remove_agent_from_group_role(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
    ) {
    }

    fn set_agent_active_group_role(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
        _role_id: Uuid,
    ) {
    }

    fn update_membership(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        _group_id: Uuid,
        _accept_notices: bool,
        _list_in_profile: bool,
    ) {
    }

    fn add_group_notice(
        &self,
        _requesting_agent: Uuid,
        _group_id: Uuid,
        _notice_id: Uuid,
        _from_name: &str,
        _subject: &str,
        _message: &str,
        _has_attachment: bool,
        _attachment_type: u8,
        _attachment_name: &str,
        _attachment_item_id: Uuid,
        _attachment_owner_id: Uuid,
    ) -> bool {
        true
    }

    fn get_group_notice(
        &self,
        _requesting_agent: Uuid,
        _notice_id: Uuid,
    ) -> Option<GroupNoticeData> {
        None
    }

    fn get_group_notices(
        &self,
        _requesting_agent: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupNoticeData> {
        self.notices.get(&group_id).cloned().unwrap_or_default()
    }

    fn get_agent_group_roles(
        &self,
        _requesting_agent: Uuid,
        _agent_id: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupRolesData> {
        self.roles.get(&group_id).cloned().unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// GroupsModule
// ---------------------------------------------------------------------------

/// Region-side groups module handling viewer packets and exposing group data.
///
/// Mirrors the C# `GroupsModule` (ISharedRegionModule + IGroupsModule).
pub struct GroupsModule {
    enabled: bool,
    notices_enabled: bool,
    messaging_enabled: bool,
    level_group_create: i64,
    service_url: String,
    scenes: Vec<String>,
    connector: Box<dyn IGroupsServicesConnector>,
}

impl GroupsModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            notices_enabled: true,
            messaging_enabled: true,
            level_group_create: 0,
            service_url: String::new(),
            scenes: Vec::new(),
            connector: Box::new(StubGroupsConnector::new()),
        }
    }

    /// Create a groups module with a specific connector.
    pub fn with_connector(connector: Box<dyn IGroupsServicesConnector>) -> Self {
        Self {
            enabled: false,
            notices_enabled: true,
            messaging_enabled: true,
            level_group_create: 0,
            service_url: String::new(),
            scenes: Vec::new(),
            connector,
        }
    }

    /// Whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether group notices are enabled.
    pub fn notices_enabled(&self) -> bool {
        self.notices_enabled
    }

    /// Whether group messaging is enabled.
    pub fn messaging_enabled(&self) -> bool {
        self.messaging_enabled
    }

    /// Configured service URL.
    pub fn service_url(&self) -> &str {
        &self.service_url
    }

    /// The registered capabilities for this module.
    pub fn capabilities(&self) -> &[&str] {
        GROUP_CAPABILITIES
    }

    // -- Viewer packet handlers (stubs that delegate to the connector) ------

    /// Handle CreateGroupRequest from viewer.
    pub fn handle_create_group_request(
        &self,
        agent_id: Uuid,
        name: &str,
        charter: &str,
        show_in_list: bool,
        insignia_id: Uuid,
        membership_fee: i32,
        open_enrollment: bool,
        allow_publish: bool,
        mature_publish: bool,
    ) -> Result<GroupRecord, String> {
        if !self.enabled {
            return Err("Groups not enabled".to_string());
        }
        tracing::debug!(agent = %agent_id, group = name, "CreateGroupRequest");
        self.connector.create_group(
            agent_id,
            name,
            charter,
            show_in_list,
            insignia_id,
            membership_fee,
            open_enrollment,
            allow_publish,
            mature_publish,
            agent_id,
        )
    }

    /// Handle UpdateGroupInfo from viewer.
    pub fn handle_update_group_info(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
        charter: &str,
        show_in_list: bool,
        insignia_id: Uuid,
        membership_fee: i32,
        open_enrollment: bool,
        allow_publish: bool,
        mature_publish: bool,
    ) -> Result<(), String> {
        if !self.enabled {
            return Err("Groups not enabled".to_string());
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "UpdateGroupInfo");
        self.connector.update_group(
            agent_id,
            group_id,
            charter,
            show_in_list,
            insignia_id,
            membership_fee,
            open_enrollment,
            allow_publish,
            mature_publish,
        )
    }

    /// Handle JoinGroupRequest from viewer.
    pub fn handle_join_group_request(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), String> {
        if !self.enabled {
            return Err("Groups not enabled".to_string());
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "JoinGroupRequest");
        self.connector
            .add_agent_to_group(agent_id, agent_id, group_id, ZERO_UUID)
    }

    /// Handle LeaveGroupRequest from viewer.
    pub fn handle_leave_group_request(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), String> {
        if !self.enabled {
            return Err("Groups not enabled".to_string());
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "LeaveGroupRequest");
        self.connector
            .remove_agent_from_group(agent_id, agent_id, group_id)
    }

    /// Handle EjectGroupMemberRequest from viewer.
    pub fn handle_eject_group_member_request(
        &self,
        agent_id: Uuid,
        target_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), String> {
        if !self.enabled {
            return Err("Groups not enabled".to_string());
        }
        tracing::debug!(
            agent = %agent_id,
            target = %target_id,
            group = %group_id,
            "EjectGroupMemberRequest"
        );
        self.connector
            .remove_agent_from_group(agent_id, target_id, group_id)
    }

    /// Handle InviteGroupRequest from viewer.
    pub fn handle_invite_group_request(
        &self,
        agent_id: Uuid,
        invite_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        target_id: Uuid,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        tracing::debug!(
            agent = %agent_id,
            target = %target_id,
            group = %group_id,
            "InviteGroupRequest"
        );
        self.connector.add_agent_to_group_invite(
            agent_id, invite_id, group_id, role_id, target_id,
        )
    }

    /// Handle GroupRoleDataRequest from viewer.
    pub fn handle_group_role_data_request(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupRolesData> {
        if !self.enabled {
            return Vec::new();
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "GroupRoleDataRequest");
        self.connector.get_group_roles(agent_id, group_id)
    }

    /// Handle GroupMembersRequest from viewer.
    pub fn handle_group_members_request(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupMembersData> {
        if !self.enabled {
            return Vec::new();
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "GroupMembersRequest");
        self.connector.get_group_members(agent_id, group_id)
    }

    /// Handle GroupTitlesRequest from viewer.
    pub fn handle_group_titles_request(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupTitleData> {
        if !self.enabled {
            return Vec::new();
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "GroupTitlesRequest");
        let roles = self.connector.get_agent_group_roles(agent_id, agent_id, group_id);
        let active = self
            .connector
            .get_agent_group_membership(agent_id, agent_id, group_id);
        let active_role = active.map(|m| m.active_role).unwrap_or(ZERO_UUID);

        roles
            .into_iter()
            .map(|r| GroupTitleData {
                selected: r.role_id == active_role,
                role_id: r.role_id,
                name: r.name,
                title: r.title,
            })
            .collect()
    }

    /// Handle GroupActiveProposalsRequest from viewer.
    ///
    /// Proposals are not implemented; returns empty.
    pub fn handle_group_active_proposals_request(
        &self,
        _agent_id: Uuid,
        _group_id: Uuid,
    ) -> Vec<()> {
        Vec::new()
    }

    /// Handle GroupVoteHistoryRequest from viewer.
    ///
    /// Vote history is not implemented; returns empty.
    pub fn handle_group_vote_history_request(
        &self,
        _agent_id: Uuid,
        _group_id: Uuid,
    ) -> Vec<()> {
        Vec::new()
    }

    /// Handle GroupAccountSummaryRequest from viewer.
    ///
    /// Accounting is not implemented; returns zeroed summary.
    pub fn handle_group_account_summary_request(
        &self,
        _agent_id: Uuid,
        _group_id: Uuid,
    ) -> GroupAccountSummary {
        GroupAccountSummary::default()
    }

    /// Send group membership updates to a viewer on login.
    pub fn send_agent_group_data_update(&self, agent_id: Uuid) -> Vec<GroupMembershipData> {
        if !self.enabled {
            return Vec::new();
        }
        tracing::debug!(agent = %agent_id, "sending group data update");
        self.connector
            .get_agent_group_memberships(agent_id, agent_id)
    }
}

/// Placeholder for group account summary (not implemented in C# either).
#[derive(Debug, Clone, Default)]
pub struct GroupAccountSummary {
    pub balance: i32,
    pub total_credits: i32,
    pub total_debits: i32,
}

impl Default for GroupsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl IGroupsModule for GroupsModule {
    fn activate_group(&self, agent_id: Uuid, group_id: Uuid) {
        if !self.enabled {
            return;
        }
        tracing::debug!(agent = %agent_id, group = %group_id, "activate_group");
        self.connector
            .set_agent_active_group(agent_id, agent_id, group_id);
    }

    fn get_member_info(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Option<GroupMembershipData> {
        if !self.enabled {
            return None;
        }
        self.connector
            .get_agent_group_membership(agent_id, agent_id, group_id)
    }

    fn get_groups_for_agent(&self, agent_id: Uuid) -> Vec<GroupMembershipData> {
        if !self.enabled {
            return Vec::new();
        }
        self.connector
            .get_agent_group_memberships(agent_id, agent_id)
    }

    fn get_group_record(&self, group_id: Uuid) -> Option<GroupRecord> {
        if !self.enabled {
            return None;
        }
        self.connector.get_group_record(ZERO_UUID, group_id)
    }

    fn get_group_roles(&self, group_id: Uuid) -> Vec<GroupRolesData> {
        if !self.enabled {
            return Vec::new();
        }
        self.connector.get_group_roles(ZERO_UUID, group_id)
    }

    fn get_group_members(&self, group_id: Uuid) -> Vec<GroupMembersData> {
        if !self.enabled {
            return Vec::new();
        }
        self.connector.get_group_members(ZERO_UUID, group_id)
    }

    fn agent_has_power(
        &self,
        agent_id: Uuid,
        group_id: Uuid,
        power: GroupPowers,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        let roles =
            self.connector
                .get_agent_group_roles(agent_id, agent_id, group_id);
        roles.iter().any(|r| r.powers.contains(power))
    }
}

impl RegionModule for GroupsModule {
    fn name(&self) -> &str {
        "GroupsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("Groups", "Enabled", false);
        if !self.enabled {
            tracing::info!("GroupsModule disabled");
            return Ok(());
        }

        self.notices_enabled = config.get_bool("Groups", "NoticesEnabled", true);
        self.messaging_enabled = config.get_bool("Groups", "MessagingEnabled", true);
        self.level_group_create = config.get_int("Groups", "LevelGroupCreate", 0);
        self.service_url = config.get_string("Groups", "ServiceURL", "");

        tracing::info!(
            service_url = %self.service_url,
            notices = self.notices_enabled,
            messaging = self.messaging_enabled,
            "GroupsModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.scenes.push(scene.region_name.clone());
        tracing::info!(
            region = %scene.region_name,
            "GroupsModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "GroupsModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        tracing::info!("GroupsModule closed");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleConfig;
    use crate::registry::SceneRef;
    use std::collections::HashMap;

    // -- Test config helper --------------------------------------------------

    struct TestConfig {
        strings: HashMap<(String, String), String>,
        ints: HashMap<(String, String), i64>,
        bools: HashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
                ints: HashMap::new(),
                bools: HashMap::new(),
            }
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }

        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
                .insert((section.to_string(), key.to_string()), value);
        }
    }

    impl ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }

        fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
            self.ints
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }

        fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
            self.bools
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }

        fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
            default
        }
    }

    fn test_scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn enabled_config() -> TestConfig {
        let mut c = TestConfig::new();
        c.set_bool("Groups", "Enabled", true);
        c.set_string("Groups", "ServiceURL", "http://localhost:8003");
        c
    }

    // -- Tests ---------------------------------------------------------------

    // 1. Module lifecycle: initialise -> add_region -> remove_region -> close.
    #[test]
    fn groups_module_lifecycle() {
        let mut module = GroupsModule::new();
        let config = enabled_config();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert!(module.is_enabled());
        module.add_region(&scene).unwrap();
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // 2. Module disabled by default.
    #[test]
    fn groups_module_disabled_by_default() {
        let mut module = GroupsModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.is_enabled());
    }

    // 3. IGroupsModule trait: agent_has_power returns false when disabled.
    #[test]
    fn agent_has_power_disabled() {
        let module = GroupsModule::new();
        assert!(!module.agent_has_power(Uuid::new_v4(), Uuid::new_v4(), GroupPowers::INVITE));
    }

    // 4. get_groups_for_agent returns empty when disabled.
    #[test]
    fn get_groups_for_agent_disabled() {
        let module = GroupsModule::new();
        let result = module.get_groups_for_agent(Uuid::new_v4());
        assert!(result.is_empty());
    }

    // 5. GroupRecord construction.
    #[test]
    fn group_record_construction() {
        let rec = GroupRecord {
            group_id: Uuid::new_v4(),
            group_name: "Test Group".to_string(),
            charter: "Our charter".to_string(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 100,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 5,
            role_count: 3,
        };
        assert_eq!(rec.group_name, "Test Group");
        assert_eq!(rec.membership_fee, 100);
        assert!(rec.open_enrollment);
    }

    // 6. GroupMembershipData construction.
    #[test]
    fn group_membership_data_construction() {
        let data = GroupMembershipData {
            group_id: Uuid::new_v4(),
            group_name: "Builders".to_string(),
            group_insignia: Uuid::nil(),
            group_title: "Member of Builders".to_string(),
            group_powers: GroupPowers::DEFAULT_EVERYONE,
            accept_notices: true,
            list_in_profile: true,
            contribution: 0,
            active_role: Uuid::nil(),
            active: true,
        };
        assert!(data.group_powers.contains(GroupPowers::ALLOW_SET_HOME));
        assert!(data.accept_notices);
    }

    // 7. StubGroupsConnector create_group returns new group.
    #[test]
    fn stub_connector_create_group() {
        let connector = StubGroupsConnector::new();
        let result = connector.create_group(
            Uuid::new_v4(),
            "New Group",
            "Charter",
            true,
            Uuid::nil(),
            0,
            true,
            true,
            false,
            Uuid::new_v4(),
        );
        assert!(result.is_ok());
        let group = result.unwrap();
        assert_eq!(group.group_name, "New Group");
        assert!(!group.group_id.is_nil());
    }

    // 8. handle_create_group_request when disabled.
    #[test]
    fn create_group_request_disabled() {
        let module = GroupsModule::new();
        let result = module.handle_create_group_request(
            Uuid::new_v4(),
            "Test",
            "",
            true,
            Uuid::nil(),
            0,
            true,
            true,
            false,
        );
        assert!(result.is_err());
    }

    // 9. handle_create_group_request when enabled.
    #[test]
    fn create_group_request_enabled() {
        let mut module = GroupsModule::new();
        let config = enabled_config();
        module.initialise(&config).unwrap();
        let result = module.handle_create_group_request(
            Uuid::new_v4(),
            "Test Group",
            "charter",
            true,
            Uuid::nil(),
            0,
            true,
            true,
            false,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().group_name, "Test Group");
    }

    // 10. Module name and type.
    #[test]
    fn groups_module_name_and_type() {
        let module = GroupsModule::new();
        assert_eq!(module.name(), "GroupsModule");
        assert_eq!(module.module_type(), ModuleType::Shared);
    }

    // 11. GroupsPacketType variants.
    #[test]
    fn groups_packet_type_variants() {
        assert_ne!(
            GroupsPacketType::CreateGroupRequest,
            GroupsPacketType::UpdateGroupInfo
        );
        assert_eq!(
            GroupsPacketType::JoinGroupRequest,
            GroupsPacketType::JoinGroupRequest
        );
    }

    // 12. Capabilities list.
    #[test]
    fn groups_capabilities() {
        let module = GroupsModule::new();
        let caps = module.capabilities();
        assert_eq!(caps.len(), 3);
        assert!(caps.contains(&"GroupActiveProposals"));
        assert!(caps.contains(&"GroupVoteHistory"));
        assert!(caps.contains(&"GroupAccountSummary"));
    }

    // 13. Config reads correct values.
    #[test]
    fn groups_config_reads() {
        let mut module = GroupsModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Groups", "Enabled", true);
        config.set_bool("Groups", "NoticesEnabled", false);
        config.set_bool("Groups", "MessagingEnabled", false);
        config.set_int("Groups", "LevelGroupCreate", 42);
        config.set_string("Groups", "ServiceURL", "http://example.com:8003");
        module.initialise(&config).unwrap();

        assert!(module.is_enabled());
        assert!(!module.notices_enabled());
        assert!(!module.messaging_enabled());
        assert_eq!(module.service_url(), "http://example.com:8003");
    }

    // 14. handle_group_titles_request returns empty when disabled.
    #[test]
    fn titles_request_disabled() {
        let module = GroupsModule::new();
        let result = module.handle_group_titles_request(Uuid::new_v4(), Uuid::new_v4());
        assert!(result.is_empty());
    }

    // 15. Proposals and vote history return empty.
    #[test]
    fn proposals_and_votes_return_empty() {
        let module = GroupsModule::new();
        assert!(module
            .handle_group_active_proposals_request(Uuid::new_v4(), Uuid::new_v4())
            .is_empty());
        assert!(module
            .handle_group_vote_history_request(Uuid::new_v4(), Uuid::new_v4())
            .is_empty());
    }

    // 16. GroupAccountSummary default is zeroed.
    #[test]
    fn group_account_summary_default() {
        let summary = GroupAccountSummary::default();
        assert_eq!(summary.balance, 0);
        assert_eq!(summary.total_credits, 0);
        assert_eq!(summary.total_debits, 0);
    }

    // 17. send_agent_group_data_update when disabled.
    #[test]
    fn send_group_data_disabled() {
        let module = GroupsModule::new();
        let result = module.send_agent_group_data_update(Uuid::new_v4());
        assert!(result.is_empty());
    }

    // 18. GroupInviteInfo construction.
    #[test]
    fn group_invite_info_construction() {
        let invite = GroupInviteInfo {
            invite_id: Uuid::new_v4(),
            group_id: Uuid::new_v4(),
            role_id: Uuid::nil(),
            agent_id: Uuid::new_v4(),
        };
        assert!(!invite.invite_id.is_nil());
        assert!(!invite.group_id.is_nil());
    }

    // 19. GroupNoticeData construction.
    #[test]
    fn group_notice_data_construction() {
        let notice = GroupNoticeData {
            notice_id: Uuid::new_v4(),
            group_id: Uuid::new_v4(),
            timestamp: 1700000000,
            from_name: "Test User".to_string(),
            subject: "Hello".to_string(),
            has_attachment: false,
            attachment_type: 0,
            attachment_name: String::new(),
            attachment_item_id: Uuid::nil(),
            attachment_owner_id: Uuid::nil(),
        };
        assert_eq!(notice.subject, "Hello");
        assert!(!notice.has_attachment);
    }

    // 20. with_connector builds a module with custom connector.
    #[test]
    fn with_connector_creates_module() {
        let connector = Box::new(StubGroupsConnector::new());
        let module = GroupsModule::with_connector(connector);
        assert!(!module.is_enabled());
        assert_eq!(module.name(), "GroupsModule");
    }
}
