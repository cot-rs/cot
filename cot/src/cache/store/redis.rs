//! Redis cache store implementation.
//!
//! This store uses Redis as the backend for caching.
//! # Examples
//! ```no_run
//! # use cot::cache::store::redis::Redis;
//! # use cot::cache::store::CacheStore;
//! # use cot::config::CacheUrl;
//! # #[tokio::main]
//! # async fn main() {
//!  let store = Redis::new(&CacheUrl::from("redis://127.0.0.1:6379"), 16).unwrap();
//!  let key = "example_key".to_string();
//!  let value = serde_json::json!({"data": "example_value"});
//!  store.insert(key.clone(), value.clone(), Default::default()).await.unwrap();
//!  let retrieved  = store.get(&key).await.unwrap();
//!
//!  assert_eq!(retrieved, Some(value));
//! # }
use cot::cache::store::CacheStoreResult;
use cot::config::Timeout;
use deadpool_redis::{Config, Connection, Pool, Runtime};
use redis::{AsyncCommands, SetExpiry, SetOptions};
use serde_json::Value;
use thiserror::Error;

use crate::cache::store::{CacheStore, CacheStoreError};
use crate::config::CacheUrl;

/// Errors specific to the Redis cache store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RedisCacheStoreError {
    /// An error occurred during Redis connection pool creation.
    #[error("redis pool creation error: {0}")]
    PoolCreation(Box<dyn std::error::Error + Send + Sync>),

    /// An error occurred during a pool connection or checkout.
    #[error("redis pool connection error: {0}")]
    PoolConnection(Box<dyn std::error::Error + Send + Sync>),

    /// An error occurred during a Redis command execution.
    #[error("redis command error: {0}")]
    RedisCommand(Box<dyn std::error::Error + Send + Sync>),

    /// The provided Redis connection string is invalid.
    #[error("invalid redis connection string: {0}")]
    InvalidConnectionString(String),

    /// An error occurred during JSON serialization.
    #[error("Serialization error: {0}")]
    Serialize(String),

    /// An error occurred during JSON deserialization.
    #[error("Deserialization error: {0}")]
    Deserialize(String),
}

impl From<RedisCacheStoreError> for CacheStoreError {
    fn from(err: RedisCacheStoreError) -> Self {
        match err {
            RedisCacheStoreError::Serialize(e) => CacheStoreError::Serialize(e),
            RedisCacheStoreError::Deserialize(e) => CacheStoreError::Deserialize(e),
            other => CacheStoreError::Backend(other.to_string()),
        }
    }
}

/// A Redis-backed cache store implementation.
///
/// This store uses Redis as the backend for caching.
///
/// # Examples
/// ```
/// use cot::cache::store::redis::Redis;
/// use cot::config::CacheUrl;
///
/// let store = Redis::new(&CacheUrl::from("redis://127.0.0.1/"), 16).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct Redis {
    pool: Pool,
}

impl Redis {
    /// Creates and configures a new Redis cache store.
    ///
    /// This initializes a connection pool to the Redis server specified by the
    /// provided URL.
    ///
    /// # Errors
    ///
    ///  Returns [`RedisCacheStoreError::InvalidConnectionString`] if the
    /// provided URL is not a valid Redis URL
    ///  and [`RedisCacheStoreError::PoolCreation`] if the connection pool could
    /// not be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::CacheUrl;
    /// use cot::cache::store::redis::Redis;
    ///
    /// let store = Redis::new(&CacheUrl::from("redis://127.0.0.1/"), 16).unwrap();
    ///  ```
    pub fn new(url: &CacheUrl, pool_size: usize) -> CacheStoreResult<Self> {
        if !url.is_redis() {
            return Err(
                RedisCacheStoreError::InvalidConnectionString(url.as_str().to_string()).into(),
            );
        }
        let cfg = Config::from_url(url.as_str())
            .builder()
            .map_err(|e| RedisCacheStoreError::PoolCreation(Box::new(e)))?
            .max_size(pool_size)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| RedisCacheStoreError::PoolCreation(Box::new(e)))?;

        Ok(Self { pool: cfg })
    }

    /// Get a connection from the Redis connection pool.
    ///
    /// # Errors
    ///
    /// Returns [`RedisCacheStoreError::PoolConnection`] if a connection could
    /// not be obtained from the pool.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cot::cache::store::redis::Redis;
    /// use cot::config::CacheUrl;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::cache::store::CacheStoreResult<()> {
    /// let store = Redis::new(&CacheUrl::from("redis://127.0.0.1/"), 16).unwrap();
    /// let mut conn = store.get_connection().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_connection(&self) -> Result<Connection, RedisCacheStoreError> {
        self.pool
            .get()
            .await
            .map_err(|e| RedisCacheStoreError::PoolConnection(Box::new(e)))
    }
}

impl CacheStore for Redis {
    async fn get(&self, key: &str) -> CacheStoreResult<Option<Value>> {
        let mut conn = self.get_connection().await?;
        let data = conn
            .get::<_, Option<String>>(key)
            .await
            .map_err(|e| RedisCacheStoreError::RedisCommand(Box::new(e)))?;

        data.map(|d| {
            let value = serde_json::from_str::<Value>(&d)
                .map_err(|err| RedisCacheStoreError::Deserialize(err.to_string()))?;
            Ok(value)
        })
        .transpose()
    }

