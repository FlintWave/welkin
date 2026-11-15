// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, fmt::Write as _, sync::Arc};

use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use quick_xml::escape::escape;
use serde::Deserialize;
use welkin_data::estate::{EstateBanRow, EstateSettingsRow, EstateStore};

#[derive(Clone)]
pub struct EstateServiceState {
    pub store: Arc<EstateStore>,
}

pub fn router(state: EstateServiceState) -> Router {
    Router::new()
        .route("/estates", get(handle_estates_get))
        .route(
            "/estates/estate/",
            get(handle_estate_get).post(handle_estate_post),
        )
        .route("/estates/regions/", get(handle_regions_get))
        .with_state(state)
}

#[derive(Default)]
struct Form {
    single: HashMap<String, String>,
}

impl Form {
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
        form.single.insert(url_decode(k), url_decode(v));
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

fn ban_to_xml(name: &str, ban: &EstateBanRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "EstateID", &ban.estate_id.to_string());
    tag(&mut s, "BannedUserID", &ban.banned_uuid);
    tag(&mut s, "BannedHostAddress", &ban.banned_ip);
    tag(&mut s, "BannedHostIPMask", &ban.banned_ip_host_mask);
    tag(
        &mut s,
        "BannedHostNameMask",
        ban.banned_name_mask.as_deref().unwrap_or(""),
    );
    tag(&mut s, "BanningUserID", &ban.banning_uuid);
    tag(&mut s, "BanTime", &ban.ban_time.to_string());
    let _ = write!(s, "</{name}>");
    s
}

async fn estate_to_xml(name: &str, e: &EstateSettingsRow, store: &EstateStore) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "EstateID", &e.estate_id.to_string());
    tag(&mut s, "EstateName", e.estate_name.as_deref().unwrap_or(""));
    tag(
        &mut s,
        "AbuseEmailToEstateOwner",
        &e.abuse_email_to_estate_owner.to_string(),
    );
    tag(&mut s, "DenyAnonymous", &e.deny_anonymous.to_string());
    tag(
        &mut s,
        "ResetHomeOnTeleport",
        &e.reset_home_on_teleport.to_string(),
    );
    tag(&mut s, "FixedSun", &e.fixed_sun.to_string());
    tag(&mut s, "DenyTransacted", &e.deny_transacted.to_string());
    tag(&mut s, "BlockDwell", &e.block_dwell.to_string());
    tag(&mut s, "DenyIdentified", &e.deny_identified.to_string());
    tag(&mut s, "AllowVoice", &e.allow_voice.to_string());
    tag(&mut s, "UseGlobalTime", &e.use_global_time.to_string());
    tag(&mut s, "PricePerMeter", &e.price_per_meter.to_string());
    tag(&mut s, "TaxFree", &e.tax_free.to_string());
    tag(
        &mut s,
        "AllowDirectTeleport",
        &e.allow_direct_teleport.to_string(),
    );
    tag(&mut s, "RedirectGridX", &e.redirect_grid_x.to_string());
    tag(&mut s, "RedirectGridY", &e.redirect_grid_y.to_string());
    tag(&mut s, "ParentEstateID", &e.parent_estate_id.to_string());
    tag(&mut s, "SunPosition", &e.sun_position.to_string());
    tag(
        &mut s,
        "EstateSkipScripts",
        &e.estate_skip_scripts.to_string(),
    );
    tag(&mut s, "BillableFactor", &e.billable_factor.to_string());
    tag(&mut s, "PublicAccess", &e.public_access.to_string());
    tag(&mut s, "AbuseEmail", &e.abuse_email);
    tag(&mut s, "EstateOwner", &e.estate_owner);
    tag(&mut s, "DenyMinors", &e.deny_minors.to_string());
    tag(&mut s, "AllowLandmark", &e.allow_landmark.to_string());
    tag(
        &mut s,
        "AllowParcelChanges",
        &e.allow_parcel_changes.to_string(),
    );
    tag(&mut s, "AllowSetHome", &e.allow_set_home.to_string());
    tag(
        &mut s,
        "AllowEnviromentOverride",
        &e.allow_environment_override.to_string(),
    );

    if let Ok(users) = store.get_estate_users(e.estate_id).await {
        tag(&mut s, "EstateAccess", &users.join(","));
    }
    if let Ok(groups) = store.get_estate_groups(e.estate_id).await {
        tag(&mut s, "EstateGroups", &groups.join(","));
    }
    if let Ok(managers) = store.get_estate_managers(e.estate_id).await {
        tag(&mut s, "EstateManagers", &managers.join(","));
    }
    if let Ok(bans) = store.get_estate_bans(e.estate_id).await {
        for (i, ban) in bans.iter().enumerate() {
            s.push_str(&ban_to_xml(&format!("ban{i}"), ban));
        }
    }

    let _ = write!(s, "</{name}>");
    s
}

#[derive(Deserialize, Default)]
struct EstatesQuery {
    name: Option<String>,
    owner: Option<String>,
}

#[derive(Deserialize, Default)]
struct EstateQuery {
    region: Option<String>,
    eid: Option<i32>,
    #[allow(dead_code)]
    create: Option<String>,
}

#[derive(Deserialize, Default)]
struct RegionsQuery {
    eid: Option<i32>,
}

#[derive(Deserialize, Default)]
struct EstatePostQuery {
    eid: Option<i32>,
    region: Option<String>,
}

