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
        const HOST_EVENT            = 1 << 49;
        const MEMBER_VISIBLE        = 1 << 50;
        const JOIN_CHAT             = 1 << 51;
        const ALLOW_ENVIRONMENT     = 1 << 52;
        const EXPERIENCE_ADMIN      = 1 << 53;
        const EXPERIENCE_CREATOR    = 1 << 54;
        const GROUP_BAN_ACCESS      = 1 << 55;

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
    pub show_in_list: String,
    #[sqlx(rename = "AllowPublish")]
    pub allow_publish: String,
    #[sqlx(rename = "MaturePublish")]
    pub mature_publish: String,
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
    pub contribution: String,
    #[sqlx(rename = "ListInProfile")]
    pub list_in_profile: String,
    #[sqlx(rename = "AcceptNotices")]
    pub accept_notices: String,
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
    pub powers: String,
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
    pub timestamp: String,
    #[sqlx(rename = "FromName")]
    pub from_name: String,
    #[sqlx(rename = "Subject")]
    pub subject: String,
    #[sqlx(rename = "Message")]
    pub message: String,
    #[sqlx(rename = "HasAttachment")]
    pub has_attachment: String,
    #[sqlx(rename = "AttachmentType")]
    pub attachment_type: String,
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
            .bind(row.show_in_list.clone())
            .bind(row.allow_publish.clone())
            .bind(row.mature_publish.clone())
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
            .bind(row.contribution.clone())
            .bind(row.list_in_profile.clone())
            .bind(row.accept_notices.clone())
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
            .bind(row.powers.clone())
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
            .bind(row.timestamp.clone())
            .bind(row.from_name.clone())
            .bind(row.subject.clone())
            .bind(row.message.clone())
            .bind(row.has_attachment.clone())
            .bind(row.attachment_type.clone())
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
            .bind(cutoff.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            show_in_list: "1".to_string(),
            allow_publish: "1".to_string(),
            mature_publish: "0".to_string(),
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
            contribution: "0".to_string(),
            list_in_profile: "1".to_string(),
            accept_notices: "1".to_string(),
            access_token: String::new(),
        };
        assert_eq!(row.list_in_profile, "1");
        assert_eq!(row.accept_notices, "1");
    }
}
