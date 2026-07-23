//! Database-backed session store.
//!
//! This module provides a session store implementation that persists session
//! records in a database using the Cot ORM. The database connection is
//! typically set via the [`cot::config::DatabaseConfig`] in the project
//! configuration and then passed to the `DbStore` constructor.
//!
//! # Examples
//!
//! ```
//! use std::sync::Arc;
//!
//! use cot::db::Database;
//! use cot::session::store::db::DbStore;
//!
//! # #[tokio::main]
//! # async fn main() -> cot::Result<()> {
//! let db = Database::new("sqlite://:memory:").await?;
//! let store = DbStore::new(db);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

use crate::db::{Auto, Database, DatabaseBackend, DatabaseError, Model, query};
use crate::session::db::Session;
use crate::session::store::{ERROR_PREFIX, MAX_COLLISION_RETRIES};
use crate::utils::chrono::DateTimeWithOffsetAdapter;

/// Errors that can occur while interacting with the database session store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DbStoreError {
    /// An error occurred while interacting with the database.
    #[error("{ERROR_PREFIX} {0} ")]
    DatabaseError(#[from] DatabaseError),
    /// The record ID collided too many times while saving in the database.
    #[error("{ERROR_PREFIX} session‐id collision retried too many times ({0})")]
    TooManyIdCollisions(u32),
    /// An error occurred during JSON serialization.
    #[error("{ERROR_PREFIX} JSON serialization error: {0}")]
    Serialize(Box<dyn Error + Send + Sync>),
    /// An error occurred during JSON deserialization.
    #[error("{ERROR_PREFIX} JSON serialization error: {0}")]
    Deserialize(Box<dyn Error + Send + Sync>),
}

impl From<DbStoreError> for session_store::Error {
    fn from(err: DbStoreError) -> Self {
        match err {
            DbStoreError::DatabaseError(db_err) => {
                session_store::Error::Backend(db_err.to_string())
            }
            DbStoreError::Serialize(ser_err) => session_store::Error::Encode(ser_err.to_string()),
            DbStoreError::Deserialize(de_err) => session_store::Error::Decode(de_err.to_string()),
            other => session_store::Error::Backend(other.to_string()),
        }
    }
}

/// A database-backed session store.
///
/// This store uses a database to persist session records, allowing for
/// session data to be stored across application restarts.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use cot::db::Database;
/// use cot::session::store::db::DbStore;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), cot::session::store::db::DbStoreError> {
/// let db = Database::new("sqlite://:memory:").await?;
/// let store = DbStore::new(db);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct DbStore {
    connection: Database,
}

