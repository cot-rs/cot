//! File based cache store implementation.
//!
//! This implementation uses file system for caching
//!
//! TODO: add example

use chrono::{DateTime, Utc};
use md5::{Digest, Md5};
use std::borrow::Cow;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use serde_json::Value;
use thiserror::Error;

use crate::cache::store::{CacheStore, CacheStoreError, CacheStoreResult};
use crate::config::Timeout;
use crate::error::error_impl::impl_into_cot_error;

const ERROR_PREFIX: &str = "file based cache store error:";
const TEMPFILE_SUFFIX: &str = ".tmp";

/// Errors specific to the file based cache store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileCacheStoreError {
    /// An error occured during directory creation
    #[error("{ERROR_PREFIX} file dir creation error: {0}")]
    DirCreation(Box<dyn std::error::Error + Send + Sync>),

    /// An error occured during temp file creation
    #[error("{ERROR_PREFIX} file temp file creation error: {0}")]
    TempFileCreation(Box<dyn std::error::Error + Send + Sync>),

    /// An error occured during write/stream file
    #[error("{ERROR_PREFIX} file io error: {0}")]
    Io(Box<dyn std::error::Error + Send + Sync>),

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

    async fn write(&self, key: String, value: Value, expiry: Timeout) -> CacheStoreResult<()> {
        self.create_dir_root().await?; // create the dir if not exist

        let key_hash = self.create_key_hash(&key);
        let (mut file, file_path) = self.create_file_temp(&key_hash).await?;

        let proc_result: CacheStoreResult<()> = async {
            let buffer = self.serialize_data(value, expiry).await?;

            file.write_all(&buffer)
                .await
                .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

            Ok(())
        }
        .await;

        if let Err(e) = proc_result {
            let _ = tokio::fs::remove_file(&file_path).await;
            return Err(e);
        }

        // rename
        file.sync_all()
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;
        tokio::fs::rename(file_path, self.dir_path.join(&key_hash))
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        Ok(())
    }

    async fn read(&self, key: &str) -> CacheStoreResult<Option<Value>> {
        let (mut file, file_path) = match self.file_open(key).await? {
            Some(f) => f,
            None => return Ok(None),
        };

        match self.deserialize_data(&mut file).await? {
            Some(value) => Ok(Some(value)),
            None => {
                // delete on expired when read
                let _ = tokio::fs::remove_file(&file_path).await;
                Ok(None)
            }
        }
    }

    fn create_key_hash(&self, key: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(key.as_bytes());
        let key_hash_hex = hasher.finalize();
        format!("{:x}", key_hash_hex)
    }

    async fn serialize_data(&self, value: Value, expiry: Timeout) -> CacheStoreResult<Vec<u8>> {
        let timeout = expiry.canonicalize();
        let seconds: u64 = match timeout {
            Timeout::Never => u64::MAX,
            Timeout::AtDateTime(date_time) => date_time.timestamp() as u64,
            Timeout::After(_) => unreachable!("should've been converted by canonicalize"),
        };
        let timeout_header = seconds.to_le_bytes();

        let data = serde_json::to_string(&value)
            .map_err(|e| FileCacheStoreError::Serialize(Box::new(e)))?;

        let mut buffer: Vec<u8> = Vec::with_capacity(8 + data.len());
        buffer.extend_from_slice(&timeout_header);
        buffer.extend_from_slice(data.as_bytes());

        Ok(buffer)
    }

    async fn deserialize_data(
        &self,
        file: &mut tokio::fs::File,
    ) -> CacheStoreResult<Option<Value>> {
        let mut header: [u8; 8] = [0; 8];

        let _ = file
            .read_exact(&mut header)
            .await
            .map_err(|e| FileCacheStoreError::Deserialize(Box::new(e)))?;
        let seconds = u64::from_le_bytes(header);

        let expiry = match seconds {
            u64::MAX => Timeout::Never,
            _ => {
                let date_time = DateTime::from_timestamp(seconds as i64, 0)
                    .ok_or_else(|| FileCacheStoreError::Deserialize("date time corrupted".into()))?
                    .with_timezone(&Utc)
                    .fixed_offset();

                Timeout::AtDateTime(date_time)
            }
        };

        if expiry.is_expired(None) {
            return Ok(None);
        }

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        let value: Value = serde_json::from_slice(&buffer)
            .map_err(|e| FileCacheStoreError::Deserialize(Box::new(e)))?;

        Ok(Some(value))
    }

    async fn create_file_temp(
        &self,
        key_hash: &str,
    ) -> CacheStoreResult<(tokio::fs::File, std::path::PathBuf)> {
        let temp_path = self.dir_path.join(format!("{}{TEMPFILE_SUFFIX}", key_hash));

        let temp_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)
            .await
            .map_err(|e| FileCacheStoreError::TempFileCreation(Box::new(e)))?;

        Ok((temp_file, temp_path))
    }

    async fn file_open(
        &self,
        key: &str,
    ) -> CacheStoreResult<Option<(tokio::fs::File, std::path::PathBuf)>> {
        let key_hash = self.create_key_hash(key);
        let path = self.dir_path.join(&key_hash);
        match tokio::fs::OpenOptions::new().read(true).open(&path).await {
            Ok(f) => Ok(Some((f, path))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(FileCacheStoreError::Io(Box::new(e)).into()),
        }
    }
}

