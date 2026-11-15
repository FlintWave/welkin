// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote inventory-service connector (client side).
//!
//! Talks to a Robust xinventory endpoint over HTTP using URL-encoded form POSTs
//! and `<ServerResponse>` XML replies.

use quick_xml::{Reader, events::Event};
use welkin_data::inventory::{InventoryFolderRow, InventoryItemRow};

use super::RemoteError;

/// HTTP client that talks to a remote xinventory service.
pub struct RemoteInventoryConnector {
    client: reqwest::Client,
    inventory_uri: String,
}

impl RemoteInventoryConnector {
    /// Create a new connector.  `server_uri` is the base URL of the Robust
    /// server (e.g. `http://inv.example.com:8003`).
    pub fn new(server_uri: &str) -> Self {
        let base = server_uri.trim_end_matches('/');
        Self {
            client: reqwest::Client::new(),
            inventory_uri: format!("{base}/xinventory"),
        }
    }

    /// Fetch the root inventory folder for a principal (user UUID).
    pub async fn get_root_folder(
        &self,
        principal: &str,
    ) -> Result<Option<InventoryFolderRow>, RemoteError> {
        let params = [("METHOD", "GETROOTFOLDER"), ("PRINCIPAL", principal)];
        let body = self
            .client
            .post(&self.inventory_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        let folders = parse_folders_from_response(&body)?;
        Ok(folders.into_iter().next())
    }

    /// Fetch the contents of a folder: child folders and child items.
    pub async fn get_folder_content(
        &self,
        folder_id: &str,
    ) -> Result<(Vec<InventoryFolderRow>, Vec<InventoryItemRow>), RemoteError> {
        let params = [("METHOD", "GETFOLDERCONTENT"), ("FOLDER", folder_id)];
        let body = self
            .client
            .post(&self.inventory_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        let folders = parse_folders_section(&body, "FOLDERS")?;
        let items = parse_items_section(&body, "ITEMS")?;
        Ok((folders, items))
    }

    /// Fetch only the items inside a folder (no sub-folders).
    pub async fn get_folder_items(
        &self,
        folder_id: &str,
    ) -> Result<Vec<InventoryItemRow>, RemoteError> {
        let params = [("METHOD", "GETFOLDERITEMS"), ("FOLDER", folder_id)];
        let body = self
            .client
            .post(&self.inventory_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        parse_items_section(&body, "ITEMS")
    }
}

// ---------------------------------------------------------------------------
// XML parsing helpers
// ---------------------------------------------------------------------------

/// Parse all folder elements that appear as direct children of `<ServerResponse>`.
/// Handles the single-folder case (e.g. `<folder type="List">...</folder>`)
/// used by GETROOTFOLDER.
fn parse_folders_from_response(xml: &str) -> Result<Vec<InventoryFolderRow>, RemoteError> {
    parse_typed_list_elements(xml, parse_folder_fields)
}

/// Parse all folder elements inside a named wrapper (e.g. `<FOLDERS type="List">`).
fn parse_folders_section(xml: &str, section: &str) -> Result<Vec<InventoryFolderRow>, RemoteError> {
    let inner = extract_section(xml, section);
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    // Wrap so the parser can walk it as a top-level element.
    let wrapped = format!("<W>{inner}</W>");
    parse_typed_list_elements(&wrapped, parse_folder_fields)
}

/// Parse all item elements inside a named wrapper (e.g. `<ITEMS type="List">`).
fn parse_items_section(xml: &str, section: &str) -> Result<Vec<InventoryItemRow>, RemoteError> {
    let inner = extract_section(xml, section);
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let wrapped = format!("<W>{inner}</W>");
    parse_typed_list_elements(&wrapped, parse_item_fields)
}

/// Extract the raw inner XML of the first occurrence of `<section ...>...</section>`.
fn extract_section(xml: &str, section: &str) -> String {
    let mut reader = Reader::from_str(xml);
    let mut depth: Vec<String> = Vec::new();
    let mut capturing = false;
    let mut capture_depth = 0usize;
    let mut start_pos: Option<u64> = None;
    let mut end_pos: Option<u64> = None;

    loop {
        let pos = reader.buffer_position();
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if !capturing && name == section {
                    capturing = true;
                    capture_depth = depth.len();
                    // Content starts after this tag.
                    start_pos = Some(reader.buffer_position());
                }
                depth.push(name);
            }
            Ok(Event::End(_)) => {
                depth.pop();
                if capturing && depth.len() == capture_depth {
                    end_pos = Some(pos);
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    match (start_pos, end_pos) {
        (Some(s), Some(e)) if e > s => xml[s as usize..e as usize].to_owned(),
        _ => String::new(),
    }
}

/// Generic parser: walk an XML fragment, find every direct child that has
/// `type="List"`, feed its child text elements to `builder`, collect results.
fn parse_typed_list_elements<T, F>(xml: &str, builder: F) -> Result<Vec<T>, RemoteError>
where
    F: Fn(&[(String, String)]) -> Option<T>,
{
    let mut reader = Reader::from_str(xml);
    let mut results: Vec<T> = Vec::new();
    let mut depth: Vec<String> = Vec::new();
    // We track: are we inside a type="List" child at depth==1?
    let mut in_list = false;
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut current_field: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if depth.len() == 1 {
                    let is_list = e.attributes().flatten().any(|a| {
                        a.key.as_ref() == b"type"
                            && a.unescape_value().map(|v| v == "List").unwrap_or(false)
                    });
                    if is_list {
                        in_list = true;
                        fields.clear();
                    }
                } else if depth.len() == 2 && in_list {
                    current_field = Some(name.clone());
                }
                depth.push(name);
            }
            Ok(Event::End(_)) => {
                depth.pop();
                if depth.len() == 1 && in_list {
                    if let Some(item) = builder(&fields) {
                        results.push(item);
                    }
                    in_list = false;
                }
                if depth.len() == 2 {
                    current_field = None;
                }
            }
            Ok(Event::Text(t)) => {
                if in_list
                    && let Some(ref field) = current_field
                    && let Ok(text) = t.unescape()
                {
                    fields.push((field.clone(), text.to_string()));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(RemoteError::Parse(e.to_string())),
            _ => {}
        }
    }
    Ok(results)
}

fn field_str<'a>(fields: &'a [(String, String)], key: &str) -> &'a str {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn field_num<T: std::str::FromStr + Default>(fields: &[(String, String)], key: &str) -> T {
    field_str(fields, key).trim().parse().unwrap_or_default()
}

fn parse_folder_fields(fields: &[(String, String)]) -> Option<InventoryFolderRow> {
    let id: &str = field_str(fields, "ID");
    if id.is_empty() {
        return None;
    }
    Some(InventoryFolderRow {
        folder_name: field_str(fields, "Name").to_string(),
        folder_type: field_num(fields, "Type"),
        version: field_num(fields, "Version"),
        folder_id: id.to_string(),
        agent_id: field_str(fields, "Owner").to_string(),
        parent_folder_id: field_str(fields, "ParentID").to_string(),
    })
}

fn parse_item_fields(fields: &[(String, String)]) -> Option<InventoryItemRow> {
    let id: &str = field_str(fields, "ID");
    if id.is_empty() {
        return None;
    }
    let perm = |k: &str| -> i64 { field_num::<u32>(fields, k) as i64 };
    Some(InventoryItemRow {
        asset_id: field_str(fields, "AssetID").to_string(),
        asset_type: field_num(fields, "AssetType"),
        inventory_name: field_str(fields, "Name").to_string(),
        inventory_description: field_str(fields, "Description").to_string(),
        next_permissions: perm("NextPermissions"),
        current_permissions: perm("CurrentPermissions"),
        inv_type: field_num(fields, "InvType"),
        creator_id: field_str(fields, "CreatorId").to_string(),
        base_permissions: perm("BasePermissions"),
        everyone_permissions: perm("EveryOnePermissions"),
        sale_price: field_num(fields, "SalePrice"),
        sale_type: field_num(fields, "SaleType"),
        creation_date: field_num(fields, "CreationDate"),
        group_id: field_str(fields, "GroupID").to_string(),
        group_owned: i32::from(field_str(fields, "GroupOwned").eq_ignore_ascii_case("true")),
        flags: perm("Flags"),
        inventory_id: id.to_string(),
        avatar_id: field_str(fields, "Owner").to_string(),
        parent_folder_id: field_str(fields, "Folder").to_string(),
        group_permissions: perm("GroupPermissions"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_formats_uri() {
        let c = RemoteInventoryConnector::new("http://inv.example.com:8003");
        assert_eq!(c.inventory_uri, "http://inv.example.com:8003/xinventory");

        let c2 = RemoteInventoryConnector::new("http://inv.example.com:8003/");
        assert_eq!(c2.inventory_uri, "http://inv.example.com:8003/xinventory");
    }

    #[test]
    fn parse_single_folder() {
        let xml = r#"<?xml version="1.0"?><ServerResponse><folder type="List"><ParentID>00000000-0000-0000-0000-000000000000</ParentID><Type>-1</Type><Version>12</Version><Name>My Inventory</Name><Owner>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</Owner><ID>11111111-2222-3333-4444-555555555555</ID></folder></ServerResponse>"#;
        let folders = parse_folders_from_response(xml).unwrap();
        assert_eq!(folders.len(), 1);
        let f = &folders[0];
        assert_eq!(f.folder_id, "11111111-2222-3333-4444-555555555555");
        assert_eq!(f.folder_name, "My Inventory");
        assert_eq!(f.folder_type, -1);
        assert_eq!(f.version, 12);
        assert_eq!(f.agent_id, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert_eq!(f.parent_folder_id, "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn parse_folder_content() {
        let xml = r#"<?xml version="1.0"?><ServerResponse><FID>aaa</FID><VERSION>5</VERSION><FOLDERS type="List"><folder_0 type="List"><ParentID>aaa</ParentID><Type>0</Type><Version>1</Version><Name>Textures</Name><Owner>bbb</Owner><ID>ccc</ID></folder_0></FOLDERS><ITEMS type="List"><item_0 type="List"><AssetID>ddd</AssetID><AssetType>0</AssetType><BasePermissions>581639</BasePermissions><CreationDate>1000</CreationDate><CreatorId>bbb</CreatorId><CreatorData></CreatorData><CurrentPermissions>581639</CurrentPermissions><Description>test item</Description><EveryOnePermissions>0</EveryOnePermissions><Flags>0</Flags><Folder>aaa</Folder><GroupID>00000000-0000-0000-0000-000000000000</GroupID><GroupOwned>False</GroupOwned><GroupPermissions>0</GroupPermissions><ID>eee</ID><InvType>0</InvType><Name>Test Texture</Name><NextPermissions>581639</NextPermissions><Owner>bbb</Owner><SalePrice>0</SalePrice><SaleType>0</SaleType></item_0></ITEMS></ServerResponse>"#;

        let folders = parse_folders_section(xml, "FOLDERS").unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].folder_name, "Textures");
        assert_eq!(folders[0].folder_id, "ccc");

        let items = parse_items_section(xml, "ITEMS").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].inventory_name, "Test Texture");
        assert_eq!(items[0].inventory_id, "eee");
        assert_eq!(items[0].asset_id, "ddd");
        assert_eq!(items[0].inventory_description, "test item");
    }

    #[test]
    fn parse_empty_response() {
        let xml = r#"<?xml version="1.0"?><ServerResponse></ServerResponse>"#;
        let folders = parse_folders_from_response(xml).unwrap();
        assert!(folders.is_empty());
    }
}
