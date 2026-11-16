// SPDX-License-Identifier: LGPL-3.0-or-later

pub use crate::environment::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{MethodRouter, get, post},
};
use base64::Engine as _;
use quick_xml::Reader as XmlReader;
use quick_xml::events::Event;
use tokio::sync::RwLock;
use uuid::Uuid;
use welkin_llsd::{Llsd, Map, from_xml, to_xml_string};

use crate::event_queue::{EventQueue, EventQueueResponse};

#[derive(Debug, serde::Deserialize)]
pub struct TextureQuery {
    pub texture_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MeshQuery {
    pub mesh_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AssetQuery {
    pub asset_id: Option<String>,
    pub texture_id: Option<String>,
    pub mesh_id: Option<String>,
    pub bodypart_id: Option<String>,
    pub clothing_id: Option<String>,
    pub sound_id: Option<String>,
    pub animation_id: Option<String>,
    pub gesture_id: Option<String>,
    pub notecard_id: Option<String>,
    pub script_id: Option<String>,
    pub object_id: Option<String>,
}

impl AssetQuery {
    pub fn effective_id(&self) -> Option<&str> {
        self.asset_id.as_deref()
            .or(self.texture_id.as_deref())
            .or(self.mesh_id.as_deref())
            .or(self.bodypart_id.as_deref())
            .or(self.clothing_id.as_deref())
            .or(self.sound_id.as_deref())
            .or(self.animation_id.as_deref())
            .or(self.gesture_id.as_deref())
            .or(self.notecard_id.as_deref())
            .or(self.script_id.as_deref())
            .or(self.object_id.as_deref())
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct DisplayNameQuery {
    #[serde(default)]
    pub ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ScriptUploadQuery {
    pub prim_id: Option<uuid::Uuid>,
}

fn llsd_response(llsd: Llsd) -> Response {
    let body = to_xml_string(&llsd);
    (
        StatusCode::OK,
        [("Content-Type", "application/llsd+xml")],
        body,
    )
        .into_response()
}

#[derive(Clone)]
pub struct AssetServiceState {
    pub client: reqwest::Client,
    pub asset_url: String,
}

#[derive(Clone)]
pub struct InventoryProxyState {
    pub client: reqwest::Client,
    pub inventory_url: String,
}

/// A compiled script ready to be loaded into the VM scheduler.
#[derive(Debug, Clone)]
pub struct ScriptLoadRequest {
    pub script_id: uuid::Uuid,
    pub prim_id: uuid::Uuid,
    pub luau_source: String,
}

#[derive(Clone)]
pub struct ScriptLoadState {
    pub sender: std::sync::mpsc::Sender<ScriptLoadRequest>,
}

pub type PendingScriptUploads = Arc<RwLock<HashMap<String, ScriptUploadPending>>>;

#[derive(Debug, Clone)]
pub struct ScriptUploadPending {
    pub prim_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct ScriptUploadState {
    pub script_load: ScriptLoadState,
    pub caps_url: String,
    pub pending: PendingScriptUploads,
}

pub type PendingBakeUploads = Arc<RwLock<HashMap<String, ()>>>;

#[derive(Clone)]
pub struct BakeUploadState {
    pub client: reqwest::Client,
    pub asset_url: String,
    pub caps_url: String,
    pub pending: PendingBakeUploads,
}

async fn fetch_asset(state: &AssetServiceState, asset_id: &str) -> Result<Vec<u8>, StatusCode> {
    let url = format!("{}/assets/{asset_id}/data", state.asset_url);
    let resp = state.client.get(&url).send().await.map_err(|e| {
        tracing::warn!(error = %e, "asset service request failed");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(StatusCode::NOT_FOUND);
    }
    let resp = resp.error_for_status().map_err(|e| {
        tracing::warn!(error = %e, "asset service error");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| {
        tracing::warn!(error = %e, "failed to read asset bytes");
        StatusCode::SERVICE_UNAVAILABLE
    })
}

pub async fn get_texture(
    State(state): State<AssetServiceState>,
    Query(q): Query<TextureQuery>,
) -> Response {
    let Some(texture_id) = q.texture_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing texture_id").into_response();
    };
    tracing::debug!(texture_id, "GetTexture");
    match fetch_asset(&state, texture_id).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "image/x-j2c")], data).into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn get_mesh(
    State(state): State<AssetServiceState>,
    Query(q): Query<MeshQuery>,
) -> Response {
    let Some(mesh_id) = q.mesh_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing mesh_id").into_response();
    };
    tracing::debug!(mesh_id, "GetMesh");
    match fetch_asset(&state, mesh_id).await {
        Ok(data) => (
            StatusCode::OK,
            [("Content-Type", "application/vnd.ll.mesh")],
            data,
        )
            .into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn get_mesh2(
    State(state): State<AssetServiceState>,
    Query(q): Query<MeshQuery>,
) -> Response {
    let Some(mesh_id) = q.mesh_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing mesh_id").into_response();
    };
    tracing::debug!(mesh_id, "GetMesh2");
    match fetch_asset(&state, mesh_id).await {
        Ok(data) => (
            StatusCode::OK,
            [("Content-Type", "application/vnd.ll.mesh")],
            data,
        )
            .into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn simulator_features() -> Response {
    let mut map = Map::new();
    map.insert("MeshRezEnabled".into(), Llsd::Boolean(true));
    map.insert("MeshUploadEnabled".into(), Llsd::Boolean(true));
    map.insert("PhysicsShapeTypes".into(), Llsd::Boolean(true));
    map.insert("MaxMaterialsPerTransaction".into(), Llsd::Integer(50));
    map.insert("AnimatedObjects".into(), Llsd::Boolean(true));
    map.insert("MaxAgentGroupsBasic".into(), Llsd::Integer(42));
    map.insert("MaxAgentGroupsPremium".into(), Llsd::Integer(60));
    let mut extras = Map::new();
    extras.insert(
        "map-server-url".into(),
        Llsd::String("http://127.0.0.1:8002/map/".into()),
    );
    extras.insert(
        "currency-base-uri".into(),
        Llsd::String("http://127.0.0.1:8002/".into()),
    );
    map.insert("OpenSimExtras".into(), Llsd::Map(extras));

    llsd_response(Llsd::Map(map))
}

pub async fn region_info() -> Response {
    let mut map = Map::new();
    map.insert("region_flags".into(), Llsd::Integer(0));
    map.insert("sim_access".into(), Llsd::Integer(13));
    llsd_response(Llsd::Map(map))
}

pub async fn agent_preferences() -> Response {
    llsd_response(agent_preferences_llsd())
}

pub async fn agent_preferences_post(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => tracing::debug!(?llsd, "AgentPreferences POST"),
        Err(e) => tracing::debug!(?e, "AgentPreferences POST: invalid LLSD body"),
    }
    llsd_response(agent_preferences_llsd())
}

fn agent_preferences_llsd() -> Llsd {
    let mut access_prefs = Map::new();
    access_prefs.insert("max".into(), Llsd::String("A".into()));

    let mut map = Map::new();
    map.insert("hover_height".into(), Llsd::Real(0.0));
    map.insert("language".into(), Llsd::String("en".into()));
    map.insert("access_prefs".into(), Llsd::Map(access_prefs));
    Llsd::Map(map)
}

pub async fn map_layer(_body: Bytes) -> Response {
    let mut agent_data = Map::new();
    agent_data.insert("Flags".into(), Llsd::Integer(0));
    let mut map = Map::new();
    map.insert("AgentData".into(), Llsd::Map(agent_data));
    map.insert("MapBlocks".into(), Llsd::Array(Vec::new()));
    llsd_response(Llsd::Map(map))
}

pub async fn new_file_agent_inventory(body: Bytes) -> Response {
    log_upload_body("NewFileAgentInventory", &body);
    let new_asset = Uuid::new_v4().to_string();
    let new_item = Uuid::new_v4().to_string();
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(new_asset));
    map.insert("new_inventory_item".into(), Llsd::String(new_item));
    llsd_response(Llsd::Map(map))
}

pub async fn update_notecard_agent_inventory(body: Bytes) -> Response {
    log_upload_body("UpdateNotecardAgentInventory", &body);
    let new_asset = Uuid::new_v4().to_string();
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(new_asset));
    llsd_response(Llsd::Map(map))
}

pub async fn update_script_agent(
    State(state): State<ScriptUploadState>,
    Query(params): Query<ScriptUploadQuery>,
    body: Bytes,
) -> Response {
    log_upload_body("UpdateScriptAgent", &body);

    let parsed = from_xml(&body).ok();
    if parsed.is_some() {
        if let Some(Llsd::Map(ref m)) = parsed {
            if let Some(item_id) = m.get("item_id") {
                tracing::debug!(?item_id, "UpdateScriptAgent: item_id");
            }
        }
        let token = Uuid::new_v4().to_string();
        state.pending.write().await.insert(token.clone(), ScriptUploadPending {
            prim_id: params.prim_id,
        });
        let uploader_url = format!("{}/script-upload/{}", state.caps_url, token);
        tracing::debug!(%uploader_url, "UpdateScriptAgent: created uploader");
        let mut root = Map::new();
        root.insert("uploader".into(), Llsd::String(uploader_url));
        root.insert("state".into(), Llsd::String("upload".into()));
        return llsd_response(Llsd::Map(root));
    }

    let source = String::from_utf8_lossy(&body);
    let script_load = &state.script_load;
    match welkin_script_compiler::compile_lsl(&source) {
        Ok(luau) => {
            let script_id = Uuid::new_v4();
            let new_asset = script_id.to_string();
            tracing::info!(
                asset = %new_asset,
                luau_len = luau.len(),
                "UpdateScriptAgent: compiled successfully, loading into VM"
            );
            let _ = script_load.sender.send(ScriptLoadRequest {
                script_id,
                prim_id: params.prim_id.unwrap_or(Uuid::nil()),
                luau_source: luau,
            });
            let mut map = Map::new();
            map.insert("state".into(), Llsd::String("complete".into()));
            map.insert("new_asset".into(), Llsd::String(new_asset));
            llsd_response(Llsd::Map(map))
        }
        Err(e) => {
            tracing::warn!(error = %e, "UpdateScriptAgent: compilation failed");
            let new_asset = Uuid::new_v4().to_string();
            let mut map = Map::new();
            map.insert("state".into(), Llsd::String("complete".into()));
            map.insert("new_asset".into(), Llsd::String(new_asset));
            map.insert("errors".into(), Llsd::Array(vec![
                Llsd::String(e.to_string()),
            ]));
            llsd_response(Llsd::Map(map))
        }
    }
}

pub async fn script_upload_handler(
    State(state): State<ScriptUploadState>,
    axum::extract::Path(token): axum::extract::Path<String>,
    body: Bytes,
) -> Response {
    let pending = {
        let mut pending = state.pending.write().await;
        pending.remove(&token)
    };
    let info = match pending {
        Some(p) => p,
        None => {
            tracing::warn!(%token, "script upload: unknown or expired token");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    let source = String::from_utf8_lossy(&body);
    match welkin_script_compiler::compile_lsl(&source) {
        Ok(luau) => {
            let script_id = Uuid::new_v4();
            let new_asset = script_id.to_string();
            let new_inventory_item = Uuid::new_v4().to_string();
            tracing::info!(
                asset = %new_asset,
                luau_len = luau.len(),
                "script upload: compiled successfully, loading into VM"
            );
            let _ = state.script_load.sender.send(ScriptLoadRequest {
                script_id,
                prim_id: info.prim_id.unwrap_or(Uuid::nil()),
                luau_source: luau,
            });
            let mut map = Map::new();
            map.insert("state".into(), Llsd::String("complete".into()));
            map.insert("new_asset".into(), Llsd::String(new_asset));
            map.insert("new_inventory_item".into(), Llsd::String(new_inventory_item));
            llsd_response(Llsd::Map(map))
        }
        Err(e) => {
            tracing::warn!(error = %e, "script upload: compilation failed");
            let new_asset = Uuid::new_v4().to_string();
            let mut map = Map::new();
            map.insert("state".into(), Llsd::String("complete".into()));
            map.insert("new_asset".into(), Llsd::String(new_asset));
            map.insert("errors".into(), Llsd::Array(vec![
                Llsd::String(e.to_string()),
            ]));
            llsd_response(Llsd::Map(map))
        }
    }
}

pub fn script_upload_route(script_state: ScriptUploadState) -> axum::Router {
    axum::Router::new().route(
        "/script-upload/{token}",
        post(script_upload_handler).with_state(script_state),
    )
}

#[derive(Debug, Default)]
struct XFolder {
    parent_id: Uuid,
    folder_type: i32,
    version: i32,
    name: String,
    owner: Uuid,
    id: Uuid,
}

#[derive(Debug, Default)]
struct XItem {
    asset_id: Uuid,
    asset_type: i32,
    base_permissions: i32,
    creation_date: i32,
    creator_id: Uuid,
    current_permissions: i32,
    description: String,
    everyone_permissions: i32,
    flags: i32,
    folder: Uuid,
    group_id: Uuid,
    group_owned: bool,
    group_permissions: i32,
    id: Uuid,
    inv_type: i32,
    name: String,
    next_permissions: i32,
    owner: Uuid,
    sale_price: i32,
    sale_type: i32,
}

#[derive(Debug, Default)]
struct XInventoryResponse {
    version: i32,
    folders: Vec<XFolder>,
    items: Vec<XItem>,
}

fn parse_uuid(s: &str) -> Uuid {
    let id_part = s.split(';').next().unwrap_or(s);
    Uuid::parse_str(id_part.trim()).unwrap_or(Uuid::nil())
}

fn parse_perm(s: &str) -> i32 {
    s.trim().parse::<u32>().unwrap_or(0) as i32
}

fn parse_xinventory_xml(xml_bytes: &[u8]) -> XInventoryResponse {
    let s = std::str::from_utf8(xml_bytes).unwrap_or("");
    let mut reader = XmlReader::from_str(s);
    let mut resp = XInventoryResponse::default();
    let mut cur_folder: Option<XFolder> = None;
    let mut cur_item: Option<XItem> = None;
    let mut cur_field: Option<String> = None;
    let mut text_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let tag = std::str::from_utf8(&name_bytes).unwrap_or("").to_string();
                let is_list = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .any(|a| a.key.as_ref() == b"type" && a.value.as_ref() == b"List");

                if tag.starts_with("folder_") && is_list {
                    cur_folder = Some(XFolder::default());
                    cur_field = None;
                } else if tag.starts_with("item_") && is_list {
                    cur_item = Some(XItem::default());
                    cur_field = None;
                } else if cur_folder.is_some() || cur_item.is_some() || tag == "VERSION" {
                    cur_field = Some(tag.to_string());
                    text_buf.clear();
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(unescaped) = t.unescape() {
                    text_buf.push_str(&unescaped);
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let tag = std::str::from_utf8(&name_bytes).unwrap_or("").to_string();

                if tag.starts_with("folder_") && cur_folder.is_some() {
                    if let Some(f) = cur_folder.take() {
                        resp.folders.push(f);
                    }
                    cur_field = None;
                } else if tag.starts_with("item_") && cur_item.is_some() {
                    if let Some(it) = cur_item.take() {
                        resp.items.push(it);
                    }
                    cur_field = None;
                } else if let Some(field) = cur_field.take() {
                    let val = text_buf.trim();

                    if let Some(f) = cur_folder.as_mut() {
                        match field.as_str() {
                            "ParentID" => f.parent_id = parse_uuid(val),
                            "Type" => f.folder_type = val.parse().unwrap_or(0),
                            "Version" => f.version = val.parse().unwrap_or(0),
                            "Name" => f.name = val.to_string(),
                            "Owner" => f.owner = parse_uuid(val),
                            "ID" => f.id = parse_uuid(val),
                            _ => {}
                        }
                    } else if let Some(it) = cur_item.as_mut() {
                        match field.as_str() {
                            "AssetID" => it.asset_id = parse_uuid(val),
                            "AssetType" => it.asset_type = val.parse().unwrap_or(0),
                            "BasePermissions" => it.base_permissions = parse_perm(val),
                            "CreationDate" => it.creation_date = val.parse().unwrap_or(0),
                            "CreatorId" => it.creator_id = parse_uuid(val),
                            "CurrentPermissions" => it.current_permissions = parse_perm(val),
                            "Description" => it.description = val.to_string(),
                            "EveryOnePermissions" => it.everyone_permissions = parse_perm(val),
                            "Flags" => it.flags = parse_perm(val),
                            "Folder" => it.folder = parse_uuid(val),
                            "GroupID" => it.group_id = parse_uuid(val),
                            "GroupOwned" => it.group_owned = val.eq_ignore_ascii_case("true"),
                            "GroupPermissions" => it.group_permissions = parse_perm(val),
                            "ID" => it.id = parse_uuid(val),
                            "InvType" => it.inv_type = val.parse().unwrap_or(0),
                            "Name" => it.name = val.to_string(),
                            "NextPermissions" => it.next_permissions = parse_perm(val),
                            "Owner" => it.owner = parse_uuid(val),
                            "SalePrice" => it.sale_price = val.parse().unwrap_or(0),
                            "SaleType" => it.sale_type = val.parse().unwrap_or(0),
                            _ => {}
                        }
                    } else if field == "VERSION" {
                        resp.version = val.parse().unwrap_or(0);
                    }
                    text_buf.clear();
                }
            }
            Err(_) => break,
            _ => {}
        }
    }
    resp
}

fn item_to_llsd(it: &XItem) -> Llsd {
    let mut perms = Map::new();
    perms.insert("creator_id".into(), Llsd::Uuid(it.creator_id));
    perms.insert("owner_id".into(), Llsd::Uuid(it.owner));
    perms.insert("group_id".into(), Llsd::Uuid(it.group_id));
    perms.insert("base_mask".into(), Llsd::Integer(it.base_permissions));
    perms.insert("owner_mask".into(), Llsd::Integer(it.current_permissions));
    perms.insert("group_mask".into(), Llsd::Integer(it.group_permissions));
    perms.insert(
        "everyone_mask".into(),
        Llsd::Integer(it.everyone_permissions),
    );
    perms.insert("next_owner_mask".into(), Llsd::Integer(it.next_permissions));
    perms.insert("is_owner_group".into(), Llsd::Boolean(it.group_owned));

    let mut sale = Map::new();
    sale.insert("sale_price".into(), Llsd::Integer(it.sale_price));
    sale.insert("sale_type".into(), Llsd::Integer(it.sale_type));

    let mut m = Map::new();
    m.insert("item_id".into(), Llsd::Uuid(it.id));
    m.insert("parent_id".into(), Llsd::Uuid(it.folder));
    m.insert("name".into(), Llsd::String(it.name.clone()));
    m.insert("type".into(), Llsd::Integer(it.asset_type));
    m.insert("inv_type".into(), Llsd::Integer(it.inv_type));
    m.insert("permissions".into(), Llsd::Map(perms));
    m.insert("asset_id".into(), Llsd::Uuid(it.asset_id));
    m.insert("description".into(), Llsd::String(it.description.clone()));
    m.insert("sale_info".into(), Llsd::Map(sale));
    m.insert("flags".into(), Llsd::Integer(it.flags));
    m.insert("created_at".into(), Llsd::Integer(it.creation_date));
    Llsd::Map(m)
}

fn folder_to_category(f: &XFolder) -> Llsd {
    let mut m = Map::new();
    m.insert("folder_id".into(), Llsd::Uuid(f.id));
    m.insert("parent_id".into(), Llsd::Uuid(f.parent_id));
    m.insert("name".into(), Llsd::String(f.name.clone()));
    m.insert("type".into(), Llsd::Integer(f.folder_type));
    m.insert("preferred_type".into(), Llsd::Integer(f.folder_type));
    Llsd::Map(m)
}

pub async fn fetch_inventory_descendents2(
    State(inv): State<InventoryProxyState>,
    body: Bytes,
) -> Response {
    let parsed = from_xml(&body).unwrap_or(Llsd::Undef);
    let folders = match parsed.get("folders") {
        Llsd::Array(arr) => arr.as_slice(),
        _ => &[],
    };

    let mut result_folders = Vec::new();
    for folder in folders {
        let folder_id = folder.get("folder_id").as_uuid();
        let owner_id = folder.get("owner_id").as_uuid();
        tracing::debug!(%folder_id, %owner_id, "FetchInventoryDescendents2 request");

        let url = format!("{}/xinventory", inv.inventory_url);
        let form = format!("METHOD=GETFOLDERCONTENT&FOLDER={folder_id}");

        let mut items_out = Vec::new();
        let mut cats_out = Vec::new();
        let mut version = 1i32;

        match inv
            .client
            .post(&url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form)
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(bytes) = resp.bytes().await {
                    let xinv = parse_xinventory_xml(&bytes);
                    version = xinv.version;
                    for it in &xinv.items {
                        items_out.push(item_to_llsd(it));
                    }
                    for fc in &xinv.folders {
                        cats_out.push(folder_to_category(fc));
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, %folder_id, "xinventory GETFOLDERCONTENT failed");
            }
        }

        let descendents = (items_out.len() + cats_out.len()) as i32;
        let mut entry = Map::new();
        entry.insert("agent_id".into(), Llsd::Uuid(owner_id));
        entry.insert("descendents".into(), Llsd::Integer(descendents));
        entry.insert("folder_id".into(), Llsd::Uuid(folder_id));
        entry.insert("items".into(), Llsd::Array(items_out));
        entry.insert("categories".into(), Llsd::Array(cats_out));
        entry.insert("owner_id".into(), Llsd::Uuid(owner_id));
        entry.insert("version".into(), Llsd::Integer(version));
        result_folders.push(Llsd::Map(entry));
    }

    let mut root = Map::new();
    root.insert("folders".into(), Llsd::Array(result_folders));
    llsd_response(Llsd::Map(root))
}

pub async fn fetch_inventory2(State(inv): State<InventoryProxyState>, body: Bytes) -> Response {
    let parsed = from_xml(&body).unwrap_or(Llsd::Undef);
    let agent_id = parsed.get("agent_id").as_uuid();

    let items_req = match parsed.get("items") {
        Llsd::Array(arr) => arr.as_slice(),
        _ => &[],
    };
    let ids: Vec<String> = items_req
        .iter()
        .map(|r| r.get("item_id").as_uuid().to_string())
        .collect();

    tracing::debug!(agent = %agent_id, count = ids.len(), "FetchInventory2 request");

    let mut items_out = Vec::new();
    if !ids.is_empty() {
        let url = format!("{}/xinventory", inv.inventory_url);
        let form = format!("METHOD=GETMULTIPLEITEMS&ITEMS={}", ids.join(","));

        match inv
            .client
            .post(&url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form)
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(bytes) = resp.bytes().await {
                    let xinv = parse_xinventory_xml(&bytes);
                    for it in &xinv.items {
                        items_out.push(item_to_llsd(it));
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, %agent_id, "xinventory GETMULTIPLEITEMS failed");
            }
        }
    }

    let mut root = Map::new();
    root.insert("agent_id".into(), Llsd::Uuid(agent_id));
    root.insert("items".into(), Llsd::Array(items_out));
    llsd_response(Llsd::Map(root))
}

pub async fn upload_baked_texture(State(bake): State<BakeUploadState>, _body: Bytes) -> Response {
    let token = Uuid::new_v4().to_string();
    bake.pending.write().await.insert(token.clone(), ());
    let uploader_url = format!("{}/bake-upload/{}", bake.caps_url, token);
    tracing::debug!(%uploader_url, "UploadBakedTexture: created uploader");

    let mut root = Map::new();
    root.insert("uploader".into(), Llsd::String(uploader_url));
    root.insert("state".into(), Llsd::String("upload".into()));
    llsd_response(Llsd::Map(root))
}

pub async fn bake_upload_handler(
    State(bake): State<BakeUploadState>,
    axum::extract::Path(token): axum::extract::Path<String>,
    body: Bytes,
) -> Response {
    {
        let mut pending = bake.pending.write().await;
        if pending.remove(&token).is_none() {
            tracing::warn!(%token, "bake upload: unknown or expired token");
            return StatusCode::NOT_FOUND.into_response();
        }
    }

    let new_asset_id = Uuid::new_v4();
    let b64_data = base64::engine::general_purpose::STANDARD.encode(&body);
    let asset_xml = format!(
        "<?xml version=\"1.0\"?>\n\
        <AssetBase xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
        xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n\
        <Data>{b64_data}</Data>\n\
        <FullID><Guid>{new_asset_id}</Guid></FullID>\n\
        <ID>{new_asset_id}</ID>\n\
        <Name>Baked Texture</Name>\n\
        <Description>Baked Texture</Description>\n\
        <Type>0</Type>\n\
        <Local>false</Local>\n\
        <Temporary>true</Temporary>\n\
        <CreatorID>00000000-0000-0000-0000-000000000000</CreatorID>\n\
        <Flags>Normal</Flags>\n\
        </AssetBase>"
    );

    let url = format!("{}/assets", bake.asset_url);
    match bake
        .client
        .post(&url)
        .header("Content-Type", "text/xml")
        .body(asset_xml)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(%new_asset_id, bytes = body.len(), "stored baked texture");
        }
        Ok(resp) => {
            tracing::warn!(status = %resp.status(), %new_asset_id, "asset service rejected baked texture");
        }
        Err(e) => {
            tracing::warn!(error = %e, %new_asset_id, "failed to store baked texture");
        }
    }

    let mut root = Map::new();
    root.insert("new_asset".into(), Llsd::Uuid(new_asset_id));
    root.insert("new_inventory_item".into(), Llsd::Uuid(Uuid::nil()));
    root.insert("state".into(), Llsd::String("complete".into()));
    llsd_response(Llsd::Map(root))
}

pub async fn viewer_asset(
    State(state): State<AssetServiceState>,
    Query(q): Query<AssetQuery>,
) -> Response {
    let Some(asset_id) = q.effective_id() else {
        return (StatusCode::BAD_REQUEST, "missing asset_id").into_response();
    };
    tracing::debug!(asset_id, "ViewerAsset");
    match fetch_asset(&state, asset_id).await {
        Ok(data) => (
            StatusCode::OK,
            [("Content-Type", "application/octet-stream")],
            data,
        )
            .into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn fetch_lib2(body: Bytes) -> Response {
    let parsed = from_xml(&body).unwrap_or(Llsd::Undef);
    let folders = match parsed.get("folders") {
        Llsd::Array(arr) => arr.as_slice(),
        _ => &[],
    };
    let mut result_folders = Vec::new();
    for folder in folders {
        let folder_id = folder.get("folder_id").as_uuid();
        let owner_id = folder.get("owner_id").as_uuid();
        let mut entry = Map::new();
        entry.insert("agent_id".into(), Llsd::Uuid(owner_id));
        entry.insert("descendents".into(), Llsd::Integer(0));
        entry.insert("folder_id".into(), Llsd::Uuid(folder_id));
        entry.insert("items".into(), Llsd::Array(Vec::new()));
        entry.insert("categories".into(), Llsd::Array(Vec::new()));
        entry.insert("owner_id".into(), Llsd::Uuid(owner_id));
        entry.insert("version".into(), Llsd::Integer(1));
        result_folders.push(Llsd::Map(entry));
    }
    let mut root = Map::new();
    root.insert("folders".into(), Llsd::Array(result_folders));
    llsd_response(Llsd::Map(root))
}

pub async fn get_display_names(req: axum::http::Request<axum::body::Body>) -> Response {
    let query = req.uri().query().unwrap_or("");
    let ids: Vec<&str> = query
        .split('&')
        .filter_map(|p| p.strip_prefix("ids="))
        .collect();
    let mut agents = Vec::new();
    for id_str in &ids {
        if let Ok(id) = Uuid::parse_str(id_str) {
            let mut agent = Map::new();
            agent.insert("username".into(), Llsd::String("User".into()));
            agent.insert("display_name".into(), Llsd::String("User".into()));
            agent.insert("legacy_first_name".into(), Llsd::String("User".into()));
            agent.insert("legacy_last_name".into(), Llsd::String("Resident".into()));
            agent.insert("id".into(), Llsd::Uuid(id));
            agent.insert("is_display_name_default".into(), Llsd::Boolean(true));
            agents.push(Llsd::Map(agent));
        }
    }
    let mut root = Map::new();
    root.insert("agents".into(), Llsd::Array(agents));
    llsd_response(Llsd::Map(root))
}

pub async fn home_location(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => tracing::debug!(?llsd, "HomeLocation request"),
        Err(e) => tracing::debug!(?e, "HomeLocation: invalid LLSD body"),
    }
    let mut root = Map::new();
    root.insert("success".into(), Llsd::Boolean(true));
    llsd_response(Llsd::Map(root))
}

pub async fn create_inventory_category(
    State(inv): State<InventoryProxyState>,
    body: Bytes,
) -> Response {
    let parsed = from_xml(&body).unwrap_or(Llsd::Undef);
    let folder_id = parsed.get("folder_id").as_uuid();
    let parent_id = parsed.get("parent_id").as_uuid();
    let name = parsed.get("name").as_str().to_string();
    let folder_type = parsed.get("type").as_i32();

    if folder_id.is_nil() || name.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let url = format!("{}/xinventory", inv.inventory_url);
    let form = format!(
        "METHOD=ADDFOLDER&ID={folder_id}&ParentID={parent_id}&Name={}&Type={folder_type}&Version=1&Owner=00000000-0000-0000-0000-000000000000",
        urlencoding_simple(&name),
    );

    match inv
        .client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
    {
        Ok(_) => {
            tracing::info!(%folder_id, %parent_id, %name, "created inventory folder");
        }
        Err(e) => {
            tracing::warn!(error = %e, %folder_id, "failed to create inventory folder");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }

    let mut root = Map::new();
    root.insert("folder_id".into(), Llsd::Uuid(folder_id));
    root.insert("parent_id".into(), Llsd::Uuid(parent_id));
    root.insert("name".into(), Llsd::String(name));
    root.insert("type".into(), Llsd::Integer(folder_type));
    llsd_response(Llsd::Map(root))
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.bytes() {
        match c {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(c as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(c >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(c & 0xf) as usize]));
            }
        }
    }
    out
}

fn log_upload_body(cap_name: &str, body: &[u8]) {
    match from_xml(body) {
        Ok(llsd) => {
            let asset_type = llsd.get("asset_type");
            let name = llsd.get("name");
            tracing::debug!(%cap_name, ?asset_type, ?name, "upload request");
        }
        Err(e) => {
            tracing::debug!(%cap_name, ?e, "upload request with invalid LLSD body");
        }
    }
}

pub type EventQueueMap = Arc<RwLock<HashMap<Uuid, Arc<EventQueue>>>>;

/// Per-agent secret that is part of the agent's capability URLs (agent id -> secret).
pub type CapSecrets = Arc<RwLock<HashMap<Uuid, String>>>;

/// State of the per-agent event queue capability.
#[derive(Clone)]
pub struct AgentEventQueueState {
    pub queues: EventQueueMap,
    pub secrets: CapSecrets,
}

/// Split a capability token `<agent-uuid>-<secret>` into its parts.
pub fn split_agent_token(token: &str) -> Option<(Uuid, String)> {
    let token = token.trim_matches('/');
    if token.len() < 38 || !token.is_char_boundary(36) || token.as_bytes().get(36) != Some(&b'-') {
        return None;
    }
    let agent = Uuid::parse_str(&token[..36]).ok()?;
    let secret = token[37..].to_string();
    if secret.is_empty() {
        return None;
    }
    Some((agent, secret))
}

/// `EventQueueGet` for one agent: `/EventQueueGet/<agent-uuid>-<secret>`.
pub async fn event_queue_get_for_agent(
    State(state): State<AgentEventQueueState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Response {
    let Some((agent, secret)) = split_agent_token(&token) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let bound = state.secrets.read().await.get(&agent).cloned();
    if bound.as_deref() != Some(secret.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let eq = state.queues.read().await.get(&agent).cloned();
    let Some(eq) = eq else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match eq.poll(Duration::from_secs(25)).await {
        EventQueueResponse::Events { events, .. } => (
            StatusCode::OK,
            [("content-type", "application/llsd+xml")],
            events,
        )
            .into_response(),
        EventQueueResponse::Timeout => StatusCode::BAD_GATEWAY.into_response(),
    }
}

pub async fn event_queue_get(State(eq_map): State<EventQueueMap>, body: Bytes) -> Response {
    let agent_id = match from_xml(&body) {
        Ok(llsd) => {
            let id_str = llsd.get("agent-id").as_str().to_string();
            id_str.parse::<Uuid>().ok()
        }
        Err(_) => None,
    };

    let eq = match agent_id {
        Some(id) => {
            let map = eq_map.read().await;
            map.get(&id).cloned()
        }
        None => {
            let map = eq_map.read().await;
            if map.len() == 1 {
                map.values().next().cloned()
            } else {
                None
            }
        }
    };

    let eq = match eq {
        Some(eq) => eq,
        None => {
            tracing::debug!(?agent_id, "EventQueueGet: no queue for agent");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    match eq.poll(Duration::from_secs(25)).await {
        EventQueueResponse::Events { events, .. } => (
            StatusCode::OK,
            [("content-type", "application/llsd+xml")],
            events,
        )
            .into_response(),
        EventQueueResponse::Timeout => StatusCode::BAD_GATEWAY.into_response(),
    }
}

pub fn bake_upload_route(bake_state: BakeUploadState) -> axum::Router {
    axum::Router::new().route(
        "/bake-upload/{token}",
        post(bake_upload_handler).with_state(bake_state),
    )
}

pub async fn viewer_stats(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => {
            let channel = llsd.get("channel").as_str().to_string();
            let version = llsd.get("version").as_str().to_string();
            tracing::info!(%channel, %version, "ViewerStats received");
        }
        Err(e) => tracing::debug!(?e, "ViewerStats: could not parse body"),
    }
    llsd_response(Llsd::Map(Map::new()))
}

pub async fn update_agent_language(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => {
            let language = llsd.get("language").as_str().to_string();
            let is_public = matches!(llsd.get("language_is_public"), Llsd::Boolean(true));
            tracing::info!(%language, %is_public, "UpdateAgentLanguage");
        }
        Err(e) => tracing::debug!(?e, "UpdateAgentLanguage: could not parse body"),
    }
    llsd_response(Llsd::Map(Map::new()))
}

pub async fn update_agent_information(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => {
            tracing::info!(?llsd, "UpdateAgentInformation");
        }
        Err(e) => tracing::debug!(?e, "UpdateAgentInformation: could not parse body"),
    }
    llsd_response(Llsd::Map(Map::new()))
}

pub async fn parcel_voice_info() -> Response {
    let mut map = Map::new();
    map.insert("voice_enabled".into(), Llsd::Boolean(false));
    map.insert("error".into(), Llsd::String("voice service not configured".into()));
    llsd_response(Llsd::Map(map))
}

pub async fn provision_voice_account() -> Response {
    let mut map = Map::new();
    map.insert("voice_enabled".into(), Llsd::Boolean(false));
    map.insert("error".into(), Llsd::String("voice service not configured".into()));
    llsd_response(Llsd::Map(map))
}

pub async fn avatar_picker_search(body: Bytes) -> Response {
    if let Ok(llsd) = from_xml(&body) {
        tracing::debug!(?llsd, "AvatarPickerSearch");
    }
    let mut map = Map::new();
    map.insert("agents".into(), Llsd::Array(Vec::new()));
    llsd_response(Llsd::Map(map))
}

pub async fn mesh_upload_flag() -> Response {
    let mut map = Map::new();
    map.insert("mesh_upload_status".into(), Llsd::Integer(1));
    map.insert("MeshUploadEnabled".into(), Llsd::Boolean(true));
    llsd_response(Llsd::Map(map))
}

pub async fn server_release_notes() -> Response {
    let mut map = Map::new();
    map.insert("release_notes_url".into(), Llsd::String("about:blank".into()));
    llsd_response(Llsd::Map(map))
}

pub async fn viewer_metrics(body: Bytes) -> Response {
    if let Ok(llsd) = from_xml(&body) {
        tracing::debug!(?llsd, "ViewerMetrics");
    }
    llsd_response(Llsd::Map(Map::new()))
}

pub async fn sim_console_async(body: Bytes) -> Response {
    if let Ok(llsd) = from_xml(&body) {
        tracing::info!(?llsd, "SimConsoleAsync command");
    }
    llsd_response(Llsd::Map(Map::new()))
}

pub async fn chat_session_request(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => {
            let method = llsd.get("method").as_str().to_string();
            tracing::debug!(%method, "ChatSessionRequest");
        }
        Err(e) => tracing::debug!(?e, "ChatSessionRequest: could not parse body"),
    }
    let mut map = Map::new();
    map.insert("success".into(), Llsd::Boolean(true));
    map.insert(
        "temp_session_id".into(),
        Llsd::String(Uuid::new_v4().to_string()),
    );
    llsd_response(Llsd::Map(map))
}

pub async fn group_member_data(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => {
            let group_id = llsd.get("group_id").as_str().to_string();
            tracing::debug!(%group_id, "GroupMemberData");
        }
        Err(e) => tracing::debug!(?e, "GroupMemberData: could not parse body"),
    }
    let mut map = Map::new();
    map.insert("member_count".into(), Llsd::Integer(0));
    map.insert("members".into(), Llsd::Array(vec![]));
    llsd_response(Llsd::Map(map))
}

