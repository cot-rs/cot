//! In-memory cache store implementation.
//!
//! This module provides a simple thread-safe, process-local cache store that
//! implements the generic [`CacheStore`] trait. It is primarily intended for
//! development, testing, and low-concurrency scenarios where a shared in-memory
//! map is sufficient.

use std::collections::HashMap;
use std::sync::Arc;

use cot::cache::stores::{CacheStore, CacheStoreError, CacheStoreResult};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::config::Timeout;

/// Errors specific to the in-memory cache store.
#[derive(Debug, Error, Clone, Copy)]
pub enum MemoryCacheStoreError {
    /// The requested key was not found.
    #[error("key not found")]
    KeyNotFound,
}

impl From<MemoryCacheStoreError> for CacheStoreError {
    fn from(err: MemoryCacheStoreError) -> Self {
        CacheStoreError::Backend(err.to_string())
    }
}

type MemoryMap = HashMap<String, (serde_json::Value, Option<Timeout>)>;

/// A simple in-memory cache backed by a `Mutex<HashMap<..>>`.
pub struct Memory {
    map: Arc<Mutex<MemoryMap>>,
}

impl Memory {
    /// Create a new, empty `Memory` cache store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl CacheStore for Memory {
    type Key = String;
    type Value = serde_json::Value;
    /// Get a value by key.
    async fn get(&self, key: &Self::Key) -> CacheStoreResult<Option<Self::Value>> {
        let map = self.map.lock().await;
        let value = map.get(key).map(|(v, _)| v.clone());
        Ok(value)
    }

    /// Insert a value without expiry.
    async fn insert(
        &self,
        key: Self::Key,
        value: Self::Value,
        expiry: Timeout,
    ) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.insert(key, (value, Some(expiry)));
        Ok(())
    }

    /// Remove a value by key.
    async fn remove(&self, key: &Self::Key) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.remove(key);
        Ok(())
    }

    /// Clear all entries.
    async fn clear(&self) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.clear();
        Ok(())
    }

    /// Return the number of entries in the cache.
    async fn len(&self) -> CacheStoreResult<usize> {
        let map = self.map.lock().await;
        Ok(map.len())
    }

    /// Check if the cache is empty.
    async fn is_empty(&self) -> CacheStoreResult<bool> {
        let map = self.map.lock().await;
        Ok(map.is_empty())
    }

    /// Check if a given key is present.
    async fn contains_key(&self, key: &Self::Key) -> CacheStoreResult<bool> {
        let map = self.map.lock().await;
        Ok(map.contains_key(key))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::config::Timeout;

    #[cot::test]
    async fn test_insert_and_get() {
        let store = Memory::new();
        let key = "test_key".to_string();
        let value = json!({"data": 123});

        store.insert(key, value, Timeout::default()).await.unwrap();
        let retrieved = store.get(&"test_key".to_string()).await.unwrap();
        assert_eq!(retrieved, Some(json!({"data": 123})));
    }

    #[cot::test]
    async fn test_remove() {
        let store = Memory::new();
        let key = "test_key".to_string();
        let value = json!({"data": 123});

        store
            .insert(key.clone(), value, Timeout::default())
            .await
            .unwrap();
        store.remove(&key).await.unwrap();
        let retrieved = store.get(&key).await.unwrap();
        assert_eq!(retrieved, None);
    }

    #[cot::test]
    async fn test_clear() {
        let store = Memory::new();
        store
            .insert("key1".to_string(), json!(1), Timeout::default())
            .await
            .unwrap();
        store
            .insert("key2".to_string(), json!(2), Timeout::default())
            .await
            .unwrap();
        assert_eq!(store.len().await.unwrap(), 2);
        store.clear().await.unwrap();
        assert_eq!(store.len().await.unwrap(), 0);
    }

    #[cot::test]
    async fn test_contains_key() {
        let store = Memory::new();
        let key = "test_key".to_string();
        let value = json!({"data": 123});

        store
            .insert(key.clone(), value, Timeout::default())
            .await
            .unwrap();
        assert!(store.contains_key(&key).await.unwrap());
        store.remove(&key).await.unwrap();
        assert!(!store.contains_key(&key).await.unwrap());
    }
}
