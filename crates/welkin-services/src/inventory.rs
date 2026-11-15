// SPDX-License-Identifier: LGPL-3.0-or-later
use std::{collections::HashMap, fmt::Write as _, sync::Arc};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use quick_xml::escape::escape;
use welkin_data::inventory::{InventoryFolderRow, InventoryItemRow, InventoryStore};

const XML_DECL: &str = "<?xml version=\"1.0\"?>";

#[derive(Clone)]
pub struct InventoryServiceState {
    pub store: Arc<InventoryStore>,
}

pub fn router(state: InventoryServiceState) -> Router {
    Router::new()
        .route("/xinventory", post(handle_xinventory))
        .with_state(state)
}

#[derive(Default)]
struct Form {
    single: HashMap<String, String>,
    multi: HashMap<String, Vec<String>>,
}

impl Form {
    fn get(&self, key: &str) -> Option<&String> {
        self.single.get(key)
    }

    fn str(&self, key: &str) -> &str {
        self.single.get(key).map(String::as_str).unwrap_or("")
    }

    fn num<T: std::str::FromStr + Default>(&self, key: &str) -> T {
        self.str(key).trim().parse().unwrap_or_default()
    }

    fn list(&self, key: &str) -> Vec<&str> {
        self.multi
            .get(key)
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    fn csv(&self, key: &str) -> Vec<&str> {
        self.str(key)
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }
}

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

fn parse_form(body: &[u8]) -> Form {
    let text = String::from_utf8_lossy(body);
    let mut form = Form::default();
    for pair in text.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let key = url_decode(k);
        let value = url_decode(v);
        if let Some(base) = key.strip_suffix("[]") {
            form.multi.entry(base.to_string()).or_default().push(value);
        } else {
            form.single.insert(key, value);
        }
    }
    form
}

fn xml_response(content: &str) -> Response {
    let body = format!("{XML_DECL}<ServerResponse>{content}</ServerResponse>");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn server_response_fail() -> Response {
    xml_response("<RESULT>False</RESULT>")
}

fn result_response(ok: bool) -> Response {
    xml_response(if ok {
        "<RESULT>True</RESULT>"
    } else {
        "<RESULT>False</RESULT>"
    })
}

fn tag(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, "<{name}>{}</{name}>", escape(value));
}

fn folder_xml(name: &str, f: &InventoryFolderRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "ParentID", &f.parent_folder_id);
    tag(&mut s, "Type", &f.folder_type.to_string());
    tag(&mut s, "Version", &f.version.to_string());
    tag(&mut s, "Name", &f.folder_name);
    tag(&mut s, "Owner", &f.agent_id);
    tag(&mut s, "ID", &f.folder_id);
    let _ = write!(s, "</{name}>");
    s
}

fn item_xml(name: &str, i: &InventoryItemRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "AssetID", &i.asset_id);
    tag(&mut s, "AssetType", &i.asset_type.to_string());
    tag(
        &mut s,
        "BasePermissions",
        &(i.base_permissions as u32).to_string(),
    );
    tag(&mut s, "CreationDate", &i.creation_date.to_string());
    tag(&mut s, "CreatorId", &i.creator_id);
    tag(&mut s, "CreatorData", "");
    tag(
        &mut s,
        "CurrentPermissions",
        &(i.current_permissions as u32).to_string(),
    );
    tag(&mut s, "Description", &i.inventory_description);
    tag(
        &mut s,
        "EveryOnePermissions",
        &(i.everyone_permissions as u32).to_string(),
    );
    tag(&mut s, "Flags", &(i.flags as u32).to_string());
    tag(&mut s, "Folder", &i.parent_folder_id);
    tag(&mut s, "GroupID", &i.group_id);
    tag(
        &mut s,
        "GroupOwned",
        if i.group_owned != 0 { "True" } else { "False" },
    );
    tag(
        &mut s,
        "GroupPermissions",
        &(i.group_permissions as u32).to_string(),
    );
    tag(&mut s, "ID", &i.inventory_id);
    tag(&mut s, "InvType", &i.inv_type.to_string());
    tag(&mut s, "Name", &i.inventory_name);
    tag(
        &mut s,
        "NextPermissions",
        &(i.next_permissions as u32).to_string(),
    );
    tag(&mut s, "Owner", &i.avatar_id);
    tag(&mut s, "SalePrice", &i.sale_price.to_string());
    tag(&mut s, "SaleType", &i.sale_type.to_string());
    let _ = write!(s, "</{name}>");
    s
}