pub async fn set_display_name(body: Bytes) -> Response {
    let display_name = match from_xml(&body) {
        Ok(llsd) => {
            let name = llsd.get("display_name").as_str().to_string();
            tracing::info!(%name, "SetDisplayName");
            name
        }
        Err(e) => {
            tracing::debug!(?e, "SetDisplayName: could not parse body");
            String::new()
        }
    };
    let mut map = Map::new();
    map.insert("status".into(), Llsd::Integer(200));
    map.insert("display_name".into(), Llsd::String(display_name));
    map.insert("display_name_next_update".into(), Llsd::Integer(0));
    llsd_response(Llsd::Map(map))
}

pub async fn search_stat_request(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => {
            let query = llsd.get("query").as_str().to_string();
            tracing::debug!(%query, "SearchStatRequest");
        }
        Err(e) => tracing::debug!(?e, "SearchStatRequest: could not parse body"),
    }
    let mut map = Map::new();
    map.insert("classifieds".into(), Llsd::Array(vec![]));
    map.insert("events".into(), Llsd::Array(vec![]));
    map.insert("places".into(), Llsd::Array(vec![]));
    map.insert("land".into(), Llsd::Array(vec![]));
    llsd_response(Llsd::Map(map))
}

pub async fn remote_parcel_request(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => tracing::debug!(?llsd, "RemoteParcelRequest"),
        Err(e) => tracing::debug!(?e, "RemoteParcelRequest: could not parse body"),
    }
    let mut map = Map::new();
    map.insert(
        "parcel_id".into(),
        Llsd::String("00000000-0000-0000-0000-000000000000".into()),
    );
    map.insert("name".into(), Llsd::String("Unknown".into()));
    map.insert("desc".into(), Llsd::String(String::new()));
    map.insert("actual_area".into(), Llsd::Integer(256));
    map.insert(
        "owner_id".into(),
        Llsd::String("00000000-0000-0000-0000-000000000000".into()),
    );
    llsd_response(Llsd::Map(map))
}

