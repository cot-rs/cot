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
            .expect("Session extension missing. Did you forget to add the SessionMiddleware?")
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
