use async_trait::async_trait;
use cot::config::CacheUrl;
use deadpool_redis::{Config, Pool as RedisPool, Runtime};
use redis::{AsyncCommands, Commands, Connection, ExistenceCheck, SetExpiry, SetOptions};
use thiserror::Error;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RedisStoreError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Create error: {0}")]
    CreateError(String),

    #[error("Save error: {0}")]
    SaveError(String),

    #[error("Delete error: {0}")]
    DeleteError(String),

    #[error("Load error: {0}")]
    LoadError(String),

    #[error("Json deserialization error: {0}")]
    DeserializeError(#[from] serde_json::Error),
}

impl From<RedisStoreError> for session_store::Error {
    fn from(err: RedisStoreError) -> session_store::Error {
        match err {
            RedisStoreError::ConnectionError(inner) => session_store::Error::Backend(inner),
            RedisStoreError::CreateError(inner) => session_store::Error::Backend(inner),
            RedisStoreError::SaveError(inner) => session_store::Error::Backend(inner),
            RedisStoreError::DeleteError(inner) => session_store::Error::Backend(inner),
            RedisStoreError::LoadError(inner) => session_store::Error::Backend(inner),
            RedisStoreError::DeserializeError(inner) => {
                session_store::Error::Backend(inner.to_string())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RedisStore {
    pool: RedisPool,
    namespace: String,
    ttl_seconds: usize,
}

impl RedisStore {
    pub(crate) fn new(
        url: &CacheUrl,
        namespace: impl Into<String>,
        ttl_seconds: usize,
    ) -> Result<RedisStore, RedisStoreError> {
        let mut cfg = Config::from_url(url.as_str());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|err| RedisStoreError::ConnectionError(err.to_string()))?;

        Ok(Self {
            pool,
            namespace: namespace.into(),
            ttl_seconds,
        })
    }

    fn generate_key(&self, key: impl Into<String>) -> String {
        format!("{}-{}", self.namespace, key.into())
    }

    async fn get_connection(&self) -> Result<deadpool_redis::Connection, RedisStoreError> {
        self.pool
            .get()
            .await
            .map_err(|err| RedisStoreError::ConnectionError(err.to_string()))
    }
}

#[async_trait]
impl SessionStore for RedisStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        println!("Creating session");
        let mut conn = self.get_connection().await?;
        let data: String = serde_json::to_string(&session_record)
            .map_err(|err| RedisStoreError::SaveError(err.to_string()))?;
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::EX(self.ttl_seconds as u64));

        loop {
            let id = session_record.id.to_string();
            let key: String = self.generate_key(id);
            let set_ok: bool = conn
                .set_options(key, &data, options)
                .await
                .map_err(|err| RedisStoreError::SaveError(err.to_string()))?;
            if set_ok {
                break;
            }
            session_record.id = Id::default();
        }
        Ok(())
    }
    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        println!("Saving session");
        let mut conn = self.get_connection().await?;
        let key: String = self.generate_key(session_record.id.to_string());
        let data: String = serde_json::to_string(&session_record)
            .map_err(|err| RedisStoreError::SaveError(err.to_string()))?;
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::XX)
            .with_expiration(SetExpiry::EX(self.ttl_seconds as u64));
        let set_ok: bool = conn
            .set_options(key, data, options)
            .await
            .map_err(|err| RedisStoreError::SaveError(err.to_string()))?;
        if !set_ok {
            let mut record = session_record.clone();
            self.create(&mut record).await?
        }
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        println!("Loading session");
        let mut conn = self.get_connection().await?;
        let key = self.generate_key(session_id.to_string());
        let data: Option<String> = conn
            .get(key)
            .await
            .map_err(|err| RedisStoreError::LoadError(err.to_string()))?;
        if let Some(data) = data {
            let rec = serde_json::from_str::<Record>(&data)
                .map_err(|err| RedisStoreError::DeserializeError(err))?;
            Ok(Some(rec))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        println!("Deleting session");
        let mut conn = self.get_connection().await?;
        let key = self.generate_key(session_id.to_string());
        let _: () = conn
            .del(key)
            .await
            .map_err(|err| RedisStoreError::DeleteError(err.to_string()))?;
        Ok(())
    }
}
