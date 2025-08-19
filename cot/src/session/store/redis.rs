//! Redis session store
//!
//! This module provides a session store that uses Redis as the storage backend.
//!
//! # Examples
//!
//! ```
//! use cot::config::CacheUrl;
//! use cot::session::store::redis::RedisStore;
//!
//! let store = RedisStore::new(&CacheUrl::from("redis://127.0.0.1/")).unwrap();
//! ```

use std::error::Error;

use async_trait::async_trait;
use deadpool_redis::{Config, Pool as RedisPool, Runtime};
use redis::{AsyncCommands, ExistenceCheck, SetExpiry, SetOptions};
use thiserror::Error;
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

use crate::config::CacheUrl;
use crate::session::store::{ERROR_PREFIX, MAX_COLLISION_RETRIES};

#[derive(Debug, Error)]
/// Errors that can occur when using the Redis session store.
#[non_exhaustive]
pub enum RedisStoreError {
    /// An error occurred during a pool connection or checkout.
    #[error("{ERROR_PREFIX} pool connection error: {0}")]
    PoolConnection(Box<dyn Error + Send + Sync>),

    /// An error occurred during Redis connection pool creation.
    #[error("{ERROR_PREFIX} pool creation error: {0}")]
    PoolCreation(Box<dyn Error + Send + Sync>),

    /// An error occurred during a Redis command execution.
    #[error("{ERROR_PREFIX} command error: {0}")]
    Command(Box<dyn Error + Send + Sync>),

    /// The record ID collided too many times while saving in the store.
    #[error("{ERROR_PREFIX} session-id collision retried too many times ({0})")]
    TooManyIdCollisions(u32),

    /// An error occurred during JSON serialization.
    #[error("{ERROR_PREFIX} serialization error: {0}")]
    Serialize(Box<dyn Error + Send + Sync>),

    /// An error occurred during JSON deserialization.
    #[error("{ERROR_PREFIX} deserialization error: {0}")]
    Deserialize(Box<dyn Error + Send + Sync>),
}

impl From<RedisStoreError> for session_store::Error {
    fn from(err: RedisStoreError) -> session_store::Error {
        match err {
            RedisStoreError::PoolConnection(inner) | RedisStoreError::PoolCreation(inner) => {
                session_store::Error::Backend(inner.to_string())
            }
            RedisStoreError::Command(inner) => session_store::Error::Backend(inner.to_string()),
            RedisStoreError::Serialize(inner) => session_store::Error::Encode(inner.to_string()),
            RedisStoreError::Deserialize(inner) => session_store::Error::Decode(inner.to_string()),
            other => session_store::Error::Backend(other.to_string()),
        }
    }
}

/// A Redis-backed session store implementation.
///
/// This store persists sessions in Redis, providing a scalable and
/// production-ready session storage solution.
///
/// # Examples
///
/// ```
/// use cot::config::CacheUrl;
/// use cot::session::store::redis::RedisStore;
///
/// let store = RedisStore::new(&CacheUrl::from("redis://127.0.0.1/")).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct RedisStore {
    /// The Redis connection pool.
    pool: RedisPool,
}

impl RedisStore {
    /// Creates and configures a new Redis-backed session store.
    ///
    /// This initializes a `deadpool_redis::Pool` immediately, so your
    /// URL is validated and the pool’s settings are applied right away.
    /// **However**, no actual TCP connections to the Redis server are opened
    /// until you call `get_connection()` for the first time. This avoids
    /// connection overhead at startup but still fails fast if your URL is
    /// invalid.
    ///
    ///  # Errors
    ///
    ///  Returns [`RedisStoreError::PoolCreation`] if it fails to create a redis
    /// connection.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::CacheUrl;
    /// use cot::session::store::redis::RedisStore;
    ///
    /// let store = RedisStore::new(&CacheUrl::from("redis://127.0.0.1/"))
    ///     .expect("failed to configure RedisStore");
    /// ```
    pub fn new(url: &CacheUrl) -> Result<RedisStore, RedisStoreError> {
        let cfg = Config::from_url(url.as_str());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|err| RedisStoreError::PoolCreation(Box::new(err)))?;

        Ok(Self { pool })
    }

    /// Asynchronously checks out a Redis connection from the internal pool.
    ///
    /// You’ll typically call this at the start of each session operation.
    /// The returned `Connection` implements
    /// `AsyncCommands` so you can run Redis commands directly.
    ///
    /// # Errors
    ///
    /// Returns [`RedisStoreError::PoolConnection`] if it fails to get a
    /// connection from the pool.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cot::config::CacheUrl;
    /// use cot::session::store::redis::RedisStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cot::session::store::redis::RedisStoreError> {
    /// let store = RedisStore::new(&CacheUrl::from("redis://127.0.0.1/"))?;
    /// // Actual TCP connection happens here:
    /// let mut conn = store.get_connection().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_connection(&self) -> Result<deadpool_redis::Connection, RedisStoreError> {
        self.pool
            .get()
            .await
            .map_err(|err| RedisStoreError::PoolConnection(Box::new(err)))
    }
}

fn get_expiry_as_u64(expiry: OffsetDateTime) -> u64 {
    let now = OffsetDateTime::now_utc();
    expiry
        .unix_timestamp()
        .saturating_sub(now.unix_timestamp())
        .max(0)
        .unsigned_abs()
}

