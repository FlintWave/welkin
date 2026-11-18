use std::collections::HashMap;
use std::sync::Mutex;

use super::eep::ViewerEnvironment;

#[derive(Debug)]
pub enum EnvironmentStoreError {
    LockPoisoned,
}

impl std::fmt::Display for EnvironmentStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvironmentStoreError::LockPoisoned => write!(f, "environment store lock poisoned"),
        }
    }
}

impl std::error::Error for EnvironmentStoreError {}

pub trait RegionEnvironmentStore {
    fn load(&self, region_id: &str) -> Result<Option<ViewerEnvironment>, EnvironmentStoreError>;
    fn save(&self, region_id: &str, environment: ViewerEnvironment) -> Result<(), EnvironmentStoreError>;
    fn delete(&self, region_id: &str) -> Result<(), EnvironmentStoreError>;
}

#[derive(Default)]
pub struct InMemoryRegionEnvironmentStore {
    pub environments: Mutex<HashMap<String, ViewerEnvironment>>,
}

impl InMemoryRegionEnvironmentStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RegionEnvironmentStore for InMemoryRegionEnvironmentStore {
    fn load(&self, region_id: &str) -> Result<Option<ViewerEnvironment>, EnvironmentStoreError> {
        let environments = self
            .environments
            .lock()
            .map_err(|_| EnvironmentStoreError::LockPoisoned)?;
        Ok(environments.get(region_id).cloned())
    }

    fn save(&self, region_id: &str, environment: ViewerEnvironment) -> Result<(), EnvironmentStoreError> {
        let mut environments = self
            .environments
            .lock()
            .map_err(|_| EnvironmentStoreError::LockPoisoned)?;
        environments.insert(region_id.to_owned(), environment);
        Ok(())
    }

    fn delete(&self, region_id: &str) -> Result<(), EnvironmentStoreError> {
        let mut environments = self
            .environments
            .lock()
            .map_err(|_| EnvironmentStoreError::LockPoisoned)?;
        environments.remove(region_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_region_returns_none() {
        let store = InMemoryRegionEnvironmentStore::default();
        assert!(store.load("missing-region").unwrap().is_none());
    }

    #[test]
    fn save_then_load_returns_saved_environment() {
        let store = InMemoryRegionEnvironmentStore::default();
        let environment = ViewerEnvironment::default();

        store.save("region-a", environment).unwrap();
        assert!(store.load("region-a").unwrap().is_some());
    }

    #[test]
    fn delete_removes_saved_environment() {
        let store = InMemoryRegionEnvironmentStore::default();

        store
            .save("region-a", ViewerEnvironment::default())
            .unwrap();
        store.delete("region-a").unwrap();

        assert!(store.load("region-a").unwrap().is_none());
    }
}
