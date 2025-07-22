//! Database-backed session store.
//!
//! This module provides a session store implementation that persists session
//! records in a database using the Cot ORM. It enables durable session storage
//! across application restarts and supports features such as user login
//! sessions, flash messages, and other stateful interactions.
//!
//! # Examples
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use cot::db::Database;
//! use cot::session::store::db::DbStore;
//!
//! #[tokio::main]
//! async fn main() -> cot::Result<()> {
//!     let db = Arc::new(Database::new("sqlite://:memory:").await?);
//!     let store = DbStore::new(db);
//!     Ok(())
//! }
//! ```

use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use cot::db::{Auto, DatabaseError, Model, query};
use thiserror::Error;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

use crate::db::Database;
use crate::session::db::Session;

/// Errors that can occur while interacting with the database session store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DbStoreError {
    /// An error occurred while interacting with the database.
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    /// An error occurred during JSON serialization.
    #[error("JSON serialization error: {0}")]
    Serialize(Box<dyn Error + Send + Sync>),
    /// An error occurred during JSON deserialization.
    #[error("JSON serialization error: {0}")]
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
/// ```no_run
/// use std::sync::Arc;
///
/// use cot::db::Database;
/// use cot::session::store::db::DbStore;
///
/// #[tokio::main]
/// async fn main() -> Result<(), cot::session::store::db::DbStoreError> {
///     let db = Arc::new(Database::new("sqlite://:memory:").await?);
///     let store = DbStore::new(db);
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct DbStore {
    connection: Arc<Database>,
}

impl DbStore {
    /// Creates a new `DbStore` instance with the provided database connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use cot::db::Database;
    /// use cot::session::store::db::DbStore;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), cot::session::store::db::DbStoreError> {
    ///     let db = Arc::new(Database::new("sqlite://:memory:").await?);
    ///     let store = DbStore::new(db);
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn new(connection: Arc<Database>) -> DbStore {
        DbStore { connection }
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    let db_err = match err {
        sqlx::Error::Database(db_err) => &**db_err,
        _ => return false,
    };

    let Some(code) = db_err.code() else {
        return false;
    };

    matches!(
        code.as_ref(),
        // SQLite 3.37+: 2067 (prior versions used 1555)
        "2067" | "1555"
        // Postgres unique_violation
        | "23505"
        // MySQL ER_DUP_ENTRY
        | "1062"
    )
}
#[async_trait]
impl SessionStore for DbStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        loop {
            let key = record.id.to_string();

            let data = serde_json::to_string(&record).unwrap();
            let mut model = Session {
                id: Auto::auto(),
                key,
                data,
            };
            let res = self.connection.insert(&mut model).await;
            match res {
                Ok(()) => {
                    break Ok(());
                }
                Err(DatabaseError::DatabaseEngineError(sqlx_error))
                    if is_unique_violation(&sqlx_error) =>
                {
                    // If a unique constraint violation occurs, we need to generate a new ID
                    record.id = Id::default();
                }
                Err(err) => return Err(DbStoreError::DatabaseError(err))?,
            }
        }
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let key = record.id.to_string();
        let data = serde_json::to_string(&record).unwrap();

        let query = query!(Session, $key ==key.clone())
            .get(&self.connection)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        if let Some(mut model) = query {
            model.data = data;
            model
                .update(&self.connection)
                .await
                .map_err(DbStoreError::DatabaseError)?;
        } else {
            let mut record = record.clone();
            self.create(&mut record).await?;
        }
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let key = session_id.to_string();
        let query = query!(Session, $key ==key)
            .get(&self.connection)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        if let Some(data) = query {
            let rec = serde_json::from_str::<Record>(&data.data)
                .map_err(|err| DbStoreError::Serialize(Box::new(err)))?;
            Ok(Some(rec))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let key = session_id.to_string();
        query!(Session, $key ==key)
            .delete(&self.connection)
            .await
            .map_err(DbStoreError::DatabaseError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::io;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use cot::db::DatabaseError;
    use cot::db::migrations::MigrationEngine;
    use cot::session::db::SessionApp;
    use sqlx::Error as SqlxError;
    use sqlx::error::{DatabaseError as SqlxDbErrorTrait, ErrorKind};
    use tempfile::TempDir;
    use time::{Duration, OffsetDateTime};
    use tower_sessions::session::{Id, Record};

    use super::*;
    use crate::App;

    struct TestContext {
        _temp_dir: TempDir,
        db_folder: PathBuf,
    }

    impl TestContext {
        fn get() -> &'static Self {
            static CTX: OnceLock<TestContext> = OnceLock::new();
            CTX.get_or_init(|| {
                let td = TempDir::new().expect("TempDir");
                let db_folder = td.path().join("dbstore");
                std::fs::create_dir_all(&db_folder).expect("mkdir dbstore");
                TestContext {
                    _temp_dir: td,
                    db_folder,
                }
            })
        }

        fn db_uri(&self) -> String {
            format!(
                "sqlite://{}/db_store.sqlite3?mode=rwc",
                self.db_folder.display()
            )
        }

        async fn prepare_schema(&self) -> Result<Database, DatabaseError> {
            let uri = self.db_uri();
            let engine = MigrationEngine::new(SessionApp.migrations())?;
            let db = Database::new(&uri).await?;
            engine.run(&db).await?;
            Ok(db)
        }
    }

    async fn make_db_store() -> DbStore {
        let db = Arc::new(
            TestContext::get()
                .prepare_schema()
                .await
                .expect("prepare_schema"),
        );
        DbStore::new(db)
    }

    fn make_record() -> Record {
        Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        }
    }