#[async_trait]
impl SessionStore for RedisStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let data: String = serde_json::to_string(&session_record)
            .map_err(|err| RedisStoreError::Serialize(Box::new(err)))?;
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::NX) // only create if the key does not exist.
            .with_expiration(SetExpiry::EX(get_expiry_as_u64(session_record.expiry_date)));

        for _ in 0..=MAX_COLLISION_RETRIES {
            let key = session_record.id.to_string();
            let set_ok: bool = conn
                .set_options(key, &data, options)
                .await
                .map_err(|err| RedisStoreError::Command(Box::new(err)))?;
            if set_ok {
                return Ok(());
            }
            // On collision, recycle the ID and try again.
            session_record.id = Id::default();
        }
        Err(RedisStoreError::TooManyIdCollisions(MAX_COLLISION_RETRIES))?
    }
    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let key: String = session_record.id.to_string();
        let data: String = serde_json::to_string(&session_record)
            .map_err(|err| RedisStoreError::Serialize(Box::new(err)))?;

        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::XX) // only update if the key exists.
            .with_expiration(SetExpiry::EX(get_expiry_as_u64(session_record.expiry_date)));
        let set_ok: bool = conn
            .set_options(key, data, options)
            .await
            .map_err(|err| RedisStoreError::Command(Box::new(err)))?;
        if !set_ok {
            let mut record = session_record.clone();
            self.create(&mut record).await?;
        }

        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let mut conn = self.get_connection().await?;
        let key = session_id.to_string();
        let data: Option<String> = conn
            .get(key)
            .await
            .map_err(|err| RedisStoreError::Command(Box::new(err)))?;
        if let Some(data) = data {
            let rec = serde_json::from_str::<Record>(&data)
                .map_err(|err| RedisStoreError::Deserialize(Box::new(err)))?;
            return Ok(Some(rec));
        }
        Ok(None)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let key = session_id.to_string();
        conn.del::<_, ()>(key)
            .await
            .map_err(|err| RedisStoreError::Command(Box::new(err)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::{env, io};

    use time::{Duration, OffsetDateTime};
    use tower_sessions::session::{Id, Record};

    use super::*;
    use crate::config::CacheUrl;
    async fn make_store() -> RedisStore {
        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let url = CacheUrl::from(redis_url);
        let store = RedisStore::new(&url).expect("failed to create RedisStore");
        store.get_connection().await.expect("get_connection failed");
        store
    }

    fn make_record() -> Record {
        Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        }
    }

    #[cot::test]
    #[ignore = "requires external Redis service"]
    async fn test_create_and_load() {
        let store = make_store().await;
        let mut rec = make_record();

        store.create(&mut rec).await.expect("create failed");
        let loaded = store.load(&rec.id).await.expect("load err");
        assert_eq!(Some(rec.clone()), loaded);
    }

    #[cot::test]
    #[ignore = "requires external Redis service"]
    async fn test_save_overwrites() {
        let store = make_store().await;
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        let mut rec2 = rec.clone();
        rec2.data.insert("x".into(), "y".into());
        store.save(&rec2).await.expect("save failed");

        let loaded = store.load(&rec.id).await.unwrap().unwrap();
        assert_eq!(rec2.data, loaded.data);
    }

    #[cot::test]
    #[ignore = "requires external Redis service"]
    async fn test_save_creates_if_missing() {
        let store = make_store().await;
        let rec = make_record();

        store.save(&rec).await.expect("save failed");

        let loaded = store.load(&rec.id).await.unwrap();
        assert_eq!(Some(rec), loaded);
    }

    #[cot::test]
    #[ignore = "requires external Redis service"]
    async fn test_delete() {
        let store = make_store().await;
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        store.delete(&rec.id).await.expect("delete failed");
        let loaded = store.load(&rec.id).await.unwrap();
        assert!(loaded.is_none());

        store.delete(&rec.id).await.expect("second delete");
    }

    #[cot::test]
    #[ignore = "requires external Redis service"]
    async fn test_create_id_collision() {
        let store = make_store().await;
        let expiry = OffsetDateTime::now_utc() + Duration::minutes(30);

        let mut r1 = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: expiry,
        };
        store.create(&mut r1).await.unwrap();

        let mut r2 = Record {
            id: r1.id,
            data: HashMap::default(),
            expiry_date: expiry,
        };
        store.create(&mut r2).await.unwrap();

        assert_ne!(r1.id, r2.id, "ID collision not resolved");

        let loaded1 = store.load(&r1.id).await.unwrap();
        let loaded2 = store.load(&r2.id).await.unwrap();
        assert!(loaded1.is_some() && loaded2.is_some());
    }

    #[cot::test]
    async fn test_from_redis_store_error_to_session_store_error() {
        let pool_err = io::Error::other("pool conn failure");
        let sess_err: session_store::Error =
            RedisStoreError::PoolConnection(Box::new(pool_err)).into();
        assert!(matches!(sess_err, session_store::Error::Backend(_)));

        let create_err = io::Error::other("pool creation failure");
        let sess_err: session_store::Error =
            RedisStoreError::PoolCreation(Box::new(create_err)).into();
        assert!(matches!(sess_err, session_store::Error::Backend(_)));

        let cmd_err = io::Error::other("redis command failure");
        let sess_err: session_store::Error = RedisStoreError::Command(Box::new(cmd_err)).into();
        assert!(matches!(sess_err, session_store::Error::Backend(_)));

        let ser_err = io::Error::other("serialization oops");
        let sess_err: session_store::Error = RedisStoreError::Serialize(Box::new(ser_err)).into();
        assert!(matches!(sess_err, session_store::Error::Encode(_)));

        let parse_err = serde_json::from_str::<Record>("not a json").unwrap_err();
        let sess_err: session_store::Error =
            RedisStoreError::Deserialize(Box::new(parse_err)).into();
        assert!(matches!(sess_err, session_store::Error::Decode(_)));

        let sess_err: session_store::Error = RedisStoreError::TooManyIdCollisions(99).into();
        assert!(matches!(sess_err, session_store::Error::Backend(_)));
    }
}
