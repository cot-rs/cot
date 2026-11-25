//! Session management
//!
//! This module provides a session management system that allows for storing
//! and retrieving session data.
//!
//! # Examples
//!
//! ```
//! use cot::RequestHandler;
//! use cot::html::Html;
//! use cot::router::{Route, Router};
//! use cot::session::Session;
//! use cot::test::TestRequestBuilder;
//!
//! async fn my_handler(session: Session) -> cot::Result<Html> {
//!     session.insert("user_name", "world".to_string()).await?;
//!     let name: String = session
//!         .get("user_name")
//!         .await?
//!         .expect("name was just added");
//!     Ok(Html::new(format!("Hello, {}!", name)))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> cot::Result<()> {
//! let request = TestRequestBuilder::get("/").with_session().build();
//!
//! assert_eq!(
//!     my_handler
//!         .handle(request)
//!         .await?
//!         .into_body()
//!         .into_bytes()
//!         .await?,
//!     "Hello, world!"
//! );
//! # Ok(())
//! # }
//! ```
#[cfg(feature = "db")]
pub mod db;
pub mod store;

use std::ops::{Deref, DerefMut};

/// A session object.
///
/// This is a wrapper around the `tower_sessions::Session` type.
///
/// # Examples
///
/// ```
/// use cot::RequestHandler;
/// use cot::html::Html;
/// use cot::request::Request;
/// use cot::router::{Route, Router};
/// use cot::session::Session;
/// use cot::test::TestRequestBuilder;
///
/// async fn my_handler(session: Session) -> cot::Result<Html> {
///     session.insert("user_name", "world".to_string()).await?;
///     let name: String = session
///         .get("user_name")
///         .await?
///         .expect("name was just added");
///     Ok(Html::new(format!("Hello, {}!", name)))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let request = TestRequestBuilder::get("/").with_session().build();
///
/// assert_eq!(
///     my_handler
///         .handle(request)
///         .await?
///         .into_body()
///         .into_bytes()
///         .await?,
///     "Hello, world!"
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Session {
    // tower_sessions::Session internally is two Arcs, so it's cheap to clone
    inner: tower_sessions::Session,
}

impl Session {
    pub(crate) fn new(inner: tower_sessions::Session) -> Self {
        Self { inner }
    }

    /// Get the session object from a request.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::RequestHandler;
    /// use cot::html::Html;
    /// use cot::request::Request;
    /// use cot::router::{Route, Router};
    /// use cot::session::Session;
    /// use cot::test::TestRequestBuilder;
    ///
    /// async fn my_handler(request: Request) -> cot::Result<Html> {
    ///     let session = Session::from_request(&request);
    ///
    ///     session.insert("user_name", "world".to_string()).await?;
    ///     let name: String = session
    ///         .get("user_name")
    ///         .await?
    ///         .expect("name was just added");
    ///     Ok(Html::new(format!("Hello, {}!", name)))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let request = TestRequestBuilder::get("/").with_session().build();
    ///
    /// assert_eq!(
    ///     my_handler
    ///         .handle(request)
    ///         .await?
    ///         .into_body()
    ///         .into_bytes()
    ///         .await?,
    ///     "Hello, world!"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[track_caller]
    #[must_use]
    pub fn from_request(request: &crate::request::Request) -> &Self {
        Self::from_extensions(request.extensions())
    }

    #[track_caller]
    #[must_use]
    pub(crate) fn from_extensions(extensions: &http::Extensions) -> &Self {
        extensions
            .get::<Self>()
            .expect("Session extension missing. Add SessionMiddleware, and if using AuthMiddleware, register SessionMiddleware after AuthMiddleware.")
    }
}

impl Deref for Session {
    type Target = tower_sessions::Session;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A view of a [`Session`] that only has access to its own, private part of
/// the session data.
///
/// All keys are transparently prefixed with the scope and a `.` (with scope
/// `hi`, the key `key1` is stored as `hi.key1`), so that the data stored by
/// different scopes can't collide, even if they use the same keys. This is
/// used to give each [authentication
/// backend](crate::auth::backend::AuthenticationBackendCommon) a private space
/// in the session.
///
/// Note that the data is still stored in the same session, so it is removed
/// when the session is flushed (for instance, when the user logs out).
#[derive(Debug, Clone)]
pub struct ScopedSession {
    session: Session,
    scope: String,
}

impl ScopedSession {
    /// Creates a new scoped session. `scope` must be unique among all the
    /// scopes stored in the session.
    #[expect(
        dead_code,
        reason = "only used by the authentication system, which doesn't use it yet"
    )]
    pub(crate) fn new(session: Session, scope: &str) -> Self {
        Self {
            session,
            scope: scope.to_owned(),
        }
    }

    fn key(&self, key: &str) -> String {
        format!("{}.{key}", self.scope)
    }

    /// Gets a value from the scoped session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session could not be loaded or the value could
    /// not be deserialized.
    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, tower_sessions::session::Error> {
        self.session.get(&self.key(key)).await
    }

    /// Inserts a value into the scoped session, replacing the previous value
    /// for the key, if any.
    ///
    /// # Errors
    ///
    /// Returns an error if the session could not be loaded or the value could
    /// not be serialized.
    pub async fn insert<T: serde::Serialize>(
        &self,
        key: &str,
        value: T,
    ) -> Result<(), tower_sessions::session::Error> {
        self.session.insert(&self.key(key), value).await
    }

    /// Removes a value from the scoped session and returns it, if it was
    /// present.
    ///
    /// # Errors
    ///
    /// Returns an error if the session could not be loaded or the value could
    /// not be deserialized.
    pub async fn remove<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, tower_sessions::session::Error> {
        self.session.remove(&self.key(key)).await
    }
}
