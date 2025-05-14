//! File session store
//!
//! This module provides a session store that uses the file system to store
//! session records.
//!
//! # Examples
//!
//! ```
//! use cot::session::store::file::FileStore;
//!
//! let store = FileStore::new("/var/lib/cot/sessions").unwrap();
//! ```
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::{fs, io};

use async_trait::async_trait;
use thiserror::Error;
use tokio::fs::remove_file;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

/// Errors that can occur when using the File session store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileStoreError {
    /// An error occurred during an I/O operation.
    #[error(transparent)]
    IoError(#[from] io::Error),
    /// An error occurred during JSON serialization.
    #[error("JSON serialization error: {0}")]
    SerializeError(serde_json::Error),
    /// An error occurred during JSON deserialization.
    #[error("JSON serialization error: {0}")]
    DeserializeError(serde_json::Error),
}

impl From<FileStoreError> for session_store::Error {
    fn from(error: FileStoreError) -> session_store::Error {
        match error {
            FileStoreError::IoError(inner) => session_store::Error::Backend(inner.to_string()),
            FileStoreError::SerializeError(inner) => {
                session_store::Error::Encode(inner.to_string())
            }
            FileStoreError::DeserializeError(inner) => {
                session_store::Error::Decode(inner.to_string())
            }
        }
    }
}

/// A file-based session store implementation.
///
/// This store persists sessions in a directory on the file system, providing
/// a simple and lightweight session storage solution.
///
/// # Examples
///
/// ```
/// use cot::session::store::file::FileStore;
/// use time::{Duration, OffsetDateTime};
/// use tower_sessions::SessionStore;
/// use tower_sessions::session::{Id, Record};
///
/// let store = FileStore::new("/var/lib/cot/sessions").unwrap();
/// let mut record = Record {
///     id: Default::default(),
///     data: Default::default(),
///     expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
/// };
/// let _ = store.create(&mut record).await.unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStore {
    /// The directory to save session files.
    dir_path: Cow<'static, Path>,
}

impl FileStore {
    #[must_use]
    pub fn new(dir_path: impl Into<Cow<'static, Path>>) -> Result<Self, FileStoreError> {
        let dir: PathBuf = dir_path.into().into();
        fs::create_dir_all(&dir).map_err(FileStoreError::IoError)?;
        let canonical = dir.canonicalize().map_err(FileStoreError::IoError)?;
        Ok(Self {
            dir_path: canonical.into(),
        })
    }
}

#[async_trait]
impl SessionStore for FileStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        tokio::fs::create_dir_all(&self.dir_path)
            .await
            .map_err(FileStoreError::IoError)?;

        loop {
            let file_path = self.dir_path.join(record.id.to_string());
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&file_path)
            {
                Ok(mut file) => {
                    serde_json::to_writer(file, &record).map_err(FileStoreError::SerializeError)?;
                    break;
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    record.id = Id::default();
                    continue;
                }
                Err(err) => return Err(FileStoreError::IoError(err))?,
            }
        }

        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        match OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.dir_path.join(record.id.to_string()))
        {
            Ok(mut file) => {
                serde_json::to_writer(file, &record).map_err(FileStoreError::SerializeError)?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                // create the file if it does not exist.
                let mut record = record.clone();
                self.create(&mut record).await?;
            }
            Err(err) => Err(FileStoreError::IoError(err))?,
        }

        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let path = self.dir_path.join(session_id.to_string());
        if !path.is_file() {
            return Ok(None);
        }
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|err| FileStoreError::IoError(err))?;
        let out = serde_json::from_reader(file).map_err(FileStoreError::SerializeError)?;

        Ok(out)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let res = remove_file(self.dir_path.join(session_id.to_string())).await;
        match res {
            Ok(_) => {}
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    return Err(FileStoreError::IoError(e))?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use time::{Duration, OffsetDateTime};
    use tower_sessions::session::{Id, Record};

    use super::*;

    fn make_store() -> FileStore {
        let dir = tempdir().expect("failed to make tempdir");
        FileStore::new(dir.into_path()).expect("failed to init FileStore")
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
        let store = make_store();
        let mut rec = make_record();
        store.create(&mut rec).await.expect("create failed");
        let path = store.dir_path.join(rec.id.to_string());
        assert!(path.is_file(), "session file wasn't created");

        let loaded = store.load(&rec.id).await.unwrap();
        assert_eq!(Some(rec.clone()), loaded);
    }

    #[tokio::test]
    async fn test_save_overwrites() {
        let store = make_store();
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        let mut rec2 = rec.clone();
        rec2.data.insert("foo".into(), "bar".into());
        store.save(&rec2).await.expect("save failed");

        let loaded = store.load(&rec.id).await.unwrap().unwrap();
        assert_eq!(rec2.data, loaded.data);
    }

    #[tokio::test]
    async fn test_save_creates_if_missing() {
        let store = make_store();
        let rec = make_record();
        store.save(&rec).await.unwrap();

        let path = store.dir_path.join(rec.id.to_string());
        assert!(path.is_file());
    }

    #[tokio::test]
    async fn test_delete() {
        let store = make_store();
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        store.delete(&rec.id).await.unwrap();
        let path = store.dir_path.join(rec.id.to_string());
        assert!(!path.exists());

        store.delete(&rec.id).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_id_collision() {
        let store = make_store();
        let expiry = OffsetDateTime::now_utc() + Duration::minutes(30);

        let mut r1 = Record {
            id: Id::default(),
            data: Default::default(),
            expiry_date: expiry,
        };
        store.create(&mut r1).await.unwrap();

        let collision_path = store.dir_path.join(r1.id.to_string());
        let mut r2 = Record {
            id: r1.id,
            data: Default::default(),
            expiry_date: expiry,
        };
        store.create(&mut r2).await.unwrap();

        assert_ne!(r1.id, r2.id, "ID collision not resolved");
        let p1 = store.dir_path.join(r1.id.to_string());
        let p2 = store.dir_path.join(r2.id.to_string());
        assert!(p1.is_file() && p2.is_file());
    }
}
