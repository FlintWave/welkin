// SPDX-License-Identifier: GPL-3.0-or-later

//! Local LRU asset cache with filesystem backing store.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use opensim_data::assets::AssetRow;
use tokio::sync::Mutex;

struct CacheEntry {
    asset: AssetRow,
    cached_at: Instant,
    size: usize,
}

struct CacheInner {
    entries: HashMap<String, CacheEntry>,
    order: VecDeque<String>,
    current_size: usize,
}

pub struct AssetCache {
    inner: Mutex<CacheInner>,
    ttl: Duration,
    max_size: usize,
    cache_dir: Option<PathBuf>,
}

impl AssetCache {
    pub fn new(max_size: usize, ttl: Duration, cache_dir: Option<PathBuf>) -> Self {
        if let Some(ref dir) = cache_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        Self {
            inner: Mutex::new(CacheInner {
                entries: HashMap::new(),
                order: VecDeque::new(),
                current_size: 0,
            }),
            ttl,
            max_size,
            cache_dir,
        }
    }

    pub async fn get(&self, id: &str) -> Option<AssetRow> {
        let mut inner = self.inner.lock().await;

        // Check in-memory cache.
        if let Some(entry) = inner.entries.get(id) {
            if entry.cached_at.elapsed() < self.ttl {
                let asset = entry.asset.clone();
                // Move to back of LRU order.
                if let Some(pos) = inner.order.iter().position(|k| k == id) {
                    inner.order.remove(pos);
                }
                inner.order.push_back(id.to_string());
                return Some(asset);
            }
            // Expired — remove it.
            let size = entry.size;
            inner.entries.remove(id);
            inner.current_size -= size;
            if let Some(pos) = inner.order.iter().position(|k| k == id) {
                inner.order.remove(pos);
            }
        }

        // Try disk cache.
        if let Some(ref dir) = self.cache_dir {
            let path = dir.join(format!("{}.bin", id));
            if path.exists() {
                // Check file age via mtime.
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(age) = SystemTime::now().duration_since(modified) {
                            if age < self.ttl {
                                // Read and deserialize.
                                if let Ok(bytes) = tokio::fs::read(&path).await {
                                    if let Some(asset) = deserialize_asset(id, &bytes) {
                                        let size = asset.data.len();
                                        // Insert into memory cache.
                                        inner.entries.insert(
                                            id.to_string(),
                                            CacheEntry {
                                                asset: asset.clone(),
                                                cached_at: Instant::now(),
                                                size,
                                            },
                                        );
                                        inner.order.push_back(id.to_string());
                                        inner.current_size += size;
                                        evict_if_needed(&mut inner, self.max_size);
                                        return Some(asset);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Drop lock before returning.
        drop(inner);
        None
    }

    pub async fn put(&self, asset: AssetRow) {
        let mut inner = self.inner.lock().await;
        let id = asset.id.clone();
        let size = asset.data.len();

        // Remove old entry if present.
        if let Some(old) = inner.entries.remove(&id) {
            inner.current_size -= old.size;
            if let Some(pos) = inner.order.iter().position(|k| k == &id) {
                inner.order.remove(pos);
            }
        }

        inner.entries.insert(
            id.clone(),
            CacheEntry {
                asset: asset.clone(),
                cached_at: Instant::now(),
                size,
            },
        );
        inner.order.push_back(id.clone());
        inner.current_size += size;

        evict_if_needed(&mut inner, self.max_size);

        // Write to disk in background.
        if let Some(ref dir) = self.cache_dir {
            let path = dir.join(format!("{}.bin", &id));
            let bytes = serialize_asset(&asset);
            tokio::spawn(async move {
                if let Err(e) = tokio::fs::write(&path, &bytes).await {
                    tracing::warn!("failed to write cache file {}: {}", path.display(), e);
                }
            });
        }
    }

    pub async fn contains(&self, id: &str) -> bool {
        let inner = self.inner.lock().await;

        // Check memory.
        if let Some(entry) = inner.entries.get(id) {
            if entry.cached_at.elapsed() < self.ttl {
                return true;
            }
        }

        // Check disk.
        if let Some(ref dir) = self.cache_dir {
            let path = dir.join(format!("{}.bin", id));
            if path.exists() {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(age) = SystemTime::now().duration_since(modified) {
                            return age < self.ttl;
                        }
                    }
                }
            }
        }

        false
    }

    pub async fn remove(&self, id: &str) {
        let mut inner = self.inner.lock().await;
        if let Some(entry) = inner.entries.remove(id) {
            inner.current_size -= entry.size;
            if let Some(pos) = inner.order.iter().position(|k| k == id) {
                inner.order.remove(pos);
            }
        }

        if let Some(ref dir) = self.cache_dir {
            let path = dir.join(format!("{}.bin", id));
            if let Err(e) = tokio::fs::remove_file(&path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("failed to remove cache file {}: {}", path.display(), e);
                }
            }
        }
    }

    pub async fn clear(&self) {
        let mut inner = self.inner.lock().await;
        inner.entries.clear();
        inner.order.clear();
        inner.current_size = 0;

        if let Some(ref dir) = self.cache_dir {
            let dir = dir.clone();
            // Best-effort removal of disk files.
            if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("bin") {
                        let _ = tokio::fs::remove_file(&path).await;
                    }
                }
            }
        }
    }
}

/// Evict oldest entries until `current_size <= max_size`.
fn evict_if_needed(inner: &mut CacheInner, max_size: usize) {
    while inner.current_size > max_size {
        if let Some(oldest_id) = inner.order.pop_front() {
            if let Some(entry) = inner.entries.remove(&oldest_id) {
                inner.current_size -= entry.size;
            }
        } else {
            break;
        }
    }
}

fn serialize_asset(asset: &AssetRow) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&asset.asset_type.to_le_bytes());
    buf.extend_from_slice(&asset.local.to_le_bytes());
    buf.extend_from_slice(&asset.temporary.to_le_bytes());
    buf.extend_from_slice(&asset.asset_flags.to_le_bytes());
    buf.extend_from_slice(&asset.create_time.to_le_bytes());
    buf.extend_from_slice(&asset.access_time.to_le_bytes());

    let name = asset.name.as_bytes();
    buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
    buf.extend_from_slice(name);

    let desc = asset.description.as_bytes();
    buf.extend_from_slice(&(desc.len() as u32).to_le_bytes());
    buf.extend_from_slice(desc);

    let creator = asset.creator_id.as_bytes();
    buf.extend_from_slice(&(creator.len() as u32).to_le_bytes());
    buf.extend_from_slice(creator);

    let data = &asset.data;
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);

    buf
}

