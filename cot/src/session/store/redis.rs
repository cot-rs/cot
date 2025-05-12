use async_trait::async_trait;
use cot::config::CacheUrl;
use cot::session::store::redis::RedisStoreError::PoolConnectionError;
use deadpool_redis::{Config, ConfigError, CreatePoolError, Pool as RedisPool, Runtime};
use redis::{AsyncCommands, Commands, Connection, ExistenceCheck, SetExpiry, SetOptions};
use thiserror::Error;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RedisStoreError {
    /// Pool creation or checkout failures
    #[error(transparent)]
    PoolConnectionError(#[from] deadpool_redis::PoolError),

    #[error(transparent)]
    PoolCreationError(#[from] CreatePoolError),

    /// Any Redis‐client command or protocol error
    #[error(transparent)]
    CommandError(#[from] redis::RedisError),

    /// JSON serialization failures
    #[error("Serialization error: {0}")]
    SerializeError(serde_json::Error),

    /// JSON deserialization failures
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

#[derive(Debug, Clone)]
pub struct RedisStore {
    pool: RedisPool,
    ttl_seconds: usize,
}

impl RedisStore {
    pub(crate) fn new(url: &CacheUrl, ttl_seconds: usize) -> Result<RedisStore, RedisStoreError> {
        let mut cfg = Config::from_url(url.as_str());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(RedisStoreError::PoolCreationError)?;

        Ok(Self { pool, ttl_seconds })
    }

    async fn get_connection(&self) -> Result<deadpool_redis::Connection, RedisStoreError> {
        self.pool.get().await.map_err(PoolConnectionError)
    }
}

#[async_trait]
impl SessionStore for RedisStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        let mut conn = self.get_connection().await?;
        let data: String =
            serde_json::to_string(&session_record).map_err(RedisStoreError::SerializeError)?;
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::EX(self.ttl_seconds as u64));

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
            .with_expiration(SetExpiry::EX(self.ttl_seconds as u64));
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
