//! Cache store abstractions and implementations.
//!
//! This module defines a generic `CacheStore` trait and common types used by
//! in-memory and Redis-backed cache implementations. The main goal is to
//! provide a simple asynchronous interface for putting, getting, and managing
//! cached values, optionally with expiration policies.

mod memory;
mod redis;

use std::future::Future;
use std::time::Duration;

use thiserror::Error;

/// Errors that can occur when interacting with a cache store.
#[derive(Debug, Clone, Error)]
pub enum CacheStoreError {
    /// The requested key was not found.
    #[error("Key not found")]
    NotFound,
    /// The underlying cache backend returned an error.
    #[error("Cache store backend error: {0}")]
    Backend(String),
    /// Failed to serialize a value for storage.
    #[error("Serialization error: {0}")]
    Serialize(String),
    /// Failed to deserialize a stored value.
    #[error("Deserialization error: {0}")]
    Deserialize(String),
    /// Any other error represented as a string.
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Convenience alias for results returned by cache store operations.
pub type CacheStoreResult<T> = Result<T, CacheStoreError>;

/// A generic asynchronous cache interface.
///
/// The `CacheStore` trait abstracts over different cache backends. It supports
/// basic CRUD operations as well as helpers to lazily compute and insert
/// values, with optional expiration policies.
#[async_trait::async_trait]
pub trait CacheStore<K, V>: Send + Sync + 'static
where
    K: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
    V: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Get a value by key. Returns `Ok(None)` if the key does not exist.
    async fn get(&self, key: &K) -> CacheStoreResult<Option<V>>;
    /// Insert a value under the given key.
    async fn insert(&self, key: K, value: V) -> CacheStoreResult<()>;
    /// Insert a lazily computed value under the given key.
    async fn insert_with<F, Fut>(&self, key: K, f: F) -> CacheStoreResult<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<V>> + Send;
    /// Insert a value with an expiration policy.
    async fn insert_expiring(&self, key: K, value: V, expiry: Expiry) -> CacheStoreResult<()>;
    /// Get the value for `key`, or compute, insert, and return it.
    async fn get_or_insert_with<F, Fut>(&self, key: K, f: F) -> CacheStoreResult<V>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<V>> + Send;
    /// Get the value for `key`, or compute, insert it with an expiration, and
    /// return it.
    async fn get_or_insert_expiring_with<F, Fut>(
        &self,
        key: K,
        f: F,
        expiry: Expiry,
    ) -> CacheStoreResult<V>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<V>> + Send;
    /// Remove a value by key. Succeeds even if the key was absent.
    async fn remove(&self, key: &K) -> CacheStoreResult<()>;
    /// Clear all entries in the cache.
    async fn clear(&self) -> CacheStoreResult<()>;
    /// Return the number of entries in the cache.
    async fn len(&self) -> CacheStoreResult<usize>;
    /// Check whether the cache is empty.
    async fn is_empty(&self) -> CacheStoreResult<bool>;
    /// Returns `true` if the cache contains the specified key.
    async fn contains_key(&self, key: &K) -> CacheStoreResult<bool>;
}

/// Expiration policy for cached values.
#[derive(Debug, Clone, Copy)]
pub enum Expiry {
    /// Never expire the value.
    Never,
    /// Expire after the specified duration from the insertion time.
    After(Duration),
    /// Expire at the specific UTC datetime.
    AtDateTime(chrono::DateTime<chrono::Utc>),
}
