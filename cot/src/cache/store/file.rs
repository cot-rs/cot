//! File-based cache store implementation.
//!
//! This store uses the local file system as the backend for caching. It
//! provides atomic writes via sync-then-rename and active validation for
//! TTL-based expiration.
//!
//! # Examples
//!
//! ```no_run
//! # use cot::cache::store::file::FileStore;
//! # use cot::cache::store::CacheStore;
//! # use cot::config::Timeout;
//! # use std::path::PathBuf;
//! # #[tokio::main]
//! # async fn main() {
//!
//! let path = PathBuf::from("./cache_data");
//! let store = FileStore::new(path).expect("Failed to initialize store");
//!
//! let key = "example_key".to_string();
//! let value = serde_json::json!({"data": "example_value"});
//!
//! store.insert(key.clone(), value.clone(), Timeout::default()).await.unwrap();
//!
//! let retrieved = store.get(&key).await.unwrap();
//! assert_eq!(retrieved, Some(value));
//! # }
//! ```
//!
//! # Expiration Policy
//!
//! Cache files are evicted on `contains_key` and `get`.
//! No background collector is implemented.
//!
//! # Cache File Format
//!
//! The cache file consists of a timestamp header, which
//! currently is as long as the byte representation of
//! `DateTime`, an i64 integer.
//!
//! | Section       | Start-Index | End-Index | Size           |
//! |---------------|-------------|-----------|----------------|
//! | Expiry header | 0           | 7         | i64 (8 bytes)  |
//! | Cache data    | 8           | EOF       | length of data |
use std::borrow::Cow;
use std::path::Path;

use blake3::hash;
use chrono::{DateTime, Utc};
use fs4::fs_std::FileExt;
use fs4::tokio::AsyncFileExt;
use serde_json::Value;
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};

use crate::cache::store::{CacheStore, CacheStoreError, CacheStoreResult};
use crate::config::Timeout;
use crate::error::error_impl::impl_into_cot_error;

const ERROR_PREFIX: &str = "file-based cache store error:";
const TEMPFILE_SUFFIX: &str = "tmp";

// this is a Windows-specific error code
// when we try to rename a file where the lock
// might not be completely dropped
const ERROR_ACCESS_DENIED: i32 = 5;

// this header offset skips exactly one i64 integer,
// which is the basis of our current expiry timestamp
const EXPIRY_HEADER_OFFSET: usize = size_of::<i64>();

/// Errors specific to the file based cache store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileCacheStoreError {
    /// An error occured during directory creation
    #[error("{ERROR_PREFIX} directory creation error: {0}")]
    DirCreation(Box<dyn std::error::Error + Send + Sync>),

    /// An error occured during temp file creation
    #[error("{ERROR_PREFIX} temporary file creation error: {0}")]
    TempFileCreation(Box<dyn std::error::Error + Send + Sync>),

    /// An error occured during write/stream file
    #[error("{ERROR_PREFIX} I/O error: {0}")]
    Io(Box<dyn std::error::Error + Send + Sync>),

    /// An error occured during data serialization
    #[error("{ERROR_PREFIX} serialization error: {0}")]
    Serialize(Box<dyn std::error::Error + Send + Sync>),

    /// An error occured during data deserialization
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

/// A file-backed cache store implementation.
///
/// This store uses the local file system for caching.
///
/// # Examples
/// ```no_run
/// use std::path::Path;
///
/// use cot::cache::store::file::FileStore;
///
/// let store = FileStore::new(Path::new("./cache_dir")).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct FileStore {
    dir_path: Cow<'static, Path>,
}

impl FileStore {
    /// Creates a new `FileStore` at the specified directory.
    ///
    /// This will attempt to create the directory and its parents if they do not
    /// exist.
    ///
    /// # Errors
    ///
    /// Returns [`FileCacheStoreError::DirCreation`] if the directory cannot be
    /// created due to permissions or other I/O issues.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    ///
    /// use cot::cache::store::file::FileStore;
    ///
    /// // Using a string slice
    /// let path = PathBuf::from("./cache");
    /// let store = FileStore::new(path).unwrap();
    ///
    /// // Using a PathBuf
    /// let path = PathBuf::from("/var/lib/myapp/cache");
    /// let store = FileStore::new(path).unwrap();
    /// ```
    pub fn new(dir: impl Into<Cow<'static, Path>>) -> CacheStoreResult<Self> {
        let dir_path = dir.into();

        let store = Self { dir_path };
        store.create_dir_root_sync()?;

        Ok(store)
    }

