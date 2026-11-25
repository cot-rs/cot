//! Authentication backends.
//!
//! An authentication backend checks credentials of some kind and returns the
//! [`UserId`] of the [canonical user](crate::auth::canonical::CanonicalUser)
//! they belong to. A project can use any number of backends.
//!
//! Backends are grouped by the kind of authentication flow they implement (see
//! [`AuthBackend`]), so that Cot can provide special behavior for some flows
//! (e.g. in the admin backend).
//!
//! Some functionality is shared by all backends, such as computing
//! the session authentication hash, this is represented by the
//! [`AuthenticationBackendCommon`] supertrait.

use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use async_trait::async_trait;

use crate::auth::canonical::CanonicalUser;
use crate::auth::{Result, SessionAuthHash, UserId};
use crate::common_types::{Password, Url};
use crate::config::SecretKey;
use crate::request::RequestHead;
use crate::session::ScopedSession;

/// An authentication backend for the canonical user type `U`.
#[non_exhaustive]
pub enum AuthBackend<U: CanonicalUser> {
    /// A backend that authenticates users with an opaque key, such as an API
    /// token.
    Key(Arc<dyn KeyBackend<U>>),
    /// A backend that authenticates users with a username and a password.
    UsernamePassword(Arc<dyn UsernamePasswordBackend<U>>),
    /// A backend that authenticates users by redirecting them to an external
    /// service, such as a single sign-on provider.
    RedirectAuthenticate(Arc<dyn RedirectAuthenticateBackend<U>>),
    /// A backend with a custom authentication flow.
    Generic(Arc<dyn GenericAuthBackend<U>>),
}

impl<U: CanonicalUser> Clone for AuthBackend<U> {
    fn clone(&self) -> Self {
        match self {
            Self::Key(backend) => Self::Key(Arc::clone(backend)),
            Self::UsernamePassword(backend) => Self::UsernamePassword(Arc::clone(backend)),
            Self::RedirectAuthenticate(backend) => Self::RedirectAuthenticate(Arc::clone(backend)),
            Self::Generic(backend) => Self::Generic(Arc::clone(backend)),
        }
    }
}

impl<U: CanonicalUser> AuthBackend<U> {
    /// Returns the name of the backend.
    ///
    /// See [`AuthenticationBackendCommon::name`].
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.common().name()
    }

    /// Returns the functionality shared by all kinds of backends.
    #[must_use]
    pub fn common(&self) -> &dyn AuthenticationBackendCommon<U> {
        match self {
            Self::Key(backend) => &**backend,
            Self::UsernamePassword(backend) => &**backend,
            Self::RedirectAuthenticate(backend) => &**backend,
            Self::Generic(backend) => &**backend,
        }
    }
}

impl<U: CanonicalUser> Debug for AuthBackend<U> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Key(_) => "Key",
            Self::UsernamePassword(_) => "UsernamePassword",
            Self::RedirectAuthenticate(_) => "RedirectAuthenticate",
            Self::Generic(_) => "Generic",
        };
        f.debug_tuple(kind).field(&"..").finish()
    }
}

