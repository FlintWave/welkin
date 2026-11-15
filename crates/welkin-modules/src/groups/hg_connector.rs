// SPDX-License-Identifier: LGPL-3.0-or-later

//! Hypergrid groups service connector — cross-grid group federation.
//!
//! Ports the C# classes:
//! - `GroupsServiceHGConnectorModule` — HG-aware connector dispatching to
//!   local or remote groups services
//! - `ForeignImporter` — converts foreign (HG) agent UUIDs for cross-grid
//!   group membership
//! - `RemoteConnectorCacheWrapper` — caches group data with configurable TTL
//!   to reduce cross-grid calls

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;
use welkin_data::groups::GroupPowers;

use super::{
    GroupInviteInfo, GroupMembersData, GroupMembershipData, GroupNoticeData, GroupRecord,
    GroupRolesData, IGroupsServicesConnector,
};

// ---------------------------------------------------------------------------
// ForeignImporter
// ---------------------------------------------------------------------------

/// Converts foreign agent identifiers for cross-grid group membership.
///
/// In the C# code, `ForeignImporter` parses Universal User Identifiers
/// (UUI — "uuid;homeURI;first last") and registers foreign users with
/// the local UserManagement service.
///
/// This Rust port stores known mappings in memory.
#[derive(Debug, Clone)]
pub struct ForeignImporter {
    /// Maps foreign UUI strings to local UUIDs.
    known_users: HashMap<String, Uuid>,
}

impl ForeignImporter {
    pub fn new() -> Self {
        Self {
            known_users: HashMap::new(),
        }
    }

    /// Parse a Universal User Identifier string and return the UUID portion.
    ///
    /// UUI format: `"uuid;homeURI;firstname lastname"` or just a plain UUID.
    pub fn parse_uui(uui: &str) -> Option<Uuid> {
        let uuid_str = uui.split(';').next()?;
        Uuid::parse_str(uuid_str.trim()).ok()
    }

    /// Extract the home URI from a UUI string, if present.
    pub fn parse_home_uri(uui: &str) -> Option<String> {
        let parts: Vec<&str> = uui.split(';').collect();
        if parts.len() >= 2 {
            Some(parts[1].trim().to_string())
        } else {
            None
        }
    }

    /// Extract the display name (first + last) from a UUI string, if present.
    pub fn parse_display_name(uui: &str) -> Option<String> {
        let parts: Vec<&str> = uui.split(';').collect();
        if parts.len() >= 3 {
            Some(parts[2].trim().to_string())
        } else {
            None
        }
    }

    /// Register a foreign user mapping.
    pub fn add_user(&mut self, uui: &str, local_id: Uuid) {
        self.known_users.insert(uui.to_string(), local_id);
    }

    /// Look up a previously registered foreign user.
    pub fn get_local_id(&self, uui: &str) -> Option<Uuid> {
        self.known_users.get(uui).copied()
    }

    /// Convert a foreign agent ID string to a local UUID.
    ///
    /// If the string is a known UUI, returns the mapped local ID.
    /// Otherwise attempts to parse it as a UUID directly.
    pub fn convert_agent_id(&self, agent_id_str: &str) -> Option<Uuid> {
        if let Some(id) = self.known_users.get(agent_id_str) {
            return Some(*id);
        }
        Self::parse_uui(agent_id_str)
    }

    /// Convert a `GroupMembersData` with a foreign agent ID.
    ///
    /// Mirrors `ForeignImporter.ConvertGroupMembersData` in C#.
    pub fn convert_group_member(&self, member: &GroupMembersData) -> GroupMembersData {
        let agent_id = self
            .convert_agent_id(&member.agent_id.to_string())
            .unwrap_or(member.agent_id);
        GroupMembersData {
            agent_id,
            is_owner: member.is_owner,
            title: member.title.clone(),
            agent_powers: member.agent_powers,
            accept_notices: member.accept_notices,
            list_in_profile: member.list_in_profile,
            contribution: member.contribution,
            online_status: member.online_status.clone(),
        }
    }

    /// Number of known foreign user mappings.
    pub fn known_user_count(&self) -> usize {
        self.known_users.len()
    }
}

impl Default for ForeignImporter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CacheEntry — TTL-based cache entry
// ---------------------------------------------------------------------------

