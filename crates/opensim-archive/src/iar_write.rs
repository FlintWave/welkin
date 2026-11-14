// SPDX-License-Identifier: GPL-3.0-or-later

//! IAR (Inventory ARchive) write support.
//!
//! Provides `InventoryItem` serialization matching the C#
//! `UserInventoryItemSerializer` format, archive naming helpers, and the
//! permission-filter logic used when building an IAR.
//!
//! Ported from:
//! - `InventoryArchiveWriteRequest.cs`
//! - `UserInventoryItemSerializer.cs`
//! - `InventoryArchiveUtils.cs` (escape/unescape)

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;
use uuid::Uuid;

use opensim_types::{AssetType, PermissionMask};

use crate::constants::INVENTORY_NODE_NAME_COMPONENT_SEPARATOR;
use crate::osp_resolver::{make_ospa_from_uuid, UserService};
use crate::ArchiveError;

// ---------------------------------------------------------------------------
// InventoryItem
// ---------------------------------------------------------------------------

/// A single inventory item, matching the C# `InventoryItemBase` fields used in
/// IAR serialization.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    /// Human-readable item name.
    pub name: String,
    /// Unique item identifier.
    pub id: Uuid,
    /// Inventory type (mirrors OpenMetaverse `InventoryType`).
    pub inv_type: i32,
    /// Creator UUID (may be an OSPA string during serialization).
    pub creator_uuid: Uuid,
    /// Raw creator identifier string (may contain OSPA or plain UUID).
    pub creator_id: String,
    /// Hypergrid creator data (`server_uri;name`), may be empty.
    pub creator_data: String,
    /// Unix timestamp of item creation.
    pub creation_date: i64,
    /// Owner UUID.
    pub owner: Uuid,
    /// Item description.
    pub description: String,
    /// Asset type (i32 value of `AssetType`).
    pub asset_type: i32,
    /// UUID of the underlying asset.
    pub asset_id: Uuid,
    /// Sale type (0=Not, 1=Original, 2=Copy, 3=Contents).
    pub sale_type: u8,
    /// Sale price in L$.
    pub sale_price: i32,
    /// Base permission mask.
    pub base_permissions: u32,
    /// Current (effective) permission mask.
    pub current_permissions: u32,
    /// Permissions granted to everyone.
    pub everyone_permissions: u32,
    /// Permissions granted on next transfer.
    pub next_permissions: u32,
    /// Item flags.
    pub flags: u32,
    /// Group UUID.
    pub group_id: Uuid,
    /// Whether the item is group-owned.
    pub group_owned: bool,
    /// Folder containing this item.
    pub folder_id: Uuid,
}

impl Default for InventoryItem {
    fn default() -> Self {
        Self {
            name: String::new(),
            id: Uuid::nil(),
            inv_type: 0,
            creator_uuid: Uuid::nil(),
            creator_id: String::new(),
            creator_data: String::new(),
            creation_date: 0,
            owner: Uuid::nil(),
            description: String::new(),
            asset_type: 0,
            asset_id: Uuid::nil(),
            sale_type: 0,
            sale_price: 0,
            base_permissions: 0,
            current_permissions: 0,
            everyone_permissions: 0,
            next_permissions: 0,
            flags: 0,
            group_id: Uuid::nil(),
            group_owned: false,
            folder_id: Uuid::nil(),
        }
    }
}

// ---------------------------------------------------------------------------
// Write options
// ---------------------------------------------------------------------------

/// Options controlling IAR write behaviour.
#[derive(Debug, Clone, Default)]
pub struct IarWriteOptions {
    /// Whether to include assets in the archive.
    pub save_assets: bool,
    /// Optional permission filter string (e.g. `"CTM"` for copy+transfer+modify).
    pub filter_content: Option<String>,
    /// Include creator data in serialized items.
    pub include_creator_data: bool,
    /// Home URL for Hypergrid creator-data generation.
    pub home_url: Option<String>,
    /// Whether to be verbose (log more info).
    pub verbose: bool,
}