    #[cfg(not(miri))]
    #[cot::test]
    async fn test_create_and_load() {
        let store = make_db_store().await;
        let mut rec = make_record();
        store.create(&mut rec).await.expect("create failed");
        let loaded = store.load(&rec.id).await.expect("load err");
        assert_eq!(Some(rec.clone()), loaded);
    }

    #[cfg(not(miri))]
    #[cot::test]
    async fn test_save_overwrites() {
        let store = make_db_store().await;
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        let mut rec2 = rec.clone();
        rec2.data.insert("foo".into(), "bar".into());
        store.save(&rec2).await.expect("save failed");

        let loaded = store.load(&rec.id).await.unwrap().unwrap();
        assert_eq!(rec2.data, loaded.data);
    }

    #[cfg(not(miri))]
    #[cot::test]
    async fn test_save_creates_if_missing() {
        let store = make_db_store().await;
        let rec = make_record();
        store.save(&rec).await.expect("save failed");
        let loaded = store.load(&rec.id).await.unwrap();
        assert_eq!(Some(rec), loaded);
    }

    #[cfg(not(miri))]
    #[cot::test]
    async fn test_delete() {
        let store = make_db_store().await;
        let mut rec = make_record();
        store.create(&mut rec).await.unwrap();

        store.delete(&rec.id).await.expect("delete failed");
        let loaded = store.load(&rec.id).await.unwrap();
        assert!(loaded.is_none());

        store.delete(&rec.id).await.expect("second delete");
    }

    #[cfg(not(miri))]
    #[cot::test]
    async fn test_create_id_collision() {
        let store = make_db_store().await;
        let expiry = OffsetDateTime::now_utc() + Duration::minutes(30);

        let mut r1 = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: expiry,
        };
        store.create(&mut r1).await.unwrap();

        let mut r2 = Record {
            id: r1.id,
            data: HashMap::default(),
            expiry_date: expiry,
        };
        store.create(&mut r2).await.unwrap();

        assert_ne!(r1.id, r2.id, "ID collision not resolved");

        let loaded1 = store.load(&r1.id).await.unwrap();
        let loaded2 = store.load(&r2.id).await.unwrap();
        assert!(loaded1.is_some() && loaded2.is_some());
    }

    #[test]
    fn test_from_db_store_error_to_session_store_error() {
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
    }

    /// A fake database error to drive `sqlx::Error::Database`.
    #[derive(Debug)]
    struct FakeDbErr(Option<String>);

    impl std::fmt::Display for FakeDbErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "FakeDbErr")
        }
    }
    impl Error for FakeDbErr {}
    impl SqlxDbErrorTrait for FakeDbErr {
        fn message(&self) -> &'static str {
            "fake error"
        }

        fn code(&self) -> Option<Cow<'_, str>> {
            self.0.as_ref().map(Cow::from)
        }

        fn as_error(&self) -> &(dyn Error + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn Error + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn Error + Send + Sync + 'static> {
            self
        }

        fn kind(&self) -> ErrorKind {
            ErrorKind::UniqueViolation
        }
    }

    #[test]
    fn exercise_fake_db_err_methods_for_coverage() {
        let mut fake = FakeDbErr(Some("XYZ".into()));
        assert_eq!(fake.message(), "fake error");
        assert_eq!(fake.code().as_deref(), Some("XYZ"));
        assert_eq!(fake.kind(), ErrorKind::UniqueViolation);

        let err_ref = SqlxDbErrorTrait::as_error(&fake);
        assert_eq!(err_ref.to_string(), "FakeDbErr");

        let err_mut = SqlxDbErrorTrait::as_error_mut(&mut fake);
        assert_eq!(err_mut.to_string(), "FakeDbErr");

        let boxed: Box<dyn Error + Send + Sync> =
            <FakeDbErr as SqlxDbErrorTrait>::into_error(Box::new(fake));
        assert_eq!(boxed.to_string(), "FakeDbErr");
    }

    #[test]
    fn non_database_variant_is_false() {
        assert!(!is_unique_violation(&SqlxError::RowNotFound));
    }

    #[test]
    fn database_with_no_code_is_false() {
        let err = SqlxError::Database(Box::new(FakeDbErr(None)));
        assert!(!is_unique_violation(&err));
    }

    #[test]
    fn database_with_unrelated_code_is_false() {
        let err = SqlxError::Database(Box::new(FakeDbErr(Some("9999".into()))));
        assert!(!is_unique_violation(&err));
    }

    #[test]
    fn database_with_unique_violation_codes_are_true() {
        for &code in &["2067", "1555", "23505", "1062"] {
            let err = SqlxError::Database(Box::new(FakeDbErr(Some(code.to_string()))));
            assert!(is_unique_violation(&err));
        }
    }
}