/// Functionality shared by all kinds of authentication backends.
///
/// This is a supertrait of [`UsernamePasswordBackend`], [`KeyBackend`],
/// [`RedirectAuthenticateBackend`] and [`GenericAuthBackend`]
/// (and should be a supertrait for all future backend types)
///
/// # Examples
///
/// ```
/// use std::any::Any;
/// use std::collections::HashMap;
///
/// use async_trait::async_trait;
/// use cot::auth::backend::{AuthenticationBackendCommon, GenericAuthBackend};
/// use cot::auth::{AuthError, SessionAuthHash, UserId};
/// use cot::config::SecretKey;
/// use cot::session::ScopedSession;
/// # use cot::test::TestUser as MyUser;
///
/// /// Authenticates users with API keys (user ID -> API key).
/// struct ApiKeyBackend {
///     keys: HashMap<i64, String>,
/// }
///
/// #[async_trait]
/// impl AuthenticationBackendCommon<MyUser> for ApiKeyBackend {
///     fn name(&self) -> &'static str {
///         "api_key"
///     }
///
///     async fn session_auth_hash(
///         &self,
///         user_id: &UserId,
///         secret_key: &SecretKey,
///     ) -> cot::auth::Result<Option<SessionAuthHash>> {
///         // thanks to this, the session hash is invalidated when the user's API key
///         // changes and the user is automatically logged out
///         const SESSION_AUTH_HASH_CONTEXT: &str = "my app session auth hash v1";
///
///         let Some(api_key) = user_id.as_int().and_then(|id| self.keys.get(&id)) else {
///             return Ok(None);
///         };
///
///         let key = blake3::derive_key(SESSION_AUTH_HASH_CONTEXT, secret_key.as_bytes());
///         let hash = blake3::keyed_hash(&key, api_key.as_bytes());
///         Ok(Some(SessionAuthHash::new(hash.as_slice())))
///     }
/// }
///
/// /// The credentials accepted by `ApiKeyBackend`.
/// struct ApiKey(String);
///
/// #[async_trait]
/// impl GenericAuthBackend<MyUser> for ApiKeyBackend {
///     async fn authenticate(
///         &self,
///         _session: &ScopedSession,
///         credentials: &(dyn Any + Send + Sync),
///     ) -> cot::auth::Result<Option<UserId>> {
///         let Some(ApiKey(key)) = credentials.downcast_ref::<ApiKey>() else {
///             return Err(AuthError::CredentialsTypeNotSupported);
///         };
///         let user_id = self
///             .keys
///             .iter()
///             .find(|(_, api_key)| *api_key == key)
///             .map(|(user_id, _)| UserId::Int(*user_id));
///         Ok(user_id)
///     }
/// }
/// ```
#[async_trait]
pub trait AuthenticationBackendCommon<U: CanonicalUser>: Send + Sync + 'static {
    /// Returns the name of the backend.
    ///
    /// The name identifies the backend within the project, so it must be
    /// unique among the backends of the project, must not contain `.` and
    /// should not change, as it is stored in the sessions of the users:
    ///
    /// * it is remembered as the backend a user has logged in with; if a
    ///   backend with that name no longer exists, the user is logged out,
    /// * the private part of the session of the backend (a [`ScopedSession`])
    ///   is derived from it.
    fn name(&self) -> &'static str;

    /// Returns the session authentication hash for the given user.
    ///
    /// If this returns [`Some`], the hash is stored in the session when the
    /// user logs in and compared on every subsequent request. If it doesn't
    /// match anymore (for instance, because the user has changed their
    /// password), the user is logged out.
    ///
    /// The hashes of all the backends of the project are combined into the
    /// one stored in the session, so a change in any of them logs the user
    /// out, no matter which backend they have logged in with. If no backend
    /// returns a hash, sessions are not invalidated this way.
    ///
    /// The hash should be derived from the `secret_key`, so that rotating the
    /// secret key invalidates all sessions.
    ///
    /// The default implementation returns [`None`].
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials of the user could not be loaded.
    #[expect(unused_variables)]
    async fn session_auth_hash(
        &self,
        user_id: &UserId,
        secret_key: &SecretKey,
    ) -> Result<Option<SessionAuthHash>> {
        Ok(None)
    }
}

/// A backend that authenticates users with a username and a password.
///
/// The default implementation is
/// [`PasswordUserBackend`](crate::auth::user_pass::PasswordUserBackend).
#[async_trait]
pub trait UsernamePasswordBackend<U: CanonicalUser>: AuthenticationBackendCommon<U> {
    /// Checks the given credentials.
    ///
    /// Returns the ID of the user the credentials belong to, or [`None`] if
    /// the credentials are invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials could not be checked.
    async fn authenticate(&self, username: &str, password: &Password) -> Result<Option<UserId>>;
}

/// A backend that authenticates users with an opaque key, such as an API
/// token.
#[async_trait]
pub trait KeyBackend<U: CanonicalUser>: AuthenticationBackendCommon<U> {
    /// Checks the given key.
    ///
    /// Returns the ID of the user the key belongs to, or [`None`] if the key
    /// is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the key could not be checked.
    async fn authenticate(&self, key: &[u8]) -> Result<Option<UserId>>;
}

/// A backend that authenticates users by redirecting them to an external
/// service, such as a single sign-on provider.
///
/// The flow consists of two steps: first, [`redirect`](Self::redirect) returns
/// the URL the user should be sent to. After the user has authenticated with
/// the external service, they are sent back to the application, which calls
/// [`authenticate`](Self::authenticate) with the incoming request. Any state
/// that needs to be kept between the two steps can be stored in the private
/// part of the session of the backend, which is passed to both methods.
#[async_trait]
pub trait RedirectAuthenticateBackend<U: CanonicalUser>: AuthenticationBackendCommon<U> {
    /// Starts the authentication flow and returns the URL the user should be
    /// redirected to.
    ///
    /// # Errors
    ///
    /// Returns an error if the flow could not be started.
    async fn redirect(&self, session: &ScopedSession) -> Result<Url>;

    /// Finishes the authentication flow using the request the external service
    /// redirected the user back with.
    ///
    /// Returns the ID of the authenticated user, or [`None`] if the
    /// authentication failed.
    ///
    /// # Errors
    ///
    /// Returns an error if the flow could not be finished.
    async fn authenticate(
        &self,
        session: &ScopedSession,
        request: &RequestHead,
    ) -> Result<Option<UserId>>;
}

/// A backend with a custom authentication flow.
///
/// Use this only if none of the other backend kinds fits your needs, as
/// generic components (such as the admin panel) can't use these backends.
#[async_trait]
pub trait GenericAuthBackend<U: CanonicalUser>: AuthenticationBackendCommon<U> {
    /// Checks the given credentials.
    ///
    /// Returns the ID of the user the credentials belong to, or [`None`] if
    /// the credentials are invalid.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::CredentialsTypeNotSupported`](crate::auth::AuthError::CredentialsTypeNotSupported)
    /// if the credentials are not of a type this backend understands.
    ///
    /// Returns an error if the credentials could not be checked.
    async fn authenticate(
        &self,
        session: &ScopedSession,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<UserId>>;
}
