// SPDX-License-Identifier: LGPL-3.0-or-later
use std::sync::Arc;

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine as _;
use quick_xml::{Reader, escape::escape, events::Event};
use welkin_data::assets::{AssetMetadataRow, AssetRow, AssetStore};

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>";
const XML_NS: &str = "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"";

const FLAG_MAPTILE: i32 = 1;
const FLAG_REWRITABLE: i32 = 2;
const FLAG_COLLECTABLE: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAllowed {
    None,
    MapTile,
    All,
}

impl std::str::FromStr for DeleteAllowed {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Self::None),
            "maptile" => Ok(Self::MapTile),
            "all" => Ok(Self::All),
            other => Err(format!("unknown allowed delete type: {other}")),
        }
    }
}

#[derive(Clone)]
pub struct AssetServiceState {
    pub store: Arc<AssetStore>,
    pub allowed_delete_types: DeleteAllowed,
}

impl AssetServiceState {
    pub fn new(store: AssetStore, allowed_delete_types: DeleteAllowed) -> Self {
        Self {
            store: Arc::new(store),
            allowed_delete_types,
        }
    }
}

pub fn router(state: AssetServiceState) -> Router {
    Router::new()
        .route("/assets", post(post_asset))
        .route(
            "/assets/{id}",
            get(get_asset).post(update_asset).delete(delete_asset),
        )
        .route("/assets/{id}/data", get(get_asset_data))
        .route("/assets/{id}/metadata", get(get_asset_metadata))
        .route("/get_assets_exist", post(assets_exist))
        .with_state(state)
}

pub fn flags_to_xml(flags: i32) -> String {
    if flags == 0 {
        return "Normal".to_string();
    }
    let mut names = Vec::new();
    if flags & FLAG_MAPTILE != 0 {
        names.push("Maptile");
    }
    if flags & FLAG_REWRITABLE != 0 {
        names.push("Rewritable");
    }
    if flags & FLAG_COLLECTABLE != 0 {
        names.push("Collectable");
    }
    let known = FLAG_MAPTILE | FLAG_REWRITABLE | FLAG_COLLECTABLE;
    if flags & !known != 0 {
        return flags.to_string();
    }
    names.join(", ")
}

pub fn flags_from_xml(text: &str) -> i32 {
    let mut flags = 0;
    for part in text.split(',') {
        let part = part.trim();
        flags |= match part.to_ascii_lowercase().as_str() {
            "normal" | "" => 0,
            "maptile" => FLAG_MAPTILE,
            "rewritable" => FLAG_REWRITABLE,
            "collectable" => FLAG_COLLECTABLE,
            other => other.parse::<i32>().unwrap_or(0),
        };
    }
    flags
}

fn bool_xml(v: i32) -> &'static str {
    if v != 0 { "true" } else { "false" }
}

fn parse_bool(s: &str) -> i32 {
    i32::from(s.trim().eq_ignore_ascii_case("true") || s.trim() == "1")
}

#[allow(clippy::too_many_arguments)]
fn write_asset_xml(
    data: Option<&[u8]>,
    id: &str,
    name: &str,
    description: &str,
    asset_type: i32,
    local: i32,
    temporary: i32,
    creator_id: &str,
    flags: i32,
) -> String {
    let mut out = String::with_capacity(512 + data.map_or(0, |d| d.len() * 4 / 3 + 4));
    out.push_str(XML_DECL);
    out.push('\n');
    out.push_str(&format!("<AssetBase {XML_NS}>\n"));
    if let Some(d) = data {
        out.push_str("  <Data>");
        out.push_str(&base64::engine::general_purpose::STANDARD.encode(d));
        out.push_str("</Data>\n");
    }
    let id = escape(id);
    out.push_str(&format!("  <FullID><Guid>{id}</Guid></FullID>\n"));
    out.push_str(&format!("  <ID>{id}</ID>\n"));
    out.push_str(&format!("  <Name>{}</Name>\n", escape(name)));
    out.push_str(&format!(
        "  <Description>{}</Description>\n",
        escape(description)
    ));
    out.push_str(&format!("  <Type>{asset_type}</Type>\n"));
    out.push_str(&format!("  <Local>{}</Local>\n", bool_xml(local)));
    out.push_str(&format!(
        "  <Temporary>{}</Temporary>\n",
        bool_xml(temporary)
    ));
    out.push_str(&format!(
        "  <CreatorID>{}</CreatorID>\n",
        escape(creator_id)
    ));
    out.push_str(&format!("  <Flags>{}</Flags>\n", flags_to_xml(flags)));
    out.push_str("</AssetBase>");
    out
}

