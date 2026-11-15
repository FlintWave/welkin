# Security Review: Social Modules (Task 7.3)

**Reviewer:** Claude Opus 4.6 (HIGH RISK — Opus-level review)
**Adversarial review pass:** 2026-11-14 (added H-3, M-8, M-9, upgraded M-7)
**Date:** 2026-11-14
**Scope:** Groups, OfflineIM, Profiles, NPC, Concierge, IRC Bridge
**Verdict:** CONDITIONAL PASS

---

## Summary

The social modules port is generally sound. SQL injection is fully mitigated
through consistent use of sqlx parameterized queries across all data stores.
Group permission checks at the service layer follow the C# V2 Groups design
where the ROBUST service trusts calling region modules, with enforcement split
between the service (EJECT, INVITE, SEND_NOTICES, CREATE_ROLE, DELETE_ROLE,
CHANGE_ACTIONS) and the region module (viewer-facing validation). The NPC owner
validation logic is correct and well-tested.

Two HIGH findings require remediation before the IRC bridge goes live and before
group creation is exposed to untrusted callers. Several MEDIUM findings relate to
missing size limits and input sanitization that should be addressed before
production deployment.

---

## Finding Counts

| Severity | Count |
|----------|-------|
| Critical | 0     |
| High     | 3     |
| Medium   | 10    |
| Low      | 6     |
| Info     | 5     |

---

## HIGH Findings

### H-1: IRC Protocol Injection via Avatar Names and Chat Messages

**Severity:** HIGH
**Affected files:**
- `crates/welkin-modules/src/irc_bridge.rs:205-207` (`format_nick`)
- `crates/welkin-modules/src/irc_bridge.rs:213-229` (`on_chat_from_world`)

**Description:** The `format_nick` method inserts avatar names into the nick
format string without any sanitization. The `on_chat_from_world` method
similarly passes message content through unsanitized. The IRC protocol uses
CRLF (`\r\n`) as a line delimiter; any occurrence in the avatar name or
message text would be interpreted as a protocol boundary by the IRC server.

An avatar named `evil\r\nQUIT :hacked\r\nPRIVMSG NickServ :DROP evil` would
inject arbitrary IRC commands when the nick is sent to the server. Similarly,
in-world chat containing CRLF would inject protocol-level commands.

The TCP I/O is currently stubbed, so this is not exploitable today, but the
formatting code is real and will be used when real I/O is connected.

**Recommendation:** Before the IRC bridge goes live, add a sanitization
function that strips or replaces `\r`, `\n`, and `\0` from both avatar names
and message text before they reach any IRC protocol output:

```rust
fn sanitize_for_irc(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\r' && *c != '\n' && *c != '\0')
        .collect()
}
```

Apply it in `format_nick` and in `on_chat_from_world` before formatting.

---

### H-2: LevelGroupCreate Not Enforced Anywhere in the Groups Stack

**Severity:** HIGH
**Affected files:**
- `crates/welkin-modules/src/groups/mod.rs:719` (config read but not used)
- `crates/welkin-modules/src/groups/mod.rs:779-807` (`handle_create_group_request`)
- `crates/welkin-services/src/groups.rs:455-528` (`handle_put_group` ADD)

**Description:** The `GroupsModule` reads `LevelGroupCreate` from config
(stored in `self.level_group_create`) but never checks it. The
`handle_create_group_request` method delegates directly to the connector
without verifying the agent's user level. The service-level `handle_put_group`
ADD handler also does not check it.

In C# OpenSimulator, the region module's `CreateGroup` handler checks
`scene.Permissions.IsGodUser(remoteClient)` when `m_levelGroupCreate > 0`,
and also compares the agent's UserLevel against `m_levelGroupCreate`. Without
this check, any agent can create groups regardless of the configured minimum
god level, bypassing grid operator access controls.

**Recommendation:** Add a user-level check in `handle_create_group_request`
before calling the connector:

```rust
if self.level_group_create > 0 {
    // Check agent's UserLevel >= self.level_group_create
    // Requires access to the user account service or scene permissions
}
```

