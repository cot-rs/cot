//! Cache store abstractions and implementations.
//!
//! This module defines a generic `CacheStore` trait and common types used by
//! in-memory, file and Redis-backed cache implementations. The main goal is to
//! provide a simple asynchronous interface for putting, getting, and managing
//! cached values, optionally with expiration policies.

pub mod memory;

use std::fmt::Debug;

use serde_json::Value;
use thiserror::Error;

use crate::config::Timeout;

const CACHE_STORE_ERROR_PREFIX: &str = "Cache store error: ";

/// Errors that can occur when interacting with a cache store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CacheStoreError {
    /// The underlying cache backend returned an error.
    #[error("{CACHE_STORE_ERROR_PREFIX} Cache store backend error: {0}")]
    Backend(String),
    /// Failed to serialize a value for storage.
    #[error("{CACHE_STORE_ERROR_PREFIX} Serialization error: {0}")]
    Serialize(String),
    /// Failed to deserialize a stored value.
    #[error("{CACHE_STORE_ERROR_PREFIX} Deserialization error: {0}")]
    Deserialize(String),
}

/// Convenience alias for results returned by cache store operations.
pub type CacheStoreResult<T> = Result<T, CacheStoreError>;

/// A generic asynchronous cache interface.
///
/// The `CacheStore` trait abstracts over different cache backends. It supports
/// basic CRUD operations as well as helpers to lazily compute and insert
/// values, with optional expiration policies.
#[async_trait::async_trait]
pub trait CacheStore: Debug + Send + Sync + 'static {
    /// Get a value by key. Returns `Ok(None)` if the key does not exist.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue retrieving the key.
    async fn get(&self, key: &String) -> CacheStoreResult<Option<Value>>;

    /// Insert a value under the given key.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue inserting the
    /// key-value pair.
    async fn insert(&self, key: String, value: Value, expiry: Timeout) -> CacheStoreResult<()>;

    /// Remove a value by key. Succeeds even if the key was absent.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue removing the key.
    async fn remove(&self, key: &String) -> CacheStoreResult<()>;

    /// Clear all entries in the cache.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue clearing the cache.
    async fn clear(&self) -> CacheStoreResult<()>;

    /// Return the number of entries in the cache.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue retrieving the length.
    async fn approx_size(&self) -> CacheStoreResult<usize>;

    /// Returns `true` if the cache contains the specified key.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue checking the presence
    /// of the key.
    async fn contains_key(&self, key: &String) -> CacheStoreResult<bool>;

    /// Check if the value associated with the key has expired.
    ///
    ///  # Errors
    ///
    /// This method can return error if there is an issue checking the
    /// expiration status.
    async fn has_expired(&self, key: String) -> CacheStoreResult<bool>;
}
