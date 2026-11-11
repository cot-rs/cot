//! The authentication state of a request.

use std::any::Any;
use std::sync::{Arc, Mutex, MutexGuard};

use derive_more::with_trait::Debug;

use super::{AnonymousUser, AuthBackend, Result, SessionAuthHash, User, UserId, UserWrapper};
use crate::config::SecretKey;
use crate::request::extractors::FromRequestHead;
use crate::request::{Request, RequestExt, RequestHead};
use crate::session::Session;

/// Authentication helper structure.
///
/// This is an object that provides methods to sign users in and out, by using
/// the authentication backend defined in the project configuration. It is
/// constructed by [`AuthMiddleware`](crate::middleware::AuthMiddleware) and
/// included in every [`Request`] object as long as the middleware is active.
///
/// # Security
///
/// This is a wrapper around a reference-counted inner object, which contains
/// the actual user data. It's safe and cheap to clone it and authenticate the
/// user, as the change will be reflected in all clones.
#[derive(Debug, Clone)]
pub struct Auth {
    inner: Arc<AuthInner>,
}

impl Auth {
    // only currently used in TestRequestBuilder if database is enabled
    #[cfg(all(feature = "db", feature = "test"))]
    pub(crate) async fn new(
        session: Session,
        backend: Arc<dyn AuthBackend>,
        secret_key: SecretKey,
        fallback_secret_keys: &[SecretKey],
    ) -> cot::Result<Self> {
        let inner = AuthInner::new(session, backend, secret_key, fallback_secret_keys).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub(crate) async fn from_request(request: &mut Request) -> cot::Result<Self> {
        let inner = AuthInner::from_request(request).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Returns the current user.
    ///
    /// This uses the auth backend configured in
    /// [`ProjectConfig::auth_backend`](crate::config::ProjectConfig::auth_backend).
    /// If the user is not authenticated, the [`AnonymousUser`] object is
    /// returned.
    #[must_use]
    pub fn user(&self) -> Arc<dyn User + Send + Sync> {
        self.inner.user()
    }

    /// Authenticates a user with the given credentials.
    ///
    /// This uses the auth backend configured in
    /// [`ProjectConfig::auth_backend`](crate::config::ProjectConfig::auth_backend).
    /// If the authentication is successful, the user object is returned. If the
    /// authentication fails, [`None`] is returned.
    ///
    /// Note that this doesn't log the user in, it only checks if the
    /// credentials are valid and returns the user object. To log the user
    /// in the current session, use the [`login`](Self::login) method.
    ///
    /// # Errors
    ///
    /// Returns an error if the [`AuthBackend`] accepts the credentials but
    /// fails to fetch the user object.
    pub async fn authenticate(
        &self,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        self.inner.authenticate(credentials).await
    }

    /// Logs in a user.
    ///
    /// This logs in the user in the current session. The user object is stored
    /// in the session object and can be accessed using the [`user`](Self::user)
    /// method.
    ///
    /// # Errors
    ///
    /// Returns an error if the user object cannot be stored in the session
    /// object.
    pub async fn login(&self, user: Box<dyn User + Send + Sync + 'static>) -> Result<()> {
        self.inner.login(user).await
    }

    /// Logs out the current user.
    ///
    /// This removes the user object from the session object and logs the user
    /// out. Subsequent calls to [`user`](Self::user) will return the
    /// [`AnonymousUser`] object, unless a user is logged in again.
    ///
    /// # Errors
    ///
    /// Returns an error if the user object cannot be removed from the session
    /// object.
    pub async fn logout(&self) -> Result<()> {
        self.inner.logout().await
    }
}

#[derive(Debug)]
struct AuthInner {
    session: Session,
    #[debug("..")]
    backend: Arc<dyn AuthBackend>,
    secret_key: SecretKey,
    // Using a standard Mutex object instead of an async version - it's unlikely
    // this will ever be accessed from multiple threads, since it's tied to a
    // single request. The mutex is used mostly to allow the `Auth` object to be
    // cloned and passed around while maintaining the reference to the same
    // `AuthInner` object with a mutable `user`.
    #[debug("..")]
    user: Mutex<UserWrapper>,
}

impl AuthInner {
    async fn new(
        session: Session,
        backend: Arc<dyn AuthBackend>,
        secret_key: SecretKey,
        fallback_secret_keys: &[SecretKey],
    ) -> cot::Result<Self> {
        #[expect(trivial_casts)] // cast to Arc<dyn User + Send + Sync>
        let user = get_user_with_saved_id(&session, &*backend, &secret_key, fallback_secret_keys)
            .await?
            .map_or_else(
                || Arc::new(AnonymousUser) as Arc<dyn User + Send + Sync>,
                Arc::from,
            );

        Ok(Self {
            session,
            backend,
            secret_key,
            user: Mutex::new(UserWrapper(user)),
        })
    }

    async fn from_request(request: &mut Request) -> cot::Result<Self> {
        let config = request.context().config();

        let session = Session::from_request(request).clone();
        let backend = request.context().auth_backend().clone();
        let secret_key = config.secret_key.clone();

        Self::new(session, backend, secret_key, &config.fallback_secret_keys).await
    }

    fn user(&self) -> Arc<dyn User + Send + Sync> {
        Arc::clone(&self.user_lock().0)
    }

    async fn authenticate(
        &self,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        self.backend.authenticate(credentials).await
    }

    async fn login(&self, user: Box<dyn User + Send + Sync + 'static>) -> Result<()> {
        // Mitigate the session fixation attack by changing the session ID:
        // https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#renew-the-session-id-after-any-privilege-level-change
        self.session.cycle_id().await?;

        if let Some(user_id) = user.id() {
            self.session.insert(USER_ID_SESSION_KEY, user_id).await?;
        }
        let secret_key = &self.secret_key;
        if let Some(session_auth_hash) = user.session_auth_hash(secret_key) {
            self.session
                .insert(SESSION_HASH_SESSION_KEY, session_auth_hash.as_bytes())
                .await?;
        }
        *self.user_lock() = UserWrapper(Arc::from(user));

        Ok(())
    }

    async fn logout(&self) -> Result<()> {
        self.session.flush().await?;
        *self.user_lock() = UserWrapper(Arc::new(AnonymousUser));

        Ok(())
    }

    fn user_lock(&self) -> MutexGuard<'_, UserWrapper> {
        self.user.lock().unwrap_or_else(|poison_error| {
            // We don't have any invariants about the structure of the
            // UserWrapper object, so we can safely clear the
            // poison.
            self.user.clear_poison();
            poison_error.into_inner()
        })
    }
}

pub(super) const USER_ID_SESSION_KEY: &str = "__cot_auth_user_id";
const SESSION_HASH_SESSION_KEY: &str = "__cot_auth_session_hash";

async fn get_user_with_saved_id(
    session: &Session,
    auth_backend: &dyn AuthBackend,
    secret_key: &SecretKey,
    fallback_secret_keys: &[SecretKey],
) -> Result<Option<Box<dyn User + Send + Sync>>> {
    let Some(user_id) = session.get::<UserId>(USER_ID_SESSION_KEY).await? else {
        return Ok(None);
    };

    let Some(user) = auth_backend.get_by_id(user_id).await? else {
        return Ok(None);
    };

    if session_auth_hash_valid(&*user, session, secret_key, fallback_secret_keys).await? {
        Ok(Some(user))
    } else {
        Ok(None)
    }
}

async fn session_auth_hash_valid(
    user: &(dyn User + Send + Sync),
    session: &Session,
    secret_key: &SecretKey,
    fallback_secret_keys: &[SecretKey],
) -> Result<bool> {
    let Some(user_hash) = user.session_auth_hash(secret_key) else {
        return Ok(true);
    };

    let stored_hash = session
        .get::<Vec<u8>>(SESSION_HASH_SESSION_KEY)
        .await?
        .expect("Session hash should be present in the session object");
    let stored_hash = SessionAuthHash::new(&stored_hash);

    if user_hash == stored_hash {
        return Ok(true);
    }

    // If the primary secret key doesn't match, try the fallback keys (in other
    // words, check if the session hash was generated with an old secret key)
    // and update the session hash with the new key if a match is found.
    for fallback_key in fallback_secret_keys {
        let user_hash_fallback = user
            .session_auth_hash(fallback_key)
            .expect("User should have a session hash for each secret key");
        if user_hash_fallback == stored_hash {
            session
                .insert(SESSION_HASH_SESSION_KEY, user_hash.as_bytes())
                .await?;

            return Ok(true);
        }
    }

    Ok(false)
}

impl FromRequestHead for Auth {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        let auth = head
            .extensions
            .get::<Auth>()
            .expect("AuthMiddleware not enabled for the route/project")
            .clone();

        Ok(auth)
    }
}