    async fn insert(&self, key: String, value: Value, expiry: Timeout) -> CacheStoreResult<()> {
        let mut conn = self.get_connection().await?;
        let data = serde_json::to_string(&value)
            .map_err(|e| RedisCacheStoreError::Serialize(e.to_string()))?;
        let mut options = SetOptions::default();

        match expiry {
            Timeout::After(duration) => {
                options = options.with_expiration(SetExpiry::EX(duration.as_secs()));
            }
            Timeout::AtDateTime(dt) => {
                let unix_timestamp = dt.timestamp().unsigned_abs();
                options = options.with_expiration(SetExpiry::EXAT(unix_timestamp));
            }
            _ => {}
        }

        conn.set_options::<_, _, bool>(key, data, options)
            .await
            .map_err(|e| RedisCacheStoreError::RedisCommand(Box::new(e)))?;
        Ok(())
    }

    async fn remove(&self, key: &str) -> CacheStoreResult<()> {
        let mut conn = self.get_connection().await?;
        conn.del::<_, usize>(key)
            .await
            .map_err(|e| RedisCacheStoreError::RedisCommand(Box::new(e)))?;
        Ok(())
    }

    async fn clear(&self) -> CacheStoreResult<()> {
        let mut conn = self.get_connection().await?;
        conn.flushdb::<bool>()
            .await
            .map_err(|e| RedisCacheStoreError::RedisCommand(Box::new(e)))?;
        Ok(())
    }

    async fn approx_size(&self) -> CacheStoreResult<usize> {
        let mut conn = self.get_connection().await?;
        let cmd = redis::cmd("DBSIZE");
        let val = cmd
            .query_async::<usize>(&mut conn)
            .await
            .map_err(|err| RedisCacheStoreError::RedisCommand(Box::new(err)))?;
        Ok(val)
    }

    async fn contains_key(&self, key: &str) -> CacheStoreResult<bool> {
        let mut conn = self.get_connection().await?;
        let exists = conn
            .exists(key)
            .await
            .map_err(|e| RedisCacheStoreError::RedisCommand(Box::new(e)))?;
        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::config::Timeout;

    async fn make_store(db: &str) -> Redis {
        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());
        let mut url = CacheUrl::from(redis_url);
        url.inner_mut().set_path(db);
        let store = Redis::new(&url, 16).expect("failed to create redis store");
        store
            .get_connection()
            .await
            .expect("failed to get redis connection");
        store
    }

    #[cot::test]
    async fn test_new_redis_store_invalid_url() {
        let store = Redis::new(&CacheUrl::from("file://tmp/random"), 16);
        assert!(store.is_err());
    }

    #[cot::test]
    #[ignore = "requires a running redis instance"]
    async fn test_insert_and_get() {
        let store = make_store("0").await;
        let key = "test_key".to_string();
        let value = json!({"data": 123});

        store
            .insert(key.clone(), value.clone(), Timeout::default())
            .await
            .unwrap();
        let retrieved = store.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(value));
    }

    #[cot::test]
    #[ignore = "requires a running redis instance"]
    async fn test_get_after_expiry() {
        let store = make_store("0").await;
        let key = "temp_key__".to_string();
        let value = json!({"data": "temporary"});
        let short_timeout = Timeout::After(Duration::from_secs(1));
        store
            .insert(key.clone(), value, short_timeout)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        let retrieved = store.get(&key).await.unwrap();
        assert_eq!(retrieved, None);
    }

    #[cot::test]
    #[ignore = "requires a running redis instance"]
    async fn test_insert_with_expiry_types() {
        let store = make_store("0").await;

        macro_rules! run_expiry {
            ($idx:expr, $timeout:expr) => {
                {
                    let key = format!("temp_key_{}", $idx);
                    let value = json!({"data": "temporary"});
                    println!("key: {key}, timeout: {:?}", $timeout);
                    store
                        .insert(key.clone(), value.clone(), $timeout)
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let retrieved = store.get(&key).await.unwrap();
                    assert_eq!(retrieved, None);
                }
            };
        }

        let timeouts = vec![
            Timeout::After(Duration::from_secs(1)),
            Timeout::AtDateTime(
                (chrono::Utc::now() + chrono::Duration::seconds(1))
                    .with_timezone(&chrono::FixedOffset::east_opt(0).unwrap()),
            ),
        ];

        for (i, t) in timeouts.into_iter().enumerate() {
            run_expiry!(i, t);
        }
    }

    #[cot::test]
    #[ignore = "requires a running redis instance"]
    async fn test_remove() {
        let store = make_store("0").await;
        let key = "test_key_remove".to_string();
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
    #[ignore = "requires a running redis instance"]
    async fn test_clear() {
        let store = make_store("1").await;
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
    #[ignore = "requires a running redis instance"]
    async fn test_contains_key() {
        let store = make_store("0").await;
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

    #[cot::test]
    #[ignore = "requires a running redis instance"]
    async fn test_approx_size() {
        let store = make_store("2").await;
        store.clear().await.unwrap();
        store
            .insert("key1".to_string(), json!(1), Timeout::default())
            .await
            .unwrap();
        store
            .insert("key2".to_string(), json!(2), Timeout::default())
            .await
            .unwrap();
        let size = store.approx_size().await.unwrap();
        assert_eq!(size, 2);
    }
}