pub fn asset_to_xml(a: &AssetRow) -> String {
    write_asset_xml(
        Some(&a.data),
        &a.id,
        &a.name,
        &a.description,
        a.asset_type,
        a.local,
        a.temporary,
        &a.creator_id,
        a.asset_flags,
    )
}

pub fn metadata_to_xml(a: &AssetMetadataRow) -> String {
    write_asset_xml(
        None,
        &a.id,
        &a.name,
        &a.description,
        a.asset_type,
        a.local,
        a.temporary,
        &a.creator_id,
        a.asset_flags,
    )
}

#[derive(Debug, Default)]
pub struct ParsedAsset {
    pub data: Vec<u8>,
    pub id: String,
    pub name: String,
    pub description: String,
    pub asset_type: i32,
    pub local: i32,
    pub temporary: i32,
    pub creator_id: String,
    pub flags: i32,
}

pub fn parse_asset_xml(xml: &str) -> Result<ParsedAsset, String> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut asset = ParsedAsset::default();
    let mut full_id = String::new();
    let mut data_b64 = String::new();
    let mut saw_root = false;

    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if stack.is_empty() {
                    if name != "AssetBase" {
                        return Err(format!("unexpected root element {name}"));
                    }
                    saw_root = true;
                }
                stack.push(name);
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(t) => {
                let text = t.unescape().map_err(|e| e.to_string())?;
                apply_text(&stack, &text, &mut asset, &mut full_id, &mut data_b64);
            }
            Event::CData(t) => {
                let text = String::from_utf8_lossy(&t.into_inner()).into_owned();
                apply_text(&stack, &text, &mut asset, &mut full_id, &mut data_b64);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !saw_root {
        return Err("missing AssetBase element".to_string());
    }
    if asset.id.is_empty() {
        asset.id = full_id;
    }
    let cleaned: String = data_b64.chars().filter(|c| !c.is_whitespace()).collect();
    asset.data = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| format!("invalid base64 data: {e}"))?;
    Ok(asset)
}

fn apply_text(
    stack: &[String],
    text: &str,
    asset: &mut ParsedAsset,
    full_id: &mut String,
    data_b64: &mut String,
) {
    let path: Vec<&str> = stack.iter().map(String::as_str).collect();
    match path.as_slice() {
        ["AssetBase", "Data"] => data_b64.push_str(text),
        ["AssetBase", "FullID", "Guid"] => full_id.push_str(text.trim()),
        ["AssetBase", "ID"] => asset.id.push_str(text.trim()),
        ["AssetBase", "Name"] => asset.name.push_str(text),
        ["AssetBase", "Description"] => asset.description.push_str(text),
        ["AssetBase", "Type"] => asset.asset_type = text.trim().parse().unwrap_or(0),
        ["AssetBase", "Local"] => asset.local = parse_bool(text),
        ["AssetBase", "Temporary"] => asset.temporary = parse_bool(text),
        ["AssetBase", "CreatorID"] => asset.creator_id.push_str(text.trim()),
        ["AssetBase", "Flags"] => asset.flags = flags_from_xml(text),
        _ => {}
    }
}

pub fn parse_string_array(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut in_string = false;
    let mut current = String::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == b"string" => {
                in_string = true;
                current.clear();
            }
            Event::Empty(e) if e.local_name().as_ref() == b"string" => out.push(String::new()),
            Event::Text(t) if in_string => {
                current.push_str(&t.unescape().map_err(|e| e.to_string())?);
            }
            Event::End(e) if e.local_name().as_ref() == b"string" => {
                in_string = false;
                out.push(current.trim().to_string());
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn xml_response(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "text/xml")], body).into_response()
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

fn server_error(op: &str, err: impl std::fmt::Display) -> Response {
    tracing::error!(op, error = %err, "asset service error");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

fn bool_response(v: bool) -> Response {
    xml_response(
        StatusCode::OK,
        format!("{XML_DECL}\n<boolean>{v}</boolean>"),
    )
}

fn now_secs() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0)
}

fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}