/// A cached value with an expiry timestamp.
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires
    }
}

// ---------------------------------------------------------------------------
// RemoteConnectorCacheWrapper
// ---------------------------------------------------------------------------

/// Caches group data to reduce repeated cross-grid calls.
///
/// Mirrors the C# `RemoteConnectorCacheWrapper` with its key scheme:
/// - `group-{GroupID}` / `group-{Name}` => GroupRecord
/// - `membership-{AgentID}-{GroupID}` => GroupMembershipData
/// - `memberships-{AgentID}` => Vec<GroupMembershipData>
/// - `members-{GroupID}` => Vec<GroupMembersData>
/// - `roles-{GroupID}` => Vec<GroupRolesData>
/// - `notice-{NoticeID}` => GroupNoticeData
/// - `notices-{GroupID}` => Vec<GroupNoticeData>
pub struct RemoteConnectorCacheWrapper {
    ttl: Duration,
    importer: ForeignImporter,

    group_cache: HashMap<String, CacheEntry<GroupRecord>>,
    membership_cache: HashMap<String, CacheEntry<GroupMembershipData>>,
    memberships_cache: HashMap<String, CacheEntry<Vec<GroupMembershipData>>>,
    members_cache: HashMap<String, CacheEntry<Vec<GroupMembersData>>>,
    roles_cache: HashMap<String, CacheEntry<Vec<GroupRolesData>>>,
    notice_cache: HashMap<String, CacheEntry<GroupNoticeData>>,
    notices_cache: HashMap<String, CacheEntry<Vec<GroupNoticeData>>>,
}