fn deserialize_asset(id: &str, bytes: &[u8]) -> Option<AssetRow> {
    let mut pos = 0;

    fn read_i32(bytes: &[u8], pos: &mut usize) -> Option<i32> {
        if *pos + 4 > bytes.len() {
            return None;
        }
        let val = i32::from_le_bytes(bytes[*pos..*pos + 4].try_into().ok()?);
        *pos += 4;
        Some(val)
    }

    fn read_bytes(bytes: &[u8], pos: &mut usize) -> Option<Vec<u8>> {
        if *pos + 4 > bytes.len() {
            return None;
        }
        let len = u32::from_le_bytes(bytes[*pos..*pos + 4].try_into().ok()?) as usize;
        *pos += 4;
        if *pos + len > bytes.len() {
            return None;
        }
        let data = bytes[*pos..*pos + len].to_vec();
        *pos += len;
        Some(data)
    }

    fn read_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
        let raw = read_bytes(bytes, pos)?;
        String::from_utf8(raw).ok()
    }

    let asset_type = read_i32(bytes, &mut pos)?;
    let local = read_i32(bytes, &mut pos)?;
    let temporary = read_i32(bytes, &mut pos)?;
    let asset_flags = read_i32(bytes, &mut pos)?;
    let create_time = read_i32(bytes, &mut pos)?;
    let access_time = read_i32(bytes, &mut pos)?;
    let name = read_string(bytes, &mut pos)?;
    let description = read_string(bytes, &mut pos)?;
    let creator_id = read_string(bytes, &mut pos)?;
    let data = read_bytes(bytes, &mut pos)?;

    Some(AssetRow {
        name,
        description,
        asset_type,
        local,
        temporary,
        data,
        id: id.to_string(),
        create_time,
        access_time,
        asset_flags,
        creator_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_asset(id: &str, data_size: usize) -> AssetRow {
        AssetRow {
            name: format!("test-{}", id),
            description: format!("desc-{}", id),
            asset_type: 0,
            local: 0,
            temporary: 0,
            data: vec![0xAB; data_size],
            id: id.to_string(),
            create_time: 1000,
            access_time: 2000,
            asset_flags: 0,
            creator_id: "00000000-0000-0000-0000-000000000000".to_string(),
        }
    }

    #[tokio::test]
    async fn test_cache_put_get() {
        let cache = AssetCache::new(1_000_000, Duration::from_secs(60), None);
        let asset = make_test_asset("asset-1", 64);
        cache.put(asset.clone()).await;

        let got = cache.get("asset-1").await.expect("asset should be cached");
        assert_eq!(got.id, "asset-1");
        assert_eq!(got.name, "test-asset-1");
        assert_eq!(got.description, "desc-asset-1");
        assert_eq!(got.data.len(), 64);
        assert_eq!(got.creator_id, asset.creator_id);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = AssetCache::new(1_000_000, Duration::from_secs(60), None);
        assert!(cache.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        // Max 100 bytes total.
        let cache = AssetCache::new(100, Duration::from_secs(60), None);
        let a = make_test_asset("a", 80);
        let b = make_test_asset("b", 80);

        cache.put(a).await;
        assert!(cache.get("a").await.is_some());

        cache.put(b).await;
        // "a" should have been evicted to make room for "b".
        assert!(cache.get("a").await.is_none());
        assert!(cache.get("b").await.is_some());
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let cache = AssetCache::new(1_000_000, Duration::ZERO, None);
        let asset = make_test_asset("expire-me", 32);
        cache.put(asset).await;

        // TTL is zero, so the entry is already expired.
        assert!(cache.get("expire-me").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_disk_persistence() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let dir = tmp.path().to_path_buf();

        // Put an asset in the first cache.
        {
            let cache = AssetCache::new(1_000_000, Duration::from_secs(60), Some(dir.clone()));
            let asset = make_test_asset("disk-1", 128);
            cache.put(asset).await;
            // Give the background write task time to complete.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Create a new cache pointing at the same directory.
        let cache2 = AssetCache::new(1_000_000, Duration::from_secs(60), Some(dir));
        let got = cache2.get("disk-1").await.expect("asset should load from disk");
        assert_eq!(got.id, "disk-1");
        assert_eq!(got.name, "test-disk-1");
        assert_eq!(got.data.len(), 128);
    }
}
