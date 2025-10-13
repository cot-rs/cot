//! Caching functionality for Cot applications.
//!
//! This module provides a high-level caching interface that supports multiple
//! storage backends and automatic serialization/deserialization of values.
//! The cache is designed to be thread-safe and can be used across multiple
//! async tasks concurrently.
//!
//! # Features
//!
//! - **Multiple Storage Backends**: Support for in-memory, Redis, and other
//!   cache stores through a pluggable architecture
//! - **Automatic Serialization**: Values are automatically serialized to JSON
//!   for storage and deserialized when retrieved
//! - **Key Prefixing**: Support for key namespacing to avoid collisions
//! - **Expiration**: Configurable expiration times for cached values
//! - **Lazy Loading**: Support for computing values on-demand and caching them
//!
//! # Basic Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! use cot::cache::Cache;
//! use cot::config::{CacheConfig, CacheStoreConfig, CacheStoreTypeConfig, Timeout};
//!
//! #[tokio::main]
//! async fn main() -> cot::Result<()> {
//!     let config = CacheConfig::builder()
//!         .store(
//!             CacheStoreConfig::builder()
//!                 .store_type(CacheStoreTypeConfig::Memory)
//!                 .build(),
//!         )
//!         .prefix("v1")
//!         .timeout(Timeout::After(Duration::from_secs(1800)))
//!         .build();
//!
//!     let cache = Cache::from_config(&config).await?;
//!
//!     // Store a value
//!     cache.insert("user:123", "John Doe").await?;
//!
//!     // Retrieve a value
//!     let user: Option<String> = cache.get("user:123").await?;
//!     println!("User: {:?}", user);
//!
//!     // Use lazy loading
//!     let expensive_value: String = cache
//!         .get_or_insert_with("expensive", || async {
//!             // Some expensive computation
//!             tokio::time::sleep(std::time::Duration::from_secs(1)).await;
//!             Ok("computed result".to_string())
//!         })
//!         .await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Advanced Usage
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use cot::cache::Cache;
//! use cot::config::{CacheConfig, CacheStoreConfig, CacheStoreTypeConfig, Timeout};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
//! struct User {
//!     id: u32,
//!     name: String,
//!     email: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> cot::Result<()> {
//!     let config = CacheConfig::builder()
//!         .store(
//!             CacheStoreConfig::builder()
//!                 .store_type(CacheStoreTypeConfig::Memory)
//!                 .build(),
//!         )
//!         .prefix("v1")
//!         .timeout(Timeout::After(Duration::from_secs(3600)))
//!         .build();
//!
//!     let cache = Cache::from_config(&config).await?;
//!
//!     // Store complex objects
//!     let user = User {
//!         id: 123,
//!         name: "John Doe".to_string(),
//!         email: "john@example.com".to_string(),
//!     };
//!     cache.insert("user:123", &user).await?;
//!
//!     // Retrieve complex objects
//!     let cached_user: Option<User> = cache.get("user:123").await?;
//!     assert_eq!(cached_user, Some(user));
//!
//!     // Store with custom expiration
//!     cache
//!         .insert_expiring(
//!             "temp:data",
//!             "temporary",
//!             Timeout::After(Duration::from_secs(300)),
//!         )
//!         .await?;
//!
//!     // Check if key exists
//!     let exists = cache.contains_key("user:123").await?;
//!     println!("User exists in cache: {}", exists);
//!
//!     // Get cache statistics
//!     let count = cache.len().await?;
//!     let is_empty = cache.is_empty().await?;
//!     println!("Cache has {} items, empty: {}", count, is_empty);
//!
//!     Ok(())
//! }
//! ```

pub mod stores;

use std::future::Future;
use std::sync::Arc;

use cot::config::CacheStoreTypeConfig;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::cache::stores::CacheStore;
use crate::cache::stores::memory::Memory;
use crate::config::{CacheConfig, Timeout};
use crate::error::error_impl::impl_into_cot_error;

