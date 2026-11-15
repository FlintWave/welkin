// SPDX-License-Identifier: LGPL-3.0-or-later
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Deserialize;
use serde_json::{Value, json};
use welkin_data::profiles::{ClassifiedRow, PickRow, ProfileRow, ProfilesStore, UserSettingsRow};

#[derive(Clone)]
pub struct ProfilesServiceState {
    pub store: Arc<ProfilesStore>,
}

pub fn router(state: ProfilesServiceState) -> Router {
    Router::new()
        .route("/json_rpc", post(handle_json_rpc))
        .with_state(state)
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Value,
    method: String,
    params: Value,
}

fn json_response(body: Value) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        body.to_string(),
    )
        .into_response()
}

fn success(id: &Value, result: Value) -> Response {
    json_response(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
}

fn error(id: &Value, message: &str) -> Response {
    json_response(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32603,
            "message": message,
        },
    }))
}

fn str_param<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(Value::as_str).unwrap_or("")
}

fn int_param(params: &Value, key: &str) -> i32 {
    params
        .get(key)
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(0) as i32
}

fn bool_param(params: &Value, key: &str) -> bool {
    params
        .get(key)
        .map(|v| match v {
            Value::Bool(b) => *b,
            Value::String(s) => s.eq_ignore_ascii_case("true"),
            Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
            _ => false,
        })
        .unwrap_or(false)
}

fn bool_to_binary(b: bool) -> Vec<u8> {
    if b { vec![1] } else { vec![0] }
}

fn binary_to_bool(v: &[u8]) -> bool {
    v.first().copied().unwrap_or(0) != 0
}

fn classified_to_json(c: &ClassifiedRow) -> Value {
    json!({
        "ClassifiedId": c.classified_uuid,
        "CreatorId": c.creator_uuid,
        "CreationDate": c.creation_date,
        "ExpirationDate": c.expiration_date,
        "Category": c.category,
        "Name": c.name,
        "Description": c.description,
        "ParcelId": c.parcel_uuid,
        "ParentEstate": c.parent_estate,
        "SnapshotId": c.snapshot_uuid,
        "SimName": c.sim_name,
        "GlobalPos": c.pos_global,
        "ParcelName": c.parcel_name,
        "Flags": c.classified_flags,
        "Price": c.price_for_listing,
    })
}

fn pick_to_json(p: &PickRow) -> Value {
    json!({
        "PickId": p.pick_uuid,
        "CreatorId": p.creator_uuid,
        "TopPick": p.top_pick,
        "ParcelId": p.parcel_uuid,
        "Name": p.name,
        "Desc": p.description,
        "SnapshotId": p.snapshot_uuid,
        "User": p.user,
        "OriginalName": p.original_name,
        "SimName": p.sim_name,
        "GlobalPos": p.pos_global,
        "SortOrder": p.sort_order,
        "Enabled": p.enabled,
        "Gatekeeper": p.gatekeeper.as_deref().unwrap_or(""),
    })
}

fn profile_to_json(p: &ProfileRow) -> Value {
    json!({
        "UserId": p.user_uuid,
        "Partner": p.profile_partner,
        "PublishProfile": binary_to_bool(&p.profile_allow_publish),
        "PublishMature": binary_to_bool(&p.profile_mature_publish),
        "WebUrl": p.profile_url,
        "WantToMask": p.profile_want_to_mask,
        "WantToText": p.profile_want_to_text,
        "SkillsMask": p.profile_skills_mask,
        "SkillsText": p.profile_skills_text,
        "Language": p.profile_languages,
        "Image": p.profile_image,
        "AboutText": p.profile_about_text,
        "FirstLifeImage": p.profile_first_image,
        "FirstLifeAboutText": p.profile_first_text,
    })
}

fn settings_to_json(s: &UserSettingsRow) -> Value {
    json!({
        "UserId": s.user_uuid,
        "IMViaEmail": s.im_via_email,
        "Visible": s.visible,
        "EMail": s.email,
    })
}

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

fn default_profile(user_uuid: &str) -> ProfileRow {
    ProfileRow {
        user_uuid: user_uuid.to_string(),
        profile_partner: ZERO_UUID.to_string(),
        profile_allow_publish: vec![0],
        profile_mature_publish: vec![0],
        profile_url: String::new(),
        profile_want_to_mask: 0,
        profile_want_to_text: String::new(),
        profile_skills_mask: 0,
        profile_skills_text: String::new(),
        profile_languages: String::new(),
        profile_image: ZERO_UUID.to_string(),
        profile_about_text: String::new(),
        profile_first_image: ZERO_UUID.to_string(),
        profile_first_text: String::new(),
    }
}

fn default_settings(user_uuid: &str) -> UserSettingsRow {
    UserSettingsRow {
        user_uuid: user_uuid.to_string(),
        im_via_email: "false".to_string(),
        visible: "true".to_string(),
        email: String::new(),
    }
}

