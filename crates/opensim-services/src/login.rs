// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;

use quick_xml::events::Event;
use quick_xml::Reader;
use uuid::Uuid;

use opensim_data::accounts::UserAccountStore;
use opensim_data::auth::AuthStore;
use opensim_data::friends::FriendsStore;
use opensim_data::grid::GridStore;
use opensim_data::grid_user::GridUserStore;
use opensim_data::inventory::InventoryStore;
use opensim_data::presence::PresenceStore;

#[derive(Clone)]
pub struct LoginServiceState {
    pub auth_store: Arc<AuthStore>,
    pub accounts_store: Arc<UserAccountStore>,
    pub inventory_store: Arc<InventoryStore>,
    pub grid_store: Arc<GridStore>,
    pub presence_store: Arc<PresenceStore>,
    pub griduser_store: Arc<GridUserStore>,
    pub friends_store: Arc<FriendsStore>,
}

pub fn router(state: LoginServiceState) -> Router {
    Router::new()
        .route("/", post(login_handler))
        .with_state(state)
}

async fn login_handler(State(state): State<LoginServiceState>, body: String) -> Response {
    let trimmed = body.trim_start();
    let is_llsd = trimmed.starts_with("<llsd>") || trimmed.starts_with("<map>");
    if is_llsd {
        tracing::debug!("LLSD login request — extracting params");
        let params = match parse_llsd_login(&body) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "malformed LLSD login request");
                return error_response("Malformed", "Could not parse the login request");
            }
        };
        let xmlrpc_response = handle_login(state, params).await;
        return xmlrpc_response_to_llsd(xmlrpc_response).await;
    }

    let (method, params) = match parse_xml_rpc(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "malformed XML-RPC request");
            return error_response("Malformed", "Could not parse the login request");
        }
    };

    tracing::debug!(method = %method, "login XML-RPC request");

    match method.as_str() {
        "login_to_simulator" => handle_login(state, params).await,
        "set_login_level" => error_response("AdminRequired", "Admin access required"),
        _ => error_response("UnsupportedMethod", "Unknown method"),
    }
}

