pub mod stores;

use std::sync::Arc;

use cot::config::CacheStoreTypeConfig;
use serde_json::Value;
use thiserror::Error;

use crate::cache::stores::memory::Memory;
use crate::cache::stores::{CacheStore, CacheStoreResult};
use crate::config::{CacheConfig, Timeout};
use crate::error::error_impl::impl_into_cot_error;

const CACHE_ERROR_PREFIX: &str = "cache error";
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CacheError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] stores::CacheStoreError),
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

    pub async fn get<K: AsRef<str>>(&self, key: K) -> CacheStoreResult<Option<Value>> {
        let k = self.format_key(key.as_ref());
        self.store.get(&k).await
    }

    pub async fn insert(&self, key: impl AsRef<str>, value: Value) -> CacheStoreResult<()> {
        let k = self.format_key(key.as_ref());
        self.store.insert(k, value, self.expiry.clone()).await
    }

    pub async fn insert_expiring<K: AsRef<str>>(
        &self,
        key: K,
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
    pub async fn insert_with<F, Fut>(&self, key: String, f: F) -> CacheStoreResult<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<Value>> + Send,
    {
        let value = f().await?;
        self.insert(key, value).await?;
        Ok(())
    }

    // Get the value for `key`, or compute, insert, and return it.
    pub async fn get_or_insert_with<F, Fut, K>(&self, key: K, f: F) -> CacheStoreResult<Value>
    where
        K: AsRef<str>,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<Value>> + Send,
    {
        let key = key.as_ref();
        let v = self.get(&key).await?;
        if let Some(value) = v {
            return Ok(value);
        }
        let value = f().await?;
        self.insert(key, value.clone()).await?;
        Ok(value)
    }

    // Get the value for `key`, or compute, insert it with an expiration, and return
    // it.
    pub async fn get_or_insert_expiring_with<F, Fut, K>(
        &self,
        key: K,
        f: F,
        expiry: Timeout,
    ) -> CacheStoreResult<Value>
    where
        K: AsRef<str>,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = CacheStoreResult<Value>> + Send,
    {
        let key = key.as_ref();
        let value = self.get(&key).await?;
        if let Some(value) = value {
            return Ok(value);
        }
        let value = f().await?;
        self.insert_expiring(key, value.clone(), expiry).await?;
        Ok(value)
    }
}

impl TryFrom<&CacheConfig> for Cache {
    type Error = CacheError;

    fn try_from(config: &CacheConfig) -> Result<Self, Self::Error> {
        let store_cfg = &config.store;

        let store = match store_cfg.store_type {
            CacheStoreTypeConfig::Memory => {
                let mem_store = Memory::new();
                Arc::new(mem_store) as Arc<(dyn CacheStore<Key = String, Value = Value>)>
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CacheConfig, CacheStoreTypeConfig, Timeout};
    use serde_json::json;
    use tokio::runtime::Runtime;

    fn rt() -> Runtime {
        Runtime::new().unwrap()
    }

    fn memory_cache() -> Cache {
        let config = CacheConfig::builder()
            .store(Some(crate::config::CacheStoreConfig {
                store_type: CacheStoreTypeConfig::Memory,
            }))
            .build();
        Cache::try_from(&config).unwrap()
    }

    #[test]
    fn test_cache_basic_operations() {
        let rt = rt();
        rt.block_on(async {
            let cache = memory_cache();
            let key = "foo";
            let value = json!({"bar": 42});
            // Insert and get
            cache.insert(key, value.clone()).await.unwrap();
            let got = cache.get(key).await.unwrap();
            assert_eq!(got, Some(value.clone()));
            // Remove
            cache.remove(key).await.unwrap();
            assert_eq!(cache.get(key).await.unwrap(), None);
        });
    }

    #[test]
    fn test_cache_clear_and_len() {
        let rt = rt();
        rt.block_on(async {
            let cache = memory_cache();
            cache.insert("a", json!(1)).await.unwrap();
            cache.insert("b", json!(2)).await.unwrap();
            assert_eq!(cache.len().await.unwrap(), 2);
            assert!(!cache.is_empty().await.unwrap());
            cache.clear().await.unwrap();
            assert_eq!(cache.len().await.unwrap(), 0);
            assert!(cache.is_empty().await.unwrap());
        });
    }

    #[test]
    fn test_cache_insert_with_and_get_or_insert_with() {
        let rt = rt();
        rt.block_on(async {
            let cache = memory_cache();
            let key = "lazy";
            let value = json!(123);
            cache
                .insert_with(key.to_string(), || async { Ok(value.clone()) })
                .await
                .unwrap();
            let got = cache.get(key).await.unwrap();
            assert_eq!(got, Some(value.clone()));
            // get_or_insert_with should return the existing value
            let got2 = cache
                .get_or_insert_with(key, || async { Ok(json!(999)) })
                .await
                .unwrap();
            assert_eq!(got2, value);
        });
    }

    #[test]
    fn test_cache_get_or_insert_expiring_with() {
        let rt = rt();
        rt.block_on(async {
            let cache = memory_cache();
            let key = "expiring";
            let value = json!(456);
            let expiry = Timeout::default();
            let got = cache
                .get_or_insert_expiring_with(key, || async { Ok(value.clone()) }, expiry)
                .await
                .unwrap();
            assert_eq!(got, value);
        });
    }

    #[test]
    fn test_cache_try_from_config_memory() {
        let cache = memory_cache();
        assert!(format!("{:?}", cache).contains("Cache"));
    }
}