    fn create_dir_root_sync(&self) -> CacheStoreResult<()> {
        std::fs::create_dir_all(&self.dir_path)
            .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;

        if let Ok(entries) = std::fs::read_dir(&self.dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.extension().is_some_and(|ext| ext == TEMPFILE_SUFFIX)
                    && let Ok(file) = std::fs::File::open(&path)
                    && file
                        .try_lock_exclusive()
                        .is_ok_and(|lock_aquired| lock_aquired)
                {
                    let _ = std::fs::remove_file(path);
                }
            }
        }

        Ok(())
    }

    async fn create_dir_root(&self) -> CacheStoreResult<()> {
        tokio::fs::create_dir_all(&self.dir_path)
            .await
            .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;

        Ok(())
    }

    async fn write(&self, key: String, value: Value, expiry: Timeout) -> CacheStoreResult<()> {
        let key_hash = FileStore::create_key_hash(&key);
        let (mut file, file_path) = self.create_file_temp(&key_hash).await?;

        self.serialize_data(value, expiry, &mut file, &file_path)
            .await?;

        // rename
        file.sync_data()
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        file.unlock_async()
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        match tokio::fs::rename(&file_path, self.dir_path.join(&key_hash)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // if the source temp file is gone, it means another thread
                // likely renamed it already or the directory was cleared.
                // In a thundering herd, this is actually a "success" state
                // because the data is already there.
                Ok(())
            }
            // this branch checks for access denied that might
            // happen during race conditions on Windows
            // when we try to rename the file. We are passing this
            // because another thread/process is probably writing
            // a newer cache file
            Err(e) if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) => Ok(()),
            Err(e) => Err(FileCacheStoreError::Io(Box::new(e)))?,
        }
    }

    async fn read(&self, key: &str) -> CacheStoreResult<Option<Value>> {
        let Some((mut file, file_path)) = self.open_file_for_reading(key).await? else {
            return Ok(None);
        };

        self.deserialize_data(&mut file, &file_path).await
    }

    fn create_key_hash(key: &str) -> String {
        let key_hash_hex = hash(key.as_bytes());
        format!("{key_hash_hex}")
    }

    async fn serialize_data(
        &self,
        value: Value,
        expiry: Timeout,
        file: &mut tokio::fs::File,
        file_path: &Path,
    ) -> CacheStoreResult<()> {
        let result = async {
            let timeout = expiry.canonicalize();
            let seconds: i64 = match timeout {
                Timeout::Never => i64::MAX,
                Timeout::AtDateTime(date_time) => date_time.timestamp(),
                Timeout::After(_) => unreachable!("should've been converted by canonicalize"),
            };

            let data = serde_json::to_vec(&value)
                .map_err(|e| FileCacheStoreError::Serialize(Box::new(e)))?;

            file.write_all(&seconds.to_le_bytes())
                .await
                .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

            file.write_all(&data)
                .await
                .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

            Ok(())
        }
        .await;

        if result.is_err() {
            let _ = tokio::fs::remove_file(file_path).await;
        }

        result
    }

    // check expiry also removes the file
    // when expired. This makes the read
    // process more efficient with less
    // error propagation
    async fn check_expiry(
        &self,
        file: &mut tokio::fs::File,
        file_path: &Path,
    ) -> CacheStoreResult<bool> {
        let mut header: [u8; EXPIRY_HEADER_OFFSET] = [0; EXPIRY_HEADER_OFFSET];

        let _ = file
            .read_exact(&mut header)
            .await
            .map_err(|e| FileCacheStoreError::Deserialize(Box::new(e)))?;
        let seconds = i64::from_le_bytes(header);
        // This may look inefficient, but this ensures portability
        // By making this method reset its own cursor,
        // the logic is reusable without the risk of forgetting to reset cursor
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        let expiry = if seconds == i64::MAX {
            Timeout::Never
        } else {
            let date_time = DateTime::from_timestamp(seconds, 0)
                .ok_or_else(|| FileCacheStoreError::Deserialize("date time corrupted".into()))?
                .with_timezone(&Utc)
                .fixed_offset();
            Timeout::AtDateTime(date_time)
        };

        if expiry.is_expired(None) {
            tokio::fs::remove_file(file_path)
                .await
                .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;
            return Ok(false);
        }

        Ok(true)
    }

    async fn deserialize_data(
        &self,
        file: &mut tokio::fs::File,
        file_path: &Path,
    ) -> CacheStoreResult<Option<Value>> {
        if !self.check_expiry(file, file_path).await? {
            return Ok(None);
        }

        let mut buffer = Vec::new();

        // advances cursor by the expiry header offset
        // EXPIRY_HEADER_OFFSET is a usize that stores
        // the size of i64. It is unlikely that this will
        // overflow.
        // This direct cast currently works without any other
        // wrapping addition or fallback conversion
        file.seek(SeekFrom::Start(EXPIRY_HEADER_OFFSET as u64))
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;
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
        let temp_path = self.dir_path.join(format!("{key_hash}.{TEMPFILE_SUFFIX}"));

        let temp_file = loop {
            match OpenOptions::new()
                .write(true)
                .read(true)
                .create(true)
                .truncate(false)
                .open(&temp_path)
                .await
            {
                Ok(handle) => {
                    if let Ok(lock_aquired) = handle.try_lock_exclusive()
                        && lock_aquired
                    {
                        handle
                            .set_len(0)
                            .await
                            .map_err(|e| FileCacheStoreError::TempFileCreation(Box::new(e)))?;

                        break handle;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // if this fails then we should bail regardless
                    self.create_dir_root()
                        .await
                        .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;
                }
                Err(e) => {
                    return Err(FileCacheStoreError::TempFileCreation(Box::new(e)))?;
                }
            }
        };

        Ok((temp_file, temp_path))
    }

    async fn open_file_for_reading(
        &self,
        key: &str,
    ) -> CacheStoreResult<Option<(tokio::fs::File, std::path::PathBuf)>> {
        let key_hash = FileStore::create_key_hash(key);
        let path = self.dir_path.join(&key_hash);
        match OpenOptions::new().read(true).open(&path).await {
            Ok(f) => Ok(Some((f, path))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(FileCacheStoreError::Io(Box::new(e)).into()),
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
        if let Some((_file, file_path)) = self.open_file_for_reading(key).await? {
            tokio::fs::remove_file(file_path)
                .await
                .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;
        }

        Ok(())
    }

    async fn clear(&self) -> CacheStoreResult<()> {
        if let Err(e) = tokio::fs::remove_dir_all(&self.dir_path).await {
            // if not found try to continue, don't dip
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(FileCacheStoreError::Io(Box::new(e)).into());
            }
        }
        // even though write is self healing, this minimizes result variants on other
        // methods
        tokio::fs::create_dir_all(&self.dir_path)
            .await
            .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;
        Ok(())
    }

    async fn approx_size(&self) -> CacheStoreResult<usize> {
        let mut entries = match tokio::fs::read_dir(&self.dir_path).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(FileCacheStoreError::Io(Box::new(e)).into()),
        };

        let mut total_size: usize = 0;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let is_temp = path.extension().is_some_and(|ext| ext == TEMPFILE_SUFFIX);

            if let Ok(meta) = entry.metadata().await
                && meta.is_file()
                && !is_temp
            {
                total_size += 1;
            }
        }

        Ok(total_size)
    }

    async fn contains_key(&self, key: &str) -> CacheStoreResult<bool> {
        let Ok(Some((mut file, file_path))) = self.open_file_for_reading(key).await else {
            return Ok(false);
        };

        // cache eviction on contains_key() based on TTL
        self.check_expiry(&mut file, &file_path).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use tempfile::tempdir;
    use tokio::sync::Barrier;
    use tokio::time::sleep;

    use crate::cache::store::file::{FileCacheStoreError, FileStore};
    use crate::cache::store::{CacheStore, CacheStoreError};
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

    #[cot::test]
    async fn test_clear_double_free() {
        let path = make_store_path();

        let store = FileStore::new(path.clone()).expect("failed to init store");
        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        store
            .insert(key.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");

        store.clear().await.expect("failed to clear");
        store
            .clear()
            .await
            .expect("failed to clear the second time");

        let retrieved = store.read(&key).await.expect("failed to read from store");

        assert!(path.is_dir(), "path must be dir");
        assert!(retrieved.is_none(), "retrieved value should not be Some");

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_approx_size() {
        let path = make_store_path();

        let store = FileStore::new(path.clone()).expect("failed to init store");
        let key = "test_key".to_string();
        let key_2 = "test_key_2".to_string();

        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        store
            .insert(key.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");
        store
            .insert(key.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");
        store
            .insert(key_2.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");

        let data_length: usize = 2;

        let entry_length = store
            .approx_size()
            .await
            .expect("failed to get approx file");

        assert_eq!(data_length, entry_length);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_contains_key() {
        let path = make_store_path();

        let store = FileStore::new(path.clone()).expect("failed to init store");
        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        store
            .insert(key.clone(), value.clone(), Timeout::Never)
            .await
            .expect("failed to insert data to store");

        let exist = store
            .contains_key(&key)
            .await
            .expect("failed to check key existence");

        assert!(exist);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_expiration_integrity() {
        let path = make_store_path();

        let store = FileStore::new(path.clone()).expect("failed to init store");
        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        let past = Utc::now() - Duration::from_secs(1);
        let past_fixed = past.fixed_offset();
        let expiry = Timeout::AtDateTime(past_fixed);

        store
            .insert(key.clone(), value.clone(), expiry)
            .await
            .expect("failed to insert data to store");

        // test file is None
        let retrieved = store.get(&key).await.expect("failed to read from store");
        assert!(retrieved.is_none());

        // test file doesn't exist
        let exist = store
            .contains_key(&key)
            .await
            .expect("failed to check key existence");
        assert!(!exist);

        // test size is 0
        let size = store.approx_size().await.expect("failed to check size");
        assert_eq!(size, 0);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_interference_during_write() {
        let path = make_store_path();
        let store = FileStore::new(path.clone()).expect("failed to init store");

        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        let num_task = 10;
        let barrier = Arc::new(Barrier::new(num_task + 1));
        let mut handles = Vec::with_capacity(num_task);

        for _ in 0..num_task {
            let b = barrier.clone();
            let k = key.clone();
            let s = store.clone();
            let v = value.clone();

            handles.push(tokio::spawn(async move {
                b.wait().await;
                s.insert(k, v, Timeout::Never)
                    .await
                    .expect("failed to insert data to store");

                sleep(Duration::from_millis(10)).await;
            }));
        }

        barrier.wait().await;

        tokio::task::yield_now().await;

        let _store_2 = FileStore::new(path.clone()).expect("failed to init store");

        for handle in handles {
            handle.await.expect("task panicked");
        }

        let retrieved = store.read(&key).await.expect("failed to read from store");
        if let Some(found) = retrieved {
            assert_eq!(found, value);
        }

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_thundering_write() {
        let path = make_store_path();
        let store = FileStore::new(path.clone()).expect("failed to init store");

        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1, "message": "hello world" });

        let num_task = 10;
        let barrier = Arc::new(Barrier::new(num_task));
        let mut handles = Vec::with_capacity(num_task);

        for _ in 0..num_task {
            let b = barrier.clone();
            let k = key.clone();
            let s = store.clone();
            let v = value.clone();

            handles.push(tokio::spawn(async move {
                b.wait().await;
                s.insert(k, v, Timeout::Never)
                    .await
                    .expect("failed to insert data to store");

                sleep(Duration::from_millis(10)).await;
            }));
        }

        for handle in handles {
            handle.await.expect("task panicked");
        }

        let retrieved = store.read(&key).await.expect("failed to read from store");

        assert!(retrieved.is_some(), "retrieved value should not be None");

        // if this reads properly then its not torn
        assert_eq!(
            retrieved.unwrap(),
            value,
            "retrieved value does not match inserted value"
        );

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cot::test]
    async fn test_from_file_cache_store_error_to_cache_store_error() {
        let file_error = FileCacheStoreError::Io(Box::new(std::io::Error::other("disk failure")));
        let cache_error: CacheStoreError = file_error.into();
        assert_eq!(
            cache_error.to_string(),
            "cache store error: backend error: file-based cache store error: I/O error: disk failure"
        );

        let file_error =
            FileCacheStoreError::Serialize(Box::new(std::io::Error::other("json fail")));
        let cache_error: CacheStoreError = file_error.into();
        assert_eq!(
            cache_error.to_string(),
            "cache store error: serialization error: file-based cache store error: serialization error: json fail"
        );

        let file_error =
            FileCacheStoreError::Deserialize(Box::new(std::io::Error::other("corrupt header")));
        let cache_error: CacheStoreError = file_error.into();
        assert_eq!(
            cache_error.to_string(),
            "cache store error: deserialization error: file-based cache store error: deserialization error: corrupt header"
        );

        let file_error =
            FileCacheStoreError::DirCreation(Box::new(std::io::Error::other("permission denied")));
        let cache_error: CacheStoreError = file_error.into();
        assert_eq!(
            cache_error.to_string(),
            "cache store error: backend error: file-based cache store error: directory creation error: permission denied"
        );

        let file_error =
            FileCacheStoreError::TempFileCreation(Box::new(std::io::Error::other("no space left")));
        let cache_error: CacheStoreError = file_error.into();
        assert_eq!(
            cache_error.to_string(),
            "cache store error: backend error: file-based cache store error: temporary file creation error: no space left"
        );
    }
}
