// SPDX-License-Identifier: LGPL-3.0-or-later

use std::time::{SystemTime, UNIX_EPOCH};

use tracing::info;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct OarReport {
    pub assets_stored: usize,
    pub assets_existing: usize,
    pub assets_failed: usize,
    pub objects_added: usize,
    pub objects_failed: usize,
    pub terrain_applied: bool,
    pub warnings: Vec<String>,
}

pub fn decode_r32_terrain(bytes: &[u8], width: usize, height: usize) -> Option<Vec<f32>> {
    let expected = width.checked_mul(height)?.checked_mul(4)?;
    if bytes.len() != expected {
        return None;
    }

    let mut out = Vec::with_capacity(width * height);
    for chunk in bytes.chunks_exact(4) {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(chunk);
        let value = f32::from_le_bytes(raw);
        out.push(if value.is_finite() { value } else { 0.0 });
    }
    Some(out)
}

pub async fn load_oar(
    path: &std::path::Path,
    scene: &welkin_scene::scene::Scene,
    assets: &welkin_services::remote::asset::RemoteAssetConnector,
) -> Result<OarReport, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let parsed = tokio::task::spawn_blocking(move || {
        welkin_archive::oar_read::read_archive(
            std::io::BufReader::new(file),
            &welkin_archive::oar_read::OarLoadOptions::default(),
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    let mut contents = parsed.map_err(|e| e.to_string())?;

    if contents.regions.is_empty() {
        return Err("archive contains no region data".to_string());
    }

    let region_key = if contents.regions.contains_key("") {
        String::new()
    } else {
        match contents.regions.keys().min() {
            Some(key) => key.clone(),
            None => return Err("archive contains no region data".to_string()),
        }
    };

    let region_contents = match contents.regions.remove(&region_key) {
        Some(region) => region,
        None => return Err("archive contains no region data".to_string()),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i32)
        .unwrap_or_default();

    let mut report = OarReport::default();

    for asset in contents.assets {
        let id = asset.uuid.to_string();

        if let Ok(true) = assets.asset_exists(&id).await {
            report.assets_existing += 1;
            continue;
        }

        let row = welkin_data::assets::AssetRow {
            name: id.chars().take(64).collect(),
            description: String::new(),
            asset_type: asset.asset_type as i8 as i32,
            local: 0,
            temporary: 0,
            data: asset.data,
            id: id.clone(),
            create_time: now,
            access_time: now,
            asset_flags: 0,
            creator_id: "00000000-0000-0000-0000-000000000000".to_string(),
        };

        match assets.store_asset(&row).await {
            Ok(()) => report.assets_stored += 1,
            Err(error) => {
                report.assets_failed += 1;
                if report.warnings.len() < 5 {
                    report.warnings.push(error.to_string());
                }
            }
        }
    }

    if let Some(terrain_bytes) = &region_contents.terrain {
        if let Some(filename) = region_contents.terrain_filename.as_deref() {
            if filename.to_ascii_lowercase().ends_with(".r32") {
                if let Some(heights) = decode_r32_terrain(terrain_bytes, 256, 256) {
                    scene.set_terrain(&heights);
                    report.terrain_applied = true;
                } else if report.warnings.len() < 5 {
                    report.warnings.push(format!("terrain data invalid size: {filename}"));
                }
            } else if report.warnings.len() < 5 {
                report.warnings.push(format!("terrain format {filename} not supported"));
            }
        } else if report.warnings.len() < 5 {
            report.warnings.push("terrain format unknown not supported".to_string());
        }
    }

    for xml in region_contents.objects_xml {
        match welkin_scene::xml2::deserialize_group(&xml) {
            Ok(group) => {
                let _ = scene.add_object(group);
                report.objects_added += 1;
            }
            Err(error) => {
                report.objects_failed += 1;
                if report.warnings.len() < 5 {
                    report.warnings.push(error.to_string());
                }
            }
        }
    }

    info!(report = ?report, "oar load complete");
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_r32_wrong_length_returns_none() {
        assert!(decode_r32_terrain(&[], 1, 1).is_none());
        assert!(decode_r32_terrain(&[0u8; 15], 2, 2).is_none());
    }

    #[test]
    fn decode_r32_sample_replaces_nan() {
        let mut bytes = Vec::new();
        for value in [1.0f32, 2.0, 3.0, f32::NAN] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            decode_r32_terrain(&bytes, 2, 2),
            Some(vec![1.0, 2.0, 3.0, 0.0])
        );
    }

    #[test]
    fn oar_report_default_is_all_zero() {
        assert_eq!(
            OarReport::default(),
            OarReport {
                assets_stored: 0,
                assets_existing: 0,
                assets_failed: 0,
                objects_added: 0,
                objects_failed: 0,
                terrain_applied: false,
                warnings: Vec::new(),
            }
        );
    }
}
