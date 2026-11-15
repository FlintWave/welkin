// SPDX-License-Identifier: GPL-3.0-or-later

//! Async asset gathering for OAR archive operations.
//!
//! Ported from `OpenSim.Region.CoreModules.World.Archiver.AssetsRequest`.

use std::collections::HashSet;
use std::time::Duration;

use welkin_types::AssetType;
use uuid::Uuid;

/// Data for a single asset to be included in (or loaded from) an archive.
#[derive(Debug, Clone)]
pub struct AssetData {
    /// Unique asset identifier.
    pub uuid: Uuid,
    /// The type of this asset.
    pub asset_type: AssetType,
    /// Raw asset data bytes.
    pub data: Vec<u8>,
}

/// A trait for fetching assets by UUID.
///
/// Implementors connect to the actual asset service (database, HTTP, etc.).
#[allow(async_fn_in_trait)]
pub trait AssetService: Send + Sync {
    /// Fetch a single asset by its UUID.
    ///
    /// Returns `None` if the asset does not exist.
    async fn get_asset(&self, uuid: Uuid) -> Option<AssetData>;
}

/// Progress information emitted during asset gathering.
#[derive(Debug, Clone)]
pub struct AssetGatherProgress {
    /// Number of assets successfully fetched so far.
    pub fetched: usize,
    /// Number of assets that could not be found.
    pub not_found: usize,
    /// Number of assets that timed out.
    pub timed_out: usize,
    /// Total number of unique asset UUIDs to fetch.
    pub total: usize,
}

/// Gathers unique assets from a set of UUIDs, deduplicating and fetching
/// them through an [`AssetService`].
#[derive(Debug, Clone)]
pub struct AssetGatherer {
    /// UUIDs that need to be fetched, with their expected asset type.
    uuids: Vec<(Uuid, AssetType)>,
    /// Already-seen UUIDs for deduplication.
    seen: HashSet<Uuid>,
    /// Per-asset fetch timeout.
    timeout: Duration,
}

impl AssetGatherer {
    /// Create a new gatherer with the given per-asset timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            uuids: Vec::new(),
            seen: HashSet::new(),
            timeout,
        }
    }

    /// Add an asset UUID to the gather list. Duplicates are silently ignored.
    pub fn add_asset(&mut self, uuid: Uuid, asset_type: AssetType) {
        if uuid.is_nil() {
            return;
        }
        if self.seen.insert(uuid) {
            self.uuids.push((uuid, asset_type));
        }
    }

    /// Add multiple asset UUIDs at once.
    pub fn add_assets(&mut self, assets: &[(Uuid, AssetType)]) {
        for &(uuid, asset_type) in assets {
            self.add_asset(uuid, asset_type);
        }
    }

    /// Return the number of unique assets queued.
    pub fn len(&self) -> usize {
        self.uuids.len()
    }

    /// Whether the gather list is empty.
    pub fn is_empty(&self) -> bool {
        self.uuids.is_empty()
    }

    /// Return the set of unique UUIDs.
    pub fn uuids(&self) -> &[(Uuid, AssetType)] {
        &self.uuids
    }

    /// Fetch all queued assets through the given service.
    ///
    /// Returns the successfully fetched assets, along with lists of
    /// not-found and timed-out UUIDs. The operation never aborts on a
    /// single failure; all assets are attempted.
    ///
    /// An optional progress callback is invoked after each asset fetch.
    pub async fn gather_all<S, F>(
        &self,
        service: &S,
        mut progress: Option<F>,
    ) -> GatherResult
    where
        S: AssetService,
        F: FnMut(&AssetGatherProgress),
    {
        let mut result = GatherResult {
            assets: Vec::with_capacity(self.uuids.len()),
            not_found: Vec::new(),
            timed_out: Vec::new(),
        };

        let total = self.uuids.len();

        for &(uuid, _expected_type) in &self.uuids {
            match tokio::time::timeout(self.timeout, service.get_asset(uuid)).await {
                Ok(Some(asset)) => {
                    result.assets.push(asset);
                }
                Ok(None) => {
                    tracing::warn!("asset not found: {uuid}");
                    result.not_found.push(uuid);
                }
                Err(_elapsed) => {
                    tracing::warn!("asset fetch timed out: {uuid}");
                    result.timed_out.push(uuid);
                }
            }

            if let Some(ref mut cb) = progress {
                cb(&AssetGatherProgress {
                    fetched: result.assets.len(),
                    not_found: result.not_found.len(),
                    timed_out: result.timed_out.len(),
                    total,
                });
            }
        }

        result
    }
}

/// The result of a [`AssetGatherer::gather_all`] call.
#[derive(Debug, Clone)]
pub struct GatherResult {
    /// Assets that were successfully fetched.
    pub assets: Vec<AssetData>,
    /// UUIDs that were not found.
    pub not_found: Vec<Uuid>,
    /// UUIDs whose fetch timed out.
    pub timed_out: Vec<Uuid>,
}

impl GatherResult {
    /// Whether the gather completed without any failures.
    pub fn is_complete(&self) -> bool {
        self.not_found.is_empty() && self.timed_out.is_empty()
    }

    /// Total number of failures (not-found + timed-out).
    pub fn failure_count(&self) -> usize {
        self.not_found.len() + self.timed_out.len()
    }
}

/// Synchronous (non-async) asset gathering for environments without a
/// tokio runtime. This simply collects UUIDs for later use.
#[derive(Debug, Clone)]
pub struct SyncAssetCollector {
    /// Unique asset UUIDs collected.
    seen: HashSet<Uuid>,
    /// Ordered list of (uuid, type) pairs.
    assets: Vec<(Uuid, AssetType)>,
}