/// An error that can occur when interacting with the cache.
///
/// This error type encompasses all possible errors that can occur during cache
/// operations, including serialization errors and store-specific errors.
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
///
/// use cot::cache::{Cache, CacheError};
/// use cot::config::{CacheConfig, CacheStoreConfig, CacheStoreTypeConfig, Timeout};
///
/// #[tokio::main]
/// async fn main() -> Result<(), CacheError> {
///     let config = CacheConfig::builder()
///         .store(
///             CacheStoreConfig::builder()
///                 .store_type(CacheStoreTypeConfig::Memory)
///                 .build(),
///         )
///         .prefix("v1")
///         .timeout(Timeout::After(Duration::from_secs(3600)))
///         .build();
///
///     let cache = Cache::from_config(&config).await?;
///
///     cache.insert("key", "value").await.unwrap();
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CacheError {
    /// An error occurred during JSON serialization or deserialization.
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    /// An error occurred in the underlying cache store.
    #[error(transparent)]
    Store(#[from] stores::CacheStoreError),
}

impl_into_cot_error!(CacheError, INTERNAL_SERVER_ERROR);

/// A type alias for results that can contain a [`CacheError`].
///
/// This is a convenience type alias for `Result<T, CacheError>`.
pub type CacheResult<T> = Result<T, CacheError>;

/// A high-level cache interface that provides automatic serialization and
/// deserialization of values.
///
/// The `Cache` struct wraps a cache store implementation and provides a
/// convenient interface for storing and retrieving values. All values are
/// automatically serialized to JSON for storage and deserialized when
/// retrieved.
///
///
/// # Key Formatting
///
/// Keys can be prefixed to avoid collisions between different parts of your
/// application. If a prefix is set, all keys will be formatted as
/// `{prefix}:{key}`.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// use cot::cache::Cache;
/// use cot::cache::stores::memory::Memory;
/// use cot::config::Timeout;
///
/// #[tokio::main]
/// async fn main() -> cot::Result<()> {
///     let store = Arc::new(Memory::new());
///
///     let cache = Cache::new(
///         store,
///         Some("myapp".to_string()),
///         Timeout::After(Duration::from_secs(300)),
///     );
///
///     cache.insert("user:123", "John Doe").await?;
///
///     let user: Option<String> = cache.get("user:123").await?;
///     assert_eq!(user, Some("John Doe".to_string()));
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Cache {
    store: Arc<dyn CacheStore<Key = String, Value = Value>>,
    prefix: Option<String>,
    expiry: Timeout,
}

impl Cache {
    /// Creates a new cache instance with the specified store, prefix, and
    /// default expiration time.
    ///
    /// # Arguments
    ///
    /// * `store` - The underlying cache store implementation
    /// * `prefix` - An optional prefix for all keys to avoid collisions
    /// * `expiry` - The default expiration time for cached values
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// let store = Arc::new(Memory::new());
    /// let cache = Cache::new(
    ///     store,
    ///     Some("myapp".to_string()),
    ///     Timeout::After(Duration::from_secs(3600)),
    /// );
    /// ```
    pub fn new(
        store: Arc<dyn CacheStore<Key = String, Value = Value>>,
        prefix: Option<String>,
        expiry: Timeout,
    ) -> Self {
        Self {
            store,
            prefix,
            expiry,
        }
    }

    /// Formats a key with the cache prefix if one is set.
    ///
    /// This is an internal method used to ensure consistent key formatting
    /// across all cache operations.
    fn format_key<K: AsRef<str>>(&self, key: K) -> String {
        let k = key.as_ref();
        if let Some(pref) = &self.prefix {
            return format!("{pref}:{k}");
        }
        k.to_string()
    }

    /// Retrieves a value from the cache.
    ///
    /// Returns `Some(value)` if the key exists and the value can be
    /// deserialized, or `None` if the key doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to retrieve the value for
    ///
    /// # Errors
    ///
    /// Returns an error if there was a problem deserializing the value or
    /// accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     cache.insert("user:123", "John Doe").await?;
    ///
    ///     let user: Option<String> = cache.get("user:123").await?;
    ///     assert_eq!(user, Some("user:123".to_string()));
    ///
    ///     let missing: Option<String> = cache.get("nonexistent").await?;
    ///     assert!(missing.is_none());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn get<K, V>(&self, key: K) -> CacheResult<Option<V>>
    where
        K: AsRef<str>,
        V: DeserializeOwned,
    {
        let k = self.format_key(key.as_ref());
        let result = self
            .store
            .get(&k)
            .await?
            .map(serde_json::from_value)
            .transpose()?;
        Ok(result)
    }

    /// Stores a value in the cache with the default expiration time of 5
    /// minutes.
    ///
    /// The value will be serialized to JSON before storage. If the key already
    /// exists, the value will be overwritten.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store the value under
    /// * `value` - The value to store (must implement `Serialize`)
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be serialized or if there was a
    /// problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Serialize, Deserialize, Debug)]
    /// struct User {
    ///     id: u32,
    ///     name: String,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(60)));
    ///
    ///     cache.insert("greeting", "Hello, World!").await?;
    ///
    ///     let user = User {
    ///         id: 123,
    ///         name: "John Doe".to_string(),
    ///     };
    ///     cache.insert("user:123", &user).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn insert<K, V>(&self, key: K, value: V) -> CacheResult<()>
    where
        K: Into<String>,
        V: Serialize,
    {
        let k = self.format_key(key.into());
        self.store
            .insert(k, serde_json::to_value(value)?, self.expiry)
            .await?;
        Ok(())
    }

    /// Stores a value in the cache with a custom expiration time.
    ///
    /// This method allows you to override the default expiration time for a
    /// specific value. The value will be serialized to JSON before storage.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store the value under
    /// * `value` - The value to store (must implement `Serialize`)
    /// * `expiry` - The custom expiration time for this value
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be serialized or if there was a
    /// problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use chrono::{DateTime, Utc};
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(3600)));
    ///
    ///     // Store a value with custom expiration
    ///     let dt = DateTime::parse_from_rfc3339("2025-11-21T22:00:00Z").unwrap();
    ///
    ///     cache
    ///         .insert_expiring("temp:data", "temporary", Timeout::AtDateTime(dt))
    ///         .await?;
    ///
    ///     // Store a value that never expires
    ///     cache
    ///         .insert_expiring("user:session", "session_data", Timeout::Never)
    ///         .await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn insert_expiring<K, V>(&self, key: K, value: V, expiry: Timeout) -> CacheResult<()>
    where
        K: Into<String>,
        V: Serialize,
    {
        let k = self.format_key(key.into());
        self.store
            .insert(k, serde_json::to_value(value)?, expiry)
            .await?;
        Ok(())
    }

    /// Removes a value from the cache.
    ///
    /// If the key doesn't exist, this operation is a no-op and no error is
    /// returned.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to remove from the cache
    ///
    /// # Errors
    ///
    /// Returns an error if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(200)));
    ///
    ///     cache.insert("user:123", "John Doe").await?;
    ///     cache.remove("user:123").await?;
    ///     let user: Option<String> = cache.get("user:123").await?;
    ///     assert!(user.is_none());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn remove<K: AsRef<str>>(&self, key: K) -> CacheResult<()> {
        let k = self.format_key(key.as_ref());
        self.store.remove(&k).await?;
        Ok(())
    }

    /// Removes all values from the cache.
    ///
    /// This operation clears the entire cache, removing all stored values.
    ///
    /// # Errors
    ///
    /// Returns an error if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     use std::time::Duration;
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     // Store some values
    ///     cache.insert("key1", "value1").await?;
    ///     cache.insert("key2", "value2").await?;
    ///
    ///     // Clear the cache
    ///     cache.clear().await?;
    ///
    ///     // Check that cache is empty
    ///     let count = cache.len().await?;
    ///     assert_eq!(count, 0); // 0
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn clear(&self) -> CacheResult<()> {
        self.store.clear().await?;
        Ok(())
    }

    /// Returns the number of items currently stored in the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     use std::time::Duration;
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     let count = cache.len().await?;
    ///     assert_eq!(count, 0);
    ///
    ///     cache.insert("key1", "value1").await?;
    ///     cache.insert("key2", "value2").await?;
    ///
    ///     let count = cache.len().await?;
    ///     assert_eq!(count, 2);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn len(&self) -> CacheResult<usize> {
        let result = self.store.len().await?;
        Ok(result)
    }

    /// Returns `true` if the cache contains no items.
    ///
    /// # Errors
    ///
    /// Returns an error if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     use std::time::Duration;
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     let is_empty = cache.is_empty().await?;
    ///     assert!(is_empty);
    ///
    ///     cache.insert("key", "value").await?;
    ///
    ///     let is_empty = cache.is_empty().await?;
    ///     assert!(!is_empty);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn is_empty(&self) -> CacheResult<bool> {
        let result = self.store.is_empty().await?;
        Ok(result)
    }

    /// Returns `true` if the cache contains the specified key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to check for existence
    ///
    /// # Errors
    ///
    /// Returns an error if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     use std::time::Duration;
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     // Check for non-existent key
    ///     let exists = cache.contains_key("nonexistent").await?;
    ///     assert!(!exists);
    ///
    ///     cache.insert("user:123", "John Doe").await?;
    ///     let exists = cache.contains_key("user:123").await?;
    ///     assert!(exists);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn contains_key<K: AsRef<str>>(&self, key: K) -> CacheResult<bool> {
        let k = self.format_key(key.as_ref());
        let result = self.store.contains_key(&k).await?;
        Ok(result)
    }

    /// Computes a value lazily and stores it in the cache.
    ///
    /// This method executes the provided closure to compute a value and then
    /// stores the result in the cache with the default expiration time. The
    /// computation is performed every time this method is called.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store the computed value under
    /// * `f` - A closure that computes the value to store
    ///
    /// # Errors
    ///
    /// Returns an error if the computation fails, the value cannot be
    /// serialized, or if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     cache
    ///         .insert_with("expensive", || async { Ok("computed result".to_string()) })
    ///         .await?;
    ///
    ///     // The value is now cached
    ///     let value: Option<String> = cache.get("expensive").await?;
    ///     assert_eq!(value, Some("computed result".to_string()));
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn insert_with<F, Fut, K, V>(&self, key: K, f: F) -> CacheResult<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheResult<V>> + Send,
        K: Into<String>,
        V: DeserializeOwned + Serialize,
    {
        let computed_value = f().await?;
        self.insert(key.into(), computed_value).await?;
        Ok(())
    }

    /// Gets a value from the cache, or computes, stores, and returns it if not
    /// present.
    ///
    /// This method first attempts to retrieve the value from the cache. If the
    /// key doesn't exist, it executes the provided closure to compute the
    /// value, stores the result in the cache with the default expiration
    /// time, and returns the computed value.
    ///
    /// This is useful for implementing the "cache-aside" pattern where
    /// expensive computations are cached to avoid repeated work.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to retrieve or store the computed value under
    /// * `f` - A closure that computes the value if it's not in the cache
    ///
    /// # Errors
    ///
    /// Returns an error if the computation fails, the value cannot be
    /// serialized, or if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     let value1: String = cache
    ///         .get_or_insert_with("expensive", || async { Ok("computed result".to_string()) })
    ///         .await?;
    ///
    ///     let value2: String = cache
    ///         .get_or_insert_with("expensive", || async { Ok("different result".to_string()) })
    ///         .await?;
    ///
    ///     assert_eq!(value1, value2);
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_or_insert_with<F, Fut, K, V>(&self, key: K, f: F) -> CacheResult<V>
    where
        K: Into<String>,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheResult<V>> + Send,
        V: DeserializeOwned + Serialize,
    {
        let key = key.into();
        if let Some(value) = self.get(&key).await? {
            return Ok(value);
        }

        let computed_value = f().await?;
        let value = serde_json::to_value(&computed_value)?;

        self.insert(key, serde_json::to_value(&value)?).await?;
        Ok(computed_value)
    }

    /// Gets a value from the cache, or computes, stores with custom expiration,
    /// and returns it.
    ///
    /// This method first attempts to retrieve the value from the cache. If the
    /// key doesn't exist, it executes the provided closure to compute the
    /// value, stores the result in the cache with the specified expiration
    /// time, and returns the computed value.
    ///
    /// This is useful when you need different expiration times for different
    /// types of cached values.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to retrieve or store the computed value under
    /// * `f` - A closure that computes the value if it's not in the cache
    /// * `expiry` - The custom expiration time for the computed value
    ///
    /// # Errors
    ///
    /// Returns an error if the computation fails, the value cannot be
    /// serialized, or if there was a problem accessing the cache store.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::cache::stores::memory::Memory;
    /// use cot::config::Timeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let store = Arc::new(Memory::new());
    ///     let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(300)));
    ///
    ///     let value = cache
    ///         .get_or_insert_expiring_with(
    ///             "temp:data",
    ///             || async { Ok("temporary result".to_string()) },
    ///             Timeout::After(Duration::from_secs(300)),
    ///         )
    ///         .await?;
    ///
    ///     assert_eq!(value, "temporary result".to_string());
    ///
    ///     let session = cache
    ///         .get_or_insert_expiring_with(
    ///             "user:session",
    ///             || async { Ok("session_data".to_string()) },
    ///             Timeout::After(Duration::from_secs(7200)),
    ///         )
    ///         .await?;
    ///
    ///     assert_eq!(session, "session_data".to_string());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_or_insert_expiring_with<F, Fut, K, V>(
        &self,
        key: K,
        f: F,
        expiry: Timeout,
    ) -> CacheResult<V>
    where
        K: Into<String>,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheResult<V>> + Send,
        V: DeserializeOwned + Serialize,
    {
        let key = key.into();
        let value = self.get(&key).await?;
        if let Some(value) = value {
            return Ok(value);
        }
        let computed_value = f().await?;
        let value = serde_json::to_value(&computed_value)?;
        self.insert_expiring(key, serde_json::to_value(&value)?, expiry)
            .await?;
        Ok(computed_value)
    }

    /// Creates a new cache instance from the provided configuration.
    ///
    /// This method initializes the cache store based on the configuration and
    /// sets up the cache with the specified prefix and expiration time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::time::Duration;
    ///
    /// use cot::cache::Cache;
    /// use cot::config::{CacheConfig, CacheStoreConfig, CacheStoreTypeConfig, Timeout};
    ///
    /// #[tokio::main]
    /// async fn main() -> cot::Result<()> {
    ///     let config = CacheConfig::builder()
    ///         .store(
    ///             CacheStoreConfig::builder()
    ///                 .store_type(CacheStoreTypeConfig::Memory)
    ///                 .build(),
    ///         )
    ///         .prefix("v1")
    ///         .timeout(Timeout::After(Duration::from_secs(3600)))
    ///         .build();
    ///
    ///     let cache = Cache::from_config(&config).await?;
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    ///
    /// Returns an error if there was a problem initializing the cache store.
    #[expect(clippy::unused_async)]
    pub async fn from_config(config: &CacheConfig) -> CacheResult<Self> {
        let store_cfg = &config.store;

        let store = match store_cfg.store_type {
            CacheStoreTypeConfig::Memory => {
                let mem_store = Memory::new();
                Arc::new(mem_store)
            }
            _ => {
                unimplemented!();
            }
        };

        let this = Self::new(store, config.prefix.clone(), config.timeout);
        Ok(this)
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("store", &"<CacheStore>")
            .field("prefix", &self.prefix)
            .field("expiry", &self.expiry)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::cache::stores::memory::Memory;
    use crate::config::Timeout;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct User {
        id: u32,
        name: String,
        email: String,
    }

    #[cot::test]
    async fn test_cache_basic_operations() {
        let store = Arc::new(Memory::new());
        let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(60)));

        // Test insert and get
        cache.insert("user:1", "John Doe").await.unwrap();
        let user: Option<String> = cache.get("user:1").await.unwrap();
        assert_eq!(user, Some("John Doe".to_string()));

        // Test remove
        cache.remove("user:1").await.unwrap();
        let user: Option<String> = cache.get("user:1").await.unwrap();
        assert_eq!(user, None);
    }

    #[cot::test]
    async fn test_cache_with_prefix() {
        let store = Arc::new(Memory::new());
        let cache = Cache::new(
            store,
            Some("myapp".to_string()),
            Timeout::After(Duration::from_secs(60)),
        );

        cache.insert("user:1", "John Doe").await.unwrap();
        let user: Option<String> = cache.get("user:1").await.unwrap();
        assert_eq!(user, Some("John Doe".to_string()));
    }

    #[cot::test]
    async fn test_cache_complex_objects() {
        let store = Arc::new(Memory::new());
        let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(60)));

        let user = User {
            id: 1,
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        cache.insert("user:1", &user).await.unwrap();
        let cached_user: Option<User> = cache.get("user:1").await.unwrap();
        assert_eq!(cached_user, Some(user));
    }

    #[cot::test]
    async fn test_cache_get_or_insert_with() {
        let store = Arc::new(Memory::new());
        let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(60)));

        let mut call_count = 0;

        // First call should compute the value
        let value1: String = cache
            .get_or_insert_with("expensive", || async {
                call_count += 1;
                Ok("computed".to_string())
            })
            .await
            .unwrap();

        // Second call should use cached value
        let value2: String = cache
            .get_or_insert_with("expensive", || async {
                call_count += 1;
                Ok("different".to_string())
            })
            .await
            .unwrap();

        assert_eq!(value1, value2);
        assert_eq!(call_count, 1); // Should only be called once
    }

    #[cot::test]
    async fn test_cache_statistics() {
        let store = Arc::new(Memory::new());
        let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(60)));

        // Initially empty
        assert!(cache.is_empty().await.unwrap());
        assert_eq!(cache.len().await.unwrap(), 0);

        // Add some values
        cache.insert("key1", "value1").await.unwrap();
        cache.insert("key2", "value2").await.unwrap();

        assert!(!cache.is_empty().await.unwrap());
        assert_eq!(cache.len().await.unwrap(), 2);

        // Clear cache
        cache.clear().await.unwrap();
        assert!(cache.is_empty().await.unwrap());
        assert_eq!(cache.len().await.unwrap(), 0);
    }

    #[cot::test]
    async fn test_cache_contains_key() {
        let store = Arc::new(Memory::new());
        let cache = Cache::new(store, None, Timeout::After(Duration::from_secs(60)));

        // Key doesn't exist
        assert!(!cache.contains_key("nonexistent").await.unwrap());

        // Add key
        cache.insert("existing", "value").await.unwrap();
        assert!(cache.contains_key("existing").await.unwrap());
    }
}
