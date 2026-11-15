// SPDX-License-Identifier: LGPL-3.0-or-later

//! Utilities for manipulating external representations of OpenSim data.
//!
//! Handles Hypergrid UUID formats (`uuid;server_uri`) and creator/owner
//! UUID rewrites for cross-grid dearchiving.
//!
//! Ported from `OpenSim.Framework.Serialization.External.ExternalRepresentationUtils`.

use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Hypergrid UUID handling
// ---------------------------------------------------------------------------

/// Parse a Hypergrid-format identifier string.
///
/// HG identifiers use the format `uuid;server_uri` (semicolon-separated).
/// A plain UUID without a semicolon is also accepted.
///
/// Returns `(uuid, optional_server_uri)`. If the string is not a valid UUID
/// (even after stripping the HG suffix), returns `(Uuid::nil(), None)`.
pub fn resolve_hg_uuid(hg_id: &str) -> (Uuid, Option<String>) {
    let hg_id = hg_id.trim();

    if let Some((uuid_part, uri_part)) = hg_id.split_once(';') {
        let uuid = Uuid::parse_str(uuid_part.trim()).unwrap_or(Uuid::nil());
        let uri = uri_part.trim();
        let server_uri = if uri.is_empty() {
            None
        } else {
            Some(uri.to_string())
        };
        (uuid, server_uri)
    } else {
        let uuid = Uuid::parse_str(hg_id).unwrap_or(Uuid::nil());
        (uuid, None)
    }
}

// ---------------------------------------------------------------------------
// Creator data
// ---------------------------------------------------------------------------

/// Build a CreatorData string from a home URL and a display name.
///
/// Format: `homeURL;name`
pub fn calc_creator_data(home_url: &str, name: &str) -> String {
    format!("{home_url};{name}")
}

/// Build a CreatorData string that includes the creator's UUID in the URL path.
///
/// Format: `homeURL/uuid;name`
pub fn calc_creator_data_with_uuid(home_url: &str, uuid: Uuid, name: &str) -> String {
    format!("{home_url}/{uuid};{name}")
}

/// Parse a CreatorData string into `(server_uri, optional_name)`.
///
/// Format: `server_uri;name` or just `server_uri`
pub fn parse_creator_data(creator_data: &str) -> (String, Option<String>) {
    if let Some((uri, name)) = creator_data.split_once(';') {
        let name_str = name.trim();
        let name_opt = if name_str.is_empty() {
            None
        } else {
            Some(name_str.to_string())
        };
        (uri.trim().to_string(), name_opt)
    } else {
        (creator_data.trim().to_string(), None)
    }
}

// ---------------------------------------------------------------------------
// UUID rewrite context
// ---------------------------------------------------------------------------

/// Holds a mapping table for rewriting UUIDs when dearchiving on a different
/// grid.
///
/// Used to replace creator/owner/lastOwner UUIDs in serialized XML data.
#[derive(Debug, Clone, Default)]
pub struct RewriteContext {
    mapping: HashMap<Uuid, Uuid>,
}

impl RewriteContext {
    /// Create a new, empty rewrite context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a UUID mapping: `old_uuid` → `new_uuid`.
    pub fn add_mapping(&mut self, old_uuid: Uuid, new_uuid: Uuid) {
        self.mapping.insert(old_uuid, new_uuid);
    }

    /// Look up the replacement UUID for a given original.
    pub fn get_mapping(&self, old_uuid: Uuid) -> Option<Uuid> {
        self.mapping.get(&old_uuid).copied()
    }

    /// Return the number of mappings.
    pub fn len(&self) -> usize {
        self.mapping.len()
    }

    /// Check if the context has no mappings.
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }

    /// Rewrite a UUID string if a mapping exists, otherwise return the
    /// original.
    pub fn rewrite_uuid_str(&self, uuid_str: &str) -> String {
        if let Ok(uuid) = Uuid::parse_str(uuid_str.trim())
            && let Some(new_uuid) = self.mapping.get(&uuid)
        {
            return new_uuid.to_string();
        }
        uuid_str.to_string()
    }
}

// ---------------------------------------------------------------------------
// XML UUID rewriting
// ---------------------------------------------------------------------------

/// Rewrite creator UUIDs in an XML string.
///
/// Searches for `<CreatorID>` and `<CreatorUUID>` elements and replaces
/// matching UUIDs using the rewrite context.
pub fn rewrite_creator_id(xml: &str, ctx: &RewriteContext) -> String {
    let mut result = xml.to_string();
    for (old_uuid, new_uuid) in &ctx.mapping {
        let old_str = old_uuid.to_string();
        let new_str = new_uuid.to_string();
        result = rewrite_element_uuid(&result, "CreatorID", &old_str, &new_str);
        result = rewrite_element_uuid(&result, "CreatorUUID", &old_str, &new_str);
    }
    result
}

