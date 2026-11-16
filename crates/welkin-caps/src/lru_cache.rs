// SPDX-License-Identifier: LGPL-3.0-or-later

//! In-process LRU cache keyed by an arbitrary key (typically a `Uuid`
//! texture/asset request).
//!
//! Provides hit/miss/eviction counters for hit-rate reporting and explicit
//! invalidation for stale entries.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub invalidations: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

struct Inner<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    order: VecDeque<K>,
    stats: CacheStats,
}

pub struct LruCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    inner: Mutex<Inner<K, V>>,
}

impl<K, V> LruCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            inner: Mutex::new(Inner {
                capacity: cap,
                map: HashMap::with_capacity(cap),
                order: VecDeque::with_capacity(cap),
                stats: CacheStats::default(),
            }),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut inner = self.inner.lock().expect("lru cache poisoned");
        if let Some(value) = inner.map.get(key).cloned() {
            inner.order.retain(|k| k != key);
            inner.order.push_back(key.clone());
            inner.stats.hits += 1;
            Some(value)
        } else {
            inner.stats.misses += 1;
            None
        }
    }

    pub fn put(&self, key: K, value: V) {
        let mut inner = self.inner.lock().expect("lru cache poisoned");
        inner.stats.inserts += 1;
        if inner.map.contains_key(&key) {
            inner.map.insert(key.clone(), value);
            inner.order.retain(|k| k != &key);
            inner.order.push_back(key);
            Self::evict_locked(&mut inner);
            return;
        }
        inner.map.insert(key.clone(), value);
        inner.order.push_back(key);
        Self::evict_locked(&mut inner);
    }

    pub fn invalidate(&self, key: &K) -> bool {
        let mut inner = self.inner.lock().expect("lru cache poisoned");
        if inner.map.remove(key).is_some() {
            inner.order.retain(|k| k != key);
            inner.stats.invalidations += 1;
            true
        } else {
            false
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().expect("lru cache poisoned");
        inner.map.clear();
        inner.order.clear();
    }

    pub fn contains(&self, key: &K) -> bool {
        let inner = self.inner.lock().expect("lru cache poisoned");
        inner.map.contains_key(key)
    }

    pub fn len(&self) -> usize {
        let inner = self.inner.lock().expect("lru cache poisoned");
        inner.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn stats(&self) -> CacheStats {
        let inner = self.inner.lock().expect("lru cache poisoned");
        inner.stats.clone()
    }

    pub fn capacity(&self) -> usize {
        let inner = self.inner.lock().expect("lru cache poisoned");
        inner.capacity
    }

    fn evict_locked(inner: &mut Inner<K, V>) {
        while inner.map.len() > inner.capacity {
            if let Some(oldest) = inner.order.pop_front() {
                if inner.map.remove(&oldest).is_some() {
                    inner.stats.evictions += 1;
                }
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_and_miss_stats() {
        let cache = LruCache::<i32, i32>::new(4);
        cache.put(1, 10);
        assert_eq!(cache.get(&1), Some(10));
        assert_eq!(cache.get(&2), None);
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!(cache.stats().hit_rate() > 0.4 && cache.stats().hit_rate() < 0.6);
    }

    #[test]
    fn invalidation() {
        let cache = LruCache::<i32, i32>::new(4);
        cache.put(1, 10);
        assert!(cache.invalidate(&1));
        assert_eq!(cache.get(&1), None);
        assert!(!cache.invalidate(&1));
        assert_eq!(cache.stats().invalidations, 1);
    }

    #[test]
    fn eviction_respects_capacity() {
        let cache = LruCache::<i32, i32>::new(2);
        cache.put(1, 1);
        cache.put(2, 2);
        cache.put(3, 3);
        assert!(!cache.contains(&1));
        assert!(cache.contains(&2));
        assert!(cache.contains(&3));
        assert!(cache.stats().evictions >= 1);
    }

    #[test]
    fn lru_order_promotes_on_get() {
        let cache = LruCache::<i32, i32>::new(2);
        cache.put(1, 1);
        cache.put(2, 2);
        assert_eq!(cache.get(&1), Some(1));
        cache.put(3, 3);
        assert!(cache.contains(&1));
        assert!(!cache.contains(&2));
    }
}