// ---------------------------------------------------------------------------
// Archive name helpers
// ---------------------------------------------------------------------------

/// Escape an archive path component.
///
/// Encodes `&` as `&amp;` and `/` as `&#47;` so that names containing
/// slashes survive embedding in tar paths.
pub fn escape_archive_path(path: &str) -> String {
    path.replace('&', "&amp;").replace('/', "&#47;")
}

/// Unescape an archive path component.
///
/// Reverses the encoding applied by [`escape_archive_path`].
pub fn unescape_archive_path(path: &str) -> String {
    path.replace("&#47;", "/").replace("&amp;", "&")
}

/// Create an IAR archive folder name.
///
/// Format: `{escaped_name}__{uuid}/`
pub fn create_archive_folder_name(name: &str, id: Uuid) -> String {
    format!(
        "{}{INVENTORY_NODE_NAME_COMPONENT_SEPARATOR}{id}/",
        escape_archive_path(name),
    )
}

/// Create an IAR archive item name.
///
/// Format: `{escaped_name}__{uuid}.xml`
pub fn create_archive_item_name(name: &str, id: Uuid) -> String {
    format!(
        "{}{INVENTORY_NODE_NAME_COMPONENT_SEPARATOR}{id}.xml",
        escape_archive_path(name),
    )
}

// ---------------------------------------------------------------------------
// Permission check
// ---------------------------------------------------------------------------

