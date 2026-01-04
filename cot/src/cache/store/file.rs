//! File based cache store implementation.
//!
//! This implementation uses file system for caching
//!
//! TODO: add example

use std::borrow::Cow;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

use crate::cache::store::{CacheStore, CacheStoreError, CacheStoreResult};
use crate::error::error_impl::impl_into_cot_error;

const ERROR_PREFIX: &str = "file based cache store error:";

/// Errors specific to the file based cache store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileCacheStoreError {
    /// An error occured during directory creation
    #[error("{ERROR_PREFIX} file dir creation error: {0}")]
    DirCreation(Box<dyn std::error::Error + Send + Sync>),

    // TODO: add more errors

    // To fullfil trait
    /// TODO: add docs
    #[error("{ERROR_PREFIX} serialization error: {0}")]
    Serialize(Box<dyn std::error::Error + Send + Sync>),

    /// TODO: add docs
    #[error("{ERROR_PREFIX} deserialization error: {0}")]
    Deserialize(Box<dyn std::error::Error + Send + Sync>),
}

impl_into_cot_error!(FileCacheStoreError);

impl From<FileCacheStoreError> for CacheStoreError {
    fn from(err: FileCacheStoreError) -> Self {
        let full = err.to_string();

        match err {
            FileCacheStoreError::Serialize(_) => CacheStoreError::Serialize(full),
            FileCacheStoreError::Deserialize(_) => CacheStoreError::Deserialize(full),
            _ => CacheStoreError::Backend(full),
        }
    }
}

/// File based cache store implementation
///
/// This implementation uses file system for caching
///
/// TODO: add example

#[derive(Debug, Clone)]
pub struct FileStore {
    dir_path: Cow<'static, Path>,
}

impl FileStore {
    /// TODO: add docs
    pub fn new(dir: impl Into<Cow<'static, Path>>) -> CacheStoreResult<Self> {
        let dir_path = dir.into();

        let store = Self { dir_path };
        store.create_dir_sync_root()?;

        Ok(store)
    }

    fn create_dir_sync_root(&self) -> CacheStoreResult<()> {
        std::fs::create_dir_all(&self.dir_path)
            .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;

        Ok(())
    }

    async fn create_dir_root(&self) -> CacheStoreResult<()> {
        tokio::fs::create_dir_all(&self.dir_path)
            .await
            .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;

        Ok(())
    }
}
