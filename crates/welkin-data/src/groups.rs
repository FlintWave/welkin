// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::DataError;

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GroupPowers: u64 {
        const NONE = 0;
        // Communication
        const INVITE                = 1 << 1;
        const EJECT                 = 1 << 2;
        const CHANGE_OPTIONS        = 1 << 3;
        const CREATE_ROLE           = 1 << 4;
        const DELETE_ROLE           = 1 << 5;
        const ROLE_PROPERTIES       = 1 << 6;
        const ASSIGN_MEMBER_LIMITED = 1 << 7;
        const ASSIGN_MEMBER         = 1 << 8;
        const REMOVE_MEMBER         = 1 << 9;
        const CHANGE_ACTIONS        = 1 << 10;
        const CHANGE_IDENTITY       = 1 << 11;
        const LAND_DEED             = 1 << 12;
        const LAND_RELEASE          = 1 << 13;
        const LAND_SET_SALE         = 1 << 14;
        const LAND_DIVIDE_JOIN      = 1 << 15;
        const FIND_PLACES           = 1 << 17;
        const LAND_CHANGE_IDENTITY  = 1 << 18;
        const SET_LANDING_POINT     = 1 << 19;
        const CHANGE_MEDIA          = 1 << 20;
        const LAND_EDIT             = 1 << 21;
        const LAND_OPTIONS          = 1 << 22;
        const ALLOW_EDIT_LAND       = 1 << 23;
        const ALLOW_FLY             = 1 << 24;
        const ALLOW_REZ             = 1 << 25;
        const ALLOW_LANDMARK        = 1 << 26;
        const ALLOW_VOICE_CHAT      = 1 << 27;
        const ALLOW_SET_HOME        = 1 << 28;
        const LAND_MANAGE_ALLOWED   = 1 << 29;
        const LAND_MANAGE_BANNED    = 1 << 30;
        const LAND_MANAGE_PASSES    = 1 << 31;
        const LAND_EJECT_AND_FREEZE = 1 << 32;
        const RETURN_GROUP_SET      = 1 << 33;
        const RETURN_NON_GROUP      = 1 << 34;
        const LAND_GARDENING        = 1 << 35;
        const DEED_OBJECT           = 1 << 36;
        const MODERATE_CHAT         = 1 << 37;
        const OBJECT_MANIPULATE     = 1 << 38;
        const OBJECT_SET_FOR_SALE   = 1 << 39;
        const ACCOUNTABLE           = 1 << 40;
        const SEND_NOTICES          = 1 << 42;
        const RECEIVE_NOTICES       = 1 << 43;
        const START_PROPOSAL        = 1 << 44;
        const VOTE_ON_PROPOSAL      = 1 << 45;
        const RETURN_GROUP_OWNED    = 1 << 48;
        const HOST_EVENT            = 1 << 41;
        const MEMBER_VISIBLE        = 1 << 47;
        const JOIN_CHAT             = 1 << 16;
        const ALLOW_ENVIRONMENT     = 1 << 46;
        const EXPERIENCE_ADMIN      = 1 << 49;
        const EXPERIENCE_CREATOR    = 1 << 50;
        const GROUP_BAN_ACCESS      = 1 << 51;

        const DEFAULT_EVERYONE = Self::ALLOW_SET_HOME.bits()
            | Self::ACCOUNTABLE.bits()
            | Self::JOIN_CHAT.bits()
            | Self::ALLOW_VOICE_CHAT.bits()
            | Self::RECEIVE_NOTICES.bits()
            | Self::START_PROPOSAL.bits()
            | Self::VOTE_ON_PROPOSAL.bits();

        const OFFICERS = Self::DEFAULT_EVERYONE.bits()
            | Self::ALLOW_FLY.bits()
            | Self::ALLOW_LANDMARK.bits()
            | Self::ALLOW_REZ.bits()
            | Self::ASSIGN_MEMBER_LIMITED.bits()
            | Self::CHANGE_IDENTITY.bits()
            | Self::CHANGE_MEDIA.bits()
            | Self::CHANGE_OPTIONS.bits()
            | Self::DEED_OBJECT.bits()
            | Self::EJECT.bits()
            | Self::FIND_PLACES.bits()
            | Self::INVITE.bits()
            | Self::LAND_CHANGE_IDENTITY.bits()
            | Self::LAND_DEED.bits()
            | Self::LAND_DIVIDE_JOIN.bits()
            | Self::LAND_EDIT.bits()
            | Self::ALLOW_ENVIRONMENT.bits()
            | Self::LAND_EJECT_AND_FREEZE.bits()
            | Self::LAND_GARDENING.bits()
            | Self::LAND_MANAGE_ALLOWED.bits()
            | Self::LAND_MANAGE_BANNED.bits()
            | Self::LAND_MANAGE_PASSES.bits()
            | Self::LAND_OPTIONS.bits()
            | Self::LAND_RELEASE.bits()
            | Self::LAND_SET_SALE.bits()
            | Self::MEMBER_VISIBLE.bits()
            | Self::MODERATE_CHAT.bits()
            | Self::OBJECT_MANIPULATE.bits()
            | Self::OBJECT_SET_FOR_SALE.bits()
            | Self::RETURN_GROUP_OWNED.bits()
            | Self::RETURN_GROUP_SET.bits()
            | Self::RETURN_NON_GROUP.bits()
            | Self::ROLE_PROPERTIES.bits()
            | Self::SEND_NOTICES.bits()
            | Self::SET_LANDING_POINT.bits();

        const OWNERS = Self::OFFICERS.bits()
            | Self::ACCOUNTABLE.bits()
            | Self::ALLOW_EDIT_LAND.bits()
            | Self::ASSIGN_MEMBER.bits()
            | Self::CHANGE_ACTIONS.bits()
            | Self::CREATE_ROLE.bits()
            | Self::DELETE_ROLE.bits()
            | Self::EXPERIENCE_ADMIN.bits()
            | Self::EXPERIENCE_CREATOR.bits()
            | Self::GROUP_BAN_ACCESS.bits()
            | Self::HOST_EVENT.bits()
            | Self::REMOVE_MEMBER.bits();
    }
}

