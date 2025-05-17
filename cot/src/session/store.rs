//! Session store management
//!
//! This module provides a trait for converting configuration options into
//! concrete session store implementations. It also includes wrappers around
//! session stores to enable dynamic dispatch and proper reference counting.
//!
//! Session stores are responsible for persisting session data between requests.
//! Different implementations store data in different places, such as memory,
//! files, databases, or external caching services like Redis.
#[cfg(feature = "json")]
pub mod file;
pub mod memory;
#[cfg(feature = "json")]
#[cfg(feature = "redis")]
pub mod redis;

use std::sync::Arc;

use async_trait::async_trait;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store;

use crate::ProjectContext;
use crate::middleware::SessionStore;
use crate::project::WithDatabase;

/// A trait for types that can be converted into a session store.
///
/// This trait enables configuration options to be transformed into concrete
/// session store implementations that can be used by the framework.
/// Implementing this trait allows a configuration type to be used directly with
/// Cot's session middleware system.
///
/// The conversion process may require access to the project context to
/// establish connections to external services or databases.
///
/// # Examples
///
/// ```
/// use cot::ProjectContext;
/// use cot::middleware::SessionStore;
/// use cot::project::WithDatabase;
/// use cot::session::store::ToSessionStore;
/// use tower_sessions::session_store;
///
/// enum MyStoreConfig {
///     InMemory,
///     // Other variants...
/// }
///
/// impl ToSessionStore for MyStoreConfig {
///     fn to_session_store(
///         self,
///         context: &ProjectContext<WithDatabase>,
///     ) -> Result<Box<dyn SessionStore + Send + Sync>, session_store::Error> {
///         match self {
///             MyStoreConfig::InMemory => {
///                 Ok(Box::new(cot::session::store::memory::MemoryStore::new()))
///             } // Handle other variants...
///         }
///     }
/// }
/// ```
pub trait ToSessionStore {
    /// Converts self into a boxed session store implementation.
    ///
    /// This method creates a concrete session store from the configuration
    /// that can be used by the session middleware.
    ///
    /// # Errors
    ///
    /// will return `Err` if the conversion fails.
    fn to_session_store(
        self,
        context: &ProjectContext<WithDatabase>,
    ) -> Result<Box<dyn SessionStore + Send + Sync>, session_store::Error>;
}

/// A wrapper around a session store that implements the `SessionStore` trait
/// which allows for dynamic dispatch of session store operations.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use cot::session::store::SessionStoreWrapper;
/// use cot::session::store::memory::MemoryStore;
/// use tower_sessions::session_store::SessionStore;
///
/// // Create a memory-based session store
/// let memory_store = MemoryStore::new();
///
/// // Wrap it for shared ownership and dynamic dispatch
/// let store = SessionStoreWrapper::new(Arc::new(memory_store));
///
/// // The wrapper can be cloned cheaply
/// let store_clone = store.clone();
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
