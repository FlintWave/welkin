// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;
use uuid::Uuid;
use welkin_data::assets;

pub fn parse_nini(xml: &str) -> Vec<HashMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut sections = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => match element.name().as_ref() {
                b"Section" => {
                    let mut map = HashMap::new();
                    if let Some(name) = attr_value(&element, b"Name") {
                        map.insert("__section".to_string(), name);
                    }
                    current = Some(map);
                }
                b"Key" => {
                    if let Some(map) = &mut current {
                        if let Some(key) = attr_value(&element, b"Name") {
                            let value = attr_value(&element, b"Value").unwrap_or_default();
                            map.insert(key, value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(element)) => match element.name().as_ref() {
                b"Section" => {
                    let mut map = HashMap::new();
                    if let Some(name) = attr_value(&element, b"Name") {
                        map.insert("__section".to_string(), name);
                    }
                    sections.push(map);
                }
                b"Key" => {
                    if let Some(map) = &mut current {
                        if let Some(key) = attr_value(&element, b"Name") {
                            let value = attr_value(&element, b"Value").unwrap_or_default();
                            map.insert(key, value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(element)) => {
                if element.name().as_ref() == b"Section" {
                    if let Some(map) = current.take() {
                        sections.push(map);
                    }
                }
            }
            Ok(Event::Eof) => {
                if let Some(map) = current.take() {
                    sections.push(map);
                }
                break;
            }
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    sections
}

fn attr_value(
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Option<String> {
    for attribute in element.attributes().flatten() {
        if attribute.key.as_ref() == name {
            return attribute.unescape_value().ok().map(|value| value.into_owned());
        }
    }
    None
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LoadReport {
    pub sets_loaded: usize,
    pub assets_stored: usize,
    pub assets_existing: usize,
    pub files_missing: usize,
    pub errors: Vec<String>,
}

pub async fn load_asset_sets(
    store: &assets::AssetStore,
    control_file: &Path,
) -> LoadReport {
    let mut report = LoadReport::default();

    let control_xml = match fs::read_to_string(control_file) {
        Ok(contents) => contents,
        Err(error) => {
            report
                .errors
                .push(format!("unable to read control file {:?}: {}", control_file, error));
            info!(
                sets_loaded = report.sets_loaded,
                assets_stored = report.assets_stored,
                assets_existing = report.assets_existing,
                files_missing = report.files_missing,
                errors = report.errors.len(),
                "asset sets load complete"
            );
            return report;
        }
    };

    let control_dir = control_file.parent().unwrap_or_else(|| Path::new("."));

    for control_section in parse_nini(&control_xml) {
        let file = control_section
            .get("file")
            .map(|value| value.trim())
            .unwrap_or("");
        if file.is_empty() {
            continue;
        }

        let set_path = control_dir.join(file);
        let set_xml = match fs::read_to_string(&set_path) {
            Ok(contents) => contents,
            Err(error) => {
                report
                    .errors
                    .push(format!("unable to read asset set file {:?}: {}", set_path, error));
                continue;
            }
        };
        report.sets_loaded += 1;

        let set_dir = set_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

        for asset_section in parse_nini(&set_xml) {
            let raw_id = match asset_section.get("assetID") {
                Some(value) => value,
                None => continue,
            };
            let uuid = match Uuid::parse_str(raw_id) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let id = uuid.to_string();

            let name: String = asset_section
                .get("name")
                .cloned()
                .unwrap_or_default()
                .chars()
                .take(64)
                .collect();
            let description: String = asset_section
                .get("description")
                .cloned()
                .unwrap_or_default()
                .chars()
                .take(64)
                .collect();
            let asset_type: i32 = asset_section
                .get("assetType")
                .and_then(|value| value.trim().parse::<i32>().ok())
                .unwrap_or(0);

            let file_name = asset_section.get("fileName").cloned().unwrap_or_default();
            let data = if file_name.is_empty() {
                Vec::new()
            } else {
                match fs::read(set_dir.join(&file_name)) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        report.files_missing += 1;
                        continue;
                    }
                }
            };

            match store.exists(&id).await {
                Ok(true) => {
                    report.assets_existing += 1;
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    report
                        .errors
                        .push(format!("failed to check asset {}: {}", id, error));
                    continue;
                }
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i32)
                .unwrap_or(0);

            let asset = assets::AssetRow {
                name,
                description,
                asset_type,
                local: 0,
                temporary: 0,
                data,
                id,
                create_time: now,
                access_time: now,
                asset_flags: 0,
                creator_id: "00000000-0000-0000-0000-000000000000".to_string(),
            };

            match store.store(&asset).await {
                Ok(()) => report.assets_stored += 1,
                Err(error) => report.errors.push(format!("failed to store asset {}: {}", asset.id, error)),
            }
        }
    }

    info!(
        sets_loaded = report.sets_loaded,
        assets_stored = report.assets_stored,
        assets_existing = report.assets_existing,
        files_missing = report.files_missing,
        errors = report.errors.len(),
        "asset sets load complete"
    );

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::AnyPool;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::atomic::AtomicU64 as _AtomicU64;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: std::path::PathBuf,
        sqlite: std::path::PathBuf,
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
            let _ = std::fs::remove_file(&self.sqlite);
        }
    }

    #[test]
    fn parse_nini_ignores_comments_and_returns_section_names() {
        let xml = r#"<Nini>
            <!-- <Section Name="Ignored"><Key Name="file" Value="nope.xml"/></Section> -->
            <Section Name="A"><Key Name="file" Value="a.xml"/></Section>
            <Section Name="B"><Key Name="key" Value="value"/></Section>
        </Nini>"#;
        let sections = parse_nini(xml);
        let names: Vec<String> = sections
            .iter()
            .map(|section| {
                section
                    .get("__section")
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(names, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(
            sections[1].get("key"),
            Some(&"value".to_string())
        );
    }

    #[tokio::test]
    async fn loads_default_asset_sets_idempotently() {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let base = std::env::temp_dir();
        let dir = base.join(format!("welkin-asset-loader-{}-{}", pid, counter));
        std::fs::create_dir_all(&dir).unwrap();
        let sqlite = base.join(format!("welkin-asset-loader-{}-{}.sqlite", pid, counter));
        let guard = TestDir {
            path: dir.clone(),
            sqlite: sqlite.clone(),
        };

        let asset_uuid = "11111111-2222-3333-4444-555555555555";
        let second_uuid = "22222222-3333-4444-5555-666666666666";
        let third_uuid = "33333333-4444-5555-6666-777777777777";

        std::fs::write(dir.join("data.dat"), [1u8, 2, 3]).unwrap();

        let set_xml = format!(
            r#"<Nini>
                <!-- <Section Name="Ignored"><Key Name="assetID" Value="99999999-9999-9999-9999-999999999999"/><Key Name="fileName" Value="ghost.dat"/></Section> -->
                <Section Name="dat">
                    <Key Name="assetID" Value="{asset_uuid}"/>
                    <Key Name="name" Value="Dat Asset"/>
                    <Key Name="description" Value="Has bytes"/>
                    <Key Name="assetType" Value="1"/>
                    <Key Name="fileName" Value="data.dat"/>
                </Section>
                <Section Name="missing">
                    <Key Name="assetID" Value="{second_uuid}"/>
                    <Key Name="name" Value="Missing File"/>
                    <Key Name="assetType" Value="2"/>
                    <Key Name="fileName" Value="does-not-exist.dat"/>
                </Section>
                <Section Name="invalid">
                    <Key Name="assetID" Value="not-a-uuid"/>
                    <Key Name="name" Value="Invalid"/>
                    <Key Name="fileName" Value=""/>
                </Section>
                <Section Name="empty">
                    <Key Name="assetID" Value="{third_uuid}"/>
                    <Key Name="name" Value="Empty Data"/>
                    <Key Name="assetType" Value="3"/>
                </Section>
            </Nini>"#
        );
        std::fs::write(dir.join("ValidSet.xml"), set_xml).unwrap();

        let control_xml = r#"<Nini>
            <Section Name="valid"><Key Name="file" Value="ValidSet.xml"/></Section>
            <Section Name="missing-set"><Key Name="file" Value="MissingSet.xml"/></Section>
            <!-- <Section Name="commented"><Key Name="file" Value="Commented.xml"/></Section> -->
        </Nini>"#;
        let control_file = dir.join("AssetSets.xml");
        std::fs::write(&control_file, control_xml).unwrap();

        sqlx::any::install_default_drivers();
        let url = format!("sqlite:{}?mode=rwc", sqlite.display());
        let pool: AnyPool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&url)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS assets(id VARCHAR(36) NOT NULL PRIMARY KEY, name VARCHAR(64), description VARCHAR(64), assetType INTEGER NOT NULL, local INTEGER NOT NULL, temporary INTEGER NOT NULL, data BLOB, create_time INTEGER, access_time INTEGER, asset_flags INTEGER NOT NULL DEFAULT 0, CreatorID VARCHAR(128) DEFAULT '')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let store = assets::AssetStore::new(pool.clone());

        let report = load_asset_sets(&store, &control_file).await;
        assert_eq!(report.sets_loaded, 1);
        assert_eq!(report.assets_stored, 2);
        assert_eq!(report.files_missing, 1);
        assert_eq!(report.errors.len(), 1);

        assert!(store.exists(asset_uuid).await.unwrap());
        assert!(store.exists(third_uuid).await.unwrap());

        let second_report = load_asset_sets(&store, &control_file).await;
        assert_eq!(second_report.assets_stored, 0);
        assert_eq!(second_report.assets_existing, 2);

        drop(guard);
    }

    #[tokio::test]
    async fn missing_control_file_reports_one_error() {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let base = std::env::temp_dir();
        let dir = base.join(format!("welkin-asset-loader-{}-{}", pid, counter));
        std::fs::create_dir_all(&dir).unwrap();
        let sqlite = base.join(format!("welkin-asset-loader-{}-{}.sqlite", pid, counter));
        let _guard = TestDir {
            path: dir.clone(),
            sqlite: sqlite.clone(),
        };

        sqlx::any::install_default_drivers();
        let url = format!("sqlite:{}?mode=rwc", sqlite.display());
        let pool: AnyPool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect(&url)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS assets(id VARCHAR(36) NOT NULL PRIMARY KEY, name VARCHAR(64), description VARCHAR(64), assetType INTEGER NOT NULL, local INTEGER NOT NULL, temporary INTEGER NOT NULL, data BLOB, create_time INTEGER, access_time INTEGER, asset_flags INTEGER NOT NULL DEFAULT 0, CreatorID VARCHAR(128) DEFAULT '')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let store = assets::AssetStore::new(pool);

        let missing_control = dir.join("does-not-exist.xml");
        let report = load_asset_sets(&store, &missing_control).await;
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.sets_loaded, 0);
    }

    // Keep an unused alias referenced to avoid warnings in minimal builds.
    #[allow(dead_code)]
    fn _use_atomic_alias(_value: _AtomicU64) {}
}