fn folders_list(name: &str, folders: &[InventoryFolderRow]) -> String {
    let mut s = format!("<{name} type=\"List\">");
    for (n, f) in folders.iter().enumerate() {
        s.push_str(&folder_xml(&format!("folder_{n}"), f));
    }
    let _ = write!(s, "</{name}>");
    s
}

fn items_list(name: &str, items: &[InventoryItemRow]) -> String {
    let mut s = format!("<{name} type=\"List\">");
    for (n, i) in items.iter().enumerate() {
        s.push_str(&item_xml(&format!("item_{n}"), i));
    }
    let _ = write!(s, "</{name}>");
    s
}

fn item_from_form(f: &Form) -> InventoryItemRow {
    let perm = |k: &str| f.num::<u32>(k) as i64;
    InventoryItemRow {
        asset_id: f.str("AssetID").to_string(),
        asset_type: f.num("AssetType"),
        inventory_name: f.str("Name").to_string(),
        inventory_description: f.str("Description").to_string(),
        next_permissions: perm("NextPermissions"),
        current_permissions: perm("CurrentPermissions"),
        inv_type: f.num("InvType"),
        creator_id: f.str("CreatorId").to_string(),
        base_permissions: perm("BasePermissions"),
        everyone_permissions: perm("EveryOnePermissions"),
        sale_price: f.num("SalePrice"),
        sale_type: f.num("SaleType"),
        creation_date: f.num("CreationDate"),
        group_id: f.str("GroupID").to_string(),
        group_owned: i32::from(f.str("GroupOwned").eq_ignore_ascii_case("true")),
        flags: perm("Flags"),
        inventory_id: f.str("ID").to_string(),
        avatar_id: f.str("Owner").to_string(),
        parent_folder_id: f.str("Folder").to_string(),
        group_permissions: perm("GroupPermissions"),
    }
}

fn folder_from_form(f: &Form) -> InventoryFolderRow {
    InventoryFolderRow {
        folder_name: f.str("Name").to_string(),
        folder_type: f.num("Type"),
        version: f.num("Version"),
        folder_id: f.str("ID").to_string(),
        agent_id: f.str("Owner").to_string(),
        parent_folder_id: f.str("ParentID").to_string(),
    }
}

fn db_err<T>(method: &str, r: Result<T, welkin_data::DataError>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::error!(method, error = %e, "xinventory database error");
            None
        }
    }
}