async fn handle_login(state: LoginServiceState, params: HashMap<String, String>) -> Response {
    let first = params.get("first").cloned().unwrap_or_default();
    let last = params.get("last").cloned().unwrap_or_default();
    let passwd = params.get("passwd").cloned().unwrap_or_default();
    let start = params.get("start").cloned().unwrap_or_else(|| "home".to_string());

    if first.is_empty() || last.is_empty() {
        return error_response("key", "Missing name");
    }

    tracing::debug!(first = %first, last = %last, start = %start, "login attempt");

    // 1. Account lookup
    let account = match state.accounts_store.get_by_name(&first, &last).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            tracing::debug!("login: user not found");
            return error_response("key", "Could not authenticate your avatar. Please check your username and password, and check the grid if problems persist.");
        }
        Err(e) => {
            tracing::warn!(error = %e, "login: account lookup failed");
            return error_response("key", "Could not authenticate your avatar. Please check your username and password, and check the grid if problems persist.");
        }
    };

    let principal_id = account.principal_id.clone();

    // 2. Authentication
    let auth = match state.auth_store.get(&principal_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return error_response("key", "Could not authenticate your avatar. Please check your username and password, and check the grid if problems persist."),
        Err(e) => {
            tracing::warn!(error = %e, "login: auth lookup failed");
            return error_response("key", "Could not authenticate your avatar. Please check your username and password, and check the grid if problems persist.");
        }
    };

    // C# pre-hashes cleartext passwords: if no $1$ prefix, md5 the raw value first
    let passwd = if passwd.starts_with("$1$") {
        passwd
    } else {
        format!("$1${}", md5_hex(&passwd))
    };
    let stripped = passwd.strip_prefix("$1$").unwrap();
    let computed = md5_hex(&format!("{}:{}", stripped, auth.password_salt));
    if !crate::constant_time_eq(&computed, &auth.password_hash) {
        tracing::debug!(principal = %principal_id, "login: password mismatch");
        return error_response("key", "Could not authenticate your avatar. Please check your username and password, and check the grid if problems persist.");
    }

    // 3. Generate session tokens
    let secure_session_id = Uuid::new_v4().to_string();
    if let Err(e) = state
        .auth_store
        .store_token(&principal_id, &secure_session_id, 30)
        .await
    {
        tracing::warn!(error = %e, "login: store token failed");
        return error_response("Internal Error", "Internal Error");
    }
    let session_id = Uuid::new_v4().to_string();

    // 4. Clear existing presence
    if let Ok(existing) = state.presence_store.get_by_user(&principal_id).await {
        for presence in existing {
            let _ = state.presence_store.logout(&presence.session_id).await;
        }
    }

    // 5. Presence login
    if let Err(e) = state
        .presence_store
        .login(&principal_id, &session_id, &secure_session_id)
        .await
    {
        tracing::warn!(error = %e, "login: presence login failed");
        return error_response("presence", "Internal Error");
    }

    // 6. GridUser info (for home/last region)
    let griduser = state.griduser_store.get(&principal_id).await.ok().flatten();

    // 7. Resolve destination region
    let mut region = None;
    match start.as_str() {
        "home" => {
            if let Some(gu) = &griduser
                && !gu.home_region_id.is_empty()
                && gu.home_region_id != ZERO_UUID
            {
                region = state
                    .grid_store
                    .get_region_by_uuid(&account.scope_id, &gu.home_region_id)
                    .await
                    .ok()
                    .flatten();
            }
        }
        "last" => {
            if let Some(gu) = &griduser
                && !gu.last_region_id.is_empty()
                && gu.last_region_id != ZERO_UUID
            {
                region = state
                    .grid_store
                    .get_region_by_uuid(&account.scope_id, &gu.last_region_id)
                    .await
                    .ok()
                    .flatten();
            }
        }
        s if s.starts_with("uri:") => {
            let rest = &s[4..];
            let region_name = rest.split('&').next().unwrap_or(rest);
            if !region_name.is_empty() {
                region = state
                    .grid_store
                    .get_region_by_name(region_name)
                    .await
                    .ok()
                    .flatten();
            }
        }
        _ => {}
    }

    if region.is_none()
        && let Ok(mut defaults) = state.grid_store.get_default_regions(&account.scope_id).await
    {
        region = defaults.drain(..).next();
    }
    if region.is_none()
        && let Ok(mut fallbacks) =
            state.grid_store.get_fallback_regions(&account.scope_id).await
    {
        region = fallbacks.drain(..).next();
    }

    let region = match region {
        Some(r) => r,
        None => {
            tracing::warn!(principal = %principal_id, "login: no destination region available");
            return error_response("presence", "destination not available");
        }
    };

    // 8. Mark logged in
    let _ = state.griduser_store.logged_in(&principal_id).await;

    // 9. Inventory skeleton
    let folders = state
        .inventory_store
        .get_user_folders(&principal_id)
        .await
        .unwrap_or_default();
    let root_folder = state
        .inventory_store
        .get_user_root_folder(&principal_id)
        .await
        .ok()
        .flatten();
    let gestures = state
        .inventory_store
        .get_active_gestures(&principal_id)
        .await
        .unwrap_or_default();

    // 10. Friends list
    let friends = state
        .friends_store
        .get_friends_with_their_flags(&principal_id)
        .await
        .unwrap_or_default();

    // 11. Circuit code (random u32)
    let circuit_uuid = Uuid::new_v4();
    let circuit_bytes: [u8; 4] = circuit_uuid.as_bytes()[0..4].try_into().unwrap();
    let circuit_code = u32::from_be_bytes(circuit_bytes) as i32;

    let start_location = match start.as_str() {
        "home" => "home",
        "last" => "last",
        s if s.starts_with("uri:") => "url",
        _ => "home",
    };

    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0);

    let caps_path = Uuid::new_v4().to_string();
    let seed_capability = format!(
        "{}/CAPS/{}0000/",
        region.server_uri.trim_end_matches('/'),
        caps_path
    );

    // Build home field
    let (home_loc_x, home_loc_y) = if let Some(gu) = &griduser {
        if !gu.home_region_id.is_empty() && gu.home_region_id != ZERO_UUID {
            match state
                .grid_store
                .get_region_by_uuid(&account.scope_id, &gu.home_region_id)
                .await
            {
                Ok(Some(hr)) => (hr.loc_x, hr.loc_y),
                _ => (region.loc_x, region.loc_y),
            }
        } else {
            (region.loc_x, region.loc_y)
        }
    } else {
        (region.loc_x, region.loc_y)
    };

    let (px, py, pz) = griduser
        .as_ref()
        .map(|g| parse_position(&g.home_position))
        .unwrap_or((128.0, 128.0, 70.0));
    let (lx, ly, lz) = griduser
        .as_ref()
        .map(|g| parse_position(&g.home_look_at))
        .unwrap_or((0.0, 0.0, 0.0));

    let home_field = format!(
        "{{'region_handle':[r{},r{}], 'position':[r{},r{},r{}], 'look_at':[r{},r{},r{}]}}",
        home_loc_x, home_loc_y, px, py, pz, lx, ly, lz
    );

    // Build login-flags
    let login_flags_members = format!(
        "{}{}{}{}",
        xml_str("daylight_savings", "N"),
        xml_str("stipend_since_login", "N"),
        xml_str("gendered", "Y"),
        xml_str("ever_logged_in", "Y")
    );
    let login_flags_member =
        xml_array_of_structs("login-flags", &xml_struct(&login_flags_members));

    // Build global-textures
    let global_textures_members = format!(
        "{}{}{}",
        xml_str(
            "sun_texture_id",
            "cce0f112-878f-4586-a2e2-a8f104bba271"
        ),
        xml_str(
            "cloud_texture_id",
            "dc4b9f0b-d008-45c6-96a4-01dd947ac621"
        ),
        xml_str(
            "moon_texture_id",
            "ec4b9f0b-d008-45c6-96a4-01dd947ac621"
        )
    );
    let global_textures_member =
        xml_array_of_structs("global-textures", &xml_struct(&global_textures_members));

    // Build ui-config
    let ui_config_member = xml_array_of_structs(
        "ui-config",
        &xml_struct(&xml_str("allow_first_life", "Y")),
    );

    // Build classified_categories
    let classified_cats = [
        ("Shopping", 1),
        ("Land Rental", 2),
        ("Property Rental", 3),
        ("Special Attraction", 4),
        ("New Products", 5),
        ("Employment", 6),
        ("Wanted", 7),
        ("Service", 8),
        ("Personal", 9),
    ];
    let mut classified_items = String::new();
    for (name, id) in classified_cats {
        let members = format!("{}{}", xml_str("category_name", name), xml_i4("category_id", id));
        classified_items.push_str(&xml_struct(&members));
    }
    let classified_member = xml_array_of_structs("classified_categories", &classified_items);

    // Build initial-outfit
    let outfit_members = format!(
        "{}{}",
        xml_str("folder_name", "Nightclub Female"),
        xml_str("gender", "female")
    );
    let initial_outfit_member =
        xml_array_of_structs("initial-outfit", &xml_struct(&outfit_members));

    // Build inventory-skeleton
    let mut inventory_structs = String::new();
    for folder in &folders {
        let folder_members = format!(
            "{}{}{}{}{}",
            xml_str("name", &folder.folder_name),
            xml_str("parent_id", &folder.parent_folder_id),
            xml_i4("version", folder.version),
            xml_i4("type_default", folder.folder_type as i32),
            xml_str("folder_id", &folder.folder_id)
        );
        inventory_structs.push_str(&xml_struct(&folder_members));
    }
    let inventory_skeleton_member =
        xml_array_of_structs("inventory-skeleton", &inventory_structs);

    // Build inventory-root
    let root_folder_id = root_folder.map(|f| f.folder_id).unwrap_or_default();
    let inventory_root_member = xml_array_of_structs(
        "inventory-root",
        &xml_struct(&xml_str("folder_id", &root_folder_id)),
    );

    // Library stubs
    let inventory_lib_root_member = xml_array_of_structs(
        "inventory-lib-root",
        &xml_struct(&xml_str(
            "folder_id",
            "00000112-000f-0000-0000-000100bba000",
        )),
    );
    let inventory_lib_owner_member = xml_array_of_structs(
        "inventory-lib-owner",
        &xml_struct(&xml_str(
            "agent_id",
            "11111111-1111-0000-0000-000100bba000",
        )),
    );

    // Build gestures
    let mut gesture_structs = String::new();
    for gesture in &gestures {
        let gesture_members = format!(
            "{}{}",
            xml_str("item_id", &gesture.inventory_id),
            xml_str("asset_id", &gesture.asset_id)
        );
        gesture_structs.push_str(&xml_struct(&gesture_members));
    }
    let gestures_member = xml_array_of_structs("gestures", &gesture_structs);

    // Build buddy-list
    let mut buddy_structs = String::new();
    for friend in &friends {
        if friend.their_flags != "-1" {
            let buddy_id = friend.friend.split(';').next().unwrap_or("").trim();
            let has: i32 = friend.their_flags.parse().unwrap_or(0);
            let given: i32 = friend.my_flags.parse().unwrap_or(0);
            let buddy_members = format!(
                "{}{}{}",
                xml_str("buddy_id", buddy_id),
                xml_i4("buddy_rights_has", has),
                xml_i4("buddy_rights_given", given)
            );
            buddy_structs.push_str(&xml_struct(&buddy_members));
        }
    }
    let buddy_list_member = xml_array_of_structs("buddy-list", &buddy_structs);

    // Assemble full response
    let mut members = String::new();
    members.push_str(&xml_str("login", "true"));
    members.push_str(&xml_str("first_name", &account.first_name));
    members.push_str(&xml_str("last_name", &account.last_name));
    members.push_str(&xml_str("agent_id", &principal_id));
    members.push_str(&xml_str("session_id", &session_id));
    members.push_str(&xml_str("secure_session_id", &secure_session_id));
    members.push_str(&xml_i4("circuit_code", circuit_code));
    members.push_str(&xml_str("sim_ip", &region.server_ip));
    members.push_str(&xml_i4("sim_port", region.server_port.unwrap_or(0)));
    members.push_str(&xml_i4("http_port", region.server_http_port.unwrap_or(0)));
    members.push_str(&xml_i4("region_x", region.loc_x));
    members.push_str(&xml_i4("region_y", region.loc_y));
    members.push_str(&xml_i4("region_size_x", region.size_x));
    members.push_str(&xml_i4("region_size_y", region.size_y));
    members.push_str(&xml_str("seed_capability", &seed_capability));
    members.push_str(&xml_i4("seconds_since_epoch", seconds_since_epoch));
    members.push_str(&xml_str("agent_access", "M"));
    members.push_str(&xml_str("agent_access_max", "A"));
    members.push_str(&xml_str("start_location", start_location));
    members.push_str(&xml_str("message", ""));
    members.push_str(&xml_i4("max-agent-groups", 42));
    members.push_str(&xml_str("look_at", "[r0,r0,r0]"));
    members.push_str(&xml_str("home", &home_field));
    members.push_str(&login_flags_member);
    members.push_str(&global_textures_member);
    members.push_str(&ui_config_member);
    members.push_str(&xml_array_of_structs("event_categories", ""));
    members.push_str(&xml_array_of_structs("event_notifications", ""));
    members.push_str(&classified_member);
    members.push_str(&initial_outfit_member);
    members.push_str(&inventory_skeleton_member);
    members.push_str(&inventory_root_member);
    members.push_str(&xml_array_of_structs("inventory-skel-lib", ""));
    members.push_str(&inventory_lib_root_member);
    members.push_str(&inventory_lib_owner_member);
    members.push_str(&gestures_member);
    members.push_str(&buddy_list_member);

    tracing::info!(
        principal = %principal_id,
        region = %region.region_name,
        "login successful"
    );

    let body = wrap_response(&members);
    (
        StatusCode::OK,
        [("content-type", "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

fn parse_xml_rpc(input: &str) -> Result<(String, HashMap<String, String>), String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut method = String::new();
    let mut params = HashMap::new();

    let mut in_method = false;
    let mut in_name = false;
    let mut in_value = false;
    let mut current_name = String::new();
    let mut current_value = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = e.name();
                let local = local.as_ref();
                if local == b"methodName" {
                    in_method = true;
                } else if local == b"name" {
                    in_name = true;
                    current_name.clear();
                } else if local == b"value" {
                    in_value = true;
                    current_value.clear();
                }
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).to_string());
                if in_method {
                    method.push_str(text.trim());
                } else if in_name {
                    current_name.push_str(text.trim());
                } else if in_value {
                    current_value.push_str(&text);
                }
            }
            Ok(Event::CData(t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).to_string();
                if in_method {
                    method.push_str(text.trim());
                } else if in_name {
                    current_name.push_str(text.trim());
                } else if in_value {
                    current_value.push_str(&text);
                }
            }
            Ok(Event::End(e)) => {
                let local = e.name();
                let local = local.as_ref();
                if local == b"methodName" {
                    in_method = false;
                } else if local == b"name" {
                    in_name = false;
                } else if local == b"value" {
                    in_value = false;
                    if !current_name.is_empty() {
                        params.insert(current_name.clone(), current_value.clone());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
    }

    if method.is_empty() {
        return Err("missing method name".to_string());
    }
    Ok((method, params))
}

fn parse_position(s: &str) -> (f64, f64, f64) {
    let trimmed = s
        .trim()
        .trim_matches(|c: char| c == '<' || c == '>' || c.is_whitespace());
    let parts: Vec<&str> = trimmed.split(',').collect();
    let x = parts
        .first()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0);
    let y = parts
        .get(1)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0);
    let z = parts
        .get(2)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0);
    (x, y, z)
}

