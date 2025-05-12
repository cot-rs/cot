use std::borrow::Cow;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::{fs, io};

use async_trait::async_trait;
use thiserror::Error;
use tokio::fs::remove_file;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileStoreError {
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Failed to serialize the record to JSON.
    #[error("JSON serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl From<FileStoreError> for session_store::Error {
    fn from(error: FileStoreError) -> session_store::Error {
        match error {
            FileStoreError::Io(inner) => session_store::Error::Backend(inner.to_string()),
            FileStoreError::Serialize(inner) => session_store::Error::Backend(inner.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStore {
    dir_path: Cow<'static, Path>,
}

impl FileStore {
    #[must_use]
    pub fn new(dir_path: impl Into<Cow<'static, Path>>) -> Result<Self, FileStoreError> {
        let dir: PathBuf = dir_path.into().into();
        fs::create_dir_all(&dir).map_err(FileStoreError::Io)?;
        let canonicalized = dir.canonicalize().map_err(FileStoreError::Io)?;
        Ok(Self {
            dir_path: canonicalized.into(),
        })
    }
}

#[async_trait]
impl SessionStore for FileStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        tokio::fs::create_dir_all(&self.dir_path)
            .await
            .map_err(|err| FileStoreError::Io(err))?;

        loop {
            let file_path = self.dir_path.join(record.id.to_string());
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&file_path)
            {
                Ok(mut file) => {
                    serde_json::to_writer(file, &record)
                        .map_err(|err| FileStoreError::Serialize(err))?;
                    break;
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    record.id = Id::default();
                    continue;
                }
                Err(err) => return Err(FileStoreError::Io(err))?,
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
                serde_json::to_writer(file, &record)
                    .map_err(|err| FileStoreError::Serialize(err))?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                // create the file if it does not exist.
                let mut record = record.clone();
                self.create(&mut record).await?;
            }
            Err(err) => Err(FileStoreError::Io(err))?,
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
            .map_err(|err| FileStoreError::Io(err))?;
        let out = serde_json::from_reader(file).map_err(|err| FileStoreError::Serialize(err))?;

        Ok(out)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let res = remove_file(self.dir_path.join(session_id.to_string())).await;
        match res {
            Ok(_) => {}
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    return Err(session_store::Error::Backend(
                        "Failed to Delete".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}
