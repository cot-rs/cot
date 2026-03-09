//! In-memory cache store implementation.
//!
//! This module provides a simple thread-safe, process-local cache store that
//! implements the generic [`CacheStore`] trait. It is primarily intended for
//! development, testing, and low-concurrency scenarios where a shared in-memory
//! map is sufficient.
//!
//! # Examples
//!
//! ```
//! # use cot::cache::store::memory::Memory;
//! # use cot::cache::store::CacheStore;
//! # use serde_json::json;
//! #
//! # #[tokio::main]
//! # async fn main() {
//! let store = Memory::new();
//! let key = "example_key".to_string();
//! let value = json!({"data": 42});
//!
//! store.insert(key.clone(), value, Default::default()).await.unwrap();
//! let retrieved = store.get(&key).await.unwrap();
//!
//! assert_eq!(retrieved, Some(json!({"data": 42})));
//! # }
//! ```
//!
//! # Expiration Policies
//!
//! Keys are only removed eagerly when accessed via `get` or `contains_key`.
//! There is no background task to clean up expired keys.

use std::collections::HashMap;
use std::sync::Arc;

use cot::cache::store::{CacheStore, CacheStoreError, CacheStoreResult};
use serde_json::Value;
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

type InMemoryMap = HashMap<String, (Value, Option<Timeout>)>;

/// An in-memory cache store implementation.
///
/// This is an in-memory implementation of the [`CacheStore`] trait that uses a
/// thread-safe hashmap to store entries. It's primarily useful for development
/// and testing environments.
///
/// # Examples
/// ```
/// use cot::cache::store::memory::Memory;
/// let store = Memory::new();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Memory {
    map: Arc<Mutex<InMemoryMap>>,
}

impl Memory {
    /// Creates a new, empty `Memory` cache store.
    ///
    /// # Examples
    /// ```
    /// use cot::cache::store::memory::Memory;
    /// let store = Memory::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl CacheStore for Memory {
    async fn get(&self, key: &str) -> CacheStoreResult<Option<Value>> {
        let mut map = self.map.lock().await;
        if let Some((value, timeout)) = map.get(key) {
            if let Some(timeout) = timeout
                && timeout.is_expired(None)
            {
                map.remove(key);
                return Ok(None);
            }
            return Ok(Some(value.clone()));
        }
        Ok(None)
    }

    async fn insert(&self, key: String, value: Value, expiry: Timeout) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.insert(key, (value, Some(expiry.canonicalize())));
        Ok(())
    }

    async fn remove(&self, key: &str) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.remove(key);
        Ok(())
    }

    async fn clear(&self) -> CacheStoreResult<()> {
        let mut map = self.map.lock().await;
        map.clear();
        Ok(())
    }

    async fn approx_size(&self) -> CacheStoreResult<usize> {
        let map = self.map.lock().await;
        Ok(map.len())
    }

    async fn contains_key(&self, key: &str) -> CacheStoreResult<bool> {
        let mut map = self.map.lock().await;
        if let Some((_, Some(timeout))) = map.get(key) {
            if timeout.is_expired(None) {
                map.remove(key);
                return Ok(false);
            }
            return Ok(true);
        }
        Ok(false)
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
        let retrieved = store.get("test_key").await.unwrap();
        assert_eq!(retrieved, Some(json!({"data": 123})));
    }

    #[cot::test]
    async fn test_get_after_expiry() {
        let store = Memory::new();
        let key = "temp_key".to_string();
        let value = json!({"data": "temporary"});
        let short_timeout = Timeout::After(std::time::Duration::from_millis(100));
        store
            .insert(key.clone(), value, short_timeout)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let retrieved = store.get(&key).await.unwrap();
        assert_eq!(retrieved, None);
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
        assert_eq!(store.approx_size().await.unwrap(), 2);
        store.clear().await.unwrap();
        assert_eq!(store.approx_size().await.unwrap(), 0);
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