// ---- Row types matching C# Groups V2 DB schema ----

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupRow {
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "Name")]
    pub name: String,
    #[sqlx(rename = "Charter")]
    pub charter: String,
    #[sqlx(rename = "InsigniaID")]
    pub insignia_id: String,
    #[sqlx(rename = "FounderID")]
    pub founder_id: String,
    #[sqlx(rename = "MembershipFee")]
    pub membership_fee: i32,
    #[sqlx(rename = "OpenEnrollment")]
    pub open_enrollment: String,
    #[sqlx(rename = "ShowInList")]
    pub show_in_list: i32,
    #[sqlx(rename = "AllowPublish")]
    pub allow_publish: i32,
    #[sqlx(rename = "MaturePublish")]
    pub mature_publish: i32,
    #[sqlx(rename = "OwnerRoleID")]
    pub owner_role_id: String,
    #[sqlx(rename = "Location")]
    pub location: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MembershipRow {
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "SelectedRoleID")]
    pub selected_role_id: String,
    #[sqlx(rename = "Contribution")]
    pub contribution: i32,
    #[sqlx(rename = "ListInProfile")]
    pub list_in_profile: i32,
    #[sqlx(rename = "AcceptNotices")]
    pub accept_notices: i32,
    #[sqlx(rename = "AccessToken")]
    pub access_token: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoleRow {
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "RoleID")]
    pub role_id: String,
    #[sqlx(rename = "Name")]
    pub name: String,
    #[sqlx(rename = "Description")]
    pub description: String,
    #[sqlx(rename = "Title")]
    pub title: String,
    #[sqlx(rename = "Powers")]
    pub powers: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoleMembershipRow {
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "RoleID")]
    pub role_id: String,
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PrincipalRow {
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "ActiveGroupID")]
    pub active_group_id: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InvitationRow {
    #[sqlx(rename = "InviteID")]
    pub invite_id: String,
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "RoleID")]
    pub role_id: String,
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "TMStamp")]
    pub timestamp: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoticeRow {
    #[sqlx(rename = "GroupID")]
    pub group_id: String,
    #[sqlx(rename = "NoticeID")]
    pub notice_id: String,
    #[sqlx(rename = "TMStamp")]
    pub timestamp: i64,
    #[sqlx(rename = "FromName")]
    pub from_name: String,
    #[sqlx(rename = "Subject")]
    pub subject: String,
    #[sqlx(rename = "Message")]
    pub message: String,
    #[sqlx(rename = "HasAttachment")]
    pub has_attachment: i32,
    #[sqlx(rename = "AttachmentType")]
    pub attachment_type: i32,
    #[sqlx(rename = "AttachmentName")]
    pub attachment_name: String,
    #[sqlx(rename = "AttachmentItemID")]
    pub attachment_item_id: String,
    #[sqlx(rename = "AttachmentOwnerID")]
    pub attachment_owner_id: String,
}

// ---- Column lists for queries ----

const GROUP_COLS: &str = "GroupID, Name, Charter, InsigniaID, FounderID, MembershipFee, \
    OpenEnrollment, ShowInList, AllowPublish, MaturePublish, OwnerRoleID, Location";

const MEMBERSHIP_COLS: &str = "GroupID, PrincipalID, SelectedRoleID, Contribution, \
    ListInProfile, AcceptNotices, AccessToken";

const ROLE_COLS: &str = "GroupID, RoleID, Name, Description, Title, Powers";

const ROLEMEMBERSHIP_COLS: &str = "GroupID, RoleID, PrincipalID";

const PRINCIPAL_COLS: &str = "PrincipalID, ActiveGroupID";

const INVITATION_COLS: &str = "InviteID, GroupID, RoleID, PrincipalID, TMStamp";

const NOTICE_COLS: &str = "GroupID, NoticeID, TMStamp, FromName, Subject, Message, \
    HasAttachment, AttachmentType, AttachmentName, AttachmentItemID, AttachmentOwnerID";

// ---- Store ----

#[derive(Clone)]
pub struct GroupsStore {
    pool: sqlx::AnyPool,
}

