mod handlers;
mod pbr_material;

use actix_web::{App, HttpServer};
use handlers::render_materials::{config as render_materials_cfg, AppState};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Shared in‑memory store for materials
    let material_store: std::collections::HashMap<Uuid, pbr_material::Material> = HashMap::new();

    let app_state = AppState {
        materials: actix_web::web::Data::new(Mutex::new(material_store)),
    };

    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::Data::new(app_state.clone()))
            .configure(render_materials_cfg)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}