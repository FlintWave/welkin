use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

// Reuse the existing PBR material definition
use crate::pbr_material::Material;

// Application state holding the materials mapped by object ID
#[derive(Clone)]
pub struct AppState {
    pub materials: web::Data<Mutex<std::collections::HashMap<Uuid, Material>>>,
}

// GET /render_materials
// Returns all materials for objects currently in view.
pub async fn get_render_materials(state: web::Data<AppState>) -> impl Responder {
    let materials_guard = state.materials.lock().unwrap();
    let all_materials: Vec<Material> = materials_guard.values().cloned().collect();
    HttpResponse::Ok().json(all_materials)
}

// Expected payload for PUT /render_materials
#[derive(Debug, Deserialize, Serialize)]
pub struct MaterialUpdate {
    pub object_id: Uuid,
    pub material: Material,
}

// PUT /render_materials
// Stores/updates materials for objects.
pub async fn put_render_materials(
    state: web::Data<AppState>,
    payload: web::Json<Vec<MaterialUpdate>>,
) -> impl Responder {
    let mut materials_guard = state.materials.lock().unwrap();

    for update in payload.iter() {
        materials_guard.insert(update.object_id, update.material.clone());
    }

    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

// Register routes (to be called from main or config)
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/render_materials")
            .route(web::get().to(get_render_materials))
            .route(web::put().to(put_render_materials)),
    );
}