This requires plumbing the agent's user level into the groups module, either
through the scene reference or a permission-checking callback.

---

### H-3: Group Owner Can Be Ejected From Their Own Group

**Severity:** HIGH
**Affected file:** `crates/welkin-services/src/groups.rs:623-657`

**Description:** `handle_remove_agent_from_group` checks
`GroupPowers::EJECT` when the requester differs from the target, but does
not check whether the target holds the owner role. An officer with EJECT
power can remove the group founder/owner from the group, seizing effective
control.

In C# OpenSim, the groups service verifies the target is not in the
`OwnerRoleID` before allowing removal. The Rust port skips this check.

**Recommendation:** Before deleting the member, retrieve the group record
and verify the target agent is not in `owner_role_id`:

```rust
if let Ok(Some(group)) = store.retrieve_group_by_id(group_id).await {
    if store.retrieve_role_member(group_id, &group.owner_role_id, agent_id)
        .await.ok().flatten().is_some()
    {
        return null_result("Cannot eject group owner");
    }
}
```

---

## MEDIUM Findings

### M-1: ADDAGENTTOGROUP Service Handler Has No Authorization Check

**Severity:** MEDIUM
**Affected file:** `crates/welkin-services/src/groups.rs:598-621`

**Description:** The `handle_add_agent_to_group` handler adds any agent to any
group/role without checking whether the requesting agent has permission, whether
the group allows open enrollment, or whether a valid invitation exists. This is
consistent with the C# V2 Groups service design (the service trusts the calling
region module), but if the service endpoint is accessible from untrusted
networks, any caller can add arbitrary agents to groups.

**Recommendation:** Ensure the groups service HTTP endpoint is bound to a
private/loopback interface and not exposed to the public internet. Document
this trust boundary. Consider adding an optional shared-secret or token-based
authentication for service-to-service calls.

---

### M-2: SQL LIKE Wildcards Not Escaped in Group Search

**Severity:** MEDIUM
**Affected file:** `crates/welkin-data/src/groups.rs:325-333`

