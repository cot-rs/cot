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
use async_trait::async_trait;
use deadpool_redis::{Config, CreatePoolError, Pool as RedisPool, Runtime};
use redis::{AsyncCommands, ExistenceCheck, SetExpiry, SetOptions};
use thiserror::Error;
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

use crate::config::CacheUrl;

#[derive(Debug, Error)]
/// Errors that can occur when using the Redis session store.
#[non_exhaustive]
pub enum RedisStoreError {
    /// An error occurred during a pool connection or checkout.
    #[error(transparent)]
    PoolConnectionError(#[from] deadpool_redis::PoolError),

    /// An error occurred during Redis connection pool creation.
    #[error(transparent)]
    PoolCreationError(#[from] CreatePoolError),

    /// An error occurred during a Redis command execution.
    #[error(transparent)]
    CommandError(#[from] redis::RedisError),

    /// An error occurred during JSON serialization.
    #[error("Serialization error: {0}")]
    SerializeError(serde_json::Error),

    /// An error occurred during JSON deserialization.
    #[error("Deserialization error: {0}")]
    DeserializeError(serde_json::Error),
}

impl From<RedisStoreError> for session_store::Error {
    fn from(err: RedisStoreError) -> session_store::Error {
        match err {
            RedisStoreError::PoolConnectionError(inner) => {
                session_store::Error::Backend(inner.to_string())
            }
            RedisStoreError::PoolCreationError(inner) => {
                session_store::Error::Backend(inner.to_string())
            }
            RedisStoreError::CommandError(inner) => {
                session_store::Error::Backend(inner.to_string())
            }
            RedisStoreError::SerializeError(inner) => {
                session_store::Error::Encode(inner.to_string())
            }
            RedisStoreError::DeserializeError(inner) => {
                session_store::Error::Decode(inner.to_string())
            }
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
/// use time::{Duration, OffsetDateTime};
/// use tower_sessions::SessionStore;
/// use tower_sessions::session::{Id, Record};
///
/// let store = RedisStore::new(&CacheUrl::from("redis://127.0.0.1/")).unwrap();
/// let mut record = Record {
///     id: Id::default(),
///     data: Default::default(),
///     expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
/// };
/// let _ = store.create(&mut record).await.unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct RedisStore {
    /// The Redis connection pool.
    pool: RedisPool,
}

impl RedisStore {
    pub fn new(url: &CacheUrl) -> Result<RedisStore, RedisStoreError> {
        let mut cfg = Config::from_url(url.as_str());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(RedisStoreError::PoolCreationError)?;

        Ok(Self { pool })
    }

    async fn get_connection(&self) -> Result<deadpool_redis::Connection, RedisStoreError> {
        self.pool
            .get()
            .await
            .map_err(RedisStoreError::PoolConnectionError)
    }
}

fn get_expiry_as_u64(expiry: OffsetDateTime) -> u64 {
    let now = OffsetDateTime::now_utc();
    expiry
        .unix_timestamp()
        .saturating_sub(now.unix_timestamp())
        .max(0) as u64
}

#[async_trait]
impl SessionStore for RedisStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let data: String =
            serde_json::to_string(&session_record).map_err(RedisStoreError::SerializeError)?;
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::EX(get_expiry_as_u64(session_record.expiry_date)));

        loop {
            let key = session_record.id.to_string();
            let set_ok: bool = conn
                .set_options(key, &data, options)
                .await
                .map_err(RedisStoreError::CommandError)?;
            if set_ok {
                break;
            }
            session_record.id = Id::default();
        }
        Ok(())
    }
    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let key: String = session_record.id.to_string();
        let data: String =
            serde_json::to_string(&session_record).map_err(RedisStoreError::SerializeError)?;
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::XX)
            .with_expiration(SetExpiry::EX(get_expiry_as_u64(session_record.expiry_date)));
        let set_ok: bool = conn
            .set_options(key, data, options)
            .await
            .map_err(RedisStoreError::CommandError)?;
        if !set_ok {
            let mut record = session_record.clone();
            self.create(&mut record).await?
        }
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let mut conn = self.get_connection().await?;
        let key = session_id.to_string();
        let data: Option<String> = conn.get(key).await.map_err(RedisStoreError::CommandError)?;
        if let Some(data) = data {
            let rec =
                serde_json::from_str::<Record>(&data).map_err(RedisStoreError::DeserializeError)?;
            return Ok(Some(rec));
        };
        Ok(None)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let key = session_id.to_string();
        let _: () = conn.del(key).await.map_err(RedisStoreError::CommandError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use redis::AsyncCommands;
    use time::{Duration, OffsetDateTime};
    use tower_sessions::session::{Id, Record};

    use super::*;
    use crate::config::CacheUrl;
    async fn make_store(ttl: usize) -> RedisStore {
        let url = CacheUrl::from("redis://127.0.0.1/");
        let store = RedisStore::new(&url).expect("failed to create RedisStore");
        let mut conn = store.get_connection().await.expect("get_connection failed");
        let _: () = conn.flushdb().await.expect("flushdb failed");
        store
    }

    fn make_record() -> Record {
        Record {
            id: Id::default(),
            data: Default::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        }
    }

    #[tokio::test]
    async fn test_create_and_load() {
        let store = make_store(60).await;
        let mut rec = make_record();

        store.create(&mut rec).await.expect("create failed");
        let loaded = store.load(&rec.id).await.expect("load err");
        assert_eq!(Some(rec.clone()), loaded);
    }

    #[tokio::test]
    async fn test_save_overwrites() {
        let store = make_store(60).await;
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        let mut rec2 = rec.clone();
        rec2.data.insert("x".into(), "y".into());
        store.save(&rec2).await.expect("save failed");

        let loaded = store.load(&rec.id).await.unwrap().unwrap();
        assert_eq!(rec2.data, loaded.data);
    }

    #[tokio::test]
    async fn test_save_creates_if_missing() {
        let store = make_store(60).await;
        let rec = make_record();

        store.save(&rec).await.expect("save failed");

        let loaded = store.load(&rec.id).await.unwrap();
        assert_eq!(Some(rec), loaded);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = make_store(60).await;
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        store.delete(&rec.id).await.expect("delete failed");
        let loaded = store.load(&rec.id).await.unwrap();
        assert!(loaded.is_none());

        store.delete(&rec.id).await.expect("second delete");
    }

    #[tokio::test]
    async fn test_create_id_collision() {
        let store = make_store(60).await;
        let expiry = OffsetDateTime::now_utc() + Duration::minutes(30);

        let mut r1 = Record {
            id: Id::default(),
            data: Default::default(),
            expiry_date: expiry,
        };
        store.create(&mut r1).await.unwrap();

        let mut conn = store.get_connection().await.unwrap();
        let fake = serde_json::to_string(&r1).unwrap();
        let _: () = conn.set(&r1.id.to_string(), fake).await.unwrap();

        let mut r2 = Record {
            id: r1.id,
            data: Default::default(),
            expiry_date: expiry,
        };
        store.create(&mut r2).await.unwrap();

        assert_ne!(r1.id, r2.id, "ID collision not resolved");

        let loaded1 = store.load(&r1.id).await.unwrap();
        let loaded2 = store.load(&r2.id).await.unwrap();
        assert!(loaded1.is_some() && loaded2.is_some());
    }
}
