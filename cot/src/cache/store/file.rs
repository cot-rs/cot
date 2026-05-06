//! File-based cache store implementation.
//!
//! This store uses the local file system as the backend for caching. It
//! provides atomic writes via sync-then-rename and active validation for
//! TTL-based expiration.
//!
//! # Examples
//!
//! ```
//! # use cot::cache::store::file::{FileStore, FileStorePoolConfig};
//! # use cot::cache::store::CacheStore;
//! # use cot::config::Timeout;
//! # use std::path::PathBuf;
//! # #[tokio::main]
//! # async fn main() {
//!
//! let path = PathBuf::from("./cache_data");
//! let store = FileStore::new(path.clone(), FileStorePoolConfig::builder()
//!     .worker_count(8)
//!     .queue_size(128)
//!     .acquisition_timeout_ms(2000)
//!     .waiting_timeout_ms(4000)
//!     .build()
//! )
//! .expect("Failed to initialize store");
//!
//! let key = "example_key".to_string();
//! let value = serde_json::json!({"data": "example_value"});
//!
//! store.insert(key.clone(), value.clone(), Timeout::default()).await.unwrap();
//!
//! let retrieved = store.get(&key).await.unwrap();
//! assert_eq!(retrieved, Some(value));
//!
//! # let _ = tokio::fs::remove_dir_all(&path).await;
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
use std::fs::TryLockError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use blake3::hash;
use chrono::{DateTime, Utc};
use cot_core::error::impl_into_cot_error;
use fs4::tokio::AsyncFileExt;
use serde_json::Value;
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::task::spawn_blocking;

use crate::cache::store::{CacheStore, CacheStoreError, CacheStoreResult};
use crate::config::Timeout;

const ERROR_PREFIX: &str = "file-based cache store error:";
const TEMPFILE_SUFFIX: &str = "tmp";

// Custom error messages for implementation related
// failure modes
const FILE_SYSTEM_BUSY: &str = "file-system too busy to carry out normal operation";
const POOL_QUEUE_FULL: &str = "file-store pool queue full";
const POOL_BUSY: &str = "file-store pool busy";
// Use this for cases that should not happen under normal
// operating conditions, e.g., runtime quirks.
const UNEXPECTED_ERROR: &str = "unexpected error";
const TIMEOUT_REACHED: &str = "file-store pool took to long to respond";

// This is a retry limit for edge-cases where a file has been
// created but failed to be locked immediately, where such case
// happens multiple times.
const INTERNAL_MAX_RETRIES: i32 = 5;

// This header offset skips exactly one i64 integer,
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

struct FileAcquisitionWork {
    path: PathBuf,
    file_handle_sender: tokio::sync::oneshot::Sender<Result<tokio::fs::File, FileCacheStoreError>>,
}

struct FileStorePool {
    permits: Arc<tokio::sync::Semaphore>,
    work_receiver: tokio::sync::mpsc::Receiver<FileAcquisitionWork>,
    acquisiton_timeout_duration: tokio::time::Duration,
}

/// Config builder for `FileStorePoolConfig`
///
/// This provides the config required to instantiate
/// a `FileStore` instance.
///
/// The `new()` method will provide default values for  `FileStorePoolConfig`
///
/// #  Examples
/// ```
/// use std::path::PathBuf;
///
/// use cot::cache::store::file::{FileStore, FileStorePoolConfig};
///
/// # #[tokio::main]
/// # async fn main() {
/// let store_path = PathBuf::from("cache");
///
/// // Using default values
/// let default_config = FileStorePoolConfig::builder().build();
///
/// assert_eq!(default_config.worker_count(), 10);
/// assert_eq!(default_config.queue_size(), 128);
/// assert_eq!(default_config.acquisition_timeout_ms(), 2000);
/// assert_eq!(default_config.waiting_timeout_ms(), 4000);
/// let store_default = FileStore::new(store_path.clone(), default_config).unwrap();
///
/// // Specifying values
/// let specified_config = FileStorePoolConfig::builder()
///     .worker_count(4)
///     .queue_size(100)
///     .acquisition_timeout_ms(1000)
///     .waiting_timeout_ms(2000)
///     .build();
///
/// assert_eq!(specified_config.worker_count(), 4);
/// assert_eq!(specified_config.queue_size(), 100);
/// assert_eq!(specified_config.acquisition_timeout_ms(), 1000);
/// assert_eq!(specified_config.waiting_timeout_ms(), 2000);
/// let store_custom = FileStore::new(store_path.clone(), specified_config).unwrap();
/// # let _ = tokio::fs::remove_dir_all(&store_path).await;
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct FileStorePoolConfigBuilder {
    file_store_pool_config: FileStorePoolConfig,
}