impl DbStore {
    /// Creates a new `DbStore` instance with the provided database connection.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use cot::db::Database;
    /// use cot::session::store::db::DbStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cot::session::store::db::DbStoreError> {
    /// let db = Database::new("sqlite://:memory:").await?;
    /// let store = DbStore::new(db);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new(connection: Database) -> DbStore {
        DbStore { connection }
    }

    async fn create_in_executor<DB: DatabaseBackend>(
        &self,
        mut db: DB,
        record: &mut Record,
    ) -> session_store::Result<()> {
        for _ in 0..=MAX_COLLISION_RETRIES {
            let mut model = Self::build_session_model(record)?;

            let res = db.insert(&mut model).await;
            match res {
                Ok(()) => {
                    return Ok(());
                }
                Err(DatabaseError::UniqueViolation) => {
                    // If a unique constraint violation occurs, we need to generate a new ID
                    record.id = Id::default();
                }
                Err(err) => return Err(DbStoreError::DatabaseError(err))?,
            }
        }
        Err(DbStoreError::TooManyIdCollisions(MAX_COLLISION_RETRIES))?
    }

    fn build_session_model(record: &Record) -> session_store::Result<Session> {
        let key = record.id.to_string();
        let data = serde_json::to_string(&record.data)
            .map_err(|err| DbStoreError::Serialize(Box::new(err)))?;
        let expiry = DateTimeWithOffsetAdapter::try_from(record.expiry_date)
            .map_err(|err| session_store::Error::Backend(err.to_string()))?
            .into_chrono_db_safe();

        Ok(Session {
            id: Auto::auto(),
            key,
            data,
            expiry,
        })
    }

    async fn update_session_model<DB: DatabaseBackend>(
        db: DB,
        mut model: Session,
        record: &Record,
    ) -> session_store::Result<()> {
        model.data = serde_json::to_string(&record.data)
            .map_err(|err| DbStoreError::Serialize(Box::new(err)))?;
        model.expiry = DateTimeWithOffsetAdapter::try_from(record.expiry_date)
            .map_err(|err| session_store::Error::Backend(err.to_string()))?
            .into_chrono_db_safe();
        model
            .update(db)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for DbStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        self.create_in_executor(&self.connection, record).await
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let mut transaction = self
            .connection
            .begin()
            .await
            .map_err(DbStoreError::DatabaseError)?;

        let key = record.id.to_string();
        let query = query!(Session, $key == key.clone())
            .get(&mut transaction)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        if let Some(model) = query {
            Self::update_session_model(&mut transaction, model, record).await?;
        } else {
            let mut model = Self::build_session_model(record)?;
            // Insert inside a savepoint: on PostgreSQL, a failed statement
            // aborts the whole enclosing transaction unless it's isolated in
            // a savepoint, so we need one to be able to recover and fall
            // back to an update below.
            let mut savepoint = transaction
                .begin()
                .await
                .map_err(DbStoreError::DatabaseError)?;
            match savepoint.insert(&mut model).await {
                Ok(()) => {
                    savepoint
                        .commit()
                        .await
                        .map_err(DbStoreError::DatabaseError)?;
                }
                Err(DatabaseError::UniqueViolation) => {
                    // Another writer created a session with this key
                    // concurrently; fall back to updating it instead.
                    savepoint
                        .rollback()
                        .await
                        .map_err(DbStoreError::DatabaseError)?;
                    let model = query!(Session, $key == key)
                        .get(&mut transaction)
                        .await
                        .map_err(DbStoreError::DatabaseError)?
                        .expect(
                            "row must exist after a unique key violation on insert with the \
                            same key",
                        );
                    Self::update_session_model(&mut transaction, model, record).await?;
                }
                Err(err) => return Err(DbStoreError::DatabaseError(err))?,
            }
        }

        transaction
            .commit()
            .await
            .map_err(DbStoreError::DatabaseError)?;
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let key = session_id.to_string();
        let query = query!(Session, $key == key)
            .get(&self.connection)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        if let Some(session) = query {
            let data = serde_json::from_str::<HashMap<String, serde_json::Value>>(&session.data)
                .map_err(|err| DbStoreError::Serialize(Box::new(err)))?;

            let id = session
                .key
                .parse::<Id>()
                .map_err(|err| DbStoreError::Deserialize(Box::new(err)))?;

            let expiry_date = DateTimeWithOffsetAdapter::new(session.expiry).into_offsetdatetime();

            let rec = Record {
                id,
                data,
                expiry_date,
            };

            Ok(Some(rec))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let key = session_id.to_string();
        query!(Session, $key == key)
            .delete(&self.connection)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use tower_sessions::session::Record;

    use super::*;
    use crate::db::DatabaseError;

    #[cot::test]
    async fn test_from_db_store_error_to_session_store_error() {
        let sqlx_err = sqlx::Error::Protocol("protocol error".into());
        let db_err = DatabaseError::DatabaseEngineError(sqlx_err);
        let sess_err: session_store::Error = DbStoreError::DatabaseError(db_err).into();
        assert!(matches!(sess_err, session_store::Error::Backend(_)));

        let io_err = io::Error::other("oops");
        let serialize_err: session_store::Error = DbStoreError::Serialize(Box::new(io_err)).into();

        assert!(matches!(serialize_err, session_store::Error::Encode(_)));

        let parse_err = serde_json::from_str::<Record>("not a json").unwrap_err();
        let deserialize_err: session_store::Error =
            DbStoreError::Deserialize(Box::new(parse_err)).into();
        assert!(matches!(deserialize_err, session_store::Error::Decode(_)));

        let sess_err: session_store::Error = DbStoreError::TooManyIdCollisions(99).into();
        assert!(matches!(sess_err, session_store::Error::Backend(_)));
    }
}
