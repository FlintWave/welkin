// SPDX-License-Identifier: GPL-3.0-or-later
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
use welkin_data::grid::{GridStore, RegionRow};

#[derive(Clone)]
pub struct GridServiceState {
    pub store: Arc<GridStore>,
}

pub fn router(state: GridServiceState) -> Router {
    Router::new().route("/grid", post(handle_grid)).with_state(state)
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
    let body = format!("<?xml version=\"1.0\"?><ServerResponse>{content}</ServerResponse>");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn tag(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, "<{name}>{}</{name}>", escape(value));
}

fn ok(success: bool, msg: &str) -> Response {
    if success {
        xml_response("<Result>Success</Result>")
    } else {
        xml_response(&format!("<Result>Failure</Result><Message>{}</Message>", escape(msg)))
    }
}

fn region_xml(name: &str, r: &RegionRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "uuid", &r.uuid);
    tag(&mut s, "locX", &r.loc_x.to_string());
    tag(&mut s, "locY", &r.loc_y.to_string());
    tag(&mut s, "sizeX", &r.size_x.to_string());
    tag(&mut s, "sizeY", &r.size_y.to_string());
    tag(&mut s, "regionName", &r.region_name);
    tag(&mut s, "flags", &r.flags.to_string());
    tag(&mut s, "serverIP", &r.server_ip);
    tag(&mut s, "serverHttpPort", &r.server_http_port.unwrap_or(0).to_string());
    tag(&mut s, "serverURI", &r.server_uri);
    tag(&mut s, "serverPort", &r.server_port.unwrap_or(0).to_string());
    tag(&mut s, "regionMapTexture", &r.region_map_texture);
    tag(&mut s, "parcelMapTexture", &r.parcel_map_texture);
    tag(&mut s, "access", &r.access.to_string());
    tag(&mut s, "regionSecret", &r.region_secret);
    tag(&mut s, "owner_uuid", &r.owner_uuid);
    tag(&mut s, "Token", &r.token);
    let _ = write!(s, "</{name}>");
    s
}

fn single(r: Result<Option<RegionRow>, welkin_data::DataError>) -> Response {
    match r {
        Ok(Some(row)) => xml_response(&region_xml("result", &row)),
        _ => xml_response("<result>null</result>"),
    }
}

fn many(r: Result<Vec<RegionRow>, welkin_data::DataError>) -> Response {
    match r {
        Ok(rows) if !rows.is_empty() => {
            let mut s = String::new();
            for (i, row) in rows.iter().enumerate() {
                s.push_str(&region_xml(&format!("region{i}"), row));
            }
            xml_response(&s)
        }
        Ok(_) => xml_response("<result>null</result>"),
        Err(e) => {
            tracing::warn!(error = %e, "grid query failed");
            xml_response("<result>null</result>")
        }
    }
}

fn region_from_form(f: &Form) -> RegionRow {
    let loc_x: i32 = f.num("locX");
    let loc_y: i32 = f.num("locY");
    let scope = if f.str("ScopeID").is_empty() { f.str("SCOPEID") } else { f.str("ScopeID") };
    let zero = "00000000-0000-0000-0000-000000000000";
    let or_zero = |s: &str| if s.is_empty() { zero.to_string() } else { s.to_string() };
    let size_x: i32 = f.num("sizeX");
    let size_y: i32 = f.num("sizeY");
    RegionRow {
        uuid: f.str("uuid").to_string(),
        region_handle: (i64::from(loc_x) << 32) | i64::from(loc_y),
        region_name: f.str("regionName").to_string(),
        region_secret: f.str("regionSecret").to_string(),
        server_ip: f.str("serverIP").to_string(),
        server_port: Some(f.num("serverPort")),
        server_uri: f.str("serverURI").to_string(),
        loc_x,
        loc_y,
        loc_z: 0,
        region_map_texture: or_zero(f.str("regionMapTexture")),
        server_http_port: Some(f.num("serverHttpPort")),
        owner_uuid: or_zero(f.str("owner_uuid")),
        access: if f.get("access").is_some() { f.num("access") } else { 1 },
        scope_id: or_zero(scope),
        size_x: if size_x == 0 { 256 } else { size_x },
        size_y: if size_y == 0 { 256 } else { size_y },
        flags: f.num("flags"),
        last_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i32)
            .unwrap_or(0),
        principal_id: or_zero(f.str("PrincipalID")),
        token: f.str("Token").to_string(),
        parcel_map_texture: or_zero(f.str("parcelMapTexture")),
    }
}

async fn handle_grid(State(state): State<GridServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    let scope = f.str("SCOPEID");
    tracing::debug!(method, "grid request");

    match method {
        "register" => {
            let region = region_from_form(&f);
            if region.uuid.is_empty() {
                return ok(false, "missing region uuid");
            }
            match store.store_region(&region).await {
                Ok(()) => ok(true, ""),
                Err(e) => {
                    tracing::warn!(error = %e, "register failed");
                    ok(false, "Region registration failed")
                }
            }
        }
        "deregister" => match store.deregister(f.str("REGIONID")).await {
            Ok(true) => ok(true, ""),
            Ok(false) => ok(false, "Region not found"),
            Err(e) => {
                tracing::warn!(error = %e, "deregister failed");
                ok(false, "Deregistration failed")
            }
        },
        "get_neighbours" => many(store.get_neighbours(scope, f.str("REGIONID")).await),
        "get_region_by_uuid" => single(store.get_region_by_uuid(scope, f.str("REGIONID")).await),
        "get_region_by_position" => single(
            store
                .get_region_by_position(scope, f.num("X"), f.num("Y"))
                .await,
        ),
        "get_region_by_name" | "get_localregion_by_name" => {
            single(store.get_region_by_name(f.str("NAME")).await)
        }
        "get_regions_by_name" => {
            let max: usize = f.num("MAX");
            let pattern = format!("{}%", f.str("NAME"));
            many(store.get_regions_by_name(scope, &pattern).await.map(|mut v| {
                if max > 0 {
                    v.truncate(max);
                }
                v
            }))
        }
        "get_region_range" => many(
            store
                .get_regions_in_range(scope, f.num("XMIN"), f.num("XMAX"), f.num("YMIN"), f.num("YMAX"))
                .await,
        ),
        "get_default_regions" => many(store.get_default_regions(scope).await),
        "get_default_hypergrid_regions" => many(store.get_default_hypergrid_regions(scope).await),
        "get_fallback_regions" => many(store.get_fallback_regions(scope).await),
        "get_online_regions" => many(
            store
                .get_online_regions(scope, f.num("X"), f.num("Y"), f.num("MC"))
                .await,
        ),
        "get_hyperlinks" => many(store.get_hyperlinks(scope).await),
        "get_region_flags" => match store.get_region_flags(scope, f.str("REGIONID")).await {
            Ok(Some(flags)) => xml_response(&format!("<result>{flags}</result>")),
            _ => xml_response("<result>-1</result>"),
        },
        "get_grid_extra_features" => xml_response("<result type=\"List\" />"),
        _ => {
            tracing::warn!(method, "unknown grid method");
            ok(false, "unknown method")
        }
    }
}