/// Rewrite owner UUIDs in an XML string.
///
/// Searches for `<Owner>` and `<OwnerID>` elements and replaces matching
/// UUIDs using the rewrite context.
pub fn rewrite_owner_id(xml: &str, ctx: &RewriteContext) -> String {
    let mut result = xml.to_string();
    for (old_uuid, new_uuid) in &ctx.mapping {
        let old_str = old_uuid.to_string();
        let new_str = new_uuid.to_string();
        result = rewrite_element_uuid(&result, "Owner", &old_str, &new_str);
        result = rewrite_element_uuid(&result, "OwnerID", &old_str, &new_str);
    }
    result
}

/// Rewrite last-owner UUIDs in an XML string.
pub fn rewrite_last_owner_id(xml: &str, ctx: &RewriteContext) -> String {
    let mut result = xml.to_string();
    for (old_uuid, new_uuid) in &ctx.mapping {
        let old_str = old_uuid.to_string();
        let new_str = new_uuid.to_string();
        result = rewrite_element_uuid(&result, "LastOwnerID", &old_str, &new_str);
    }
    result
}

/// Replace a UUID within a specific XML element.
///
/// Finds `<tag>...old_uuid...</tag>` and replaces the old UUID with the new
/// one. Only replaces when the element content contains the exact UUID string.
fn rewrite_element_uuid(xml: &str, tag: &str, old_uuid: &str, new_uuid: &str) -> String {
    let open_tag = format!("<{tag}>");
    let close_tag = format!("</{tag}>");

    let mut result = String::with_capacity(xml.len());
    let mut search_from = 0;

    while let Some(start) = xml[search_from..].find(&open_tag) {
        let abs_start = search_from + start;
        let content_start = abs_start + open_tag.len();

        if let Some(end) = xml[content_start..].find(&close_tag) {
            let content_end = content_start + end;
            let content = &xml[content_start..content_end];

            result.push_str(&xml[search_from..content_start]);

            // Replace the old UUID in the element content, handling
            // both plain UUID and HG-format (uuid;uri)
            if content.contains(old_uuid) {
                result.push_str(&content.replace(old_uuid, new_uuid));
            } else {
                result.push_str(content);
            }

            search_from = content_end;
        } else {
            // No closing tag found — copy remainder and stop
            break;
        }
    }

    result.push_str(&xml[search_from..]);
    result
}

// ---------------------------------------------------------------------------
// XML sanitization
// ---------------------------------------------------------------------------