impl CacheStore for FileStore {
    async fn get(&self, key: &str) -> CacheStoreResult<Option<Value>> {
        match self.read(key).await? {
            Some(value) => Ok(Some(value)),
            None => Ok(None),
        }
    }

    async fn insert(&self, key: String, value: Value, expiry: Timeout) -> CacheStoreResult<()> {
        self.write(key, value, expiry).await?;
        Ok(())
    }

    async fn remove(&self, key: &str) -> CacheStoreResult<()> {
        if let Some((_file, file_path)) = self.file_open(key).await? {
            tokio::fs::remove_file(file_path)
                .await
                .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;
        }

        Ok(())
    }

    async fn clear(&self) -> CacheStoreResult<()> {
        todo!()
    }

    async fn approx_size(&self) -> CacheStoreResult<usize> {
        todo!()
    }

    async fn contains_key(&self, key: &str) -> CacheStoreResult<bool> {
        Ok(self.file_open(key).await?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::cache::store::CacheStore;
    use crate::cache::store::file::FileStore;
    use crate::config::Timeout;

    fn make_store_path() -> std::path::PathBuf {
        tempdir().expect("failed to create dir").keep()
    }

    #[cot::test]
    async fn test_create_dir() {
        let path = make_store_path();
        let _ = FileStore::new(path.clone()).expect("failed to init store");

        assert!(path.exists());
        assert!(path.is_dir());

        tokio::fs::remove_dir_all(path)
            .await
            .expect("failed to cleanup tempdir");
    }

    #[cot::test]
    async fn test_create_dir_on_existing() {
        let path = make_store_path();
        let _ = FileStore::new(path.clone()).expect("failed to init store");
        let _ = FileStore::new(path.clone()).expect("failed to init second store");

        assert!(path.exists());
        assert!(path.is_dir());

        tokio::fs::remove_dir_all(path)
            .await
            .expect("failed to cleanup tempdir");
    }

    #[cot::test]
    async fn test_insert_and_read_single() {
        let path = make_store_path();

        let store = FileStore::new(path.clone()).expect("failed to init store");
        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        store
            .insert(key.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");

        let retrieved = store.read(&key).await.expect("failed to read from store");

        assert!(retrieved.is_some(), "retrieved value should not be None");
        assert_eq!(
            retrieved.unwrap(),
            value,
            "retrieved value does not match inserted value"
        );

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_insert_and_read_after_delete_single() {
        let path = make_store_path();

        let store = FileStore::new(path.clone()).expect("failed to init store");
        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        store
            .insert(key.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");

        store.remove(&key).await.expect("failed to delete entry");

        let retrieved = store.read(&key).await.expect("failed to read from store");
        assert!(retrieved.is_none(), "retrieved value should not be Some");

        let _ = tokio::fs::remove_dir_all(&path).await;
    }
}