fn md5_hex(input: &str) -> String {
    format!("{:x}", md5::compute(input.as_bytes()))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_str(name: &str, value: &str) -> String {
    format!(
        "<member><name>{}</name><value><string>{}</string></value></member>",
        xml_escape(name),
        xml_escape(value)
    )
}

fn xml_i4(name: &str, value: i32) -> String {
    format!(
        "<member><name>{}</name><value><i4>{}</i4></value></member>",
        xml_escape(name),
        value
    )
}

fn xml_struct(members: &str) -> String {
    format!("<value><struct>{}</struct></value>", members)
}

fn xml_array_of_structs(name: &str, structs_xml: &str) -> String {
    format!(
        "<member><name>{}</name><value><array><data>{}</data></array></value></member>",
        xml_escape(name),
        structs_xml
    )
}

fn wrap_response(members: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>{}</struct></value></param></params></methodResponse>",
        members
    )
}

fn error_response(reason: &str, message: &str) -> Response {
    let members = format!(
        "{}{}{}",
        xml_str("login", "false"),
        xml_str("reason", reason),
        xml_str("message", message)
    );
    let body = wrap_response(&members);
    (
        StatusCode::OK,
        [("content-type", "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn parse_llsd_login(input: &str) -> Result<HashMap<String, String>, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut params = HashMap::new();
    let mut current_key = String::new();
    let mut in_key = false;
    let mut in_value = false;
    let mut depth = 0u32;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = e.name();
                let local = local.as_ref();
                match local {
                    b"map" => depth += 1,
                    b"key" if depth == 1 => {
                        in_key = true;
                        current_key.clear();
                    }
                    b"string" | b"integer" | b"boolean" if depth == 1 => {
                        in_value = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).to_string());
                if in_key {
                    current_key.push_str(&text);
                } else if in_value {
                    params.insert(current_key.clone(), text);
                    in_value = false;
                }
            }
            Ok(Event::End(e)) => {
                let local = e.name();
                let local = local.as_ref();
                match local {
                    b"map" => {
                        if depth > 0 {
                            depth -= 1;
                        }
                    }
                    b"key" => in_key = false,
                    b"string" | b"integer" | b"boolean" => in_value = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("LLSD parse error: {e}")),
            _ => {}
        }
    }

    if params.is_empty() {
        return Err("no parameters found in LLSD body".into());
    }
    Ok(params)
}

async fn xmlrpc_response_to_llsd(response: Response) -> Response {
    use http_body_util::BodyExt;
    let (parts, body) = response.into_parts();
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("content-type", "application/llsd+xml")],
                "<llsd><map><key>login</key><string>false</string><key>message</key><string>Internal error</string></map></llsd>",
            ).into_response();
        }
    };
    let xmlrpc_body = String::from_utf8_lossy(&bytes);
    let llsd_body = convert_xmlrpc_to_llsd(&xmlrpc_body);
    let mut resp = (
        parts.status,
        [("content-type", "application/llsd+xml")],
        llsd_body,
    ).into_response();
    resp
}

