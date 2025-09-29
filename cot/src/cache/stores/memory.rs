//! In-memory cache store implementation.
//!
//! This module provides a simple thread-safe, process-local cache store that
//! implements the generic [`CacheStore`] trait. It is primarily intended for
//! development, testing, and low-concurrency scenarios where a shared in-memory
//! map is sufficient.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use cot::cache::stores::{CacheStore, CacheStoreError, CacheStoreResult, Expiry};
use thiserror::Error;
use tokio::sync::Mutex;

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

/// A simple in-memory cache backed by a `Mutex<HashMap<..>>`.
pub struct Memory<K, V> {
    map: Arc<Mutex<HashMap<K, (V, Option<Expiry>)>>>,
}

impl<K, V> Memory<K, V> {
    /// Create a new, empty `Memory` cache store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl<K, V> CacheStore<K, V> for Memory<K, V>
where
    K: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
    V: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Get a value by key.
    async fn get(&self, key: &K) -> CacheStoreResult<Option<V>> {
        let map = self.map.lock().await;
        let value = map.get(key).map(|(v, _)| v.clone());
        Ok(value)
    }

    /// Insert a value without expiry.
    async fn insert(&self, key: K, value: V) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.insert(key, (value, None));
        Ok(())
    }

    /// Insert a lazily computed value without expiry.
    async fn insert_with<F, Fut>(&self, key: K, f: F) -> CacheStoreResult<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<V>> + Send,
    {
        let mut map = self.map.lock().await;
        map.insert(key, (f().await?, None));
        Ok(())
    }

    /// Insert a value with the provided expiry policy.
    async fn insert_expiring(&self, key: K, value: V, expiry: Expiry) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.insert(key, (value, Some(expiry)));
        Ok(())
    }

    /// Get the value for `key`, or compute, insert, and return it without
    /// expiry.
    async fn get_or_insert_with<F, Fut>(&self, key: K, f: F) -> CacheStoreResult<V>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<V>> + Send,
    {
        let mut map = self.map.lock().await;
        if let Some(value) = map.get(&key) {
            return Ok(value.0.clone());
        }
        let value = f().await?;
        map.insert(key, (value.clone(), None));
        Ok(value)
    }

    /// Get the value for `key`, or compute and insert it with the provided
    /// expiry.
    async fn get_or_insert_expiring_with<F, Fut>(
        &self,
        key: K,
        f: F,
        expiry: Expiry,
    ) -> CacheStoreResult<V>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<V>> + Send,
    {
        let mut map = self.map.lock().await;
        if let Some((value, _existing_expiry)) = map.get(&key) {
            return Ok(value.clone());
        }

        let value = f().await?;
        map.insert(key, (value.clone(), Some(expiry)));
        Ok(value)
    }

    /// Remove a value by key.
    async fn remove(&self, key: &K) -> CacheStoreResult<()> {
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
    async fn contains_key(&self, key: &K) -> CacheStoreResult<bool> {
        let map = self.map.lock().await;
        Ok(map.contains_key(key))
    }
}