**Description:** The `retrieve_groups` method wraps the user-supplied search
pattern with `%` for a SQL LIKE query, but does not escape special LIKE
metacharacters (`%`, `_`, `\`) in the input. A search for `%` returns all
groups. A search for `_` matches any single character. This enables an
attacker to enumerate all group names on the grid, which may include private
or unlisted groups.

**Recommendation:** Escape LIKE metacharacters before wrapping:

```rust
let escaped = pattern.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
let like = format!("%{escaped}%");
```

---

### M-3: No Field Size Limits on Profile Text

**Severity:** MEDIUM
**Affected files:**
- `crates/welkin-data/src/profiles.rs` (all store methods)
- `crates/welkin-services/src/profiles.rs` (all update handlers)

**Description:** Profile fields (`profile_about_text`, `profile_first_text`,
`profile_want_to_text`, `profile_skills_text`), classified descriptions, pick
descriptions, and user notes have no size limits enforced at the service or
data layer. An attacker could store arbitrarily large strings, consuming
database storage. The C# implementation also relies on database column limits
rather than application-level enforcement.

**Recommendation:** Add application-level size limits matching the SL viewer
expectations: 510 bytes for about text, 255 bytes for first-life text,
1024 bytes for classified/pick descriptions, 65535 bytes for notes. Enforce
in the service handlers before storing.

---

### M-4: No HTML/Script Sanitization on User-Submitted Text

**Severity:** MEDIUM
**Affected files:**
- `crates/welkin-services/src/profiles.rs` (all update handlers)
- `crates/welkin-services/src/groups.rs:1116-1169` (notice text)

**Description:** Profile text, picks, classifieds, notes, and group notice
content are stored and returned as-is without any HTML or script sanitization.
While the SL/OS in-world viewer does not render HTML, profile data may be
displayed in web-based profile viewers, search engines, or third-party tools.
Stored XSS payloads in profile text could execute in those contexts.

**Recommendation:** At minimum, strip or escape HTML tags (`<`, `>`, `&`) in
user-submitted text fields before storage. Consider a whitelist approach if
limited formatting is desired.

---

### M-5: No Message Size Limit in Offline IM Storage

**Severity:** MEDIUM
**Affected files:**
- `crates/welkin-data/src/offline_im.rs:66-88` (`store_message`)
- `crates/welkin-services/src/offline_im.rs:136-167` (`handle_store`)

**Description:** The offline IM `Message` field (which contains an
XML-serialized `GridInstantMessage`) has no size limit. The count limit
(`max_offline_ims = 25`) prevents excessive message count but not excessive
message size. An attacker could store 25 multi-megabyte messages per
recipient, consuming significant database storage.

**Recommendation:** Add a maximum message size (e.g., 64 KiB per message,
matching reasonable IM payload limits) and reject oversized messages in both
the data store and the service handler.

---

### M-6: HG Groups Cache Has No Maximum Size Bound

**Severity:** MEDIUM
**Affected file:** `crates/welkin-modules/src/groups/hg_connector.rs:166-420`

**Description:** The `RemoteConnectorCacheWrapper` uses `HashMap` for all
caches with no maximum entry count. While entries expire after the configured
TTL (default 60 seconds), if entries are created faster than they expire
(e.g., a foreign grid triggering lookups for many distinct groups/agents),
memory usage grows without bound.

**Recommendation:** Add a maximum cache size (e.g., 10,000 entries across all
caches). When the limit is reached, either reject new entries or evict the
oldest. Consider using an LRU cache implementation.

---

### M-7: NPC Control Operations Do Not Validate Caller Ownership

**Severity:** MEDIUM (upgraded to HIGH in adversarial review — see H-3 note)
**Affected file:** `crates/welkin-modules/src/npc/mod.rs:221-283`

**Description:** The `move_to`, `say`, `sit`, `stand`, `rotate`, and `touch`
trait methods on `INPCModule` do not take a `caller_id` parameter and do not
call `validate_owner`. Only `delete_npc` validates ownership. Any code with
access to the module interface can control any NPC regardless of ownership.
The trait definition itself (`INPCModule`) omits `caller_id` from these method
signatures, meaning callers *cannot* pass ownership information even if they
wanted to — this is a design-level gap, not just a missing check.

In C#, the OSSL functions (`osNpcMoveTo`, `osNpcSay`, etc.) each call
`CheckPermissions` which verifies the calling script's owner matches the NPC
owner. The Rust port must add caller_id parameters to these trait methods.

**Recommendation:** Add `caller_id: Uuid` to all NPC control methods in both
the trait and implementation, and call `validate_owner` in each:

```rust
fn move_to(&mut self, npc_id: Uuid, caller_id: Uuid, position: [f32; 3]) -> Result<(), ModuleError>;
fn say(&self, npc_id: Uuid, caller_id: Uuid, channel: i32, message: &str) -> Result<(), ModuleError>;
fn sit(&mut self, npc_id: Uuid, caller_id: Uuid, target: Uuid) -> Result<(), ModuleError>;
fn stand(&mut self, npc_id: Uuid, caller_id: Uuid) -> Result<(), ModuleError>;
fn rotate(&mut self, npc_id: Uuid, caller_id: Uuid, rotation: [f32; 4]) -> Result<(), ModuleError>;
fn touch(&self, npc_id: Uuid, caller_id: Uuid, target: Uuid) -> Result<(), ModuleError>;
```

---

### M-8: Group Chat Session Join Does Not Verify Group Membership

**Severity:** MEDIUM
**Affected file:** `crates/welkin-modules/src/groups/messaging.rs:183-197`

**Description:** `handle_session_group_start` adds any `agent_id` to a group
chat session by calling `self.sessions.invite_agent(group_id, agent_id)` without
verifying the agent is a member of the group. An agent who is not a group member
could inject themselves into group chat and receive all subsequent messages
fanned out to active session participants.

In C# OpenSim, `GroupsMessagingModule.StartGroupChatSession` verifies group
membership before adding the agent to the session.

**Recommendation:** Before calling `self.sessions.invite_agent`, query the
groups connector to verify the agent is a member of `group_id`. Return without
adding if not a member.

---

### M-9: handle_update_membership Lacks Authorization Check

**Severity:** MEDIUM
**Affected file:** `crates/welkin-services/src/groups.rs:1014-1046`

**Description:** The `UPDATEMEMBERSHIP` handler accepts `RequestingAgentID` and
`AgentID` but does not verify that the requester is the same as the target agent
or that the requester has any group power. Any caller can modify any agent's
`AcceptNotices` and `ListInProfile` settings for any group.

While AcceptNotices and ListInProfile are low-impact preference fields, this
violates the principle that agents should only modify their own membership
preferences.

**Recommendation:** Enforce `requesting_agent == agent_id` (agents should only
update their own membership preferences). If admin override is needed, check for
an appropriate group power.

---

### M-10: Offline IM Sender UUID Not Validated

**Severity:** MEDIUM
**Affected file:** `crates/welkin-services/src/offline_im.rs:136-167`

**Description:** The `handle_store` handler accepts the `FromID` field without
validation. If the service endpoint is accessible to untrusted callers, a
spoofed `FromID` allows impersonating any agent as the sender of an offline
message. The message recipient would see the spoofed sender when they log in.

**Recommendation:** Same as M-1 — ensure the offline IM service is bound to a
private interface. Consider adding UUID format validation and optional
caller authentication.

---

## LOW Findings

### L-1: NPC Names Not Sanitized

**Severity:** LOW
**Affected file:** `crates/welkin-modules/src/npc/mod.rs:165-218`

**Description:** NPC first and last names are accepted without validation for
control characters, excessive length, or empty strings. Names containing null
bytes, newlines, or Unicode control characters could cause display issues in
viewers. Names exceeding ~63 characters could cause protocol-level issues.

**Recommendation:** Validate NPC names: reject empty names, enforce a maximum
length (63 bytes), and strip control characters (U+0000-U+001F, U+007F-U+009F).

---

### L-2: UUI Home URI Not Validated for Proper URI Format

**Severity:** LOW
**Affected file:** `crates/welkin-modules/src/groups/hg_connector.rs:57-64`

**Description:** `ForeignImporter::parse_home_uri` extracts the home URI from
a UUI string but does not validate it is a well-formed HTTP(S) URL. If the URI
is later used for HTTP requests to remote groups services, a crafted URI could
cause SSRF (server-side request forgery) targeting internal services.

**Recommendation:** Validate the extracted URI against a whitelist of allowed
schemes (http/https) and reject URIs targeting private/loopback addresses.

---

### L-3: IRC Reconnect Has No Maximum Retry Count

**Severity:** LOW
**Affected file:** `crates/welkin-modules/src/irc_bridge.rs:108-143`

**Description:** The `ReconnectBackoff` caps delay at 300 seconds but has no
maximum failure count. If the IRC server is permanently unreachable, the bridge
retries indefinitely every 5 minutes, consuming resources (DNS lookups, TCP
connection attempts, log entries).

**Recommendation:** Add a configurable maximum retry count (e.g., 100). After
exhausting retries, log a final error and disable the bridge until manually
re-enabled or the module is restarted.

---

### L-4: Classified/Pick Updates Do Not Verify Ownership at Service Level

**Severity:** LOW
**Affected file:** `crates/welkin-services/src/profiles.rs:232-256, 329-361`

**Description:** The `classified_update` and `picks_update` JSON-RPC handlers
do not verify that the requesting agent owns the classified or pick being
modified. Anyone who knows the UUID of a classified or pick could update or
replace it. This is consistent with C# design where ownership checks happen at
the region module level.

**Recommendation:** Document the trust boundary. Consider adding a
`creator_uuid` check in the service handler as defense-in-depth.

---

### L-5: Missing ROLE_PROPERTIES Power Check on Role Update

**Severity:** LOW
**Affected file:** `crates/welkin-services/src/groups.rs:788-789`

**Description:** The `handle_put_role` UPDATE operation checks for
`CHANGE_ACTIONS` power but not `ROLE_PROPERTIES`. In C#, modifying role
properties (name, title, description, powers) also requires the
`RoleProperties` power.

**Recommendation:** Add a check for `GroupPowers::ROLE_PROPERTIES` alongside
`CHANGE_ACTIONS` for role UPDATE operations.

---

### L-6: HG Cache Key Collision on Group Name

**Severity:** LOW
**Affected file:** `crates/welkin-modules/src/groups/hg_connector.rs:214-221`

**Description:** The `cache_group` method caches by both group ID and group
name. If two groups on different grids share the same name, the name-based
cache entry will return whichever was cached last, potentially leaking data
from one grid's group to another grid's lookup.

**Recommendation:** Namespace the name-based cache key with the group's home
grid URI, e.g., `format!("group-{home_uri}-{name}")`.

---

## INFO / Positive Findings

### I-1: All SQL Queries Properly Parameterized via sqlx (POSITIVE)

All data stores (`GroupsStore`, `OfflineImStore`, `ProfilesStore`) use sqlx
parameterized queries (`$1`, `$2`, etc.) with `.bind()` for every
user-supplied value. No string interpolation of user input into SQL. This
completely mitigates SQL injection.

### I-2: GroupPowers Bitflags Correctly Map All 48 C# Powers (POSITIVE)

All 48 individual powers from the C# `GroupPowers` enum are represented with
the correct bit positions. The bit gaps at positions 16, 41, and 46-47 match
the C# definition. The composite powers (DEFAULT_EVERYONE, OFFICERS, OWNERS)
are correctly defined as supersets. Unit tests verify the superset relationships.

### I-3: NPC Owner Validation Logic Is Correct (POSITIVE)

The `validate_owner` method correctly handles all cases:
- Returns error for nonexistent NPCs
- Allows anyone to manage unowned NPCs only when `ALLOW_NOT_OWNED` is set
- Requires exact owner match otherwise
- Unit tests cover owner/stranger/unowned scenarios

### I-4: Offline IM Retention Cleanup Correctly Implemented (POSITIVE)

Both the data store (`delete_old` with 14-day cutoff) and the module
(`max_offline_ims` count limit) implement proper cleanup. The service
correctly deletes messages after retrieval (matching C# behavior).

### I-5: Group Service Permission Checks Are Consistent (POSITIVE)

The groups service correctly enforces permissions for:
- `EJECT` power for removing members (with self-removal exemption)
- `INVITE` power for creating invitations
- `SEND_NOTICES` power for adding group notices
- `CREATE_ROLE` power for adding roles
- `DELETE_ROLE` power for removing roles (with Everyone/Owner role protection)
- `CHANGE_ACTIONS` power for updating group properties
- `ASSIGN_MEMBER` / `ASSIGN_MEMBER_LIMITED` for role assignments

The Owner role and Everyone role are protected from deletion.

---

## Recommendations Summary

**Before production deployment:**
1. Fix H-1 (IRC protocol injection) — add sanitization before real I/O
2. Fix H-2 (LevelGroupCreate) — enforce in region module before connector call
3. Fix H-3 (Owner ejection) — prevent ejecting group owners
4. Address M-1/M-10 — ensure service endpoints are on private interfaces
5. Address M-5/M-3 — add size limits on stored text fields
6. Address M-8 — verify group membership before joining chat sessions
7. Address M-9 — enforce self-only membership preference updates

**Before HG federation goes live:**
8. Address M-6 — add cache size limits
9. Address L-2 — validate home URIs
10. Address L-6 — namespace cache keys by home grid

**Defense-in-depth improvements:**
11. Address M-2 — escape LIKE wildcards
12. Address M-4 — sanitize HTML in profile text
13. Address M-7 — add caller_id to NPC control methods