fn convert_xmlrpc_to_llsd(xmlrpc: &str) -> String {
    let mut reader = Reader::from_str(xmlrpc);
    reader.config_mut().trim_text(true);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\" ?><llsd>");
    let mut skip_depth = 0u32;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = e.name();
                let tag = tag.as_ref();
                match tag {
                    b"methodResponse" | b"params" | b"param" => { skip_depth += 1; }
                    b"struct" => out.push_str("<map>"),
                    b"member" => {}
                    b"name" => out.push_str("<key>"),
                    b"value" => {}
                    b"string" => out.push_str("<string>"),
                    b"i4" | b"int" => out.push_str("<integer>"),
                    b"boolean" => out.push_str("<boolean>"),
                    b"double" => out.push_str("<real>"),
                    b"array" => out.push_str("<array>"),
                    b"data" => {}
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let tag = e.name();
                let tag = tag.as_ref();
                match tag {
                    b"methodResponse" | b"params" | b"param" => { skip_depth -= 1; }
                    b"struct" => out.push_str("</map>"),
                    b"member" => {}
                    b"name" => out.push_str("</key>"),
                    b"value" => {}
                    b"string" => out.push_str("</string>"),
                    b"i4" | b"int" => out.push_str("</integer>"),
                    b"boolean" => out.push_str("</boolean>"),
                    b"double" => out.push_str("</real>"),
                    b"array" => out.push_str("</array>"),
                    b"data" => {}
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).to_string());
                out.push_str(&xml_escape(&text));
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    out.push_str("</llsd>");
    out
}
