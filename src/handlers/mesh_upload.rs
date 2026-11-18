use actix_web::{post, web, HttpResponse, HttpRequest, http::header::CONTENT_TYPE};
use futures::StreamExt;
use std::sync::Arc;
use crate::storage::mesh::MeshStorage;
use crate::error::Result;

/// Existing mesh content MIME types
const MESH_CONTENT_TYPES: &[&str] = &[
    "application/octet-stream", // original binary mesh format
    "model/obj",
    "model/fbx",
];

/// Additional glTF / GLB MIME types
const GLTF_CONTENT_TYPES: &[&str] = &[
    "model/gltf+json",
    "model/gltf-binary",
    "model/glb",
    "application/gltf+json",
    "application/gltf-binary",
    "application/octet-stream", // keep for generic binary uploads (including .glb fallback)
];

/// Combine all supported content types for mesh uploads
fn is_supported_mesh_content_type(content_type: &str) -> bool {
    MESH_CONTENT_TYPES.iter().any(|&t| t.eq_ignore_ascii_case(content_type))
        || GLTF_CONTENT_TYPES.iter().any(|&t| t.eq_ignore_ascii_case(content_type))
}

#[post("/mesh")]
pub async fn upload_mesh(
    req: HttpRequest,
    mut payload: web::Payload,
    storage: web::Data<Arc<MeshStorage>>,
) -> Result<HttpResponse> {
    // Verify content type
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    if !is_supported_mesh_content_type(content_type) {
        return Err(crate::error::Error::InvalidContentType(
            content_type.to_string(),
        ));
    }

    // Collect the whole binary payload
    let mut bytes = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        bytes.extend_from_slice(&chunk);
    }

    // Delegate to the mesh storage; the storage implementation already knows how to
    // parse or forward the raw bytes to the appropriate PBR material loader.
    let mesh_id = storage.store_mesh(bytes.freeze()).await?;

    Ok(HttpResponse::Created()
        .append_header(("Location", format!("/mesh/{}", mesh_id)))
        .finish())
}