/// Sanitize malformed XML namespace declarations.
///
/// Fixes a bug introduced in OpenSim Oct 2014 where `xmlns:xmlns:` prefixes
/// were duplicated.
pub fn sanitize_xml(xml: &str) -> String {
    if xml.is_empty() {
        return xml.to_string();
    }

    // Look for "xmlns:xmlns:" and collapse the duplicated prefix
    if let Some(idx) = xml.find("xmlns:xmlns:") {
        let mut end_idx = idx + 12; // length of "xmlns:xmlns:"
        // Skip any further ":xmlns:" repetitions
        while xml[end_idx..].starts_with("xmlns:") {
            end_idx += 6;
        }
        let bad = &xml[idx..end_idx];
        xml.replace(bad, "xmlns:")
    } else {
        xml.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_hg_uuid_with_uri() {
        let (uuid, uri) =
            resolve_hg_uuid("12345678-1234-1234-1234-123456789abc;http://example.com:8002");
        assert_eq!(
            uuid,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        assert_eq!(uri, Some("http://example.com:8002".to_string()));
    }

    #[test]
    fn resolve_hg_uuid_without_uri() {
        let (uuid, uri) = resolve_hg_uuid("12345678-1234-1234-1234-123456789abc");
        assert_eq!(
            uuid,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        assert!(uri.is_none());
    }

    #[test]
    fn resolve_hg_uuid_empty_uri() {
        let (uuid, uri) = resolve_hg_uuid("12345678-1234-1234-1234-123456789abc;");
        assert_eq!(
            uuid,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        assert!(uri.is_none());
    }

    #[test]
    fn resolve_hg_uuid_invalid() {
        let (uuid, uri) = resolve_hg_uuid("not-a-uuid;http://example.com");
        assert_eq!(uuid, Uuid::nil());
        assert_eq!(uri, Some("http://example.com".to_string()));
    }

    #[test]
    fn resolve_hg_uuid_with_name_in_uri() {
        // HG CreatorData format: "uuid;server_uri;display_name"
        // resolve_hg_uuid only splits on first semicolon
        let (uuid, uri) = resolve_hg_uuid(
            "12345678-1234-1234-1234-123456789abc;http://example.com:8002;Test User",
        );
        assert_eq!(
            uuid,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        // Everything after the first semicolon is the URI part
        assert_eq!(uri, Some("http://example.com:8002;Test User".to_string()));
    }

    #[test]
    fn calc_creator_data_format() {
        let cd = calc_creator_data("http://example.com:8002", "Test User");
        assert_eq!(cd, "http://example.com:8002;Test User");
    }

    #[test]
    fn calc_creator_data_with_uuid_format() {
        let uuid = Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
        let cd = calc_creator_data_with_uuid("http://example.com:8002", uuid, "Test User");
        assert_eq!(
            cd,
            "http://example.com:8002/abcdef01-2345-6789-abcd-ef0123456789;Test User"
        );
    }

    #[test]
    fn parse_creator_data_with_name() {
        let (uri, name) = parse_creator_data("http://example.com:8002;Test User");
        assert_eq!(uri, "http://example.com:8002");
        assert_eq!(name, Some("Test User".to_string()));
    }

    #[test]
    fn parse_creator_data_no_name() {
        let (uri, name) = parse_creator_data("http://example.com:8002");
        assert_eq!(uri, "http://example.com:8002");
        assert!(name.is_none());
    }

    #[test]
    fn rewrite_creator_uuid() {
        let old = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let new = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let mut ctx = RewriteContext::new();
        ctx.add_mapping(old, new);

        let xml = "<SceneObjectPart><CreatorID>11111111-1111-1111-1111-111111111111</CreatorID></SceneObjectPart>";
        let result = rewrite_creator_id(xml, &ctx);
        assert!(result.contains("22222222-2222-2222-2222-222222222222"));
        assert!(!result.contains("11111111-1111-1111-1111-111111111111"));
    }

    #[test]
    fn rewrite_owner_uuid() {
        let old = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let new = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();

        let mut ctx = RewriteContext::new();
        ctx.add_mapping(old, new);

        let xml =
            "<InventoryItem><Owner>33333333-3333-3333-3333-333333333333</Owner></InventoryItem>";
        let result = rewrite_owner_id(xml, &ctx);
        assert!(result.contains("44444444-4444-4444-4444-444444444444"));
        assert!(!result.contains("33333333-3333-3333-3333-333333333333"));
    }

    #[test]
    fn rewrite_preserves_unmapped_uuids() {
        let ctx = RewriteContext::new(); // empty mapping

        let xml = "<Part><CreatorID>55555555-5555-5555-5555-555555555555</CreatorID></Part>";
        let result = rewrite_creator_id(xml, &ctx);
        assert_eq!(result, xml);
    }

    #[test]
    fn rewrite_hg_format_uuid() {
        let old = Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap();
        let new = Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap();

        let mut ctx = RewriteContext::new();
        ctx.add_mapping(old, new);

        let xml = "<Part><CreatorID>66666666-6666-6666-6666-666666666666;http://example.com</CreatorID></Part>";
        let result = rewrite_creator_id(xml, &ctx);
        assert!(result.contains("77777777-7777-7777-7777-777777777777"));
    }

    #[test]
    fn sanitize_xml_no_issue() {
        let xml = r#"<root xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">data</root>"#;
        assert_eq!(sanitize_xml(xml), xml);
    }

    #[test]
    fn sanitize_xml_fixes_duplicated_xmlns() {
        let bad = r#"<root xmlns:xmlns:xsi="bad">data</root>"#;
        let fixed = sanitize_xml(bad);
        assert!(fixed.contains("xmlns:xsi"));
        assert!(!fixed.contains("xmlns:xmlns:"));
    }

    #[test]
    fn rewrite_context_operations() {
        let mut ctx = RewriteContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);

        let old = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let new = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        ctx.add_mapping(old, new);

        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 1);
        assert_eq!(ctx.get_mapping(old), Some(new));
        assert_eq!(ctx.get_mapping(new), None);
    }

    #[test]
    fn rewrite_uuid_str_with_mapping() {
        let old = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();
        let new = Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap();

        let mut ctx = RewriteContext::new();
        ctx.add_mapping(old, new);

        assert_eq!(
            ctx.rewrite_uuid_str("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            "dddddddd-dddd-dddd-dddd-dddddddddddd"
        );
        // Unmapped stays the same
        assert_eq!(
            ctx.rewrite_uuid_str("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
            "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"
        );
    }
}
