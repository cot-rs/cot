//! Cache store abstractions and implementations.
//!
//! This module defines a generic `CacheStore` trait and common types used by
//! in-memory, file and Redis-backed cache implementations. The main goal is to
//! provide a simple asynchronous interface for putting, getting, and managing
//! cached values, optionally with expiration policies.

pub mod memory;

use std::fmt::Debug;
use std::pin::Pin;

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
pub trait CacheStore: Send + Sync + 'static {
    /// Get a value by a given key.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue retrieving the key.
    fn get(&self, key: &str) -> impl Future<Output = CacheStoreResult<Option<Value>>> + Send;

    /// Insert a value under the given key.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue inserting the
    /// key-value pair.
    fn insert(
        &self,
        key: String,
        value: Value,
        expiry: Timeout,
    ) -> impl Future<Output = CacheStoreResult<()>> + Send;

    /// Remove a value by key. Succeeds even if the key was absent.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue removing the key.
    fn remove(&self, key: &str) -> impl Future<Output = CacheStoreResult<()>> + Send;

    /// Clear all entries in the cache.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue clearing the cache.
    fn clear(&self) -> impl Future<Output = CacheStoreResult<()>> + Send;

    /// Get an approximate count of entries in the cache.
    ///
    /// This is an approximate count and may or may not be exact depending on
    /// the backend implementation.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue retrieving the length.
    fn approx_size(&self) -> impl Future<Output = CacheStoreResult<usize>> + Send;

    /// Returns `true` if the cache contains the specified key.
    ///
    /// # Errors
    ///
    /// This method can return error if there is an issue checking the presence
    /// of the key.
    fn contains_key(&self, key: &str) -> impl Future<Output = CacheStoreResult<bool>> + Send;
}

pub(crate) trait BoxCacheStore: Send + Sync + 'static {
    fn get<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<Option<Value>>> + Send + 'a>>;

    fn insert<'a>(
        &'a self,
        key: String,
        value: Value,
        expiry: Timeout,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<()>> + Send + 'a>>;

    fn remove<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<()>> + Send + 'a>>;

    fn clear<'a>(&'a self) -> Pin<Box<dyn Future<Output = CacheStoreResult<()>> + Send + 'a>>;

    fn approx_size<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<usize>> + Send + 'a>>;

    fn contains_key<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<bool>> + Send + 'a>>;
}

impl<T: CacheStore> BoxCacheStore for T {
    fn get<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<Option<Value>>> + Send + 'a>> {
        Box::pin(async move { T::get(self, key).await })
    }

    fn insert<'a>(
        &'a self,
        key: String,
        value: Value,
        expiry: Timeout,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<()>> + Send + 'a>> {
        Box::pin(async move { T::insert(self, key, value, expiry).await })
    }

    fn remove<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<()>> + Send + 'a>> {
        Box::pin(async move { T::remove(self, key).await })
    }

    fn clear<'a>(&'a self) -> Pin<Box<dyn Future<Output = CacheStoreResult<()>> + Send + 'a>> {
        Box::pin(async move { T::clear(self).await })
    }

    fn approx_size<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<usize>> + Send + 'a>> {
        Box::pin(async move { T::approx_size(self).await })
    }

    fn contains_key<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = CacheStoreResult<bool>> + Send + 'a>> {
        Box::pin(async move { T::contains_key(self, key).await })
    }
}