async fn handle_json_rpc(
    State(state): State<ProfilesServiceState>,
    body: axum::body::Bytes,
) -> Response {
    let req: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "invalid JSON-RPC request");
            return json_response(json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "Parse error" },
            }));
        }
    };

    let store = &state.store;
    let id = &req.id;
    let params = &req.params;
    tracing::debug!(method = %req.method, "profiles json-rpc request");

    match req.method.as_str() {
        "avatarclassifiedsrequest" => {
            let creator = str_param(params, "creatorId");
            match store.get_classifieds(creator).await {
                Ok(rows) => {
                    let arr: Vec<Value> = rows
                        .iter()
                        .map(|c| json!({"classifiedId": c.classified_uuid, "name": c.name}))
                        .collect();
                    success(id, Value::Array(arr))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "avatarclassifiedsrequest failed");
                    error(id, "Failed to get classifieds")
                }
            }
        }
        "classified_update" => {
            let row = ClassifiedRow {
                classified_uuid: str_param(params, "ClassifiedId").to_string(),
                creator_uuid: str_param(params, "CreatorId").to_string(),
                creation_date: int_param(params, "CreationDate"),
                expiration_date: int_param(params, "ExpirationDate"),
                category: str_param(params, "Category").to_string(),
                name: str_param(params, "Name").to_string(),
                description: str_param(params, "Description").to_string(),
                parcel_uuid: str_param(params, "ParcelId").to_string(),
                parent_estate: int_param(params, "ParentEstate"),
                snapshot_uuid: str_param(params, "SnapshotId").to_string(),
                sim_name: str_param(params, "SimName").to_string(),
                pos_global: str_param(params, "GlobalPos").to_string(),
                parcel_name: str_param(params, "ParcelName").to_string(),
                classified_flags: int_param(params, "Flags"),
                price_for_listing: int_param(params, "Price"),
            };
            match store.store_classified(&row).await {
                Ok(()) => success(id, classified_to_json(&row)),
                Err(e) => {
                    tracing::warn!(error = %e, "classified_update failed");
                    error(id, "Failed to update classified")
                }
            }
        }
        "classifieds_info_query" => {
            let cid = str_param(params, "classifiedId");
            if cid.is_empty() {
                let cid2 = str_param(params, "ClassifiedId");
                match store.get_classified(cid2).await {
                    Ok(Some(c)) => success(id, classified_to_json(&c)),
                    Ok(None) => error(id, "Classified not found"),
                    Err(e) => {
                        tracing::warn!(error = %e, "classifieds_info_query failed");
                        error(id, "Failed to get classified")
                    }
                }
            } else {
                match store.get_classified(cid).await {
                    Ok(Some(c)) => success(id, classified_to_json(&c)),
                    Ok(None) => error(id, "Classified not found"),
                    Err(e) => {
                        tracing::warn!(error = %e, "classifieds_info_query failed");
                        error(id, "Failed to get classified")
                    }
                }
            }
        }
        "classified_delete" => {
            let cid = str_param(params, "classifiedId");
            match store.delete_classified(cid).await {
                Ok(_) => success(id, Value::Bool(true)),
                Err(e) => {
                    tracing::warn!(error = %e, "classified_delete failed");
                    error(id, "Failed to delete classified")
                }
            }
        }
        "avatarpicksrequest" => {
            let creator = str_param(params, "creatorId");
            match store.get_picks(creator).await {
                Ok(rows) => {
                    let arr: Vec<Value> = rows
                        .iter()
                        .map(|p| json!({"pickId": p.pick_uuid, "name": p.name}))
                        .collect();
                    success(id, Value::Array(arr))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "avatarpicksrequest failed");
                    error(id, "Failed to get picks")
                }
            }
        }
        "pickinforequest" => {
            let pid = str_param(params, "pickId");
            if pid.is_empty() {
                let pid2 = str_param(params, "PickId");
                match store.get_pick(pid2).await {
                    Ok(Some(p)) => success(id, pick_to_json(&p)),
                    Ok(None) => error(id, "Pick not found"),
                    Err(e) => {
                        tracing::warn!(error = %e, "pickinforequest failed");
                        error(id, "Failed to get pick")
                    }
                }
            } else {
                match store.get_pick(pid).await {
                    Ok(Some(p)) => success(id, pick_to_json(&p)),
                    Ok(None) => error(id, "Pick not found"),
                    Err(e) => {
                        tracing::warn!(error = %e, "pickinforequest failed");
                        error(id, "Failed to get pick")
                    }
                }
            }
        }
        "picks_update" => {
            let row = PickRow {
                pick_uuid: str_param(params, "PickId").to_string(),
                creator_uuid: str_param(params, "CreatorId").to_string(),
                top_pick: if bool_param(params, "TopPick") {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
                parcel_uuid: str_param(params, "ParcelId").to_string(),
                name: str_param(params, "Name").to_string(),
                description: str_param(params, "Desc").to_string(),
                snapshot_uuid: str_param(params, "SnapshotId").to_string(),
                user: str_param(params, "User").to_string(),
                original_name: str_param(params, "OriginalName").to_string(),
                sim_name: str_param(params, "SimName").to_string(),
                pos_global: str_param(params, "GlobalPos").to_string(),
                sort_order: int_param(params, "SortOrder"),
                enabled: if bool_param(params, "Enabled") {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
                gatekeeper: Some(str_param(params, "Gatekeeper").to_string()),
            };
            match store.store_pick(&row).await {
                Ok(()) => success(id, pick_to_json(&row)),
                Err(e) => {
                    tracing::warn!(error = %e, "picks_update failed");
                    error(id, "Failed to update pick")
                }
            }
        }
        "picks_delete" => {
            let pid = str_param(params, "pickId");
            match store.delete_pick(pid).await {
                Ok(_) => success(id, Value::Bool(true)),
                Err(e) => {
                    tracing::warn!(error = %e, "picks_delete failed");
                    error(id, "Failed to delete pick")
                }
            }
        }
        "avatarnotesrequest" => {
            let user = str_param(params, "UserId");
            let target = str_param(params, "TargetId");
            match store.get_note(user, target).await {
                Ok(Some(n)) => success(
                    id,
                    json!({
                        "UserId": n.user_uuid,
                        "TargetId": n.target_uuid,
                        "Notes": n.notes,
                    }),
                ),
                Ok(None) => success(
                    id,
                    json!({
                        "UserId": user,
                        "TargetId": target,
                        "Notes": "",
                    }),
                ),
                Err(e) => {
                    tracing::warn!(error = %e, "avatarnotesrequest failed");
                    error(id, "Failed to get notes")
                }
            }
        }
        "avatar_notes_update" => {
            let user = str_param(params, "UserId");
            let target = str_param(params, "TargetId");
            let notes = str_param(params, "Notes");
            match store.store_note(user, target, notes).await {
                Ok(()) => success(
                    id,
                    json!({
                        "UserId": user,
                        "TargetId": target,
                        "Notes": notes,
                    }),
                ),
                Err(e) => {
                    tracing::warn!(error = %e, "avatar_notes_update failed");
                    error(id, "Failed to update notes")
                }
            }
        }
        "avatar_properties_request" => {
            let user = str_param(params, "UserId");
            let profile = match store.get_profile(user).await {
                Ok(Some(p)) => p,
                Ok(None) => default_profile(user),
                Err(e) => {
                    tracing::warn!(error = %e, "avatar_properties_request failed");
                    return error(id, "Failed to get profile");
                }
            };
            success(id, profile_to_json(&profile))
        }
        "avatar_properties_update" => {
            let user = str_param(params, "UserId");
            let existing = match store.get_profile(user).await {
                Ok(Some(p)) => p,
                Ok(None) => default_profile(user),
                Err(e) => {
                    tracing::warn!(error = %e, "avatar_properties_update get failed");
                    return error(id, "Failed to get profile");
                }
            };
            let updated = ProfileRow {
                user_uuid: user.to_string(),
                profile_partner: if params.get("Partner").is_some() {
                    str_param(params, "Partner").to_string()
                } else {
                    existing.profile_partner
                },
                profile_allow_publish: if params.get("PublishProfile").is_some() {
                    bool_to_binary(bool_param(params, "PublishProfile"))
                } else {
                    existing.profile_allow_publish
                },
                profile_mature_publish: if params.get("PublishMature").is_some() {
                    bool_to_binary(bool_param(params, "PublishMature"))
                } else {
                    existing.profile_mature_publish
                },
                profile_url: if params.get("WebUrl").is_some() {
                    str_param(params, "WebUrl").to_string()
                } else {
                    existing.profile_url
                },
                profile_want_to_mask: if params.get("WantToMask").is_some() {
                    int_param(params, "WantToMask")
                } else {
                    existing.profile_want_to_mask
                },
                profile_want_to_text: if params.get("WantToText").is_some() {
                    str_param(params, "WantToText").to_string()
                } else {
                    existing.profile_want_to_text
                },
                profile_skills_mask: if params.get("SkillsMask").is_some() {
                    int_param(params, "SkillsMask")
                } else {
                    existing.profile_skills_mask
                },
                profile_skills_text: if params.get("SkillsText").is_some() {
                    str_param(params, "SkillsText").to_string()
                } else {
                    existing.profile_skills_text
                },
                profile_languages: if params.get("Language").is_some() {
                    str_param(params, "Language").to_string()
                } else {
                    existing.profile_languages
                },
                profile_image: if params.get("Image").is_some() {
                    str_param(params, "Image").to_string()
                } else {
                    existing.profile_image
                },
                profile_about_text: if params.get("AboutText").is_some() {
                    str_param(params, "AboutText").to_string()
                } else {
                    existing.profile_about_text
                },
                profile_first_image: if params.get("FirstLifeImage").is_some() {
                    str_param(params, "FirstLifeImage").to_string()
                } else {
                    existing.profile_first_image
                },
                profile_first_text: if params.get("FirstLifeAboutText").is_some() {
                    str_param(params, "FirstLifeAboutText").to_string()
                } else {
                    existing.profile_first_text
                },
            };
            match store.store_profile(&updated).await {
                Ok(()) => success(id, profile_to_json(&updated)),
                Err(e) => {
                    tracing::warn!(error = %e, "avatar_properties_update failed");
                    error(id, "Failed to update profile")
                }
            }
        }
        "avatar_interests_update" => {
            let user = str_param(params, "UserId");
            let existing = match store.get_profile(user).await {
                Ok(Some(p)) => p,
                Ok(None) => default_profile(user),
                Err(e) => {
                    tracing::warn!(error = %e, "avatar_interests_update get failed");
                    return error(id, "Failed to get profile");
                }
            };
            let updated = ProfileRow {
                user_uuid: user.to_string(),
                profile_partner: existing.profile_partner,
                profile_allow_publish: existing.profile_allow_publish,
                profile_mature_publish: existing.profile_mature_publish,
                profile_url: existing.profile_url,
                profile_want_to_mask: int_param(params, "WantToMask"),
                profile_want_to_text: str_param(params, "WantToText").to_string(),
                profile_skills_mask: int_param(params, "SkillsMask"),
                profile_skills_text: str_param(params, "SkillsText").to_string(),
                profile_languages: str_param(params, "Language").to_string(),
                profile_image: existing.profile_image,
                profile_about_text: existing.profile_about_text,
                profile_first_image: existing.profile_first_image,
                profile_first_text: existing.profile_first_text,
            };
            match store.store_profile(&updated).await {
                Ok(()) => success(id, profile_to_json(&updated)),
                Err(e) => {
                    tracing::warn!(error = %e, "avatar_interests_update failed");
                    error(id, "Failed to update interests")
                }
            }
        }
        "user_preferences_request" => {
            let user = str_param(params, "UserId");
            let settings = match store.get_settings(user).await {
                Ok(Some(s)) => s,
                Ok(None) => default_settings(user),
                Err(e) => {
                    tracing::warn!(error = %e, "user_preferences_request failed");
                    return error(id, "Failed to get preferences");
                }
            };
            success(id, settings_to_json(&settings))
        }
        "user_preferences_update" => {
            let user = str_param(params, "UserId");
            let existing = match store.get_settings(user).await {
                Ok(Some(s)) => s,
                Ok(None) => default_settings(user),
                Err(e) => {
                    tracing::warn!(error = %e, "user_preferences_update get failed");
                    return error(id, "Failed to get preferences");
                }
            };
            let updated = UserSettingsRow {
                user_uuid: user.to_string(),
                im_via_email: if params.get("IMViaEmail").is_some() {
                    if bool_param(params, "IMViaEmail") {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                } else {
                    existing.im_via_email
                },
                visible: if params.get("Visible").is_some() {
                    if bool_param(params, "Visible") {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                } else {
                    existing.visible
                },
                email: if params.get("EMail").is_some() {
                    str_param(params, "EMail").to_string()
                } else {
                    existing.email
                },
            };
            match store.store_settings(&updated).await {
                Ok(()) => success(id, settings_to_json(&updated)),
                Err(e) => {
                    tracing::warn!(error = %e, "user_preferences_update failed");
                    error(id, "Failed to update preferences")
                }
            }
        }
        "image_assets_request" => {
            let avatar_id = str_param(params, "avatarId");
            let profile = match store.get_profile(avatar_id).await {
                Ok(Some(p)) => p,
                Ok(None) => default_profile(avatar_id),
                Err(e) => {
                    tracing::warn!(error = %e, "image_assets_request failed");
                    return error(id, "Failed to get profile");
                }
            };
            success(
                id,
                json!([
                    {"Type": "image", "Name": "Profile", "Value": profile.profile_image},
                    {"Type": "image", "Name": "FirstLife", "Value": profile.profile_first_image},
                ]),
            )
        }
        "user_data_request" => success(id, json!({"result": "success", "token": ""})),
        "user_data_update" => success(id, json!({"result": "success"})),
        _ => {
            tracing::warn!(method = %req.method, "unknown profiles json-rpc method");
            error(id, "Method not found")
        }
    }
}