/// Check if the current permissions pass the filter requirements.
///
/// If `filter` is `None`, all items pass. Otherwise the filter string may
/// contain `C` (copy), `T` (transfer), `M` (modify) — the item must have
/// all indicated permissions.
pub fn can_user_archive_item(filter: Option<&str>, current_permissions: u32) -> bool {
    let filter = match filter {
        Some(f) => f,
        None => return true,
    };

    let perms = PermissionMask::from_bits_truncate(current_permissions);

    if filter.contains('C') && !perms.contains(PermissionMask::COPY) {
        return false;
    }
    if filter.contains('T') && !perms.contains(PermissionMask::TRANSFER) {
        return false;
    }
    if filter.contains('M') && !perms.contains(PermissionMask::MODIFY) {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Helper: write an XML element with text content.
fn write_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    value: &str,
) -> Result<(), ArchiveError> {
    writer
        .create_element(name)
        .write_text_content(BytesText::new(value))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    Ok(())
}

/// Serialize an `InventoryItem` to XML matching the C#
/// `UserInventoryItemSerializer.Serialize` format.
pub fn serialize_inventory_item(
    item: &InventoryItem,
    options: &IarWriteOptions,
    user_service: Option<&dyn UserService>,
) -> Result<String, ArchiveError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <InventoryItem>
    writer
        .write_event(Event::Start(BytesStart::new("InventoryItem")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    write_element(&mut writer, "Name", &item.name)?;
    write_element(&mut writer, "ID", &item.id.to_string())?;
    write_element(&mut writer, "InvType", &item.inv_type.to_string())?;

    // CreatorUUID — may contain OSPA if user service is available
    let creator_str = if let Some(svc) = user_service {
        make_ospa_from_uuid(item.creator_uuid, svc)
            .unwrap_or_else(|| item.creator_uuid.to_string())
    } else {
        item.creator_uuid.to_string()
    };
    write_element(&mut writer, "CreatorUUID", &creator_str)?;

    write_element(&mut writer, "CreationDate", &item.creation_date.to_string())?;
    write_element(&mut writer, "Owner", &item.owner.to_string())?;
    write_element(&mut writer, "Description", &item.description)?;
    write_element(&mut writer, "AssetType", &item.asset_type.to_string())?;
    write_element(&mut writer, "AssetID", &item.asset_id.to_string())?;
    write_element(&mut writer, "SaleType", &item.sale_type.to_string())?;
    write_element(&mut writer, "SalePrice", &item.sale_price.to_string())?;
    write_element(
        &mut writer,
        "BasePermissions",
        &item.base_permissions.to_string(),
    )?;
    write_element(
        &mut writer,
        "CurrentPermissions",
        &item.current_permissions.to_string(),
    )?;
    write_element(
        &mut writer,
        "EveryOnePermissions",
        &item.everyone_permissions.to_string(),
    )?;
    write_element(
        &mut writer,
        "NextPermissions",
        &item.next_permissions.to_string(),
    )?;
    write_element(&mut writer, "Flags", &item.flags.to_string())?;
    write_element(&mut writer, "GroupID", &item.group_id.to_string())?;
    write_element(
        &mut writer,
        "GroupOwned",
        if item.group_owned { "True" } else { "False" },
    )?;

    // CreatorData — conditional inclusion
    if options.include_creator_data && !item.creator_data.is_empty() {
        write_element(&mut writer, "CreatorData", &item.creator_data)?;
    } else if let Some(ref home_url) = options.home_url {
        if let Some(svc) = user_service {
            if let Some((first, last)) = svc.get_user_by_id(item.creator_uuid) {
                let cd = crate::external_representation::calc_creator_data_with_uuid(
                    home_url,
                    item.creator_uuid,
                    &format!("{first} {last}"),
                );
                write_element(&mut writer, "CreatorData", &cd)?;
            }
            write_element(&mut writer, "CreatorID", &item.creator_id)?;
        }
    }

    // </InventoryItem>
    writer
        .write_event(Event::End(BytesEnd::new("InventoryItem")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    let buf = writer.into_inner().into_inner();
    String::from_utf8(buf).map_err(|e| ArchiveError::Xml(e.to_string()))
}

// ---------------------------------------------------------------------------
// Deserialization
// ---------------------------------------------------------------------------

/// Deserialize an `InventoryItem` from XML matching the C#
/// `UserInventoryItemSerializer` format.
pub fn deserialize_inventory_item(xml: &str) -> Result<InventoryItem, ArchiveError> {
    let mut item = InventoryItem::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut inside_item = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;

                if tag == "InventoryItem" {
                    inside_item = true;
                    continue;
                }

                if !inside_item {
                    continue;
                }

                // Read the text content of the element
                let raw = reader
                    .read_text(e.name())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                let text = raw.as_ref();

                match tag {
                    "Name" => item.name = text.to_string(),
                    "ID" => {
                        item.id = Uuid::parse_str(text)
                            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                    }
                    "InvType" => {
                        item.inv_type = text
                            .parse()
                            .map_err(|_| ArchiveError::Invalid(format!("bad InvType: {text}")))?;
                    }
                    "CreatorUUID" => {
                        // May be an OSPA string like "ospa:n=First Last" or a plain UUID
                        item.creator_id = text.to_string();
                        if let Ok(uuid) = Uuid::parse_str(text) {
                            item.creator_uuid = uuid;
                        }
                    }
                    "CreatorID" => {
                        // Overrides CreatorUUID when present
                        item.creator_id = text.to_string();
                        if let Ok(uuid) = Uuid::parse_str(text) {
                            item.creator_uuid = uuid;
                        }
                    }
                    "CreatorData" => {
                        item.creator_data = text.to_string();
                    }
                    "CreationDate" => {
                        item.creation_date = text.parse().unwrap_or(0);
                    }
                    "Owner" => {
                        item.owner = Uuid::parse_str(text)
                            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                    }
                    "Description" => {
                        item.description = text.to_string();
                    }
                    "AssetType" => {
                        item.asset_type = text
                            .parse()
                            .map_err(|_| {
                                ArchiveError::Invalid(format!("bad AssetType: {text}"))
                            })?;
                    }
                    "AssetID" => {
                        item.asset_id = Uuid::parse_str(text)
                            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                    }
                    "SaleType" => {
                        item.sale_type = text.parse().unwrap_or(0);
                    }
                    "SalePrice" => {
                        item.sale_price = text.parse().unwrap_or(0);
                    }
                    "BasePermissions" => {
                        item.base_permissions = text.parse().unwrap_or(0);
                    }
                    "CurrentPermissions" => {
                        item.current_permissions = text.parse().unwrap_or(0);
                    }
                    "EveryOnePermissions" => {
                        item.everyone_permissions = text.parse().unwrap_or(0);
                    }
                    "NextPermissions" => {
                        item.next_permissions = text.parse().unwrap_or(0);
                    }
                    "Flags" => {
                        item.flags = text.parse().unwrap_or(0);
                    }
                    "GroupID" => {
                        item.group_id = Uuid::parse_str(text).unwrap_or(Uuid::nil());
                    }
                    "GroupOwned" => {
                        item.group_owned =
                            text.eq_ignore_ascii_case("true") || text == "1";
                    }
                    _ => {
                        // Unknown elements are silently ignored
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                if tag == "InventoryItem" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ArchiveError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(item)
}

// ---------------------------------------------------------------------------
// IAR control file
// ---------------------------------------------------------------------------

/// Create an IAR control file XML string.
///
/// Matches the C# `InventoryArchiveWriteRequest.CreateControlFile` output.
pub fn create_iar_control_file(
    save_assets: bool,
    home_url: Option<&str>,
) -> Result<String, ArchiveError> {
    let (major, minor) = if home_url.is_some() {
        (1, 2)
    } else {
        (0, 3)
    };

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", None, None)))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    let mut archive_start = BytesStart::new("archive");
    archive_start.push_attribute(("major_version", major.to_string().as_str()));
    archive_start.push_attribute(("minor_version", minor.to_string().as_str()));
    writer
        .write_event(Event::Start(archive_start))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    write_element(
        &mut writer,
        "assets_included",
        if save_assets { "True" } else { "False" },
    )?;

    writer
        .write_event(Event::End(BytesEnd::new("archive")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    let buf = writer.into_inner().into_inner();
    String::from_utf8(buf).map_err(|e| ArchiveError::Xml(e.to_string()))
}

// ---------------------------------------------------------------------------
// Asset gathering
// ---------------------------------------------------------------------------

/// Collect unique asset UUIDs from a list of inventory items.
///
/// Skips link and link-folder asset types since those point to item IDs,
/// not actual assets.
pub fn gather_asset_uuids(items: &[InventoryItem]) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    let mut uuids = Vec::new();

    for item in items {
        let asset_type = AssetType::from_i32(item.asset_type);
        // Don't chase link assets — they point to item IDs, not real assets
        if asset_type == AssetType::Link || asset_type == AssetType::LinkFolder {
            continue;
        }
        if !item.asset_id.is_nil() && seen.insert(item.asset_id) {
            uuids.push(item.asset_id);
        }
    }

    uuids
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item() -> InventoryItem {
        InventoryItem {
            name: "Test Object".to_string(),
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            inv_type: 6,
            creator_uuid: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            creator_id: "22222222-2222-2222-2222-222222222222".to_string(),
            creator_data: String::new(),
            creation_date: 1700000000,
            owner: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            description: "A test item".to_string(),
            asset_type: 6,
            asset_id: Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
            sale_type: 0,
            sale_price: 0,
            base_permissions: 2147483647,
            current_permissions: 2147483647,
            everyone_permissions: 0,
            next_permissions: 2147483647,
            flags: 0,
            group_id: Uuid::nil(),
            group_owned: false,
            folder_id: Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap(),
        }
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let item = sample_item();
        let opts = IarWriteOptions::default();

        let xml = serialize_inventory_item(&item, &opts, None).unwrap();
        let restored = deserialize_inventory_item(&xml).unwrap();

        assert_eq!(restored.name, item.name);
        assert_eq!(restored.id, item.id);
        assert_eq!(restored.inv_type, item.inv_type);
        assert_eq!(restored.creator_uuid, item.creator_uuid);
        assert_eq!(restored.creation_date, item.creation_date);
        assert_eq!(restored.owner, item.owner);
        assert_eq!(restored.description, item.description);
        assert_eq!(restored.asset_type, item.asset_type);
        assert_eq!(restored.asset_id, item.asset_id);
        assert_eq!(restored.sale_type, item.sale_type);
        assert_eq!(restored.sale_price, item.sale_price);
        assert_eq!(restored.base_permissions, item.base_permissions);
        assert_eq!(restored.current_permissions, item.current_permissions);
        assert_eq!(restored.everyone_permissions, item.everyone_permissions);
        assert_eq!(restored.next_permissions, item.next_permissions);
        assert_eq!(restored.flags, item.flags);
        assert_eq!(restored.group_id, item.group_id);
        assert_eq!(restored.group_owned, item.group_owned);
    }

    #[test]
    fn deserialize_csharp_format() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>My Shirt</Name>
  <ID>aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</ID>
  <InvType>18</InvType>
  <CreatorUUID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</CreatorUUID>
  <CreatorData>http://example.com:8002;Jane Doe</CreatorData>
  <CreationDate>1234567890</CreationDate>
  <Owner>cccccccc-cccc-cccc-cccc-cccccccccccc</Owner>
  <Description>A nice shirt</Description>
  <AssetType>5</AssetType>
  <AssetID>dddddddd-dddd-dddd-dddd-dddddddddddd</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>10</SalePrice>
  <BasePermissions>2147483647</BasePermissions>
  <CurrentPermissions>2147483647</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>2147483647</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#;

        let item = deserialize_inventory_item(xml).unwrap();
        assert_eq!(item.name, "My Shirt");
        assert_eq!(
            item.id,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );
        assert_eq!(item.inv_type, 18);
        assert_eq!(
            item.creator_uuid,
            Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap()
        );
        assert_eq!(item.creator_data, "http://example.com:8002;Jane Doe");
        assert_eq!(item.creation_date, 1234567890);
        assert_eq!(
            item.owner,
            Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap()
        );
        assert_eq!(item.description, "A nice shirt");
        assert_eq!(item.asset_type, 5);
        assert_eq!(
            item.asset_id,
            Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap()
        );
        assert_eq!(item.sale_type, 0);
        assert_eq!(item.sale_price, 10);
        assert_eq!(item.base_permissions, 2147483647);
        assert_eq!(item.current_permissions, 2147483647);
        assert_eq!(item.everyone_permissions, 0);
        assert_eq!(item.next_permissions, 2147483647);
        assert_eq!(item.flags, 0);
        assert!(item.group_id.is_nil());
        assert!(!item.group_owned);
    }

    #[test]
    fn deserialize_ospa_creator() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>Test</Name>
  <ID>11111111-1111-1111-1111-111111111111</ID>
  <InvType>0</InvType>
  <CreatorUUID>ospa:n=Test User</CreatorUUID>
  <CreationDate>0</CreationDate>
  <Owner>22222222-2222-2222-2222-222222222222</Owner>
  <Description></Description>
  <AssetType>0</AssetType>
  <AssetID>33333333-3333-3333-3333-333333333333</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>0</BasePermissions>
  <CurrentPermissions>0</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>0</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#;

        let item = deserialize_inventory_item(xml).unwrap();
        assert_eq!(item.creator_id, "ospa:n=Test User");
        // creator_uuid stays nil because OSPA isn't a UUID
        assert!(item.creator_uuid.is_nil());
    }

    #[test]
    fn escape_unescape_roundtrip() {
        let name = "My Folder/SubDir & More";
        let escaped = escape_archive_path(name);
        assert!(!escaped.contains('/'));
        assert!(escaped.contains("&#47;"));
        assert!(escaped.contains("&amp;"));

        let unescaped = unescape_archive_path(&escaped);
        assert_eq!(unescaped, name);
    }

    #[test]
    fn escape_no_special_chars() {
        let name = "Simple Name";
        let escaped = escape_archive_path(name);
        assert_eq!(escaped, name);
    }

    #[test]
    fn create_archive_folder_name_format() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let name = create_archive_folder_name("My Folder", uuid);
        assert_eq!(
            name,
            "My Folder__12345678-1234-1234-1234-123456789abc/"
        );
    }

    #[test]
    fn create_archive_item_name_format() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let name = create_archive_item_name("My Item", uuid);
        assert_eq!(
            name,
            "My Item__12345678-1234-1234-1234-123456789abc.xml"
        );
    }

    #[test]
    fn create_archive_folder_name_escapes_slash() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let name = create_archive_folder_name("A/B", uuid);
        assert!(name.starts_with("A&#47;B__"));
    }

    #[test]
    fn permission_filter_no_filter() {
        assert!(can_user_archive_item(None, 0));
    }

    #[test]
    fn permission_filter_copy_required() {
        let copy_bit = PermissionMask::COPY.bits();
        assert!(can_user_archive_item(Some("C"), copy_bit));
        assert!(!can_user_archive_item(Some("C"), 0));
    }

    #[test]
    fn permission_filter_transfer_required() {
        let transfer_bit = PermissionMask::TRANSFER.bits();
        assert!(can_user_archive_item(Some("T"), transfer_bit));
        assert!(!can_user_archive_item(Some("T"), 0));
    }

    #[test]
    fn permission_filter_modify_required() {
        let modify_bit = PermissionMask::MODIFY.bits();
        assert!(can_user_archive_item(Some("M"), modify_bit));
        assert!(!can_user_archive_item(Some("M"), 0));
    }

    #[test]
    fn permission_filter_combined() {
        let all = PermissionMask::COPY.bits()
            | PermissionMask::TRANSFER.bits()
            | PermissionMask::MODIFY.bits();
        assert!(can_user_archive_item(Some("CTM"), all));

        // Missing copy
        let no_copy = PermissionMask::TRANSFER.bits() | PermissionMask::MODIFY.bits();
        assert!(!can_user_archive_item(Some("CTM"), no_copy));
    }

    #[test]
    fn gather_assets_skips_links() {
        let mut items = vec![sample_item()];
        items.push(InventoryItem {
            name: "Link".to_string(),
            asset_type: AssetType::Link.to_i32(),
            asset_id: Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap(),
            ..Default::default()
        });
        items.push(InventoryItem {
            name: "LinkFolder".to_string(),
            asset_type: AssetType::LinkFolder.to_i32(),
            asset_id: Uuid::parse_str("88888888-8888-8888-8888-888888888888").unwrap(),
            ..Default::default()
        });

        let uuids = gather_asset_uuids(&items);
        // Only the non-link item's asset
        assert_eq!(uuids.len(), 1);
        assert_eq!(
            uuids[0],
            Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap()
        );
    }

    #[test]
    fn gather_assets_deduplicates() {
        let item = sample_item();
        let items = vec![item.clone(), item];
        let uuids = gather_asset_uuids(&items);
        assert_eq!(uuids.len(), 1);
    }

    #[test]
    fn iar_control_file_no_home() {
        let xml = create_iar_control_file(true, None).unwrap();
        assert!(xml.contains("major_version=\"0\""));
        assert!(xml.contains("minor_version=\"3\""));
        assert!(xml.contains("<assets_included>True</assets_included>"));
    }

    #[test]
    fn iar_control_file_with_home() {
        let xml = create_iar_control_file(false, Some("http://example.com")).unwrap();
        assert!(xml.contains("major_version=\"1\""));
        assert!(xml.contains("minor_version=\"2\""));
        assert!(xml.contains("<assets_included>False</assets_included>"));
    }

    #[test]
    fn serialize_xml_structure() {
        let item = sample_item();
        let opts = IarWriteOptions::default();
        let xml = serialize_inventory_item(&item, &opts, None).unwrap();

        assert!(xml.contains("<InventoryItem>"));
        assert!(xml.contains("</InventoryItem>"));
        assert!(xml.contains("<Name>Test Object</Name>"));
        assert!(xml.contains("<InvType>6</InvType>"));
        assert!(xml.contains("<AssetType>6</AssetType>"));
        assert!(xml.contains("<GroupOwned>False</GroupOwned>"));
    }
}