impl FileStorePoolConfigBuilder {
    /// Sets the maximum number of concurrent blocking tasks permitted for file
    /// lock acquisition.
    ///
    /// When a blocking task is spawned to acquire a file lock, it may remain
    /// occupied until the underlying operating system returns control to
    /// the application. If the filesystem becomes unresponsive, these tasks
    /// will remain allocated and unavailable for further requests until the
    /// kernel-level operation completes or fails.
    ///
    /// Setting this value to `0` opts out of spawning background tasks for
    /// contested file locks. In this configuration, it is recommended to
    /// also set `queue_size` to `0` to ensure that requests requiring a
    /// lock fail immediately rather than entering a pending state that
    /// cannot be serviced.
    ///
    /// If the maximum worker count has been reached and the associated request
    /// queue is full, any incoming request that requires file locking will
    /// return an error immediately to prevent further resource contention.
    #[must_use]
    pub fn worker_count(mut self, worker_count: usize) -> Self {
        self.file_store_pool_config.worker_count = worker_count;
        self
    }
    /// Sets the maximum number of waiting insertions allowed in the queue.
    ///
    /// Incoming requests that require file locking will return an
    /// error immediately if this queue is full.
    #[must_use]
    pub fn queue_size(mut self, queue_size: usize) -> Self {
        self.file_store_pool_config.queue_size = queue_size;
        self
    }
    /// Sets the  maximum duration (in milliseconds) to wait for lock
    /// acquisition.
    ///
    /// This does not include time spent waiting in the queue or
    /// the duration of I/O operations once the lock is acquired.
    #[must_use]
    pub fn acquisition_timeout_ms(mut self, acquisition_timeout_ms: u64) -> Self {
        self.file_store_pool_config.acquisition_timeout_ms = acquisition_timeout_ms;
        self
    }
    /// Sets the maximum duration (in milliseconds) for requests to wait until
    /// it can be processed.
    ///
    /// This only accounts for the time spent waiting in the queue.
    /// When the timeout is reached before the request gets the file,
    /// the request would get dropped and returns error.
    #[must_use]
    pub fn waiting_timeout_ms(mut self, waiting_timeout_ms: u64) -> Self {
        self.file_store_pool_config.waiting_timeout_ms = waiting_timeout_ms;
        self
    }