async fn handle_estates_get(
    State(state): State<EstateServiceState>,
    Query(q): Query<EstatesQuery>,
) -> Response {
    let store = &state.store;
    tracing::debug!("estates list request");

    if let Some(name) = &q.name {
        match store.get_estate_ids_by_name(name).await {
            Ok(ids) => {
                let mut s = String::new();
                for (i, id) in ids.iter().enumerate() {
                    tag(&mut s, &format!("estate{i}"), &id.to_string());
                }
                if s.is_empty() {
                    xml_response("<result>null</result>")
                } else {
                    xml_response(&s)
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "get estates by name failed");
                xml_response("<result>null</result>")
            }
        }
    } else if let Some(owner) = &q.owner {
        match store.get_estate_ids_by_owner(owner).await {
            Ok(ids) => {
                let mut s = String::new();
                for (i, id) in ids.iter().enumerate() {
                    tag(&mut s, &format!("estate{i}"), &id.to_string());
                }
                if s.is_empty() {
                    xml_response("<result>null</result>")
                } else {
                    xml_response(&s)
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "get estates by owner failed");
                xml_response("<result>null</result>")
            }
        }
    } else {
        match store.get_all_estates().await {
            Ok(estates) => {
                let mut s = String::new();
                for (i, e) in estates.iter().enumerate() {
                    s.push_str(&estate_to_xml(&format!("estate{i}"), e, store).await);
                }
                if s.is_empty() {
                    xml_response("<result>null</result>")
                } else {
                    xml_response(&s)
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "get all estates failed");
                xml_response("<result>null</result>")
            }
        }
    }
}

async fn handle_estate_get(
    State(state): State<EstateServiceState>,
    Query(q): Query<EstateQuery>,
) -> Response {
    let store = &state.store;
    tracing::debug!("estate get request");

    if let Some(eid) = q.eid {
        match store.get_estate(eid).await {
            Ok(Some(e)) => xml_response(&estate_to_xml("result", &e, store).await),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => {
                tracing::warn!(error = %e, "get estate by id failed");
                StatusCode::NOT_FOUND.into_response()
            }
        }
    } else if let Some(region) = &q.region {
        match store.get_estate_for_region(region).await {
            Ok(Some(e)) => xml_response(&estate_to_xml("result", &e, store).await),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => {
                tracing::warn!(error = %e, "get estate for region failed");
                StatusCode::NOT_FOUND.into_response()
            }
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn handle_regions_get(
    State(state): State<EstateServiceState>,
    Query(q): Query<RegionsQuery>,
) -> Response {
    let store = &state.store;
    tracing::debug!("estate regions request");

    let eid = match q.eid {
        Some(id) => id,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    match store.get_regions_for_estate(eid).await {
        Ok(regions) => {
            let mut s = String::new();
            for (i, r) in regions.iter().enumerate() {
                tag(&mut s, &format!("region{i}"), r);
            }
            if s.is_empty() {
                xml_response("<result>null</result>")
            } else {
                xml_response(&s)
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "get regions for estate failed");
            xml_response("<result>null</result>")
        }
    }
}

async fn handle_estate_post(
    State(state): State<EstateServiceState>,
    Query(q): Query<EstatePostQuery>,
    body: Bytes,
) -> Response {
    let f = parse_form(&body);
    let op = f.str("OP");
    let store = &state.store;
    tracing::debug!(op, "estate post request");

    match op {
        "STORE" => {
            let estate = EstateSettingsRow {
                estate_id: f.num("EstateID"),
                estate_name: Some(f.str("EstateName").to_string()),
                abuse_email_to_estate_owner: f.num("AbuseEmailToEstateOwner"),
                deny_anonymous: f.num("DenyAnonymous"),
                reset_home_on_teleport: f.num("ResetHomeOnTeleport"),
                fixed_sun: f.num("FixedSun"),
                deny_transacted: f.num("DenyTransacted"),
                block_dwell: f.num("BlockDwell"),
                deny_identified: f.num("DenyIdentified"),
                allow_voice: f.num("AllowVoice"),
                use_global_time: f.num("UseGlobalTime"),
                price_per_meter: f.num("PricePerMeter"),
                tax_free: f.num("TaxFree"),
                allow_direct_teleport: f.num("AllowDirectTeleport"),
                redirect_grid_x: f.num("RedirectGridX"),
                redirect_grid_y: f.num("RedirectGridY"),
                parent_estate_id: f.num("ParentEstateID"),
                sun_position: f.num("SunPosition"),
                estate_skip_scripts: f.num("EstateSkipScripts"),
                billable_factor: f.num("BillableFactor"),
                public_access: f.num("PublicAccess"),
                abuse_email: f.str("AbuseEmail").to_string(),
                estate_owner: f.str("EstateOwner").to_string(),
                deny_minors: f.num("DenyMinors"),
                allow_landmark: f.num("AllowLandmark"),
                allow_parcel_changes: f.num("AllowParcelChanges"),
                allow_set_home: f.num("AllowSetHome"),
                allow_environment_override: f.num("AllowEnviromentOverride"),
            };
            match store.store_estate(&estate).await {
                Ok(()) => xml_response("<Result>True</Result>"),
                Err(e) => {
                    tracing::warn!(error = %e, "store estate failed");
                    xml_response("<Result>False</Result>")
                }
            }
        }
        "LINK" => {
            let eid = match q.eid {
                Some(id) => id,
                None => return xml_response("<Result>False</Result>"),
            };
            let region = match &q.region {
                Some(r) => r.as_str(),
                None => return xml_response("<Result>False</Result>"),
            };
            match store.link_region(eid, region).await {
                Ok(()) => xml_response("<Result>True</Result>"),
                Err(e) => {
                    tracing::warn!(error = %e, "link region failed");
                    xml_response("<Result>False</Result>")
                }
            }
        }
        _ => {
            tracing::warn!(op, "unknown estate operation");
            xml_response("<Result>False</Result>")
        }
    }
}
