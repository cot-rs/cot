//! Session store management
//!
//! This module provides a trait for converting configuration options into
//! concrete session store implementations. It also includes wrappers around
//! session stores to enable dynamic dispatch and proper reference counting.
//!
//! Session stores are responsible for persisting session data between requests.
//! Different implementations store data in different places, such as memory,
//! files, databases, or external caching services like Redis.

#[cfg(all(feature = "db", feature = "json"))]
pub mod db;
#[cfg(feature = "json")]
pub mod file;
pub mod memory;
#[cfg(feature = "redis")]
pub mod redis;

use std::sync::Arc;

use async_trait::async_trait;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

pub(crate) const MAX_COLLISION_RETRIES: u32 = 32;
pub(crate) const ERROR_PREFIX: &str = "session store:";

/// A wrapper that provides a concrete type for
/// [`SessionManagerLayer`](tower_sessions::SessionManagerLayer) while
/// delegating to a boxed [`SessionStore`] trait
/// object.
///
/// This enables runtime selection of session store implementations, as
/// [`SessionManagerLayer`](tower_sessions::SessionManagerLayer) requires a
/// concrete type rather than a boxed trait object.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use cot::session::store::SessionStoreWrapper;
/// use cot::session::store::memory::MemoryStore;
/// use tower_sessions::{SessionManagerLayer, SessionStore};
///
/// let store = Arc::new(MemoryStore::new()) as Arc<dyn SessionStore + Send + Sync>;
/// let wrapper = SessionStoreWrapper::new(store);
/// let session_layer = SessionManagerLayer::new(wrapper);
/// ```
#[derive(Debug, Clone)]
pub struct SessionStoreWrapper(Arc<dyn SessionStore>);

impl SessionStoreWrapper {
    /// Create a new [`SessionStoreWrapper`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use cot::session::store::SessionStoreWrapper;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let wrapper = SessionStoreWrapper::new(Arc::new(store));
    /// ```
    pub fn new(boxed: Arc<dyn SessionStore + Send + Sync>) -> Self {
        Self(boxed)
    }
}

#[async_trait]
impl SessionStore for SessionStoreWrapper {
    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        self.0.save(session_record).await
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        self.0.load(session_id).await
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        self.0.delete(session_id).await
    }
}