async fn get_asset(State(state): State<AssetServiceState>, Path(id): Path<String>) -> Response {
    if !valid_id(&id) {
        return not_found();
    }
    match state.store.get(&id).await {
        Ok(Some(asset)) => xml_response(StatusCode::OK, asset_to_xml(&asset)),
        Ok(None) => not_found(),
        Err(e) => server_error("get_asset", e),
    }
}

async fn get_asset_data(
    State(state): State<AssetServiceState>,
    Path(id): Path<String>,
) -> Response {
    if !valid_id(&id) {
        return not_found();
    }
    match state.store.get(&id).await {
        Ok(Some(asset)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            asset.data,
        )
            .into_response(),
        Ok(None) => not_found(),
        Err(e) => server_error("get_asset_data", e),
    }
}

async fn get_asset_metadata(
    State(state): State<AssetServiceState>,
    Path(id): Path<String>,
) -> Response {
    if !valid_id(&id) {
        return not_found();
    }
    match state.store.get_metadata(&id).await {
        Ok(Some(meta)) => xml_response(StatusCode::OK, metadata_to_xml(&meta)),
        Ok(None) => not_found(),
        Err(e) => server_error("get_asset_metadata", e),
    }
}

async fn post_asset(State(state): State<AssetServiceState>, body: Bytes) -> Response {
    let xml = String::from_utf8_lossy(&body);
    let parsed = match parse_asset_xml(&xml) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "rejecting malformed asset POST");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !valid_id(&parsed.id) {
        tracing::warn!(id = %parsed.id, "rejecting asset POST with invalid id");
        return StatusCode::BAD_REQUEST.into_response();
    }
    let now = now_secs();
    let row = AssetRow {
        name: parsed.name,
        description: parsed.description,
        asset_type: parsed.asset_type,
        local: parsed.local,
        temporary: parsed.temporary,
        data: parsed.data,
        id: parsed.id.clone(),
        create_time: now,
        access_time: now,
        asset_flags: parsed.flags,
        creator_id: parsed.creator_id,
    };
    match state.store.store(&row).await {
        Ok(()) => xml_response(
            StatusCode::OK,
            format!("{XML_DECL}\n<string>{}</string>", escape(&row.id)),
        ),
        Err(e) => server_error("post_asset", e),
    }
}

async fn update_asset(
    State(state): State<AssetServiceState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    if !valid_id(&id) {
        return not_found();
    }
    let xml = String::from_utf8_lossy(&body);
    let parsed = match parse_asset_xml(&xml) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "rejecting malformed asset update");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    let mut row = match state.store.get(&id).await {
        Ok(Some(r)) => r,
        Ok(None) => return bool_response(false),
        Err(e) => return server_error("update_asset", e),
    };
    row.data = parsed.data;
    match state.store.store(&row).await {
        Ok(()) => bool_response(true),
        Err(e) => server_error("update_asset", e),
    }
}

async fn delete_asset(State(state): State<AssetServiceState>, Path(id): Path<String>) -> Response {
    if !valid_id(&id) {
        return not_found();
    }
    if state.allowed_delete_types == DeleteAllowed::None {
        tracing::warn!(%id, "asset delete refused: deletion disabled");
        return bool_response(false);
    }
    let meta = match state.store.get_metadata(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return bool_response(false),
        Err(e) => return server_error("delete_asset", e),
    };
    if state.allowed_delete_types == DeleteAllowed::MapTile && meta.asset_flags & FLAG_MAPTILE == 0
    {
        tracing::warn!(%id, "asset delete refused: not a maptile");
        return bool_response(false);
    }
    match state.store.delete(&id).await {
        Ok(deleted) => bool_response(deleted),
        Err(e) => server_error("delete_asset", e),
    }
}

async fn assets_exist(State(state): State<AssetServiceState>, body: Bytes) -> Response {
    let xml = String::from_utf8_lossy(&body);
    let ids = match parse_string_array(&xml) {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(error = %e, "rejecting malformed exists request");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    let mut out = format!("{XML_DECL}\n<ArrayOfBoolean {XML_NS}>\n");
    for id in ids {
        let exists = if valid_id(&id) {
            match state.store.exists(&id).await {
                Ok(v) => v,
                Err(e) => return server_error("assets_exist", e),
            }
        } else {
            false
        };
        out.push_str(&format!("  <boolean>{exists}</boolean>\n"));
    }
    out.push_str("</ArrayOfBoolean>");
    xml_response(StatusCode::OK, out)
}
