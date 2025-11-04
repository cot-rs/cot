use crate::cache::store::{CacheStore, CacheStoreError};
use crate::config::CacheUrl;
use cot::cache::store::CacheStoreResult;
use cot::config::Timeout;
use deadpool_redis::{Config, Connection, Pool, Runtime};
use redis::{AsyncCommands, SetExpiry, SetOptions};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RedisCacheStoreError {
    #[error("redis pool creation error: {0}")]
    PoolCreation(Box<dyn std::error::Error + Send + Sync>),

    #[error("redis pool connection error: {0}")]
    PoolConnection(Box<dyn std::error::Error + Send + Sync>),

    #[error("redis command error: {0}")]
    RedisCommand(Box<dyn std::error::Error + Send + Sync>),

    #[error("invalid redis connection string: {0}")]
    InvalidConnectionString(String),

    #[error("Serialization error: {0}")]
    Serialize(String),
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

#[derive(Debug, Clone)]
pub struct Redis {
    pool: Pool,
}

impl Redis {
    pub async fn new(url: CacheUrl, pool_size: usize) -> CacheStoreResult<Self> {
        if !url.is_redis() {
            return Err(
                RedisCacheStoreError::InvalidConnectionString(url.as_str().to_string()).into(),
            );
        }
        let cfg = Config::from_url(url.as_str())
            .builder()
            .unwrap()
            .max_size(pool_size)
            .runtime(Runtime::Tokio1)
            .build()
            .unwrap();

        Ok(Self { pool: cfg })
    }

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
        let options = SetOptions::default();

        match expiry {
            Timeout::After(duration) => {
                options.with_expiration(SetExpiry::EX(duration.as_secs()));
            }
            Timeout::AtDateTime(dt) => {
                let unix_timestamp = dt.timestamp() as u64;
                options.with_expiration(SetExpiry::EXAT(unix_timestamp));
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