impl SyncAssetCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
            assets: Vec::new(),
        }
    }

    /// Add an asset UUID. Duplicates and nil UUIDs are ignored.
    pub fn add(&mut self, uuid: Uuid, asset_type: AssetType) {
        if uuid.is_nil() {
            return;
        }
        if self.seen.insert(uuid) {
            self.assets.push((uuid, asset_type));
        }
    }

    /// Return the number of unique assets collected.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Whether the collector is empty.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Consume the collector and return the unique (uuid, type) pairs.
    pub fn into_assets(self) -> Vec<(Uuid, AssetType)> {
        self.assets
    }

    /// Return whether the given UUID has already been seen.
    pub fn contains(&self, uuid: &Uuid) -> bool {
        self.seen.contains(uuid)
    }

    /// Convert into an async [`AssetGatherer`] for fetching.
    pub fn into_gatherer(self, timeout: Duration) -> AssetGatherer {
        let mut g = AssetGatherer::new(timeout);
        g.uuids = self.assets;
        g.seen = self.seen;
        g
    }
}

impl Default for SyncAssetCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockAssetService {
        assets: Mutex<HashMap<Uuid, AssetData>>,
    }

    impl MockAssetService {
        fn new() -> Self {
            Self {
                assets: Mutex::new(HashMap::new()),
            }
        }

        fn insert(&self, uuid: Uuid, asset_type: AssetType, data: Vec<u8>) {
            self.assets.lock().unwrap().insert(
                uuid,
                AssetData {
                    uuid,
                    asset_type,
                    data,
                },
            );
        }
    }

    impl AssetService for MockAssetService {
        async fn get_asset(&self, uuid: Uuid) -> Option<AssetData> {
            self.assets.lock().unwrap().get(&uuid).cloned()
        }
    }

    #[tokio::test]
    async fn gather_all_fetches_assets() {
        let svc = MockAssetService::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        svc.insert(uuid1, AssetType::Texture, vec![1, 2, 3]);
        svc.insert(uuid2, AssetType::Sound, vec![4, 5, 6]);

        let mut gatherer = AssetGatherer::new(Duration::from_secs(5));
        gatherer.add_asset(uuid1, AssetType::Texture);
        gatherer.add_asset(uuid2, AssetType::Sound);

        let result = gatherer
            .gather_all(&svc, None::<fn(&AssetGatherProgress)>)
            .await;

        assert_eq!(result.assets.len(), 2);
        assert!(result.not_found.is_empty());
        assert!(result.timed_out.is_empty());
        assert!(result.is_complete());
    }

    #[tokio::test]
    async fn gather_reports_not_found() {
        let svc = MockAssetService::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        svc.insert(uuid1, AssetType::Texture, vec![1, 2, 3]);
        // uuid2 is not inserted

        let mut gatherer = AssetGatherer::new(Duration::from_secs(5));
        gatherer.add_asset(uuid1, AssetType::Texture);
        gatherer.add_asset(uuid2, AssetType::Sound);

        let result = gatherer
            .gather_all(&svc, None::<fn(&AssetGatherProgress)>)
            .await;

        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.not_found.len(), 1);
        assert_eq!(result.not_found[0], uuid2);
        assert!(!result.is_complete());
        assert_eq!(result.failure_count(), 1);
    }

    #[test]
    fn deduplication() {
        let mut gatherer = AssetGatherer::new(Duration::from_secs(5));
        let uuid = Uuid::new_v4();
        gatherer.add_asset(uuid, AssetType::Texture);
        gatherer.add_asset(uuid, AssetType::Texture);
        gatherer.add_asset(uuid, AssetType::Sound); // same UUID, different type — still deduped
        assert_eq!(gatherer.len(), 1);
    }

    #[test]
    fn nil_uuid_ignored() {
        let mut gatherer = AssetGatherer::new(Duration::from_secs(5));
        gatherer.add_asset(Uuid::nil(), AssetType::Texture);
        assert!(gatherer.is_empty());
    }

    #[test]
    fn sync_collector_deduplicates() {
        let mut collector = SyncAssetCollector::new();
        let uuid = Uuid::new_v4();
        collector.add(uuid, AssetType::Texture);
        collector.add(uuid, AssetType::Texture);
        collector.add(Uuid::nil(), AssetType::Sound);
        assert_eq!(collector.len(), 1);
        assert!(collector.contains(&uuid));
        assert!(!collector.contains(&Uuid::nil()));
    }

    #[test]
    fn sync_collector_into_gatherer() {
        let mut collector = SyncAssetCollector::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        collector.add(uuid1, AssetType::Texture);
        collector.add(uuid2, AssetType::Sound);
        let gatherer = collector.into_gatherer(Duration::from_secs(10));
        assert_eq!(gatherer.len(), 2);
    }

    #[tokio::test]
    async fn progress_callback_invoked() {
        let svc = MockAssetService::new();
        let uuid1 = Uuid::new_v4();
        svc.insert(uuid1, AssetType::Texture, vec![1]);

        let mut gatherer = AssetGatherer::new(Duration::from_secs(5));
        gatherer.add_asset(uuid1, AssetType::Texture);

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = gatherer
            .gather_all(&svc, Some(move |_progress: &AssetGatherProgress| {
                cc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }))
            .await;

        assert_eq!(result.assets.len(), 1);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