pub async fn copy_inventory_from_notecard(body: Bytes) -> Response {
    log_upload_body("CopyInventoryFromNotecard", &body);
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert(
        "new_inventory_item".into(),
        Llsd::String(Uuid::new_v4().to_string()),
    );
    llsd_response(Llsd::Map(map))
}

pub async fn dispatch_region_info() -> Response {
    let mut map = Map::new();
    map.insert("block_terraform".into(), Llsd::Boolean(false));
    map.insert("block_fly".into(), Llsd::Boolean(false));
    map.insert("allow_damage".into(), Llsd::Boolean(false));
    map.insert("restrict_pushing".into(), Llsd::Boolean(false));
    map.insert("allow_land_resell".into(), Llsd::Boolean(true));
    map.insert("agent_limit".into(), Llsd::Integer(100));
    map.insert("object_bonus".into(), Llsd::Real(1.0));
    llsd_response(Llsd::Map(map))
}

static ESTATE_INFO: Mutex<Option<(i32, String, String, bool)>> = Mutex::new(None);

pub fn set_estate_change_info(estate_id: i32, estate_name: String, estate_owner: String, public_access: bool) {
    if let Ok(mut g) = ESTATE_INFO.lock() {
        *g = Some((estate_id, estate_name, estate_owner, public_access));
    }
}