    /// Consumes this struct and builds the `FileStorePoolConfig`
    ///
    /// # Panics
    ///
    /// This method will panic if
    /// * a value > `usize::MAX >> 3`was provided in the `worker_count()` and
    ///   `queue_size()` setters.
    /// * `worker_count` == 0 with  `queue_size`!= 0 and vice-versa.
    /// * `waiting_timeout_ms` < `acquisition_timeout_ms`.
    #[must_use]
    pub fn build(self) -> FileStorePoolConfig {
        assert!(
            self.file_store_pool_config.worker_count <= usize::MAX >> 3,
            "the provided `worker_count` must not be bigger than usize::MAX >> 3"
        );
        assert!(
            self.file_store_pool_config.queue_size <= usize::MAX >> 3,
            "the provided `queue_size` must not be bigger than usize::MAX >> 3"
        );
        assert!(
            (self.file_store_pool_config.worker_count == 0)
                == (self.file_store_pool_config.queue_size == 0),
            "`queue_size` must be 0 when `worker_count` is 0 and vice versa"
        );
        assert!(
            self.file_store_pool_config.waiting_timeout_ms
                >= self.file_store_pool_config.acquisition_timeout_ms,
            "`waiting_timeout_ms` must be greater or equal to `acquisition_timeout_ms`"
        );

        self.file_store_pool_config
    }
}
/// Configuration for a file-backed cache store implementation.
///
/// This determines the concurrency limits and timeout behavior for
/// handling cache insertions that encounter file contention.
///
/// This configuration can be initialized using `FileStorePoolConfig::default()`
/// to use the default values.
///
/// Alternatively, the `FileStorePoolConfigBuilder` can be used
/// to provide custom properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileStorePoolConfig {
    worker_count: usize,
    queue_size: usize,
    acquisition_timeout_ms: u64,
    waiting_timeout_ms: u64,
}

impl FileStorePoolConfig {
    /// Instantiate the `FileStorePoolConfigBuilder` object
    ///
    /// This will provide the inner `FileStorePoolConfig` with its
    /// default values.
    ///
    /// # Examples
    /// ```
    /// use crate::cot::cache::store::file::{FileStore, FileStorePoolConfig};
    ///
    /// let default_config = FileStorePoolConfig::default();
    /// let default_config_from_builder = FileStorePoolConfig::builder().build();
    ///
    /// assert_eq!(default_config, default_config_from_builder);
    /// assert_eq!(default_config.worker_count(), 10);
    /// assert_eq!(default_config.queue_size(), 128);
    /// assert_eq!(default_config.acquisition_timeout_ms(), 2000);
    /// ```
    #[must_use]
    pub fn builder() -> FileStorePoolConfigBuilder {
        FileStorePoolConfigBuilder::default()
    }
    /// Returns the maximum number of concurrent blocking tasks permitted
    /// for file lock acquisition.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
    /// Returns the maximum number of requests allowed to wait in the
    /// queue.
    #[must_use]
    pub fn queue_size(&self) -> usize {
        self.queue_size
    }
    /// Returns the maximum duration (in milliseconds) a task will wait
    /// to acquire a file lock.
    ///
    /// This represents the timeout for the lock acquisition itself and
    /// excludes queue wait time and subsequent I/O duration.
    #[must_use]
    pub fn acquisition_timeout_ms(&self) -> u64 {
        self.acquisition_timeout_ms
    }
    /// Returns the maximum duration (in milliseconds) for requests to wait
    /// until it can be processed.
    ///
    /// This only accounts for the time spent waiting in the queue.
    /// When the timeout is reached before the request gets the file,
    /// the request would get dropped and returns error.
    #[must_use]
    pub fn waiting_timeout_ms(&self) -> u64 {
        self.waiting_timeout_ms
    }
}

impl Default for FileStorePoolConfig {
    fn default() -> Self {
        Self {
            worker_count: crate::config::DEFAULT_FILE_STORE_WORKER_COUNT,
            queue_size: crate::config::DEFAULT_FILE_STORE_QUEUE_SIZE,
            acquisition_timeout_ms: crate::config::DEFAULT_FILE_STORE_ACQUISITION_TIMEOUT_MS,
            waiting_timeout_ms: crate::config::DEFAULT_FILE_STORE_WAITING_TIMEOUT_MS,
        }
    }
}

impl FileStorePool {
    fn new(config: FileStorePoolConfig) -> (Self, tokio::sync::mpsc::Sender<FileAcquisitionWork>) {
        let (tx, rx) = tokio::sync::mpsc::channel(config.queue_size);
        let semaphore = tokio::sync::Semaphore::new(config.worker_count);
        (
            Self {
                acquisiton_timeout_duration: tokio::time::Duration::from_millis(
                    config.acquisition_timeout_ms,
                ),
                permits: Arc::new(semaphore),
                work_receiver: rx,
            },
            tx,
        )
    }

