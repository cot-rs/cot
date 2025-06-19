//! Memory session store
//!
//! This module provides an implementation of an in-memory session store.
//!
//! # Examples
//!
//! ```
//! use cot::session::store::memory::MemoryStore;
//! let store = MemoryStore::new();
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

/// An in-memory session store implementation.
///
/// This store keeps all sessions in memory using a thread-safe hashmap.
/// It's primarily useful for development and testing environments.
///
/// # Examples
///
/// ```
/// use cot::session::store::memory::MemoryStore;
/// let store = MemoryStore::new();
/// ```
#[derive(Debug, Default, Clone)]
pub struct MemoryStore(Arc<Mutex<HashMap<Id, Record>>>);

impl MemoryStore {
    /// Creates a new, empty `MemoryStore` session store.
    /// # Examples
    ///
    /// ```
    /// use cot::session::store::memory::MemoryStore;
    /// let store = MemoryStore::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for MemoryStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        let mut store_guard = self.0.lock().await;
        while store_guard.contains_key(&session_record.id) {
            // Session ID collision mitigation.
            session_record.id = Id::default();
        }
        store_guard.insert(session_record.id, session_record.clone());
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.0.lock().await.insert(record.id, record.clone());
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let record = self
            .0
            .lock()
            .await
            .get(session_id)
            .filter(|Record { expiry_date, .. }| is_active(*expiry_date))
            .cloned();
        Ok(record)
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        self.0.lock().await.remove(session_id);
        Ok(())
    }
}

fn is_active(expiry_date: OffsetDateTime) -> bool {
    expiry_date > OffsetDateTime::now_utc()
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    #[cot::test]
    async fn test_create() {
        let store = MemoryStore::default();
        let mut record = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        };
        assert!(store.create(&mut record).await.is_ok());
    }

    #[cot::test]
    async fn test_save() {
        let store = MemoryStore::default();
        let record = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        };
        assert!(store.save(&record).await.is_ok());
    }

    #[cot::test]
    async fn test_load() {
        let store = MemoryStore::default();
        let mut record = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        };
        store.create(&mut record).await.unwrap();
        let loaded_record = store.load(&record.id).await.unwrap();
        assert_eq!(Some(record), loaded_record);
    }

    #[cot::test]
    async fn test_delete() {
        let store = MemoryStore::default();
        let mut record = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
        };
        store.create(&mut record).await.unwrap();
        assert!(store.delete(&record.id).await.is_ok());
        assert_eq!(None, store.load(&record.id).await.unwrap());
    }

    #[cot::test]
    async fn test_create_id_collision() {
        let store = MemoryStore::default();
        let expiry_date = OffsetDateTime::now_utc() + Duration::minutes(30);
        let mut record1 = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date,
        };
        let mut record2 = Record {
            id: Id::default(),
            data: HashMap::default(),
            expiry_date,
        };
        store.create(&mut record1).await.unwrap();
        record2.id = record1.id; // Set the same ID for record2
        store.create(&mut record2).await.unwrap();
        assert_ne!(record1.id, record2.id); // IDs should be different
    }
}