pub async fn estate_change_info(body: Bytes) -> Response {
    if let Ok(llsd) = from_xml(&body) {
        tracing::debug!(?llsd, "EstateChangeInfo");
    }
    let (estate_id, estate_name, estate_owner, public_access) = ESTATE_INFO
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or((0, "My Estate".to_string(), String::new(), true));
    let mut map = Map::new();
    map.insert("success".into(), Llsd::Boolean(true));
    map.insert("EstateID".into(), Llsd::Integer(estate_id));
    map.insert("EstateName".into(), Llsd::String(estate_name));
    map.insert("EstateOwner".into(), Llsd::String(estate_owner));
    map.insert("public_access".into(), Llsd::Boolean(public_access));
    llsd_response(Llsd::Map(map))
}

pub async fn environment_settings_get() -> Response {
    llsd_response(default_environment_settings())
}

pub async fn environment_settings_post(body: Bytes) -> Response {
    log_upload_body("EnvironmentSettings", &body);
    let mut map = Map::new();
    map.insert("success".into(), Llsd::Boolean(true));
    llsd_response(Llsd::Map(map))
}

pub async fn land_resources(_body: Bytes) -> Response {
    let mut avail_entry = Map::new();
    avail_entry.insert("type".into(), Llsd::String("urls".into()));
    avail_entry.insert("amount".into(), Llsd::Integer(100));

    let mut used_entry = Map::new();
    used_entry.insert("type".into(), Llsd::String("urls".into()));
    used_entry.insert("amount".into(), Llsd::Integer(0));

    let mut resource_summary = Map::new();
    resource_summary.insert("available".into(), Llsd::Array(vec![Llsd::Map(avail_entry)]));
    resource_summary.insert("used".into(), Llsd::Array(vec![Llsd::Map(used_entry)]));

    let mut summary = Map::new();
    summary.insert("available_prims".into(), Llsd::Integer(15000));
    summary.insert("used_prims".into(), Llsd::Integer(0));
    summary.insert("max_prims".into(), Llsd::Integer(15000));

    let mut map = Map::new();
    map.insert("ScriptResourceSummary".into(), Llsd::Map(resource_summary));
    map.insert("summary".into(), Llsd::Map(summary));
    llsd_response(Llsd::Map(map))
}