    async fn run(&mut self) {
        while let Some(data) = self.work_receiver.recv().await {
            let permits = self.permits.clone();
            let mut tx = data.file_handle_sender;

            let acquire_permit_result = tokio::select! {
                r = permits.acquire_owned() => Ok(r),
                _r = tokio::time::sleep(self.acquisiton_timeout_duration) => Err(FileCacheStoreError::TempFileCreation(POOL_BUSY.into())),
                _r = tx.closed() => {
                     Err(FileCacheStoreError::TempFileCreation(UNEXPECTED_ERROR.into()))},
            };

            match acquire_permit_result {
                Ok(Ok(acquired_permit)) => {
                    let cloned_temp_path = data.path.clone();

                    // We spawn the task here since
                    // 1. We don't want to spawn tasks just to wait on semaphore
                    // 2. We don't want the spawn_blocking await to block the main loop
                    tokio::spawn(async move {
                        let task = spawn_blocking(move || {
                            let file_handle = std::fs::OpenOptions::new()
                                .write(true)
                                .read(true)
                                .create(true)
                                .truncate(false)
                                .open(cloned_temp_path)
                                .map_err(|e| FileCacheStoreError::TempFileCreation(Box::new(e)))?;

                            file_handle
                                .lock()
                                .map_err(|e| FileCacheStoreError::TempFileCreation(Box::new(e)))?;

                            drop(acquired_permit);
                            Ok(tokio::fs::File::from_std(file_handle))
                        });
                        if let Ok(file_acquisition_result) = task.await {
                            // No-op on error because we can't really notify the consumers if the
                            // request was aborted.
                            let _ = tx.send(file_acquisition_result);
                        } else {
                            let _ = tx.send(Err(FileCacheStoreError::TempFileCreation(
                                UNEXPECTED_ERROR.into(),
                            )));
                        }
                    });
                }
                Ok(Err(_)) => {
                    // This should not trigger under normal condition
                    let _ = tx.send(Err(FileCacheStoreError::TempFileCreation(
                        UNEXPECTED_ERROR.into(),
                    )));
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            }
        }
    }
}

/// A file-backed cache store implementation.
///
/// This store uses the local file system for caching.
///
/// # Examples
/// ```
/// use std::path::Path;
///
/// use cot::cache::store::file::{FileStore, FileStorePoolConfig};
///
/// # #[tokio::main]
/// # async fn main() {
/// let store = FileStore::new(
///     Path::new("cache_dir"),
///     FileStorePoolConfig::builder()
///         .worker_count(10)
///         .queue_size(128)
///         .acquisition_timeout_ms(2000)
///         .build(),
/// )
/// .unwrap();
/// # let _ = tokio::fs::remove_dir_all("cache_dir").await;
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct FileStore {
    dir_path: Cow<'static, Path>,
    file_path_sender: tokio::sync::mpsc::Sender<FileAcquisitionWork>,
    waiting_timeout_duration: tokio::time::Duration,
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
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// use cot::cache::store::file::{FileStore, FileStorePoolConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// // Using a string slice
    /// let store = FileStore::new(
    ///     Path::new("cache"),
    ///     FileStorePoolConfig::builder()
    ///         .worker_count(10)
    ///         .queue_size(128)
    ///         .acquisition_timeout_ms(2000)
    ///         .waiting_timeout_ms(4000)
    ///         .build(),
    /// )
    /// .unwrap();
    ///
    /// // Using a PathBuf
    /// let path_from_pathbuf = PathBuf::from("cache_lib");
    /// let store = FileStore::new(
    ///     path_from_pathbuf.clone(),
    ///     FileStorePoolConfig::builder()
    ///         .worker_count(10)
    ///         .queue_size(128)
    ///         .acquisition_timeout_ms(2000)
    ///         .waiting_timeout_ms(4000)
    ///         .build(),
    /// )
    /// .unwrap();
    /// # let _ = tokio::fs::remove_dir_all("cache").await;
    /// # let _ = tokio::fs::remove_dir_all(path_from_pathbuf).await;
    /// # }
    /// ```
    pub fn new(
        dir: impl Into<Cow<'static, Path>>,
        config: FileStorePoolConfig,
    ) -> CacheStoreResult<Self> {
        let dir_path = dir.into();

        let (mut pool, tx) = FileStorePool::new(config);
        let store = Self {
            dir_path,
            file_path_sender: tx,
            waiting_timeout_duration: tokio::time::Duration::from_millis(config.waiting_timeout_ms),
        };

        store.create_dir_root_sync()?;
        tokio::spawn(async move {
            pool.run().await;
        });

        Ok(store)
    }

    fn create_dir_root_sync(&self) -> CacheStoreResult<()> {
        std::fs::create_dir_all(&self.dir_path)
            .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;

        // When a crash happens mid-flight, we may have some .tmp files
        // This ensures that we have no .tmp files lingering around on startup
        if let Ok(entries) = std::fs::read_dir(&self.dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.extension().is_some_and(|ext| ext == TEMPFILE_SUFFIX) {
                    let file = std::fs::OpenOptions::new()
                        .write(true)
                        .truncate(false)
                        .open(&path)
                        .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;
                    match file.try_lock() {
                        Ok(()) => {
                            let _ = std::fs::remove_file(path);
                        }
                        // No-op on this since the file is currently used.
                        // This process is intented to only clean orphaned .tmp files.
                        // Therefore, we don't steal the files here.
                        Err(TryLockError::WouldBlock) => {}
                        Err(TryLockError::Error(e)) => {
                            return Err(FileCacheStoreError::Io(Box::new(e)))?;
                        }
                    }
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
        file.set_len(0)
            .await
            .map_err(|e| FileCacheStoreError::Serialize(Box::new(e)))?;

        self.serialize_data(value, expiry, &mut file, &file_path)
            .await?;

        // Sync data to synchronize content to the disk
        file.sync_data()
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        // Unlock the file to ensure that the locked file is not
        // the switched file
        file.unlock_async()
            .await
            .map_err(|e| FileCacheStoreError::Io(Box::new(e)))?;

        // The return `Ok(())`on these two methods
        // is to anticipate the "unlocked file" window mentioned in
        // `create_file_temp()`.
        let new_file_handle = match OpenOptions::new().read(true).open(&file_path).await {
            Ok(handle) => handle,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(FileCacheStoreError::Io(Box::new(e)))?,
        };

        // Try to lock one last time, but for shared so that it won't block readers
        // If `rename()` fails, we check the error result. NotFound means someone stole
        // the file. Other errors would otherwise be propagated as legitimate
        // errors.
        match new_file_handle.try_lock_shared() {
            Ok(true) => {
                if let Err(e) = tokio::fs::rename(&file_path, self.dir_path.join(&key_hash)).await {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        return Ok(());
                    }
                    return Err(FileCacheStoreError::Io(Box::new(e)))?;
                }
            }
            // Other task is currently holding the file locked
            Ok(false) => return Ok(()),
            Err(e) => return Err(FileCacheStoreError::Io(Box::new(e)))?,
        }

        Ok(())
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

    // Check expiry also removes expired files.
    // This makes the read process more efficient with less error propagation
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

        // In supported platforms, this would be succesful. Consequently, when a
        // platform does not support this operation, it would always fail and we
        // handle it as unexpected error
        let expiry_offset = u64::try_from(EXPIRY_HEADER_OFFSET)
            .map_err(|_| FileCacheStoreError::Deserialize(UNEXPECTED_ERROR.into()))?;
        let mut buffer = Vec::new();

        // Advances cursor by the expiry header offset
        file.seek(SeekFrom::Start(expiry_offset))
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
    ) -> CacheStoreResult<(tokio::fs::File, PathBuf)> {
        let temp_path = self.dir_path.join(format!("{key_hash}.{TEMPFILE_SUFFIX}"));

        // We must let the loop to propagate upwards to catch sudden
        // missing cache directory.
        //
        // Then, it would be easier for us to wait for file creation
        // where we offload one lock check into the OS by using `create_new()`
        //
        // Therefore, the flow looks like this,
        // 1. `create_new()` -> fail, we check if the error is AlreadyExists.
        // 2. In a condition where (1) is triggered, we park the task into a
        // blocking thread managed by `FileStorePool`. The request waits on the
        // result using a oneshot channel for at most `acquisition_timeout_ms`
        // + I/O processing duration.
        // 3. The blocking thread will only wait for the existing file in
        // the temp_path
        //
        // This approach was chosen because we can't possibly (at least for now)
        // to create a file AND lock that file atomically.
        //
        // A window where the existing file may get renamed or deleted
        // is expected.
        //
        // Therefore, the blocking task is a pessimistic write.

        let mut retry_count = 0;
        let temp_file = loop {
            match OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .truncate(false)
                .open(&temp_path)
                .await
            {
                Ok(handle) => {
                    match handle.try_lock_exclusive() {
                        Ok(true) => break handle,
                        Ok(false) => {
                            retry_count += 1;
                            if retry_count > INTERNAL_MAX_RETRIES {
                                return Err(FileCacheStoreError::TempFileCreation(
                                    FILE_SYSTEM_BUSY.into(),
                                ))?;
                            }
                        }
                        Err(e) => return Err(FileCacheStoreError::TempFileCreation(Box::new(e)))?,
                    }
                    continue;
                }
                // Trigger to enter the task handoff
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let cloned_temp_path = temp_path.clone();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    match self.file_path_sender.try_send(FileAcquisitionWork {
                        path: cloned_temp_path,
                        file_handle_sender: tx,
                    }) {
                        Ok(()) => {
                            let rx_result = tokio::time::timeout(self.waiting_timeout_duration, rx)
                                .await
                                .map_err(|_| {
                                    FileCacheStoreError::TempFileCreation(TIMEOUT_REACHED.into())
                                })?;

                            let new_file_temp = rx_result.map_err(|_| {
                                FileCacheStoreError::TempFileCreation(UNEXPECTED_ERROR.into())
                            })?;

                            break new_file_temp?;
                        }
                        Err(_) => {
                            return Err(FileCacheStoreError::TempFileCreation(
                                POOL_QUEUE_FULL.into(),
                            ))?;
                        }
                    }
                }
                // Trigger to create the new directory
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => self
                    .create_dir_root()
                    .await
                    .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?,
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
    ) -> CacheStoreResult<Option<(tokio::fs::File, PathBuf)>> {
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
        // First, try to remove the whole thing.
        // If failure happens, we fallback to iterative removal
        if tokio::fs::remove_dir_all(&self.dir_path).await.is_ok() {
            tokio::fs::create_dir_all(&self.dir_path)
                .await
                .map_err(|e| FileCacheStoreError::DirCreation(Box::new(e)))?;
            return Ok(());
        }

        if let Ok(mut entries) = tokio::fs::read_dir(&self.dir_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Ok(file) = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(false)
                    .open(&path)
                {
                    match file.try_lock() {
                        // We can steal the .tmp files to prevent ghost files
                        Ok(()) | Err(TryLockError::WouldBlock) => {
                            let _ = tokio::fs::remove_file(&path).await;
                        }
                        Err(TryLockError::Error(e)) => {
                            return Err(FileCacheStoreError::Io(Box::new(e)))?;
                        }
                    }
                }
            }
        }

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

        // Cache eviction on contains_key() based on TTL
        self.check_expiry(&mut file, &file_path).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use tempfile::tempdir;

    use crate::cache::store::file::{
        FileCacheStoreError, FileStore, FileStorePoolConfig, POOL_BUSY, POOL_QUEUE_FULL,
        TIMEOUT_REACHED,
    };
    use crate::cache::store::{CacheStore, CacheStoreError};
    use crate::config::Timeout;

    fn make_store_path() -> std::path::PathBuf {
        tempdir().expect("failed to create dir").keep()
    }

    #[cot::test]
    async fn test_create_dir() {
        let path = make_store_path();
        let _ = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");

        assert!(path.exists());
        assert!(path.is_dir());

        tokio::fs::remove_dir_all(path)
            .await
            .expect("failed to cleanup tempdir");
    }

    #[cot::test]
    async fn test_create_dir_on_existing() {
        let path = make_store_path();
        let _ = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
        let _ = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init second store");

        assert!(path.exists());
        assert!(path.is_dir());

        tokio::fs::remove_dir_all(path)
            .await
            .expect("failed to cleanup tempdir");
    }

    #[cot::test]
    async fn test_insert_and_read_single() {
        let path = make_store_path();

        let store = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
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

        let store = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
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

        let store = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
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

        let store = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
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

        let store = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
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

        let store = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init store");
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

    // Ignored in miri since it currently doesn't support
    // blocking lock operation
    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_interference_during_write() {
        let path = make_store_path();
        let store =
            FileStore::new(path.clone(), FileStorePoolConfig::default()).expect("failed to init");
        let key = "test_key".to_string();
        let value = serde_json::json!({ "id": 1 });

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let (s, k, v) = (store.clone(), key.clone(), value.clone());
                tokio::spawn(async move { s.insert(k, v, Timeout::Never).await })
            })
            .collect();

        let _store_2 = FileStore::new(path.clone(), FileStorePoolConfig::default())
            .expect("failed to init interference");

        for h in handles {
            h.await.unwrap().expect("insert failed");
        }

        let res = store.read(&key).await.expect("read failed");
        assert_eq!(res.unwrap(), value);
        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_clear_during_write() {
        let path = make_store_path();
        let store =
            FileStore::new(path.clone(), FileStorePoolConfig::default()).expect("failed to init");
        let value = serde_json::json!({ "id": 1 });
        let key = "key";

        let mut handles = Vec::new();
        for i in 0..10 {
            let s = store.clone();
            let v = value.clone();
            handles.push(tokio::spawn(async move {
                if i % 2 == 0 {
                    // We currently implement the "aggressive clear."
                    // This removes the files and directory  at any cycle during write,
                    // so we can't be sure this does not error
                    let _ = s.insert(key.into(), v, Timeout::Never).await;
                } else {
                    s.clear().await.expect("clear should not fail");
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }
        let received = store.read(key).await.expect("failed to read from store");
        if let Some(received_value) = received {
            assert_eq!(received_value, value);
        }

        // However, we guarantee that after clear, the system is stable again
        store
            .insert(key.into(), value, Timeout::Never)
            .await
            .expect("write should not fail");
        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_thundering_write() {
        let path = make_store_path();
        let store =
            FileStore::new(path.clone(), FileStorePoolConfig::default()).expect("failed to init");
        let value = serde_json::json!({ "id": 1 });

        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let s = store.clone();
                let v = value.clone();
                tokio::spawn(async move { s.insert("key".into(), v, Timeout::Never).await })
            })
            .collect();

        for h in tasks {
            h.await.unwrap().expect("task panicked");
        }

        let retrieved = store.read("key").await.expect("failed to read from store");
        assert_eq!(retrieved.unwrap(), value);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_thundering_write_with_semaphore_congestion() {
        let path = make_store_path();
        let store = FileStore::new(
            path.clone(),
            FileStorePoolConfig::builder().worker_count(1).build(),
        )
        .expect("failed to init");
        let value = serde_json::json!({ "id": 1 });

        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let s = store.clone();
                let v = value.clone();
                tokio::spawn(async move { s.insert("key".into(), v, Timeout::Never).await })
            })
            .collect();

        for h in tasks {
            h.await.unwrap().expect("task panicked");
        }

        let retrieved = store.read("key").await.expect("failed to read from store");
        assert_eq!(retrieved.unwrap(), value);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_thundering_write_with_queue_full() {
        let path = make_store_path();
        let store = FileStore::new(
            path.clone(),
            FileStorePoolConfig::builder().queue_size(1).build(),
        )
        .expect("failed to init");
        let value = serde_json::json!({ "id": 1 });

        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let s = store.clone();
                let v = value.clone();
                tokio::spawn(async move { s.insert("key".into(), v, Timeout::Never).await })
            })
            .collect();

        for h in tasks {
            if let Ok(result) = h.await {
                let _ = result.map_err(|e| {
                    let file_store_error =
                        FileCacheStoreError::TempFileCreation(POOL_QUEUE_FULL.into());
                    let cache_store_error: CacheStoreError = file_store_error.into();

                    assert_eq!(e.to_string(), cache_store_error.to_string());
                });
            }
        }

        let retrieved = store.read("key").await.expect("failed to read from store");
        assert_eq!(retrieved.unwrap(), value);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_thundering_write_with_acquisition_timeout_exceeded() {
        let path = make_store_path();
        let store = FileStore::new(
            path.clone(),
            FileStorePoolConfig::builder()
                .worker_count(1)
                .acquisition_timeout_ms(0)
                .build(),
        )
        .expect("failed to init");
        let value = serde_json::json!({ "id": 1 });

        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let s = store.clone();
                let v = value.clone();
                tokio::spawn(async move { s.insert("key".into(), v, Timeout::Never).await })
            })
            .collect();

        for h in tasks {
            if let Ok(result) = h.await {
                let _ = result.map_err(|e| {
                    let file_store_error = FileCacheStoreError::TempFileCreation(POOL_BUSY.into());
                    let cache_store_error: CacheStoreError = file_store_error.into();

                    assert_eq!(e.to_string(), cache_store_error.to_string());
                });
            }
        }

        let retrieved = store.read("key").await.expect("failed to read from store");
        assert_eq!(retrieved.unwrap(), value);

        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[cfg_attr(miri, ignore)]
    #[cot::test]
    async fn test_thundering_write_with_waiting_timeout_exceeded() {
        let path = make_store_path();
        let store = FileStore::new(
            path.clone(),
            FileStorePoolConfig::builder()
                .worker_count(1)
                .acquisition_timeout_ms(0)
                .waiting_timeout_ms(0)
                .build(),
        )
        .expect("failed to init");
        let value = serde_json::json!({ "id": 1 });

        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let s = store.clone();
                let v = value.clone();
                tokio::spawn(async move { s.insert("key".into(), v, Timeout::Never).await })
            })
            .collect();

        for h in tasks {
            if let Ok(result) = h.await {
                let _ = result.map_err(|e| {
                    let file_store_error =
                        FileCacheStoreError::TempFileCreation(TIMEOUT_REACHED.into());
                    let cache_store_error: CacheStoreError = file_store_error.into();

                    assert_eq!(e.to_string(), cache_store_error.to_string());
                });
            }
        }

        // When ALL tasks enter handoff, they are subject to the timeout
        if let Some(retrieved) = store.read("key").await.expect("failed to read from store") {
            assert_eq!(retrieved, value);
        }
        let _ = tokio::fs::remove_dir_all(&path).await;
    }

    #[test]
    #[should_panic(expected = "`queue_size` must be 0")]
    fn test_invalid_file_store_pool_config_creation() {
        let _file_store = FileStorePoolConfig::builder()
            .worker_count(0)
            .queue_size(10)
            .acquisition_timeout_ms(2000)
            .build();
    }

    #[test]
    #[should_panic(expected = "must be greater or equal to")]
    fn test_invalid_timeout_file_store_pool_config_creation() {
        let _file_store = FileStorePoolConfig::builder()
            .waiting_timeout_ms(1999)
            .acquisition_timeout_ms(2000)
            .build();
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