impl RemoteConnectorCacheWrapper {
    /// Default cache TTL: 60 seconds (matches C# `GROUPS_CACHE_TIMEOUT`).
    pub const DEFAULT_TTL_SECS: u64 = 60;

    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(Self::DEFAULT_TTL_SECS))
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            importer: ForeignImporter::new(),
            group_cache: HashMap::new(),
            membership_cache: HashMap::new(),
            memberships_cache: HashMap::new(),
            members_cache: HashMap::new(),
            roles_cache: HashMap::new(),
            notice_cache: HashMap::new(),
            notices_cache: HashMap::new(),
        }
    }

    /// Access the foreign importer.
    pub fn importer(&self) -> &ForeignImporter {
        &self.importer
    }

    /// Mutable access to the foreign importer.
    pub fn importer_mut(&mut self) -> &mut ForeignImporter {
        &mut self.importer
    }

    // -- Group cache --------------------------------------------------------

    /// Cache a group record by ID.
    pub fn cache_group(&mut self, group: &GroupRecord) {
        let key_id = format!("group-{}", group.group_id);
        let key_name = format!("group-{}", group.group_name);
        self.group_cache
            .insert(key_id, CacheEntry::new(group.clone(), self.ttl));
        self.group_cache
            .insert(key_name, CacheEntry::new(group.clone(), self.ttl));
    }

    /// Look up a cached group record by ID.
    pub fn get_group_by_id(&self, group_id: Uuid) -> Option<&GroupRecord> {
        let key = format!("group-{group_id}");
        self.group_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Look up a cached group record by name.
    pub fn get_group_by_name(&self, name: &str) -> Option<&GroupRecord> {
        let key = format!("group-{name}");
        self.group_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Invalidate a cached group record.
    pub fn invalidate_group(&mut self, group_id: Uuid) {
        let key = format!("group-{group_id}");
        self.group_cache.remove(&key);
    }

    // -- Membership cache ---------------------------------------------------

    /// Cache membership data for an agent in a group.
    pub fn cache_membership(&mut self, agent_id: Uuid, group_id: Uuid, data: &GroupMembershipData) {
        let key = format!("membership-{agent_id}-{group_id}");
        self.membership_cache
            .insert(key, CacheEntry::new(data.clone(), self.ttl));
    }

    /// Look up cached membership for an agent in a group.
    pub fn get_membership(&self, agent_id: Uuid, group_id: Uuid) -> Option<&GroupMembershipData> {
        let key = format!("membership-{agent_id}-{group_id}");
        self.membership_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Invalidate a membership cache entry.
    pub fn invalidate_membership(&mut self, agent_id: Uuid, group_id: Uuid) {
        let key = format!("membership-{agent_id}-{group_id}");
        self.membership_cache.remove(&key);
    }

    // -- Memberships list cache ---------------------------------------------

    /// Cache all memberships for an agent.
    pub fn cache_memberships(&mut self, agent_id: Uuid, data: &[GroupMembershipData]) {
        let key = format!("memberships-{agent_id}");
        self.memberships_cache
            .insert(key, CacheEntry::new(data.to_vec(), self.ttl));
    }

    /// Look up cached memberships for an agent.
    pub fn get_memberships(&self, agent_id: Uuid) -> Option<&Vec<GroupMembershipData>> {
        let key = format!("memberships-{agent_id}");
        self.memberships_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Invalidate cached memberships for an agent.
    pub fn invalidate_memberships(&mut self, agent_id: Uuid) {
        let key = format!("memberships-{agent_id}");
        self.memberships_cache.remove(&key);
    }

    // -- Members list cache -------------------------------------------------

    /// Cache all members for a group.
    pub fn cache_members(&mut self, group_id: Uuid, data: &[GroupMembersData]) {
        let key = format!("members-{group_id}");
        self.members_cache
            .insert(key, CacheEntry::new(data.to_vec(), self.ttl));
    }

    /// Look up cached members for a group.
    pub fn get_members(&self, group_id: Uuid) -> Option<&Vec<GroupMembersData>> {
        let key = format!("members-{group_id}");
        self.members_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Invalidate cached members for a group.
    pub fn invalidate_members(&mut self, group_id: Uuid) {
        let key = format!("members-{group_id}");
        self.members_cache.remove(&key);
    }

    // -- Roles cache --------------------------------------------------------

    /// Cache roles for a group.
    pub fn cache_roles(&mut self, group_id: Uuid, data: &[GroupRolesData]) {
        let key = format!("roles-{group_id}");
        self.roles_cache
            .insert(key, CacheEntry::new(data.to_vec(), self.ttl));
    }

    /// Look up cached roles for a group.
    pub fn get_roles(&self, group_id: Uuid) -> Option<&Vec<GroupRolesData>> {
        let key = format!("roles-{group_id}");
        self.roles_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Invalidate cached roles for a group.
    pub fn invalidate_roles(&mut self, group_id: Uuid) {
        let key = format!("roles-{group_id}");
        self.roles_cache.remove(&key);
    }

    // -- Notice cache -------------------------------------------------------

    /// Cache a single notice.
    pub fn cache_notice(&mut self, notice: &GroupNoticeData) {
        let key = format!("notice-{}", notice.notice_id);
        self.notice_cache
            .insert(key, CacheEntry::new(notice.clone(), self.ttl));
    }

    /// Look up a cached notice.
    pub fn get_notice(&self, notice_id: Uuid) -> Option<&GroupNoticeData> {
        let key = format!("notice-{notice_id}");
        self.notice_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    /// Cache all notices for a group.
    pub fn cache_notices(&mut self, group_id: Uuid, data: &[GroupNoticeData]) {
        let key = format!("notices-{group_id}");
        self.notices_cache
            .insert(key, CacheEntry::new(data.to_vec(), self.ttl));
    }

    /// Look up cached notices for a group.
    pub fn get_notices(&self, group_id: Uuid) -> Option<&Vec<GroupNoticeData>> {
        let key = format!("notices-{group_id}");
        self.notices_cache
            .get(&key)
            .filter(|e| !e.is_expired())
            .map(|e| &e.value)
    }

    // -- Maintenance --------------------------------------------------------

    /// Remove all expired entries from all caches.
    pub fn purge_expired(&mut self) {
        self.group_cache.retain(|_, e| !e.is_expired());
        self.membership_cache.retain(|_, e| !e.is_expired());
        self.memberships_cache.retain(|_, e| !e.is_expired());
        self.members_cache.retain(|_, e| !e.is_expired());
        self.roles_cache.retain(|_, e| !e.is_expired());
        self.notice_cache.retain(|_, e| !e.is_expired());
        self.notices_cache.retain(|_, e| !e.is_expired());
    }

    /// Clear all caches.
    pub fn clear_all(&mut self) {
        self.group_cache.clear();
        self.membership_cache.clear();
        self.memberships_cache.clear();
        self.members_cache.clear();
        self.roles_cache.clear();
        self.notice_cache.clear();
        self.notices_cache.clear();
    }

    /// Total number of entries across all caches (including expired).
    pub fn total_entries(&self) -> usize {
        self.group_cache.len()
            + self.membership_cache.len()
            + self.memberships_cache.len()
            + self.members_cache.len()
            + self.roles_cache.len()
            + self.notice_cache.len()
            + self.notices_cache.len()
    }
}

impl Default for RemoteConnectorCacheWrapper {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HGGroupsServiceConnector
// ---------------------------------------------------------------------------

/// Hypergrid groups service connector.
///
/// Dispatches group operations to the appropriate service — local for
/// agents on this grid, remote for foreign (HG) agents.
///
/// Mirrors the C# `GroupsServiceHGConnectorModule`.
pub struct HGGroupsServiceConnector {
    enabled: bool,
    local_service_url: String,
    local_groups_uri: String,
    cache: RemoteConnectorCacheWrapper,
    /// The local connector that handles operations for local agents.
    local_connector: Option<Box<dyn IGroupsServicesConnector>>,
}

impl HGGroupsServiceConnector {
    pub fn new() -> Self {
        Self {
            enabled: false,
            local_service_url: "local".to_string(),
            local_groups_uri: "http://127.0.0.1".to_string(),
            cache: RemoteConnectorCacheWrapper::new(),
            local_connector: None,
        }
    }

    /// Create with a local connector and configuration.
    pub fn with_config(
        local_connector: Box<dyn IGroupsServicesConnector>,
        local_service_url: &str,
        local_groups_uri: &str,
    ) -> Self {
        Self {
            enabled: true,
            local_service_url: local_service_url.to_string(),
            local_groups_uri: local_groups_uri.to_string(),
            cache: RemoteConnectorCacheWrapper::new(),
            local_connector: Some(local_connector),
        }
    }

    /// Whether the HG connector is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Access the cache.
    pub fn cache(&self) -> &RemoteConnectorCacheWrapper {
        &self.cache
    }

    /// Mutable access to the cache.
    pub fn cache_mut(&mut self) -> &mut RemoteConnectorCacheWrapper {
        &mut self.cache
    }

    /// Access the foreign importer through the cache.
    pub fn importer(&self) -> &ForeignImporter {
        self.cache.importer()
    }

    /// Determine whether an agent is local to this grid.
    ///
    /// In the C# code this checks whether the agent ID string contains
    /// a semicolon (UUI format indicates a foreign agent).
    pub fn is_local_agent(agent_id_str: &str) -> bool {
        !agent_id_str.contains(';')
    }

    /// The configured local groups service URI.
    pub fn local_groups_uri(&self) -> &str {
        &self.local_groups_uri
    }
}

impl Default for HGGroupsServiceConnector {
    fn default() -> Self {
        Self::new()
    }
}

/// The `IGroupsServicesConnector` implementation for the HG connector
/// delegates to the local connector for local agents.  Remote (HG)
/// operations are stubs in this port.
impl IGroupsServicesConnector for HGGroupsServiceConnector {
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
    ) -> Result<GroupRecord, String> {
        match &self.local_connector {
            Some(c) => c.create_group(
                requesting_agent,
                name,
                charter,
                show_in_list,
                insignia_id,
                membership_fee,
                open_enrollment,
                allow_publish,
                mature_publish,
                founder_id,
            ),
            None => Err("No local connector configured".to_string()),
        }
    }

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
    ) -> Result<(), String> {
        match &self.local_connector {
            Some(c) => c.update_group(
                requesting_agent,
                group_id,
                charter,
                show_in_list,
                insignia_id,
                membership_fee,
                open_enrollment,
                allow_publish,
                mature_publish,
            ),
            None => Err("No local connector configured".to_string()),
        }
    }

    fn get_group_record(&self, requesting_agent: Uuid, group_id: Uuid) -> Option<GroupRecord> {
        // Check cache first.
        if let Some(cached) = self.cache.get_group_by_id(group_id) {
            return Some(cached.clone());
        }
        self.local_connector
            .as_ref()?
            .get_group_record(requesting_agent, group_id)
    }

    fn find_groups(&self, requesting_agent: Uuid, search: &str) -> Vec<GroupRecord> {
        self.local_connector
            .as_ref()
            .map(|c| c.find_groups(requesting_agent, search))
            .unwrap_or_default()
    }

    fn get_group_members(&self, requesting_agent: Uuid, group_id: Uuid) -> Vec<GroupMembersData> {
        // Check cache.
        if let Some(cached) = self.cache.get_members(group_id) {
            return cached.clone();
        }
        self.local_connector
            .as_ref()
            .map(|c| c.get_group_members(requesting_agent, group_id))
            .unwrap_or_default()
    }

    fn get_group_roles(&self, requesting_agent: Uuid, group_id: Uuid) -> Vec<GroupRolesData> {
        if let Some(cached) = self.cache.get_roles(group_id) {
            return cached.clone();
        }
        self.local_connector
            .as_ref()
            .map(|c| c.get_group_roles(requesting_agent, group_id))
            .unwrap_or_default()
    }

    fn get_group_role_members(&self, requesting_agent: Uuid, group_id: Uuid) -> Vec<(Uuid, Uuid)> {
        self.local_connector
            .as_ref()
            .map(|c| c.get_group_role_members(requesting_agent, group_id))
            .unwrap_or_default()
    }

    fn add_agent_to_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    ) -> Result<(), String> {
        match &self.local_connector {
            Some(c) => c.add_agent_to_group(requesting_agent, agent_id, group_id, role_id),
            None => Err("No local connector".to_string()),
        }
    }

    fn remove_agent_from_group(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), String> {
        match &self.local_connector {
            Some(c) => c.remove_agent_from_group(requesting_agent, agent_id, group_id),
            None => Err("No local connector".to_string()),
        }
    }

    fn add_agent_to_group_invite(
        &self,
        requesting_agent: Uuid,
        invite_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        agent_id: Uuid,
    ) -> bool {
        self.local_connector
            .as_ref()
            .map(|c| {
                c.add_agent_to_group_invite(
                    requesting_agent,
                    invite_id,
                    group_id,
                    role_id,
                    agent_id,
                )
            })
            .unwrap_or(false)
    }

    fn get_agent_to_group_invite(
        &self,
        requesting_agent: Uuid,
        invite_id: Uuid,
    ) -> Option<GroupInviteInfo> {
        self.local_connector
            .as_ref()?
            .get_agent_to_group_invite(requesting_agent, invite_id)
    }

    fn remove_agent_to_group_invite(&self, requesting_agent: Uuid, invite_id: Uuid) {
        if let Some(c) = &self.local_connector {
            c.remove_agent_to_group_invite(requesting_agent, invite_id);
        }
    }

    fn set_agent_active_group(&self, requesting_agent: Uuid, agent_id: Uuid, group_id: Uuid) {
        if let Some(c) = &self.local_connector {
            c.set_agent_active_group(requesting_agent, agent_id, group_id);
        }
    }

    fn get_agent_active_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
    ) -> Option<GroupMembershipData> {
        self.local_connector
            .as_ref()?
            .get_agent_active_membership(requesting_agent, agent_id)
    }

    fn get_agent_group_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Option<GroupMembershipData> {
        if let Some(cached) = self.cache.get_membership(agent_id, group_id) {
            return Some(cached.clone());
        }
        self.local_connector.as_ref()?.get_agent_group_membership(
            requesting_agent,
            agent_id,
            group_id,
        )
    }

    fn get_agent_group_memberships(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
    ) -> Vec<GroupMembershipData> {
        if let Some(cached) = self.cache.get_memberships(agent_id) {
            return cached.clone();
        }
        self.local_connector
            .as_ref()
            .map(|c| c.get_agent_group_memberships(requesting_agent, agent_id))
            .unwrap_or_default()
    }

    fn add_group_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        name: &str,
        description: &str,
        title: &str,
        powers: GroupPowers,
    ) -> Result<(), String> {
        match &self.local_connector {
            Some(c) => c.add_group_role(
                requesting_agent,
                group_id,
                role_id,
                name,
                description,
                title,
                powers,
            ),
            None => Err("No local connector".to_string()),
        }
    }

    fn remove_group_role(&self, requesting_agent: Uuid, group_id: Uuid, role_id: Uuid) {
        if let Some(c) = &self.local_connector {
            c.remove_group_role(requesting_agent, group_id, role_id);
        }
    }

    fn update_group_role(
        &self,
        requesting_agent: Uuid,
        group_id: Uuid,
        role_id: Uuid,
        name: &str,
        description: &str,
        title: &str,
        powers: GroupPowers,
    ) {
        if let Some(c) = &self.local_connector {
            c.update_group_role(
                requesting_agent,
                group_id,
                role_id,
                name,
                description,
                title,
                powers,
            );
        }
    }

    fn add_agent_to_group_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    ) {
        if let Some(c) = &self.local_connector {
            c.add_agent_to_group_role(requesting_agent, agent_id, group_id, role_id);
        }
    }

    fn remove_agent_from_group_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    ) {
        if let Some(c) = &self.local_connector {
            c.remove_agent_from_group_role(requesting_agent, agent_id, group_id, role_id);
        }
    }

    fn set_agent_active_group_role(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        role_id: Uuid,
    ) {
        if let Some(c) = &self.local_connector {
            c.set_agent_active_group_role(requesting_agent, agent_id, group_id, role_id);
        }
    }

    fn update_membership(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
        accept_notices: bool,
        list_in_profile: bool,
    ) {
        if let Some(c) = &self.local_connector {
            c.update_membership(
                requesting_agent,
                agent_id,
                group_id,
                accept_notices,
                list_in_profile,
            );
        }
    }

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
    ) -> bool {
        self.local_connector
            .as_ref()
            .map(|c| {
                c.add_group_notice(
                    requesting_agent,
                    group_id,
                    notice_id,
                    from_name,
                    subject,
                    message,
                    has_attachment,
                    attachment_type,
                    attachment_name,
                    attachment_item_id,
                    attachment_owner_id,
                )
            })
            .unwrap_or(false)
    }

    fn get_group_notice(&self, requesting_agent: Uuid, notice_id: Uuid) -> Option<GroupNoticeData> {
        if let Some(cached) = self.cache.get_notice(notice_id) {
            return Some(cached.clone());
        }
        self.local_connector
            .as_ref()?
            .get_group_notice(requesting_agent, notice_id)
    }

    fn get_group_notices(&self, requesting_agent: Uuid, group_id: Uuid) -> Vec<GroupNoticeData> {
        if let Some(cached) = self.cache.get_notices(group_id) {
            return cached.clone();
        }
        self.local_connector
            .as_ref()
            .map(|c| c.get_group_notices(requesting_agent, group_id))
            .unwrap_or_default()
    }

    fn get_agent_group_roles(
        &self,
        requesting_agent: Uuid,
        agent_id: Uuid,
        group_id: Uuid,
    ) -> Vec<GroupRolesData> {
        self.local_connector
            .as_ref()
            .map(|c| c.get_agent_group_roles(requesting_agent, agent_id, group_id))
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groups::StubGroupsConnector;

    // 1. ForeignImporter parse_uui with full UUI.
    #[test]
    fn foreign_importer_parse_full_uui() {
        let uui = "d3a80b0e-1234-5678-9abc-def012345678;http://example.com;John Doe";
        let uuid = ForeignImporter::parse_uui(uui).unwrap();
        assert_eq!(uuid.to_string(), "d3a80b0e-1234-5678-9abc-def012345678");
    }

    // 2. ForeignImporter parse_uui with plain UUID.
    #[test]
    fn foreign_importer_parse_plain_uuid() {
        let uuid_str = "d3a80b0e-1234-5678-9abc-def012345678";
        let uuid = ForeignImporter::parse_uui(uuid_str).unwrap();
        assert_eq!(uuid.to_string(), uuid_str);
    }

    // 3. ForeignImporter parse_home_uri.
    #[test]
    fn foreign_importer_parse_home_uri() {
        let uui = "d3a80b0e-1234-5678-9abc-def012345678;http://example.com;John Doe";
        let uri = ForeignImporter::parse_home_uri(uui).unwrap();
        assert_eq!(uri, "http://example.com");
    }

    // 4. ForeignImporter parse_display_name.
    #[test]
    fn foreign_importer_parse_display_name() {
        let uui = "d3a80b0e-1234-5678-9abc-def012345678;http://example.com;John Doe";
        let name = ForeignImporter::parse_display_name(uui).unwrap();
        assert_eq!(name, "John Doe");
    }

    // 5. ForeignImporter add_user and get_local_id.
    #[test]
    fn foreign_importer_user_mapping() {
        let mut importer = ForeignImporter::new();
        let id = Uuid::new_v4();
        let uui = format!("{id};http://foreign.grid;Jane Smith");
        importer.add_user(&uui, id);
        assert_eq!(importer.get_local_id(&uui), Some(id));
        assert_eq!(importer.known_user_count(), 1);
    }

    // 6. ForeignImporter convert_agent_id with known UUI.
    #[test]
    fn foreign_importer_convert_known() {
        let mut importer = ForeignImporter::new();
        let id = Uuid::new_v4();
        let uui = format!("{id};http://grid.example;First Last");
        importer.add_user(&uui, id);
        assert_eq!(importer.convert_agent_id(&uui), Some(id));
    }

    // 7. ForeignImporter convert_agent_id with plain UUID.
    #[test]
    fn foreign_importer_convert_plain_uuid() {
        let importer = ForeignImporter::new();
        let id = Uuid::new_v4();
        assert_eq!(importer.convert_agent_id(&id.to_string()), Some(id));
    }

    // 8. RemoteConnectorCacheWrapper group caching.
    #[test]
    fn cache_group_store_and_retrieve() {
        let mut cache = RemoteConnectorCacheWrapper::new();
        let group = GroupRecord {
            group_id: Uuid::new_v4(),
            group_name: "Cached Group".to_string(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 0,
            open_enrollment: true,
            show_in_list: true,
            allow_publish: true,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 10,
            role_count: 3,
        };
        cache.cache_group(&group);

        let by_id = cache.get_group_by_id(group.group_id);
        assert!(by_id.is_some());
        assert_eq!(by_id.unwrap().group_name, "Cached Group");

        let by_name = cache.get_group_by_name("Cached Group");
        assert!(by_name.is_some());
    }

    // 9. RemoteConnectorCacheWrapper membership caching.
    #[test]
    fn cache_membership_store_and_retrieve() {
        let mut cache = RemoteConnectorCacheWrapper::new();
        let agent = Uuid::new_v4();
        let group = Uuid::new_v4();
        let data = GroupMembershipData {
            group_id: group,
            group_name: "Test".to_string(),
            group_insignia: Uuid::nil(),
            group_title: "Member".to_string(),
            group_powers: GroupPowers::DEFAULT_EVERYONE,
            accept_notices: true,
            list_in_profile: true,
            contribution: 0,
            active_role: Uuid::nil(),
            active: true,
        };
        cache.cache_membership(agent, group, &data);
        assert!(cache.get_membership(agent, group).is_some());
    }

    // 10. RemoteConnectorCacheWrapper invalidation.
    #[test]
    fn cache_invalidation() {
        let mut cache = RemoteConnectorCacheWrapper::new();
        let group_id = Uuid::new_v4();
        let group = GroupRecord {
            group_id,
            group_name: "G".to_string(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 0,
            open_enrollment: false,
            show_in_list: false,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 1,
            role_count: 1,
        };
        cache.cache_group(&group);
        assert!(cache.get_group_by_id(group_id).is_some());

        cache.invalidate_group(group_id);
        assert!(cache.get_group_by_id(group_id).is_none());
    }

    // 11. RemoteConnectorCacheWrapper TTL expiry.
    #[test]
    fn cache_ttl_expiry() {
        let mut cache = RemoteConnectorCacheWrapper::with_ttl(Duration::from_millis(0));
        let group = GroupRecord {
            group_id: Uuid::new_v4(),
            group_name: "Expired".to_string(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 0,
            open_enrollment: false,
            show_in_list: false,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 0,
            role_count: 0,
        };
        cache.cache_group(&group);
        // With a 0ms TTL, the entry should be expired immediately.
        assert!(cache.get_group_by_id(group.group_id).is_none());
    }

    // 12. RemoteConnectorCacheWrapper clear_all.
    #[test]
    fn cache_clear_all() {
        let mut cache = RemoteConnectorCacheWrapper::new();
        let group = GroupRecord {
            group_id: Uuid::new_v4(),
            group_name: "G".to_string(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 0,
            open_enrollment: false,
            show_in_list: false,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 0,
            role_count: 0,
        };
        cache.cache_group(&group);
        assert!(cache.total_entries() > 0);
        cache.clear_all();
        assert_eq!(cache.total_entries(), 0);
    }

    // 13. HGGroupsServiceConnector is_local_agent.
    #[test]
    fn hg_is_local_agent() {
        assert!(HGGroupsServiceConnector::is_local_agent(
            "d3a80b0e-1234-5678-9abc-def012345678"
        ));
        assert!(!HGGroupsServiceConnector::is_local_agent(
            "d3a80b0e-1234-5678-9abc-def012345678;http://foreign.com;John Doe"
        ));
    }

    // 14. HGGroupsServiceConnector without local connector returns errors.
    #[test]
    fn hg_no_connector_returns_error() {
        let hg = HGGroupsServiceConnector::new();
        let result = hg.create_group(
            Uuid::new_v4(),
            "Test",
            "",
            true,
            Uuid::nil(),
            0,
            true,
            true,
            false,
            Uuid::new_v4(),
        );
        assert!(result.is_err());
    }

    // 15. HGGroupsServiceConnector with local connector delegates.
    #[test]
    fn hg_with_connector_delegates() {
        let stub = Box::new(StubGroupsConnector::new());
        let hg = HGGroupsServiceConnector::with_config(stub, "local", "http://127.0.0.1");
        assert!(hg.is_enabled());

        let result = hg.create_group(
            Uuid::new_v4(),
            "HG Group",
            "charter",
            true,
            Uuid::nil(),
            0,
            true,
            true,
            false,
            Uuid::new_v4(),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().group_name, "HG Group");
    }

    // 16. Cache roles store and retrieve.
    #[test]
    fn cache_roles_store_and_retrieve() {
        let mut cache = RemoteConnectorCacheWrapper::new();
        let group_id = Uuid::new_v4();
        let roles = vec![GroupRolesData {
            role_id: Uuid::new_v4(),
            name: "Everyone".to_string(),
            title: "Member".to_string(),
            description: "Default role".to_string(),
            powers: GroupPowers::DEFAULT_EVERYONE,
            members: 5,
        }];
        cache.cache_roles(group_id, &roles);
        let cached = cache.get_roles(group_id);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
        assert_eq!(cached.unwrap()[0].name, "Everyone");
    }

    // 17. ForeignImporter parse_uui returns None for garbage.
    #[test]
    fn foreign_importer_parse_garbage() {
        assert!(ForeignImporter::parse_uui("not-a-uuid").is_none());
        assert!(ForeignImporter::parse_uui("").is_none());
    }

    // 18. ForeignImporter parse_home_uri returns None for plain UUID.
    #[test]
    fn foreign_importer_no_home_uri_plain() {
        assert!(ForeignImporter::parse_home_uri("d3a80b0e-1234-5678-9abc-def012345678").is_none());
    }

    // 19. Cache purge_expired removes stale entries.
    #[test]
    fn cache_purge_expired() {
        let mut cache = RemoteConnectorCacheWrapper::with_ttl(Duration::from_millis(0));
        let group = GroupRecord {
            group_id: Uuid::new_v4(),
            group_name: "Stale".to_string(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 0,
            open_enrollment: false,
            show_in_list: false,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 0,
            role_count: 0,
        };
        cache.cache_group(&group);
        assert!(cache.total_entries() > 0);
        cache.purge_expired();
        assert_eq!(cache.total_entries(), 0);
    }

    // 20. HGGroupsServiceConnector get_group_record uses cache.
    #[test]
    fn hg_get_group_uses_cache() {
        let stub = Box::new(StubGroupsConnector::new());
        let mut hg = HGGroupsServiceConnector::with_config(stub, "local", "http://127.0.0.1");
        let group_id = Uuid::new_v4();
        let group = GroupRecord {
            group_id,
            group_name: "CachedHG".to_string(),
            charter: String::new(),
            insignia_id: Uuid::nil(),
            founder_id: Uuid::new_v4(),
            membership_fee: 0,
            open_enrollment: false,
            show_in_list: false,
            allow_publish: false,
            mature_publish: false,
            owner_role_id: Uuid::new_v4(),
            member_count: 1,
            role_count: 1,
        };
        hg.cache_mut().cache_group(&group);
        let result = hg.get_group_record(Uuid::new_v4(), group_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().group_name, "CachedHG");
    }
}