pub async fn update_gesture_agent_inventory(body: Bytes) -> Response {
    log_upload_body("UpdateGestureAgentInventory", &body);
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(Uuid::new_v4().to_string()));
    map.insert(
        "new_inventory_item".into(),
        Llsd::String(Uuid::new_v4().to_string()),
    );
    llsd_response(Llsd::Map(map))
}

pub async fn update_gesture_task_inventory(body: Bytes) -> Response {
    log_upload_body("UpdateGestureTaskInventory", &body);
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(Uuid::new_v4().to_string()));
    map.insert(
        "new_inventory_item".into(),
        Llsd::String(Uuid::new_v4().to_string()),
    );
    llsd_response(Llsd::Map(map))
}

pub async fn update_notecard_task_inventory(body: Bytes) -> Response {
    log_upload_body("UpdateNotecardTaskInventory", &body);
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(Uuid::new_v4().to_string()));
    map.insert(
        "new_inventory_item".into(),
        Llsd::String(Uuid::new_v4().to_string()),
    );
    llsd_response(Llsd::Map(map))
}

pub async fn update_script_task(body: Bytes) -> Response {
    log_upload_body("UpdateScriptTask", &body);
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(Uuid::new_v4().to_string()));
    map.insert(
        "new_inventory_item".into(),
        Llsd::String(Uuid::new_v4().to_string()),
    );
    llsd_response(Llsd::Map(map))
}