impl GroupsStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    // ---- groups ----

    pub async fn store_group(&self, row: &GroupRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM os_groups_groups WHERE GroupID = $1")
            .bind(row.group_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO os_groups_groups ({GROUP_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
        );
        sqlx::query(&sql)
            .bind(row.group_id.clone())
            .bind(row.name.clone())
            .bind(row.charter.clone())
            .bind(row.insignia_id.clone())
            .bind(row.founder_id.clone())
            .bind(row.membership_fee)
            .bind(row.open_enrollment.clone())
            .bind(row.show_in_list)
            .bind(row.allow_publish)
            .bind(row.mature_publish)
            .bind(row.owner_role_id.clone())
            .bind(row.location.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_group_by_id(
        &self,
        group_id: &str,
    ) -> Result<Option<GroupRow>, DataError> {
        let sql = format!("SELECT {GROUP_COLS} FROM os_groups_groups WHERE GroupID = $1");
        let row = sqlx::query_as::<_, GroupRow>(&sql)
            .bind(group_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_group_by_name(&self, name: &str) -> Result<Option<GroupRow>, DataError> {
        let sql = format!("SELECT {GROUP_COLS} FROM os_groups_groups WHERE Name = $1");
        let row = sqlx::query_as::<_, GroupRow>(&sql)
            .bind(name.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_groups(&self, pattern: &str) -> Result<Vec<GroupRow>, DataError> {
        let sql = format!("SELECT {GROUP_COLS} FROM os_groups_groups WHERE Name LIKE $1");
        let like = format!("%{pattern}%");
        let rows = sqlx::query_as::<_, GroupRow>(&sql)
            .bind(like)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn delete_group(&self, group_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM os_groups_groups WHERE GroupID = $1")
            .bind(group_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn member_count(&self, group_id: &str) -> Result<i64, DataError> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM os_groups_membership WHERE GroupID = $1")
                .bind(group_id.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    pub async fn role_count(&self, group_id: &str) -> Result<i64, DataError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM os_groups_roles WHERE GroupID = $1")
            .bind(group_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn role_member_count(&self, group_id: &str, role_id: &str) -> Result<i64, DataError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM os_groups_rolemembership WHERE GroupID = $1 AND RoleID = $2",
        )
        .bind(group_id.to_string())
        .bind(role_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    // ---- membership ----

    pub async fn store_member(&self, row: &MembershipRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM os_groups_membership WHERE GroupID = $1 AND PrincipalID = $2")
            .bind(row.group_id.clone())
            .bind(row.principal_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO os_groups_membership ({MEMBERSHIP_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7)"
        );
        sqlx::query(&sql)
            .bind(row.group_id.clone())
            .bind(row.principal_id.clone())
            .bind(row.selected_role_id.clone())
            .bind(row.contribution)
            .bind(row.list_in_profile)
            .bind(row.accept_notices)
            .bind(row.access_token.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_member(
        &self,
        group_id: &str,
        principal_id: &str,
    ) -> Result<Option<MembershipRow>, DataError> {
        let sql = format!(
            "SELECT {MEMBERSHIP_COLS} FROM os_groups_membership \
             WHERE GroupID = $1 AND PrincipalID = $2"
        );
        let row = sqlx::query_as::<_, MembershipRow>(&sql)
            .bind(group_id.to_string())
            .bind(principal_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_members(&self, group_id: &str) -> Result<Vec<MembershipRow>, DataError> {
        let sql = format!("SELECT {MEMBERSHIP_COLS} FROM os_groups_membership WHERE GroupID = $1");
        let rows = sqlx::query_as::<_, MembershipRow>(&sql)
            .bind(group_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn retrieve_memberships(
        &self,
        principal_id: &str,
    ) -> Result<Vec<MembershipRow>, DataError> {
        let sql =
            format!("SELECT {MEMBERSHIP_COLS} FROM os_groups_membership WHERE PrincipalID = $1");
        let rows = sqlx::query_as::<_, MembershipRow>(&sql)
            .bind(principal_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn delete_member(
        &self,
        group_id: &str,
        principal_id: &str,
    ) -> Result<bool, DataError> {
        let res =
            sqlx::query("DELETE FROM os_groups_membership WHERE GroupID = $1 AND PrincipalID = $2")
                .bind(group_id.to_string())
                .bind(principal_id.to_string())
                .execute(&self.pool)
                .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- roles ----

    pub async fn store_role(&self, row: &RoleRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM os_groups_roles WHERE GroupID = $1 AND RoleID = $2")
            .bind(row.group_id.clone())
            .bind(row.role_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO os_groups_roles ({ROLE_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6)"
        );
        sqlx::query(&sql)
            .bind(row.group_id.clone())
            .bind(row.role_id.clone())
            .bind(row.name.clone())
            .bind(row.description.clone())
            .bind(row.title.clone())
            .bind(row.powers)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_role(
        &self,
        group_id: &str,
        role_id: &str,
    ) -> Result<Option<RoleRow>, DataError> {
        let sql =
            format!("SELECT {ROLE_COLS} FROM os_groups_roles WHERE GroupID = $1 AND RoleID = $2");
        let row = sqlx::query_as::<_, RoleRow>(&sql)
            .bind(group_id.to_string())
            .bind(role_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_roles(&self, group_id: &str) -> Result<Vec<RoleRow>, DataError> {
        let sql = format!("SELECT {ROLE_COLS} FROM os_groups_roles WHERE GroupID = $1");
        let rows = sqlx::query_as::<_, RoleRow>(&sql)
            .bind(group_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn delete_role(&self, group_id: &str, role_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM os_groups_roles WHERE GroupID = $1 AND RoleID = $2")
            .bind(group_id.to_string())
            .bind(role_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- role membership ----

    pub async fn store_role_member(&self, row: &RoleMembershipRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM os_groups_rolemembership \
             WHERE GroupID = $1 AND RoleID = $2 AND PrincipalID = $3",
        )
        .bind(row.group_id.clone())
        .bind(row.role_id.clone())
        .bind(row.principal_id.clone())
        .execute(&mut *tx)
        .await?;
        let sql = format!(
            "INSERT INTO os_groups_rolemembership ({ROLEMEMBERSHIP_COLS}) VALUES ($1, $2, $3)"
        );
        sqlx::query(&sql)
            .bind(row.group_id.clone())
            .bind(row.role_id.clone())
            .bind(row.principal_id.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_role_member(
        &self,
        group_id: &str,
        role_id: &str,
        principal_id: &str,
    ) -> Result<Option<RoleMembershipRow>, DataError> {
        let sql = format!(
            "SELECT {ROLEMEMBERSHIP_COLS} FROM os_groups_rolemembership \
             WHERE GroupID = $1 AND RoleID = $2 AND PrincipalID = $3"
        );
        let row = sqlx::query_as::<_, RoleMembershipRow>(&sql)
            .bind(group_id.to_string())
            .bind(role_id.to_string())
            .bind(principal_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_roles_members(
        &self,
        group_id: &str,
    ) -> Result<Vec<RoleMembershipRow>, DataError> {
        let sql = format!(
            "SELECT {ROLEMEMBERSHIP_COLS} FROM os_groups_rolemembership WHERE GroupID = $1"
        );
        let rows = sqlx::query_as::<_, RoleMembershipRow>(&sql)
            .bind(group_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn retrieve_member_roles(
        &self,
        group_id: &str,
        principal_id: &str,
    ) -> Result<Vec<RoleMembershipRow>, DataError> {
        let sql = format!(
            "SELECT {ROLEMEMBERSHIP_COLS} FROM os_groups_rolemembership \
             WHERE GroupID = $1 AND PrincipalID = $2"
        );
        let rows = sqlx::query_as::<_, RoleMembershipRow>(&sql)
            .bind(group_id.to_string())
            .bind(principal_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn delete_role_member(
        &self,
        group_id: &str,
        role_id: &str,
        principal_id: &str,
    ) -> Result<bool, DataError> {
        let res = sqlx::query(
            "DELETE FROM os_groups_rolemembership \
             WHERE GroupID = $1 AND RoleID = $2 AND PrincipalID = $3",
        )
        .bind(group_id.to_string())
        .bind(role_id.to_string())
        .bind(principal_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_member_all_roles(
        &self,
        group_id: &str,
        principal_id: &str,
    ) -> Result<bool, DataError> {
        let res = sqlx::query(
            "DELETE FROM os_groups_rolemembership WHERE GroupID = $1 AND PrincipalID = $2",
        )
        .bind(group_id.to_string())
        .bind(principal_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- principals ----

    pub async fn store_principal(&self, row: &PrincipalRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM os_groups_principals WHERE PrincipalID = $1")
            .bind(row.principal_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!("INSERT INTO os_groups_principals ({PRINCIPAL_COLS}) VALUES ($1, $2)");
        sqlx::query(&sql)
            .bind(row.principal_id.clone())
            .bind(row.active_group_id.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_principal(
        &self,
        principal_id: &str,
    ) -> Result<Option<PrincipalRow>, DataError> {
        let sql =
            format!("SELECT {PRINCIPAL_COLS} FROM os_groups_principals WHERE PrincipalID = $1");
        let row = sqlx::query_as::<_, PrincipalRow>(&sql)
            .bind(principal_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn delete_principal(&self, principal_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM os_groups_principals WHERE PrincipalID = $1")
            .bind(principal_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- invitations ----

    pub async fn store_invitation(&self, row: &InvitationRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM os_groups_invites WHERE InviteID = $1")
            .bind(row.invite_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO os_groups_invites ({INVITATION_COLS}) VALUES ($1, $2, $3, $4, $5)"
        );
        sqlx::query(&sql)
            .bind(row.invite_id.clone())
            .bind(row.group_id.clone())
            .bind(row.role_id.clone())
            .bind(row.principal_id.clone())
            .bind(row.timestamp)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_invitation(
        &self,
        invite_id: &str,
    ) -> Result<Option<InvitationRow>, DataError> {
        let sql = format!("SELECT {INVITATION_COLS} FROM os_groups_invites WHERE InviteID = $1");
        let row = sqlx::query_as::<_, InvitationRow>(&sql)
            .bind(invite_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_invitation_by_group(
        &self,
        group_id: &str,
        principal_id: &str,
    ) -> Result<Option<InvitationRow>, DataError> {
        let sql = format!(
            "SELECT {INVITATION_COLS} FROM os_groups_invites \
             WHERE GroupID = $1 AND PrincipalID = $2"
        );
        let row = sqlx::query_as::<_, InvitationRow>(&sql)
            .bind(group_id.to_string())
            .bind(principal_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn delete_invite(&self, invite_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM os_groups_invites WHERE InviteID = $1")
            .bind(invite_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_old_invites(&self) -> Result<(), DataError> {
        // Delete invites older than 2 days (same as C#)
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            - 2 * 24 * 3600;
        sqlx::query("DELETE FROM os_groups_invites WHERE TMStamp < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---- notices ----

    pub async fn store_notice(&self, row: &NoticeRow) -> Result<bool, DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM os_groups_notices WHERE NoticeID = $1")
            .bind(row.notice_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO os_groups_notices ({NOTICE_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
        );
        sqlx::query(&sql)
            .bind(row.group_id.clone())
            .bind(row.notice_id.clone())
            .bind(row.timestamp)
            .bind(row.from_name.clone())
            .bind(row.subject.clone())
            .bind(row.message.clone())
            .bind(row.has_attachment)
            .bind(row.attachment_type)
            .bind(row.attachment_name.clone())
            .bind(row.attachment_item_id.clone())
            .bind(row.attachment_owner_id.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn retrieve_notice(&self, notice_id: &str) -> Result<Option<NoticeRow>, DataError> {
        let sql = format!("SELECT {NOTICE_COLS} FROM os_groups_notices WHERE NoticeID = $1");
        let row = sqlx::query_as::<_, NoticeRow>(&sql)
            .bind(notice_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn retrieve_notices(&self, group_id: &str) -> Result<Vec<NoticeRow>, DataError> {
        let sql = format!(
            "SELECT {NOTICE_COLS} FROM os_groups_notices WHERE GroupID = $1 ORDER BY TMStamp DESC"
        );
        let rows = sqlx::query_as::<_, NoticeRow>(&sql)
            .bind(group_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn delete_notice(&self, notice_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM os_groups_notices WHERE NoticeID = $1")
            .bind(notice_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_old_notices(&self) -> Result<(), DataError> {
        // Delete notices older than 14 days (same as C#)
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            - 14 * 24 * 3600;
        sqlx::query("DELETE FROM os_groups_notices WHERE TMStamp < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sqlx::pool::PoolOptions;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDbGuard {
        path: PathBuf,
    }

    impl Drop for TempDbGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn unique_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "welkin-groups-test-{}-{}.db",
            std::process::id(),
            n
        ));
        path
    }

    async fn make_pool(path: &std::path::Path) -> sqlx::AnyPool {
        sqlx::any::install_default_drivers();
        let url = format!("sqlite:{}?mode=rwc", path.display());
        PoolOptions::<sqlx::Any>::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("connect sqlite test database")
    }

    async fn create_tables(pool: &sqlx::AnyPool) {
        static DDL: [&str; 7] = [
            "CREATE TABLE IF NOT EXISTS os_groups_groups (\
             GroupID CHAR(36) PRIMARY KEY, \
             Location VARCHAR(255) NOT NULL DEFAULT '', \
             Name VARCHAR(255) NOT NULL DEFAULT '', \
             Charter TEXT NOT NULL DEFAULT '', \
             InsigniaID CHAR(36) NOT NULL DEFAULT '', \
             FounderID CHAR(36) NOT NULL DEFAULT '', \
             MembershipFee INTEGER NOT NULL DEFAULT 0, \
             OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', \
             ShowInList INTEGER NOT NULL DEFAULT 0, \
             AllowPublish INTEGER NOT NULL DEFAULT 0, \
             MaturePublish INTEGER NOT NULL DEFAULT 0, \
             OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS os_groups_membership (\
             GroupID CHAR(36) NOT NULL, \
             PrincipalID VARCHAR(255) NOT NULL, \
             SelectedRoleID CHAR(36) NOT NULL DEFAULT '', \
             Contribution INTEGER NOT NULL DEFAULT 0, \
             ListInProfile INTEGER NOT NULL DEFAULT 1, \
             AcceptNotices INTEGER NOT NULL DEFAULT 1, \
             AccessToken CHAR(36) NOT NULL DEFAULT '', \
             PRIMARY KEY (GroupID, PrincipalID))",
            "CREATE TABLE IF NOT EXISTS os_groups_roles (\
             GroupID CHAR(36) NOT NULL, \
             RoleID CHAR(36) NOT NULL, \
             Name VARCHAR(255) NOT NULL DEFAULT '', \
             Description VARCHAR(255) NOT NULL DEFAULT '', \
             Title VARCHAR(255) NOT NULL DEFAULT '', \
             Powers BIGINT NOT NULL DEFAULT 0, \
             PRIMARY KEY (GroupID, RoleID))",
            "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (\
             GroupID CHAR(36) NOT NULL, \
             RoleID CHAR(36) NOT NULL, \
             PrincipalID VARCHAR(255) NOT NULL, \
             PRIMARY KEY (GroupID, RoleID, PrincipalID))",
            "CREATE TABLE IF NOT EXISTS os_groups_invites (\
             InviteID CHAR(36) PRIMARY KEY, \
             GroupID CHAR(36) NOT NULL, \
             RoleID CHAR(36) NOT NULL, \
             PrincipalID VARCHAR(255) NOT NULL, \
             TMStamp INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS os_groups_notices (\
             GroupID CHAR(36) NOT NULL, \
             NoticeID CHAR(36) PRIMARY KEY, \
             TMStamp INTEGER NOT NULL DEFAULT 0, \
             FromName VARCHAR(255) NOT NULL DEFAULT '', \
             Subject VARCHAR(255) NOT NULL DEFAULT '', \
             Message TEXT NOT NULL DEFAULT '', \
             HasAttachment INTEGER NOT NULL DEFAULT 0, \
             AttachmentType INTEGER NOT NULL DEFAULT 0, \
             AttachmentName VARCHAR(128) NOT NULL DEFAULT '', \
             AttachmentItemID CHAR(36) NOT NULL DEFAULT '', \
             AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS os_groups_principals (\
             PrincipalID VARCHAR(255) PRIMARY KEY, \
             ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
        ];
        for ddl in DDL {
            sqlx::query(ddl)
                .execute(pool)
                .await
                .expect("create test table");
        }
    }

    #[tokio::test]
    async fn sqlite_store_group_roundtrip() {
        let path = unique_path();
        let _guard = TempDbGuard { path: path.clone() };
        let pool = make_pool(&path).await;
        create_tables(&pool).await;
        let store = GroupsStore::new(pool.clone());

        let group_id = "11111111-2222-3333-4444-555555555555".to_string();
        let group = GroupRow {
            group_id: group_id.clone(),
            name: "Test Group".to_string(),
            charter: "charter".to_string(),
            insignia_id: "00000000-0000-0000-0000-000000000000".to_string(),
            founder_id: "22222222-3333-4444-5555-666666666666".to_string(),
            membership_fee: 7,
            open_enrollment: "1".to_string(),
            show_in_list: 1,
            allow_publish: 1,
            mature_publish: 0,
            owner_role_id: "33333333-4444-5555-6666-777777777777".to_string(),
            location: String::new(),
        };

        assert!(store.store_group(&group).await.unwrap());

        let by_id = store.retrieve_group_by_id(&group_id).await.unwrap().unwrap();
        assert_eq!(by_id.name, group.name);
        assert_eq!(by_id.membership_fee, 7);
        assert_eq!(by_id.show_in_list, 1);
        assert_eq!(by_id.allow_publish, 1);
        assert_eq!(by_id.mature_publish, 0);
        assert_eq!(by_id.open_enrollment, "1");

        let by_name = store
            .retrieve_group_by_name(&group.name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_name.group_id, group.group_id);

        drop(store);
        drop(pool);
    }

    #[tokio::test]
    async fn sqlite_membership_roundtrip() {
        let path = unique_path();
        let _guard = TempDbGuard { path: path.clone() };
        let pool = make_pool(&path).await;
        create_tables(&pool).await;
        let store = GroupsStore::new(pool.clone());

        let group = GroupRow {
            group_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            name: "Membership Group".to_string(),
            charter: String::new(),
            insignia_id: "00000000-0000-0000-0000-000000000000".to_string(),
            founder_id: "11111111-2222-3333-4444-555555555555".to_string(),
            membership_fee: 0,
            open_enrollment: "0".to_string(),
            show_in_list: 0,
            allow_publish: 0,
            mature_publish: 0,
            owner_role_id: "00000000-0000-0000-0000-000000000000".to_string(),
            location: String::new(),
        };
        store.store_group(&group).await.unwrap();

        let membership = MembershipRow {
            group_id: group.group_id.clone(),
            principal_id: "principal-1".to_string(),
            selected_role_id: "role-1".to_string(),
            contribution: 42,
            list_in_profile: 1,
            accept_notices: 0,
            access_token: "token".to_string(),
        };
        assert!(store.store_member(&membership).await.unwrap());

        let got = store
            .retrieve_member(&membership.group_id, &membership.principal_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.contribution, 42);
        assert_eq!(got.list_in_profile, 1);
        assert_eq!(got.accept_notices, 0);
        assert_eq!(got.access_token, "token");

        drop(store);
        drop(pool);
    }

    #[tokio::test]
    async fn sqlite_role_roundtrip() {
        let path = unique_path();
        let _guard = TempDbGuard { path: path.clone() };
        let pool = make_pool(&path).await;
        create_tables(&pool).await;
        let store = GroupsStore::new(pool.clone());

        let group = GroupRow {
            group_id: "99999999-8888-7777-6666-555555555555".to_string(),
            name: "Role Group".to_string(),
            charter: String::new(),
            insignia_id: "00000000-0000-0000-0000-000000000000".to_string(),
            founder_id: "11111111-2222-3333-4444-555555555555".to_string(),
            membership_fee: 0,
            open_enrollment: "1".to_string(),
            show_in_list: 1,
            allow_publish: 0,
            mature_publish: 0,
            owner_role_id: "00000000-0000-0000-0000-000000000000".to_string(),
            location: String::new(),
        };
        store.store_group(&group).await.unwrap();

        let role = RoleRow {
            group_id: group.group_id.clone(),
            role_id: "role-42".to_string(),
            name: "Administrator".to_string(),
            description: "desc".to_string(),
            title: "Admin".to_string(),
            powers: 0x7FFF_FFFF_FFFF_i64,
        };
        assert!(store.store_role(&role).await.unwrap());

        let got = store
            .retrieve_role(&role.group_id, &role.role_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.name, "Administrator");
        assert_eq!(got.powers, 0x7FFF_FFFF_FFFF_i64);

        drop(store);
        drop(pool);
    }

    #[tokio::test]
    async fn sqlite_notice_roundtrip() {
        let path = unique_path();
        let _guard = TempDbGuard { path: path.clone() };
        let pool = make_pool(&path).await;
        create_tables(&pool).await;
        let store = GroupsStore::new(pool.clone());

        let group = GroupRow {
            group_id: "77777777-6666-5555-4444-333333333333".to_string(),
            name: "Notice Group".to_string(),
            charter: String::new(),
            insignia_id: "00000000-0000-0000-0000-000000000000".to_string(),
            founder_id: "11111111-2222-3333-4444-555555555555".to_string(),
            membership_fee: 0,
            open_enrollment: "1".to_string(),
            show_in_list: 1,
            allow_publish: 0,
            mature_publish: 0,
            owner_role_id: "00000000-0000-0000-0000-000000000000".to_string(),
            location: String::new(),
        };
        store.store_group(&group).await.unwrap();

        let notice = NoticeRow {
            group_id: group.group_id.clone(),
            notice_id: "notice-7".to_string(),
            timestamp: 12345,
            from_name: "From".to_string(),
            subject: "Subject".to_string(),
            message: "Message".to_string(),
            has_attachment: 1,
            attachment_type: 2,
            attachment_name: "file.txt".to_string(),
            attachment_item_id: "00000000-0000-0000-0000-000000000000".to_string(),
            attachment_owner_id: "11111111-2222-3333-4444-555555555555".to_string(),
        };
        assert!(store.store_notice(&notice).await.unwrap());

        let got = store
            .retrieve_notice(&notice.notice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.timestamp, 12345);
        assert_eq!(got.has_attachment, 1);
        assert_eq!(got.attachment_type, 2);
        assert_eq!(got.subject, "Subject");

        drop(store);
        drop(pool);
    }

    #[test]
    fn group_powers_bits_match_the_protocol() {
        let table: [(&str, GroupPowers, u32); 51] = [
            ("INVITE", GroupPowers::INVITE, 1),
            ("EJECT", GroupPowers::EJECT, 2),
            ("CHANGE_OPTIONS", GroupPowers::CHANGE_OPTIONS, 3),
            ("CREATE_ROLE", GroupPowers::CREATE_ROLE, 4),
            ("DELETE_ROLE", GroupPowers::DELETE_ROLE, 5),
            ("ROLE_PROPERTIES", GroupPowers::ROLE_PROPERTIES, 6),
            ("ASSIGN_MEMBER_LIMITED", GroupPowers::ASSIGN_MEMBER_LIMITED, 7),
            ("ASSIGN_MEMBER", GroupPowers::ASSIGN_MEMBER, 8),
            ("REMOVE_MEMBER", GroupPowers::REMOVE_MEMBER, 9),
            ("CHANGE_ACTIONS", GroupPowers::CHANGE_ACTIONS, 10),
            ("CHANGE_IDENTITY", GroupPowers::CHANGE_IDENTITY, 11),
            ("LAND_DEED", GroupPowers::LAND_DEED, 12),
            ("LAND_RELEASE", GroupPowers::LAND_RELEASE, 13),
            ("LAND_SET_SALE", GroupPowers::LAND_SET_SALE, 14),
            ("LAND_DIVIDE_JOIN", GroupPowers::LAND_DIVIDE_JOIN, 15),
            ("JOIN_CHAT", GroupPowers::JOIN_CHAT, 16),
            ("FIND_PLACES", GroupPowers::FIND_PLACES, 17),
            ("LAND_CHANGE_IDENTITY", GroupPowers::LAND_CHANGE_IDENTITY, 18),
            ("SET_LANDING_POINT", GroupPowers::SET_LANDING_POINT, 19),
            ("CHANGE_MEDIA", GroupPowers::CHANGE_MEDIA, 20),
            ("LAND_EDIT", GroupPowers::LAND_EDIT, 21),
            ("LAND_OPTIONS", GroupPowers::LAND_OPTIONS, 22),
            ("ALLOW_EDIT_LAND", GroupPowers::ALLOW_EDIT_LAND, 23),
            ("ALLOW_FLY", GroupPowers::ALLOW_FLY, 24),
            ("ALLOW_REZ", GroupPowers::ALLOW_REZ, 25),
            ("ALLOW_LANDMARK", GroupPowers::ALLOW_LANDMARK, 26),
            ("ALLOW_VOICE_CHAT", GroupPowers::ALLOW_VOICE_CHAT, 27),
            ("ALLOW_SET_HOME", GroupPowers::ALLOW_SET_HOME, 28),
            ("LAND_MANAGE_ALLOWED", GroupPowers::LAND_MANAGE_ALLOWED, 29),
            ("LAND_MANAGE_BANNED", GroupPowers::LAND_MANAGE_BANNED, 30),
            ("LAND_MANAGE_PASSES", GroupPowers::LAND_MANAGE_PASSES, 31),
            ("LAND_EJECT_AND_FREEZE", GroupPowers::LAND_EJECT_AND_FREEZE, 32),
            ("RETURN_GROUP_SET", GroupPowers::RETURN_GROUP_SET, 33),
            ("RETURN_NON_GROUP", GroupPowers::RETURN_NON_GROUP, 34),
            ("LAND_GARDENING", GroupPowers::LAND_GARDENING, 35),
            ("DEED_OBJECT", GroupPowers::DEED_OBJECT, 36),
            ("MODERATE_CHAT", GroupPowers::MODERATE_CHAT, 37),
            ("OBJECT_MANIPULATE", GroupPowers::OBJECT_MANIPULATE, 38),
            ("OBJECT_SET_FOR_SALE", GroupPowers::OBJECT_SET_FOR_SALE, 39),
            ("ACCOUNTABLE", GroupPowers::ACCOUNTABLE, 40),
            ("HOST_EVENT", GroupPowers::HOST_EVENT, 41),
            ("SEND_NOTICES", GroupPowers::SEND_NOTICES, 42),
            ("RECEIVE_NOTICES", GroupPowers::RECEIVE_NOTICES, 43),
            ("START_PROPOSAL", GroupPowers::START_PROPOSAL, 44),
            ("VOTE_ON_PROPOSAL", GroupPowers::VOTE_ON_PROPOSAL, 45),
            ("ALLOW_ENVIRONMENT", GroupPowers::ALLOW_ENVIRONMENT, 46),
            ("MEMBER_VISIBLE", GroupPowers::MEMBER_VISIBLE, 47),
            ("RETURN_GROUP_OWNED", GroupPowers::RETURN_GROUP_OWNED, 48),
            ("EXPERIENCE_ADMIN", GroupPowers::EXPERIENCE_ADMIN, 49),
            ("EXPERIENCE_CREATOR", GroupPowers::EXPERIENCE_CREATOR, 50),
            ("GROUP_BAN_ACCESS", GroupPowers::GROUP_BAN_ACCESS, 51),
        ];

        for (name, power, shift) in table {
            assert_eq!(power.bits(), 1u64 << shift, "{name}");
        }

        let mut seen = 0u64;
        for (name, power, _) in &table {
            assert_eq!(seen & power.bits(), 0, "duplicate bit for {name}");
            seen |= power.bits();
        }

        assert_eq!(GroupPowers::NONE.bits(), 0);
    }

    #[test]
    fn group_powers_composites_are_pinned() {
        // OpenSim's DefaultEveryonePowers/OfficersPowers/OwnerPowers computed
        // with the corrected bit positions.
        assert_eq!(GroupPowers::DEFAULT_EVERYONE.bits(), 62672565501952);
        assert_eq!(GroupPowers::OFFICERS.bits(), 560750921775310);
        assert_eq!(GroupPowers::OWNERS.bits(), 4503599627370494);
    }

    #[test]
    fn test_group_powers_default_everyone() {
        let powers = GroupPowers::DEFAULT_EVERYONE;
        assert!(powers.contains(GroupPowers::ALLOW_SET_HOME));
        assert!(powers.contains(GroupPowers::ACCOUNTABLE));
        assert!(powers.contains(GroupPowers::JOIN_CHAT));
        assert!(powers.contains(GroupPowers::ALLOW_VOICE_CHAT));
        assert!(powers.contains(GroupPowers::RECEIVE_NOTICES));
        assert!(powers.contains(GroupPowers::START_PROPOSAL));
        assert!(powers.contains(GroupPowers::VOTE_ON_PROPOSAL));
        assert!(!powers.contains(GroupPowers::INVITE));
    }

    #[test]
    fn test_group_powers_officers_superset_of_everyone() {
        let everyone = GroupPowers::DEFAULT_EVERYONE;
        let officers = GroupPowers::OFFICERS;
        assert!(officers.contains(everyone));
        assert!(officers.contains(GroupPowers::INVITE));
        assert!(officers.contains(GroupPowers::EJECT));
        assert!(officers.contains(GroupPowers::SEND_NOTICES));
    }

    #[test]
    fn test_group_powers_owners_superset_of_officers() {
        let officers = GroupPowers::OFFICERS;
        let owners = GroupPowers::OWNERS;
        assert!(owners.contains(officers));
        assert!(owners.contains(GroupPowers::CREATE_ROLE));
        assert!(owners.contains(GroupPowers::DELETE_ROLE));
        assert!(owners.contains(GroupPowers::REMOVE_MEMBER));
    }

    #[test]
    fn test_group_powers_bits_roundtrip() {
        let powers = GroupPowers::INVITE | GroupPowers::EJECT;
        let bits = powers.bits();
        let restored = GroupPowers::from_bits_truncate(bits);
        assert_eq!(powers, restored);
    }

    #[test]
    fn test_group_row_defaults() {
        let row = GroupRow {
            group_id: "00000000-0000-0000-0000-000000000001".to_string(),
            name: "Test Group".to_string(),
            charter: String::new(),
            insignia_id: "00000000-0000-0000-0000-000000000000".to_string(),
            founder_id: "00000000-0000-0000-0000-000000000002".to_string(),
            membership_fee: 0,
            open_enrollment: "1".to_string(),
            show_in_list: 1,
            allow_publish: 1,
            mature_publish: 0,
            owner_role_id: "00000000-0000-0000-0000-000000000003".to_string(),
            location: String::new(),
        };
        assert_eq!(row.name, "Test Group");
        assert_eq!(row.membership_fee, 0);
    }

    #[test]
    fn test_membership_row_creation() {
        let row = MembershipRow {
            group_id: "g1".to_string(),
            principal_id: "p1".to_string(),
            selected_role_id: "r1".to_string(),
            contribution: 0,
            list_in_profile: 1,
            accept_notices: 1,
            access_token: String::new(),
        };
        assert_eq!(row.list_in_profile, 1);
        assert_eq!(row.accept_notices, 1);
    }
}
