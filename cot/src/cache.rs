pub mod stores;

use std::sync::Arc;

use cot::config::CacheStoreTypeConfig;
use serde_json::Value;
use thiserror::Error;

use crate::cache::stores::memory::Memory;
use crate::cache::stores::{CacheStore, CacheStoreResult};
use crate::config::{CacheConfig, Timeout};
use crate::error::error_impl::impl_into_cot_error;

#[derive(Clone, Debug, Error)]
pub enum CacheError {
    #[error("cache error: {0}")]
    Backend(String),
    #[error("cache error: {0}")]
    InvalidKey(String),
    #[error("cache error: {0}")]
    InvalidValue(String),
    #[error("cache error: {0}")]
    InvalidExpiry(String),
    #[error("cache error: {0}")]
    InvalidConfig(String),
}


impl_into_cot_error!(CacheError, INTERNAL_SERVER_ERROR);
#[derive(Clone)]
pub struct Cache {
    store: Arc<dyn CacheStore<Key = String, Value = Value>>,
    prefix: Option<String>,
    expiry: Timeout,
}

impl Cache {
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

    fn format_key<K: AsRef<str>>(&self, key: K) -> String {
        let k = key.as_ref();
        if let Some(pref) = &self.prefix {
            return format!("{}:{}", pref, k);
        }
        k.to_string()
    }

    pub async fn get(&self, key: impl AsRef<str>) -> CacheStoreResult<Option<Value>> {
        let k = self.format_key(key);
        self.store.get(&k).await
    }

    pub async fn insert(&self, key: impl AsRef<str>, value: Value) -> CacheStoreResult<()> {
        let k = self.format_key(key);
        self.store.insert(k, value, self.expiry.clone()).await
    }

    pub async fn insert_expiring(
        &self,
        key: impl AsRef<str>,
        value: Value,
        expiry: Timeout,
    ) -> CacheStoreResult<()> {
        let k = self.format_key(key);
        self.store.insert(k, value, expiry).await
    }

    pub async fn remove(&self, key: impl AsRef<str>) -> CacheStoreResult<()> {
        let k = self.format_key(key);
        self.store.remove(&k).await
    }

    pub async fn clear(&self) -> CacheStoreResult<()> {
        self.store.clear().await
    }

    pub async fn len(&self) -> CacheStoreResult<usize> {
        self.store.len().await
    }

    pub async fn is_empty(&self) -> CacheStoreResult<bool> {
        self.store.is_empty().await
    }

    //Insert a lazily computed value under the given key.
    // async fn insert_with<F, Fut>(&self, key: Self::Key, f: F) -> CacheStoreResult<()>
    // where
    //     F: FnOnce() -> Fut + Send,
    //     Fut: Future<Output = CacheStoreResult<Value>> + Send,
    // {
    //     let value = f().await?;
    //     self.insert(key, value).await?;
    //     Ok(())
    // }

    // Get the value for `key`, or compute, insert, and return it.
    // async fn get_or_insert_with<F, Fut>(&self, key: Self::Key, f: F) -> CacheStoreResult<Value>
    // where
    //     F: FnOnce() -> Fut + Send,
    //     Fut: Future<Output = CacheStoreResult<Value>> + Send,
    // {
    //     let v = self.get(&key).await?;
    //     if let Some(value) = v {
    //         return Ok(value);
    //     }
    //     let value = f().await?;
    //     self.insert(key, value).await?;
    //     Ok(value)
    // }

    // Get the value for `key`, or compute, insert it with an expiration, and return
    // it.
    // async fn get_or_insert_expiring_with<F, Fut>(
    //     &self,
    //     key: Self::Key,
    //     f: F,
    //     expiry: Expiry,
    // ) -> CacheStoreResult<Value>
    // where
    //     F: FnOnce() -> Fut + Send,
    //     Fut: Future<Output = CacheStoreResult<Value>> + Send,
    // {
    //     let v = self.get_or_insert_with(key, f).await?;
    //     self.insert_expiring(key, v, expiry).await?;
    //     Ok(v)
    // }
}

impl TryFrom<&CacheConfig> for Cache {
    type Error = CacheError;

    fn try_from(config: &CacheConfig) -> Result<Self, Self::Error> {
        let store_cfg = &config.store;

        let store = match store_cfg.store_type {
            CacheStoreTypeConfig::Memory => {
                let mem_store = Memory::new();
                Arc::new(mem_store)  as Arc<dyn CacheStore<Key = String, Value = Value>>
            }
            _ => {
                unimplemented!();
            }
        };

        let this = Self::new(store, config.prefix.clone(), config.timeout.clone());
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