pub fn cap_routes(
    asset_state: AssetServiceState,
    inv_state: InventoryProxyState,
    bake_state: BakeUploadState,
    script_upload_state: ScriptUploadState,
) -> Vec<(&'static str, MethodRouter)> {
    vec![
        (
            "GetTexture",
            get(get_texture).with_state(asset_state.clone()),
        ),
        ("GetMesh", get(get_mesh).with_state(asset_state.clone())),
        ("GetMesh2", get(get_mesh2).with_state(asset_state.clone())),
        ("ViewerAsset", get(viewer_asset).with_state(asset_state)),
        ("SimulatorFeatures", get(simulator_features)),
        ("RegionInfo", get(region_info)),
        (
            "AgentPreferences",
            get(agent_preferences).post(agent_preferences_post),
        ),
        ("MapLayer", get(map_layer).post(map_layer)),
        ("NewFileAgentInventory", post(new_file_agent_inventory)),
        (
            "UpdateNotecardAgentInventory",
            post(update_notecard_agent_inventory),
        ),
        ("UpdateScriptAgent", post(update_script_agent).with_state(script_upload_state.clone())),
        (
            "FetchInventoryDescendents2",
            post(fetch_inventory_descendents2).with_state(inv_state.clone()),
        ),
        (
            "FetchInventory2",
            post(fetch_inventory2).with_state(inv_state.clone()),
        ),
        ("FetchLib2", post(fetch_lib2)),
        ("GetDisplayNames", get(get_display_names)),
        ("HomeLocation", post(home_location)),
        (
            "CreateInventoryCategory",
            post(create_inventory_category).with_state(inv_state),
        ),
        (
            "UploadBakedTexture",
            post(upload_baked_texture).with_state(bake_state),
        ),
        ("ViewerStats", post(viewer_stats)),
        ("UpdateAgentLanguage", post(update_agent_language)),
        ("UpdateAgentInformation", post(update_agent_information)),
        (
            "ParcelVoiceInfoRequest",
            get(parcel_voice_info).post(parcel_voice_info),
        ),
        (
            "ProvisionVoiceAccountRequest",
            get(provision_voice_account).post(provision_voice_account),
        ),
        ("AvatarPickerSearch", post(avatar_picker_search)),
        ("MeshUploadFlag", get(mesh_upload_flag)),
        ("ServerReleaseNotes", get(server_release_notes)),
        ("ViewerMetrics", post(viewer_metrics)),
        ("SimConsoleAsync", post(sim_console_async)),
        ("ChatSessionRequest", post(chat_session_request)),
        ("GroupMemberData", post(group_member_data)),
        ("SetDisplayName", post(set_display_name)),
        ("SearchStatRequest", post(search_stat_request)),
        ("RemoteParcelRequest", post(remote_parcel_request)),
        (
            "CopyInventoryFromNotecard",
            post(copy_inventory_from_notecard),
        ),
        (
            "DispatchRegionInfo",
            get(dispatch_region_info).post(dispatch_region_info),
        ),
        ("EstateChangeInfo", post(estate_change_info)),
        (
            "EnvironmentSettings",
            get(environment_settings_get).post(environment_settings_post),
        ),
        (
            "ExtEnvironment",
            get(environment_settings_get).post(environment_settings_post),
        ),
        ("LandResources", post(land_resources)),
        (
            "UpdateGestureAgentInventory",
            post(update_gesture_agent_inventory),
        ),
        (
            "UpdateGestureTaskInventory",
            post(update_gesture_task_inventory),
        ),
        (
            "UpdateNotecardTaskInventory",
            post(update_notecard_task_inventory),
        ),
        ("UpdateScriptTask", post(update_script_task)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_asset_state() -> AssetServiceState {
        AssetServiceState {
            client: reqwest::Client::new(),
            asset_url: "http://localhost:99999".to_string(),
        }
    }

    fn test_inv_state() -> InventoryProxyState {
        InventoryProxyState {
            client: reqwest::Client::new(),
            inventory_url: "http://localhost:99999".to_string(),
        }
    }

    fn test_bake_state() -> BakeUploadState {
        BakeUploadState {
            client: reqwest::Client::new(),
            asset_url: "http://localhost:99999".to_string(),
            caps_url: "http://localhost:99999".to_string(),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn test_script_upload_state() -> ScriptUploadState {
        let (tx, _rx) = std::sync::mpsc::channel();
        ScriptUploadState {
            script_load: ScriptLoadState { sender: tx },
            caps_url: "http://localhost:9001".to_string(),
            pending: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    fn test_router() -> Router {
        let mut router = Router::new();
        for (name, method_router) in
            cap_routes(test_asset_state(), test_inv_state(), test_bake_state(), test_script_upload_state())
        {
            router = router.route(&format!("/{name}"), method_router);
        }
        router
    }

    async fn body_bytes(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    #[tokio::test]
    async fn test_simulator_features_response() {
        let app = test_router();
        let req = Request::get("/SimulatorFeatures")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("MeshRezEnabled"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("MeshUploadEnabled"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("PhysicsShapeTypes"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("MaxMaterialsPerTransaction"), Llsd::Integer(50));
        assert_eq!(*llsd.get("AnimatedObjects"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("MaxAgentGroupsBasic"), Llsd::Integer(42));
        assert_eq!(*llsd.get("MaxAgentGroupsPremium"), Llsd::Integer(60));
    }

    #[tokio::test]
    async fn test_region_info_response() {
        let app = test_router();
        let req = Request::get("/RegionInfo").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("region_flags"), Llsd::Integer(0));
        assert_eq!(*llsd.get("sim_access"), Llsd::Integer(13));
    }

    #[tokio::test]
    async fn test_get_texture_service_unavailable() {
        let app = test_router();
        let req = Request::get("/GetTexture?texture_id=00000000-0000-0000-0000-000000000001")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Asset service at unreachable test URL returns 503
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_get_texture_missing_param() {
        let app = test_router();
        let req = Request::get("/GetTexture").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_upload_returns_complete() {
        let upload_body = to_xml_string(&Llsd::new_map());
        let app = test_router();
        let req = Request::post("/NewFileAgentInventory")
            .header("Content-Type", "application/llsd+xml")
            .body(Body::from(upload_body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("state"), Llsd::String("complete".into()));
        assert!(matches!(llsd.get("new_asset"), Llsd::String(s) if !s.is_empty()));
        assert!(matches!(llsd.get("new_inventory_item"), Llsd::String(s) if !s.is_empty()));
    }

    #[tokio::test]
    async fn test_cap_routes_count() {
        let routes = cap_routes(test_asset_state(), test_inv_state(), test_bake_state(), test_script_upload_state());
        assert_eq!(routes.len(), 43);
    }

    #[tokio::test]
    async fn test_agent_preferences_get() {
        let app = test_router();
        let req = Request::get("/AgentPreferences")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("hover_height"), Llsd::Real(0.0));
        assert_eq!(*llsd.get("language"), Llsd::String("en".into()));
        let prefs = llsd.get("access_prefs");
        assert_eq!(*prefs.get("max"), Llsd::String("A".into()));
    }
}

#[cfg(test)]
mod agent_token_tests {
    use super::*;

    #[test]
    fn splits_agent_and_secret() {
        let agent = Uuid::new_v4();
        let secret = Uuid::new_v4().to_string();
        let token = format!("{agent}-{secret}");
        assert_eq!(split_agent_token(&token), Some((agent, secret.clone())));
        assert_eq!(split_agent_token(&format!("/{token}/")), Some((agent, secret)));
    }

    #[test]
    fn rejects_malformed_tokens() {
        assert_eq!(split_agent_token("seed"), None);
        assert_eq!(split_agent_token(""), None);
        assert_eq!(split_agent_token(&format!("{}-", Uuid::new_v4())), None);
        assert_eq!(split_agent_token(&format!("{}x{}", Uuid::new_v4(), "secret")), None);
    }
}