async fn handle_xinventory(State(state): State<InventoryServiceState>, body: Bytes) -> Response {
    let form = parse_form(&body);
    let method = form.get("METHOD").map(String::as_str).unwrap_or("");
    let store = &state.store;
    tracing::debug!(method, "xinventory request");

    match method {
        "GETROOTFOLDER" => {
            let r = db_err(
                method,
                store.get_user_root_folder(form.str("PRINCIPAL")).await,
            );
            single_folder(r)
        }
        "GETFOLDERFORTYPE" => {
            let r = db_err(
                method,
                store
                    .get_folder_for_type(form.str("PRINCIPAL"), form.num("TYPE"))
                    .await,
            );
            single_folder(r)
        }
        "GETFOLDER" => single_folder(db_err(method, store.get_folder(form.str("ID")).await)),
        "GETINVENTORYSKELETON" => {
            let Some(folders) = db_err(method, store.get_user_folders(form.str("PRINCIPAL")).await)
            else {
                return xml_response("");
            };
            xml_response(&folders_list("FOLDERS", &folders))
        }
        "GETFOLDERCONTENT" => {
            let Some(c) = folder_content(store, form.str("FOLDER")).await else {
                return xml_response("");
            };
            xml_response(&c)
        }
        "GETFOLDERITEMS" => {
            let Some(items) = db_err(method, store.get_items_in_folder(form.str("FOLDER")).await)
            else {
                return xml_response("");
            };
            xml_response(&items_list("ITEMS", &items))
        }
        "GETITEM" => match db_err(method, store.get_item(form.str("ID")).await).flatten() {
            Some(i) => xml_response(&item_xml("item", &i)),
            None => xml_response(""),
        },
        "GETMULTIPLEITEMS" => {
            let ids = form.csv("ITEMS");
            let Some(items) = db_err(method, store.get_multiple_items(&ids).await) else {
                return xml_response("");
            };
            let mut s = String::new();
            for (n, i) in items.iter().enumerate() {
                match i {
                    Some(i) => s.push_str(&item_xml(&format!("item_{n}"), i)),
                    None => {
                        let _ = write!(s, "<item_{n}>NULL</item_{n}>");
                    }
                }
            }
            xml_response(&s)
        }
        "GETACTIVEGESTURES" => {
            let Some(items) = db_err(
                method,
                store.get_active_gestures(form.str("PRINCIPAL")).await,
            ) else {
                return xml_response("");
            };
            xml_response(&items_list("ITEMS", &items))
        }
        "GETASSETPERMISSIONS" => {
            let r = store
                .get_asset_permissions(form.str("PRINCIPAL"), form.str("ASSET"))
                .await;
            let Some(p) = db_err(method, r) else {
                return xml_response("");
            };
            xml_response(&format!("<RESULT>{p}</RESULT>"))
        }
        "GETMULTIPLEFOLDERSCONTENT" => {
            let owner = form.str("PRINCIPAL");
            let mut s = String::new();
            for fid in form.csv("FOLDERS") {
                let Some(folder) = db_err(method, store.get_folder(fid).await).flatten() else {
                    continue;
                };
                let Some(inner) = folder_content_inner(store, &folder).await else {
                    continue;
                };
                let _ = write!(s, "<F_{} type=\"List\">", escape(fid));
                tag(&mut s, "FID", fid);
                tag(&mut s, "VERSION", &folder.version.to_string());
                tag(&mut s, "OWNER", owner);
                s.push_str(&inner);
                let _ = write!(s, "</F_{}>", escape(fid));
            }
            xml_response(&s)
        }
        "ADDITEM" | "UPDATEITEM" => {
            let r = store.store_item(&item_from_form(&form)).await;
            result_response(db_err(method, r).is_some())
        }
        "ADDFOLDER" | "UPDATEFOLDER" => {
            let r = store.store_folder(&folder_from_form(&form)).await;
            result_response(db_err(method, r).is_some())
        }
        "MOVEFOLDER" => {
            let r = store
                .move_folder(form.str("ID"), form.str("ParentID"))
                .await;
            result_response(db_err(method, r).unwrap_or(false))
        }
        "DELETEFOLDERS" => {
            let ids = form.list("FOLDERS");
            result_response(db_err(method, store.delete_folders(&ids).await).is_some())
        }
        "PURGEFOLDER" => {
            result_response(db_err(method, store.purge_folder(form.str("ID")).await).is_some())
        }
        "MOVEITEMS" => {
            let ids = form.list("IDLIST");
            let dests = form.list("DESTLIST");
            if ids.len() != dests.len() {
                return result_response(false);
            }
            result_response(db_err(method, store.move_items(&ids, &dests).await).is_some())
        }
        "DELETEITEMS" => {
            let ids = form.list("ITEMS");
            result_response(db_err(method, store.delete_items(&ids).await).is_some())
        }
        "CREATEUSERINVENTORY" => {
            let r = store.create_user_inventory(form.str("PRINCIPAL")).await;
            result_response(db_err(method, r).is_some())
        }
        other => {
            tracing::warn!(method = other, "unknown xinventory method");
            server_response_fail()
        }
    }
}

fn single_folder(r: Option<Option<InventoryFolderRow>>) -> Response {
    match r.flatten() {
        Some(f) => xml_response(&folder_xml("folder", &f)),
        None => xml_response(""),
    }
}

async fn folder_content_inner(
    store: &InventoryStore,
    folder: &InventoryFolderRow,
) -> Option<String> {
    let folders = db_err(
        "content",
        store.get_folders_in_folder(&folder.folder_id).await,
    )?;
    let items = db_err(
        "content",
        store.get_items_in_folder(&folder.folder_id).await,
    )?;
    let mut s = folders_list("FOLDERS", &folders);
    s.push_str(&items_list("ITEMS", &items));
    Some(s)
}

async fn folder_content(store: &InventoryStore, folder_id: &str) -> Option<String> {
    let folder = db_err("content", store.get_folder(folder_id).await)??;
    let inner = folder_content_inner(store, &folder).await?;
    let mut s = String::new();
    tag(&mut s, "FID", folder_id);
    tag(&mut s, "VERSION", &folder.version.to_string());
    s.push_str(&inner);
    Some(s)
}
