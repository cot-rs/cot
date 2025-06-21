//! Configuration data for the project.
//!
//! This module contains the configuration data for the project. This includes
//! stuff such as the secret key used for signing cookies, database connection
//! settings, whether the debug mode is enabled, and other project-specific
//! configuration data.
//!
//! The main struct in this module is [`ProjectConfig`], which contains all the
//! configuration data for the project. After creating an instance using
//! [`ProjectConfig::from_toml`] or [`ProjectConfigBuilder`], it can be passed
//! to the [`Bootstrapper`](crate::project::Bootstrapper).

// most of the config structures might be extended with non-Copy types
// in the future, so to avoid breaking backwards compatibility, we're
// not implementing Copy for them
#![allow(missing_copy_implementations)]

use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use derive_builder::Builder;
use derive_more::with_trait::{Debug, From};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, UtcOffset};

/// The configuration for a project.
///
/// This is all the project-specific configuration data that can (and makes
/// sense to) be expressed in a TOML configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(build_fn(skip, error = std::convert::Infallible))]
#[serde(default)]
pub struct ProjectConfig {
    /// Debug mode flag.
    ///
    /// This enables some expensive operations that are useful for debugging,
    /// such as logging additional information, and collecting some extra
    /// diagnostics for generating error pages. The debug flag also controls
    /// whether Cot displays nice error pages. All of this hurts the
    /// performance, so it should be disabled for production.
    ///
    /// `ProjectConfig::default()` returns `true` here when the application is
    /// compiled in debug mode, and `false` when it is compiled in release
    /// mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{AuthBackendConfig, ProjectConfig, SecretKey};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// debug = true
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.debug, true);
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub debug: bool,
    /// Whether to register a panic hook.
    ///
    /// The panic hook is used to display information about panics in the Cot
    /// error pages that are displayed in debug mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{AuthBackendConfig, ProjectConfig, SecretKey};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// register_panic_hook = false
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.register_panic_hook, false);
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub register_panic_hook: bool,
    /// The secret key used for signing cookies and other sensitive data. This
    /// is a cryptographic key, should be kept secret, and should a set to a
    /// random and unique value for each project.
    ///
    /// When you want to rotate the secret key, you can move the current key to
    /// the `fallback_secret_keys` list, and set a new key here. Eventually, you
    /// can remove the old key from the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{AuthBackendConfig, ProjectConfig, SecretKey};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// secret_key = "123abc"
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.secret_key, SecretKey::from("123abc"));
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub secret_key: SecretKey,
    /// Fallback secret keys that can be used to verify old sessions.
    ///
    /// This is useful for key rotation, where you can add a new key, gradually
    /// migrate sessions to the new key, and then remove the old key.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{AuthBackendConfig, ProjectConfig, SecretKey};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// fallback_secret_keys = ["123abc"]
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.fallback_secret_keys, vec![SecretKey::from("123abc")]);
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub fallback_secret_keys: Vec<SecretKey>,
    /// The authentication backend to use.
    ///
    /// This is the backend that is used to authenticate users. The default is
    /// the database backend, which stores user data in the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{AuthBackendConfig, ProjectConfig};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [auth_backend]
    /// type = "none"
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.auth_backend, AuthBackendConfig::None);
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub auth_backend: AuthBackendConfig,
    /// Configuration related to the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{AuthBackendConfig, DatabaseUrl, ProjectConfig};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [database]
    /// url = "sqlite::memory:"
    /// "#,
    /// )?;
    ///
    /// assert_eq!(
    ///     config.database.url,
    ///     Some(DatabaseUrl::from("sqlite::memory:"))
    /// );
    /// # Ok::<(), cot::Error>(())
    /// ```
    #[cfg(feature = "db")]
    pub database: DatabaseConfig,
    /// Configuration related to the static files.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use cot::config::{AuthBackendConfig, DatabaseUrl, ProjectConfig, StaticFilesPathRewriteMode};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [static_files]
    /// url = "/assets/"
    /// rewrite = "query_param"
    /// cache_timeout = "1h"
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.static_files.url, "/assets/");
    /// assert_eq!(
    ///     config.static_files.rewrite,
    ///     StaticFilesPathRewriteMode::QueryParam,
    /// );
    /// assert_eq!(
    ///     config.static_files.cache_timeout,
    ///     Some(Duration::from_secs(3600)),
    /// );
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub static_files: StaticFilesConfig,
    /// Configuration related to the middlewares.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{MiddlewareConfig, ProjectConfig};
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [middlewares]
    /// live_reload.enabled = true
    /// "#,
    /// )?;
    ///
    /// assert_eq!(config.middlewares.live_reload.enabled, true);
    /// # Ok::<(), cot::Error>(())
    /// ```
    pub middlewares: MiddlewareConfig,
}

const fn default_debug() -> bool {
    cfg!(debug_assertions)
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig::builder().build()
    }
}

impl ProjectConfig {
    /// Create a new [`ProjectConfigBuilder`] to build a [`ProjectConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    ///
    /// let config = ProjectConfig::builder().build();
    /// ```
    #[must_use]
    pub fn builder() -> ProjectConfigBuilder {
        ProjectConfigBuilder::default()
    }

    /// Create a new [`ProjectConfig`] with the default values for development.
    ///
    /// This is useful for development purposes, where you want to have a
    /// configuration that you can just run as quickly as possible. This is
    /// mainly useful for tests and other things that are run in the local
    /// environment.
    ///
    /// Note that what this function returns exactly is not guaranteed to be
    /// the same across different versions of Cot. It's meant to be used as a
    /// starting point for your development configuration and is subject to
    /// change.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    ///
    /// let config = ProjectConfig::dev_default();
    /// ```
    #[must_use]
    pub fn dev_default() -> ProjectConfig {
        let mut builder = ProjectConfig::builder();
        builder.debug(true).register_panic_hook(true);
        #[cfg(feature = "db")]
        builder.database(DatabaseConfig::builder().url("sqlite::memory:").build());
        builder.build()
    }

    /// Create a new [`ProjectConfig`] from a TOML string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the TOML fails to parse as a
    /// [`ProjectConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    ///
    /// let toml = r#"
    ///    secret_key = "123abc"
    /// "#;
    /// let config = ProjectConfig::from_toml(toml)?;
    /// # Ok::<_, cot::Error>(())
    /// ```
    pub fn from_toml(toml_content: &str) -> crate::Result<ProjectConfig> {
        let config: ProjectConfig = toml::from_str(toml_content)?;
        Ok(config)
    }
}

impl ProjectConfigBuilder {
    /// Builds the project configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    ///
    /// let config = ProjectConfig::builder().build();
    /// ```
    #[must_use]
    pub fn build(&self) -> ProjectConfig {
        let debug = self.debug.unwrap_or(default_debug());
        ProjectConfig {
            debug,
            register_panic_hook: self.register_panic_hook.unwrap_or(true),
            secret_key: self.secret_key.clone().unwrap_or_default(),
            fallback_secret_keys: self.fallback_secret_keys.clone().unwrap_or_default(),
            auth_backend: self.auth_backend.unwrap_or_default(),
            #[cfg(feature = "db")]
            database: self.database.clone().unwrap_or_default(),
            static_files: self.static_files.clone().unwrap_or_default(),
            middlewares: self.middlewares.clone().unwrap_or_default(),
        }
    }
}

/// The configuration for the authentication backend.
///
/// # Examples
///
/// ```
/// use cot::config::AuthBackendConfig;
///
/// let config = AuthBackendConfig::Database;
/// ```
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthBackendConfig {
    /// No authentication backend.
    ///
    /// This enables [`NoAuthBackend`](cot::auth::NoAuthBackend) to be used as
    /// the authentication backend, which effectively disables
    /// authentication.
    #[default]
    None,
    /// Database authentication backend.
    ///
    /// This enables [`DatabaseUserBackend`](cot::auth::db::DatabaseUserBackend)
    /// to be used as the authentication backend.
    #[cfg(feature = "db")]
    Database,
}

/// The configuration for the database.
///
/// It is used as part of the [`ProjectConfig`] struct.
///
/// # Examples
///
/// ```
/// use cot::config::DatabaseConfig;
///
/// let config = DatabaseConfig::builder().url("sqlite::memory:").build();
/// ```
#[cfg(feature = "db")]
#[derive(Debug, Default, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(build_fn(skip, error = std::convert::Infallible))]
#[serde(default)]
pub struct DatabaseConfig {
    /// The URL of the database, possibly with username, password, and other
    /// options.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{DatabaseConfig, DatabaseUrl};
    ///
    /// let config = DatabaseConfig::builder().url("sqlite::memory:").build();
    /// assert_eq!(config.url, Some(DatabaseUrl::from("sqlite::memory:")));
    /// ```
    #[builder(setter(into, strip_option), default)]
    pub url: Option<DatabaseUrl>,
}

#[cfg(feature = "db")]
impl DatabaseConfigBuilder {
    /// Builds the database configuration.
    ///
    /// # Panics
    ///
    /// This will panic if the database URL is not set.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::DatabaseConfig;
    ///
    /// let config = DatabaseConfig::builder().url("sqlite::memory:").build();
    /// ```
    #[must_use]
    pub fn build(&self) -> DatabaseConfig {
        DatabaseConfig {
            url: self.url.clone().expect("Database URL is required"),
        }
    }
}

#[cfg(feature = "db")]
impl DatabaseConfig {
    /// Create a new [`DatabaseConfigBuilder`] to build a [`DatabaseConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::DatabaseConfig;
    ///
    /// let config = DatabaseConfig::builder().url("sqlite::memory:").build();
    /// ```
    #[must_use]
    pub fn builder() -> DatabaseConfigBuilder {
        DatabaseConfigBuilder::default()
    }
}

/// The configuration for serving static files.
///
/// This configuration controls how static files (like CSS, JavaScript, images,
/// etc.) are served by the application. It allows you to customize the URL
/// prefix, caching behavior, and URL rewriting strategy for static assets.
///
/// # Caching
///
/// When the `cache_timeout` is set, the [`Cache-Control`] header is set to
/// `max-age=<cache_timeout>`. This allows browsers to cache the files for the
/// specified duration, improving performance by reducing the number of requests
/// to the server.
///
/// If not set, no caching headers will be sent, and **browsers will need to
/// revalidate the files on each request**.
///
/// The recommended configuration (which is also the default in the project
/// template) is to set the `cache_timeout` to 1 year and use the
/// `QueryParam` rewrite mode. This way, the files are cached for a year, and
/// the URL of the file is rewritten to include a query parameter that changes
/// when the file is updated. This allows for long-lived caching of static
/// files, while invalidating the cache when the file changes.
///
/// [`Cache-Control`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control
///
/// # See also
///
/// - ["Love your cache" article on web.dev](https://web.dev/articles/love-your-cache#fingerprinted_urls)
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use cot::config::{StaticFilesConfig, StaticFilesPathRewriteMode};
///
/// let config = StaticFilesConfig::builder()
///     .url("/assets/")
///     .rewrite(StaticFilesPathRewriteMode::QueryParam)
///     .cache_timeout(Duration::from_secs(86400))
///     .build();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(build_fn(skip, error = std::convert::Infallible))]
#[serde(default)]
pub struct StaticFilesConfig {
    /// The URL prefix for the static files to be served at (which should
    /// typically end with a slash). The default is `/static/`.
    ///
    /// This prefix is used to determine which requests should be handled by the
    /// static files middleware. For example, if set to `/assets/`, then
    /// requests to `/assets/style.css` will be served from the static files
    /// directory.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::StaticFilesConfig;
    ///
    /// let config = StaticFilesConfig::builder().url("/assets/").build();
    /// assert_eq!(config.url, "/assets/");
    /// ```
    #[builder(setter(into))]
    pub url: String,

    /// The URL rewriting mode for the static files. This is useful to allow
    /// long-lived caching of static files, while still allowing to invalidate
    /// the cache when the file changes.
    ///
    /// This affects the URL that is returned by
    /// [`StaticFiles::url_for`](crate::request::extractors::StaticFiles::url_for)
    /// and the actual URL that is used to serve the static files.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{StaticFilesConfig, StaticFilesPathRewriteMode};
    ///
    /// let config = StaticFilesConfig::builder()
    ///     .rewrite(StaticFilesPathRewriteMode::QueryParam)
    ///     .build();
    /// assert_eq!(config.rewrite, StaticFilesPathRewriteMode::QueryParam);
    /// ```
    pub rewrite: StaticFilesPathRewriteMode,

    /// The duration for which static files should be cached by browsers.
    ///
    /// When set, this value is used to set the `Cache-Control` header for
    /// static files. This allows browsers to cache the files for the
    /// specified duration, improving performance by reducing the number of
    /// requests to the server.
    ///
    /// If not set, no caching headers will be sent, and browsers will need to
    /// revalidate the files on each request.
    ///
    /// # TOML
    ///
    /// This field is serialized as a "human-readable" duration, like `4h`,
    /// `1year`, etc. Please refer to the [`humantime::parse_duration`]
    /// documentation for the supported formats for this field.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use cot::config::StaticFilesConfig;
    ///
    /// let config = StaticFilesConfig::builder()
    ///     .cache_timeout(Duration::from_secs(86400)) // 1 day
    ///     .build();
    /// assert_eq!(config.cache_timeout, Some(Duration::from_secs(86400)));
    /// ```
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use cot::config::ProjectConfig;
    ///
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [static_files]
    /// cache_timeout = "1h"
    /// "#,
    /// )?;
    ///
    /// assert_eq!(
    ///     config.static_files.cache_timeout,
    ///     Some(Duration::from_secs(3600))
    /// );
    /// # Ok::<(), cot::Error>(())
    /// ```
    #[serde(with = "crate::serializers::humantime")]
    #[builder(setter(strip_option), default)]
    pub cache_timeout: Option<Duration>,
}

/// Configuration for the URL rewriting of static files.
///
/// This is used as part of the [`StaticFilesConfig`] struct.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StaticFilesPathRewriteMode {
    /// No rewriting. The path to the static files is returned as is (with the
    /// URL prefix, if any).
    #[default]
    None,
    /// The path is suffixed with a query parameter `?v=<hash>`, where `<hash>`
    /// is the hash of the file. This is used to allow long-lived caching of
    /// static files, while still serving the files at the same URL (because
    /// providing the query parameter does not change the actual URL). The hash
    /// is used to invalidate the cache when the file changes. This is the
    /// recommended option, along with a long cache timeout (e.g., 1 year).
    QueryParam,
}

impl StaticFilesConfigBuilder {
    /// Builds the static files configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use cot::config::{StaticFilesConfig, StaticFilesPathRewriteMode};
    ///
    /// let config = StaticFilesConfig::builder()
    ///     .url("/assets/")
    ///     .rewrite(StaticFilesPathRewriteMode::QueryParam)
    ///     .cache_timeout(Duration::from_secs(3600))
    ///     .build();
    /// ```
    #[must_use]
    pub fn build(&self) -> StaticFilesConfig {
        StaticFilesConfig {
            url: self.url.clone().unwrap_or("/static/".to_string()),
            rewrite: self.rewrite.clone().unwrap_or_default(),
            cache_timeout: self.cache_timeout.unwrap_or_default(),
        }
    }
}

impl Default for StaticFilesConfig {
    fn default() -> Self {
        StaticFilesConfig::builder().build()
    }
}

impl StaticFilesConfig {
    /// Create a new [`StaticFilesConfigBuilder`] to build a
    /// [`StaticFilesConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{StaticFilesConfig, StaticFilesPathRewriteMode};
    ///
    /// let config = StaticFilesConfig::builder()
    ///     .rewrite(StaticFilesPathRewriteMode::QueryParam)
    ///     .build();
    /// ```
    #[must_use]
    pub fn builder() -> StaticFilesConfigBuilder {
        StaticFilesConfigBuilder::default()
    }
}

/// The configuration for the middlewares.
///
/// This is used as part of the [`ProjectConfig`] struct.
///
/// # Examples
///
/// ```
/// use cot::config::{LiveReloadMiddlewareConfig, MiddlewareConfig};
///
/// let config = MiddlewareConfig::builder()
///     .live_reload(LiveReloadMiddlewareConfig::builder().enabled(true).build())
///     .build();
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(build_fn(skip, error = std::convert::Infallible))]
#[serde(default)]
pub struct MiddlewareConfig {
    /// The configuration for the live reload middleware.
    pub live_reload: LiveReloadMiddlewareConfig,
    /// The configuration for the session middleware.
    pub session: SessionMiddlewareConfig,
}

impl MiddlewareConfig {
    /// Create a new [`MiddlewareConfigBuilder`] to build a
    /// [`MiddlewareConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::MiddlewareConfig;
    ///
    /// let config = MiddlewareConfig::builder().build();
    /// ```
    #[must_use]
    pub fn builder() -> MiddlewareConfigBuilder {
        MiddlewareConfigBuilder::default()
    }
}

impl MiddlewareConfigBuilder {
    /// Builds the middleware configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{LiveReloadMiddlewareConfig, MiddlewareConfig, SessionMiddlewareConfig};
    ///
    /// let config = MiddlewareConfig::builder()
    ///     .live_reload(LiveReloadMiddlewareConfig::builder().enabled(true).build())
    ///     .session(SessionMiddlewareConfig::builder().secure(false).build())
    ///     .build();
    /// ```
    #[must_use]
    pub fn build(&self) -> MiddlewareConfig {
        MiddlewareConfig {
            live_reload: self.live_reload.clone().unwrap_or_default(),
            session: self.session.clone().unwrap_or_default(),
        }
    }
}

/// The configuration for the live reload middleware.
///
/// This is used as part of the [`MiddlewareConfig`] struct.
///
/// # Examples
///
/// ```
/// use cot::config::LiveReloadMiddlewareConfig;
///
/// let config = LiveReloadMiddlewareConfig::builder().enabled(true).build();
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(build_fn(skip, error = std::convert::Infallible))]
#[serde(default)]
pub struct LiveReloadMiddlewareConfig {
    /// Whether the live reload middleware is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::LiveReloadMiddlewareConfig;
    ///
    /// let config = LiveReloadMiddlewareConfig::builder().enabled(true).build();
    /// ```
    pub enabled: bool,
}

impl LiveReloadMiddlewareConfig {
    /// Create a new [`LiveReloadMiddlewareConfigBuilder`] to build a
    /// [`LiveReloadMiddlewareConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::LiveReloadMiddlewareConfig;
    ///
    /// let config = LiveReloadMiddlewareConfig::builder().build();
    /// ```
    #[must_use]
    pub fn builder() -> LiveReloadMiddlewareConfigBuilder {
        LiveReloadMiddlewareConfigBuilder::default()
    }
}

impl LiveReloadMiddlewareConfigBuilder {
    /// Builds the live reload middleware configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::LiveReloadMiddlewareConfig;
    ///
    /// let config = LiveReloadMiddlewareConfig::builder().enabled(true).build();
    /// ```
    #[must_use]
    pub fn build(&self) -> LiveReloadMiddlewareConfig {
        LiveReloadMiddlewareConfig {
            enabled: self.enabled.unwrap_or_default(),
        }
    }
}

/// The [`SameSite`] attribute of a cookie determines how strictly browsers send
/// cookies on cross-site requests. When not explicitly configured, it defaults
/// to `Strict`, which provides the most restrictive security posture.
///
/// - `Strict`: Cookie is only sent for same-site requests (most restrictive).
/// - `Lax`: Cookie is sent for same-site requests and top-level navigations (a
///   reasonable default).
/// - `None`: Cookie is sent on all requests, including third-party contexts
///   (least restrictive).
///
///  [`SameSite`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#controlling_third-party_cookies_with_samesite
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SameSite {
    /// Only send cookie for same-site requests.
    #[default]
    Strict,

    /// Send cookie on same-site requests and top-level cross-site navigations.
    Lax,

    /// Send cookie on all requests, including third-party.
    None,
}

impl From<SameSite> for tower_sessions::cookie::SameSite {
    fn from(value: SameSite) -> Self {
        match value {
            SameSite::Strict => Self::Strict,
            SameSite::Lax => Self::Lax,
            SameSite::None => Self::None,
        }
    }
}

fn chrono_datetime_to_time_offsetdatetime(dt: DateTime<FixedOffset>) -> OffsetDateTime {
    let offset = UtcOffset::from_whole_seconds(dt.offset().local_minus_utc())
        .expect("offset within valid range");
    OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .expect("timestamp in valid range")
        .to_offset(offset)
}

/// Session expiry configuration.
/// The [`Expiry`] attribute of a cookie determines its lifetime. When not
/// explicitly configured, cookies default to `OnSessionEnd` behavior.
///
/// [`Expiry`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#removal_defining_the_lifetime_of_a_cookie
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use chrono::DateTime;
/// use cot::config::Expiry;
///
/// // Expires when the session ends.
/// let expiry = Expiry::OnSessionEnd;
///
/// // Expires 5 mins after inactivity.
/// let expiry = Expiry::OnInactivity(Duration::from_secs(5 * 60));
///
/// // Expires at the given timestamp.
/// let expired_at =
///     DateTime::parse_from_str("2025-05-27 13:03:00 -0200", "%Y-%m-%d %H:%M:%S %z").unwrap();
/// let expiry = Expiry::AtDateTime(expired_at);
/// ```
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expiry {
    /// The cookie expires when the browser session ends.
    ///
    /// This is equivalent to not setting the `max-age` or `expires` attributes
    /// in the cookie header, making it a session cookie. The cookie will be
    /// deleted when the user closes their browser or when the browser decides
    /// to end the session.
    ///
    /// This is the most secure option as it ensures sessions don't persist
    /// beyond the browser session, but it may require users to log in more
    /// frequently.
    #[default]
    OnSessionEnd,
    /// The cookie expires after the specified duration of inactivity.
    ///
    /// The session will remain valid as long as the user continues to make
    /// requests within the specified time window. Each request resets the
    /// inactivity timer, extending the session lifetime.
    ///
    /// This provides a balance between security and user convenience, as
    /// active users won't be logged out unexpectedly, but inactive sessions
    /// will eventually expire.
    OnInactivity(Duration),
    /// The cookie expires at the specified date and time.
    ///
    /// The session will remain valid until the exact datetime specified,
    /// regardless of user activity.
    AtDateTime(DateTime<FixedOffset>),
}

impl From<Expiry> for tower_sessions::Expiry {
    fn from(value: Expiry) -> Self {
        match value {
            Expiry::OnSessionEnd => Self::OnSessionEnd,
            Expiry::OnInactivity(duration) => {
                Self::OnInactivity(time::Duration::try_from(duration).unwrap_or_else(|e| {
                    panic!("could not convert {duration:?} into a valid time::Duration: {e:?}",)
                }))
            }
            Expiry::AtDateTime(time) => {
                Self::AtDateTime(chrono_datetime_to_time_offsetdatetime(time))
            }
        }
    }
}

/// The configuration for the session middleware.
///
/// This is used as part of the [`MiddlewareConfig`] struct.
///
/// # Examples
///
/// ```
/// use cot::config::SessionMiddlewareConfig;
///
/// let config = SessionMiddlewareConfig::builder().secure(false).build();
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(build_fn(skip, error = std::convert::Infallible))]
#[serde(default)]
pub struct SessionMiddlewareConfig {
    /// The [`Secure`] of the cookie determines whether the session middleware
    /// is secure.
    ///
    ///  [`Secure`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#block_access_to_your_cookies
    /// # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder().secure(false).build();
    /// ```
    pub secure: bool,
    /// The [`HttpOnly`] of the cookie used for the session. It is set to `true`
    /// by default.
    ///
    /// [`HttpOnly`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#block_access_to_your_cookies
    ///
    ///  # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder().http_only(true).build();
    /// ```
    pub http_only: bool,
    /// The [`SameSite`] attribute of the cookie used for the session.
    /// The default value is [`SameSite::Strict`]
    ///
    /// [`SameSite`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#controlling_third-party_cookies_with_samesite
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{SameSite, SessionMiddlewareConfig};
    ///
    /// let config = SessionMiddlewareConfig::builder()
    ///     .same_site(SameSite::None)
    ///     .build();
    /// ```
    pub same_site: SameSite,

    /// The [`Domain`] attribute of the cookie used for the session. When not
    /// explicitly configured, it is set to `None` by default.
    ///
    /// [`Domain`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#define_where_cookies_are_sent
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder()
    ///     .domain("localhost".to_string())
    ///     .build();
    /// ```
    #[builder(setter(strip_option), default)]
    pub domain: Option<String>,
    /// The [`Path`] attribute of the cookie used for the session. It is set to
    /// `/` by default.
    ///
    /// [`Path`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#define_where_cookies_are_sent
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder()
    ///     .path(String::from("/random/path"))
    ///     .build();
    /// ```
    pub path: String,
    /// The name of the cookie used for the session. It is set to "id" by
    /// default.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder()
    ///     .name("some.id".to_string())
    ///     .build();
    /// ```
    pub name: String,
    /// Whether the unmodified session should be saved on read or not.
    /// If set to `true`, the session will be saved even if it was not modified.
    /// It is set to `false` by default.
    /// # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder().always_save(true).build();
    /// ```
    pub always_save: bool,
    /// The [`Expiry`] behavior for session cookies.
    ///
    /// This controls when the session cookie expires and how long it remains
    /// valid. The expiry behavior affects how the cookie's `max-age` and
    /// `expires` attributes are set in the HTTP response.
    ///
    /// The available expiry modes are:
    /// - `OnSessionEnd`: The cookie expires when the browser session ends. This
    ///   is equivalent to not adding or removing the `max-age`/`expires` field
    ///   in the cookie header, making it a session cookie.
    /// - `OnInactivity`: The cookie expires after the specified duration of
    ///   inactivity. The cookie will be refreshed on each request.
    /// - `AtDateTime`: The cookie expires at the given timestamp, regardless of
    ///   user activity.
    ///
    /// The default value is [`Expiry::OnSessionEnd`] when not specified.
    ///
    /// # TOML
    ///
    /// In TOML configuration, the expiry can be specified in two formats:
    /// - For `OnInactivity`: Use the "humantime" format (e.g., `"1h"`, `"30m"`,
    ///   `"7d"`). Please refer to the [`humantime::parse_duration`]
    ///   documentation for supported formats.
    /// - For `AtDateTime`: Use a valid RFC 3339/ISO 8601 formatted timestamp
    ///   (e.g., `"2025-12-31T23:59:59+00:00"`).
    ///
    /// [`Expiry`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#removal_defining_the_lifetime_of_a_cookie
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use chrono::DateTime;
    /// use cot::config::{Expiry, SessionMiddlewareConfig};
    ///
    /// // Session expires when browser session ends (default)
    /// let config = SessionMiddlewareConfig::builder()
    ///     .expiry(Expiry::OnSessionEnd)
    ///     .build();
    ///
    /// // Session expires after 1 hour of inactivity
    /// let config = SessionMiddlewareConfig::builder()
    ///     .expiry(Expiry::OnInactivity(Duration::from_secs(3600)))
    ///     .build();
    ///
    /// // Session expires at specific datetime
    /// let expire_at =
    ///     DateTime::parse_from_str("2025-12-31 23:59:59 +0000", "%Y-%m-%d %H:%M:%S %z").unwrap();
    /// let config = SessionMiddlewareConfig::builder()
    ///     .expiry(Expiry::AtDateTime(expire_at))
    ///     .build();
    /// ```
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use cot::config::ProjectConfig;
    ///
    /// // TOML configuration for inactivity-based expiry
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [session]
    /// expiry = "2h"
    /// "#,
    /// );
    ///
    /// // TOML configuration for datetime-based expiry
    /// let config = ProjectConfig::from_toml(
    ///     r#"
    /// [session]
    /// expiry = "2025-12-31 23:59:59 +0000"
    /// "#,
    /// );
    /// ```
    #[serde(with = "crate::serializers::session_expiry_time")]
    pub expiry: Expiry,
}

impl SessionMiddlewareConfig {
    /// Create a new [`SessionMiddlewareConfigBuilder`] to build a
    /// [`SessionMiddlewareConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder().build();
    /// ```
    #[must_use]
    pub fn builder() -> SessionMiddlewareConfigBuilder {
        SessionMiddlewareConfigBuilder::default()
    }
}

impl SessionMiddlewareConfigBuilder {
    /// Builds the session middleware configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SessionMiddlewareConfig;
    ///
    /// let config = SessionMiddlewareConfig::builder().secure(false).build();
    /// ```
    #[must_use]
    pub fn build(&self) -> SessionMiddlewareConfig {
        SessionMiddlewareConfig {
            secure: self.secure.unwrap_or(true),
            http_only: self.http_only.unwrap_or(true),
            same_site: self.same_site.unwrap_or_default(),
            domain: self.domain.clone().unwrap_or_default(),
            name: self.name.clone().unwrap_or("id".to_string()),
            path: self.path.clone().unwrap_or(String::from("/")),
            always_save: self.always_save.unwrap_or(false),
            expiry: self.expiry.unwrap_or_default(),
        }
    }
}

/// A secret key.
///
/// This is a wrapper over a byte array, which is used to store a cryptographic
/// key. This is useful for [`ProjectConfig::secret_key`] and
/// [`ProjectConfig::fallback_secret_keys`], which are used to sign cookies and
/// other sensitive data.
///
/// # Security
///
/// The implementation of the [`PartialEq`] trait for this type is constant-time
/// to prevent timing attacks.
///
/// The implementation of the [`Debug`] trait for this type hides the secret key
/// to prevent it from being leaked in logs or other debug output.
///
/// # Examples
///
/// ```
/// use cot::config::SecretKey;
///
/// let key = SecretKey::new(&[1, 2, 3]);
/// assert_eq!(key.as_bytes(), &[1, 2, 3]);
/// ```
#[repr(transparent)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "String")]
pub struct SecretKey(Box<[u8]>);

impl SecretKey {
    /// Create a new [`SecretKey`] from a byte array.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SecretKey;
    ///
    /// let key = SecretKey::new(&[1, 2, 3]);
    /// assert_eq!(key.as_bytes(), &[1, 2, 3]);
    /// ```
    #[must_use]
    pub fn new(hash: &[u8]) -> Self {
        Self(Box::from(hash))
    }

    /// Get the byte array stored in the [`SecretKey`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SecretKey;
    ///
    /// let key = SecretKey::new(&[1, 2, 3]);
    /// assert_eq!(key.as_bytes(), &[1, 2, 3]);
    /// ```
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Consume the [`SecretKey`] and return the byte array stored in it.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SecretKey;
    ///
    /// let key = SecretKey::new(&[1, 2, 3]);
    /// assert_eq!(key.into_bytes(), Box::from([1, 2, 3]));
    /// ```
    #[must_use]
    pub fn into_bytes(self) -> Box<[u8]> {
        self.0
    }
}

impl From<&[u8]> for SecretKey {
    fn from(value: &[u8]) -> Self {
        Self::new(value)
    }
}

impl From<String> for SecretKey {
    fn from(value: String) -> Self {
        Self::new(value.as_bytes())
    }
}

impl From<&str> for SecretKey {
    fn from(value: &str) -> Self {
        Self::new(value.as_bytes())
    }
}

impl PartialEq for SecretKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for SecretKey {}

impl Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write in single line, regardless whether alternate mode was used or not
        write!(f, "SecretKey(\"**********\")")
    }
}

impl Default for SecretKey {
    fn default() -> Self {
        Self::new(&[])
    }
}

/// A URL for the database.
///
/// This is a wrapper over the [`url::Url`] type, which is used to store the
/// URL of the database. It parses the URL and ensures that it is valid.
///
/// # Security
///
/// The implementation of the [`Debug`] trait for this type hides the password
/// from the debug output.
///
/// # Examples
///
/// ```
/// use cot::config::DatabaseUrl;
///
/// let url = DatabaseUrl::from("postgres://user:password@localhost:5432/database");
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg(feature = "db")]
pub struct DatabaseUrl(url::Url);

#[cfg(feature = "db")]
impl DatabaseUrl {
    /// Returns the string representation of the database URL.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::DatabaseUrl;
    ///
    /// let url = DatabaseUrl::from("postgres://user:password@localhost:5432/database");
    /// assert_eq!(
    ///     url.as_str(),
    ///     "postgres://user:password@localhost:5432/database"
    /// );
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(feature = "db")]
impl From<String> for DatabaseUrl {
    fn from(url: String) -> Self {
        Self(url::Url::parse(&url).expect("valid URL"))
    }
}

#[cfg(feature = "db")]
impl From<&str> for DatabaseUrl {
    fn from(url: &str) -> Self {
        Self(url::Url::parse(url).expect("valid URL"))
    }
}

#[cfg(feature = "db")]
impl Debug for DatabaseUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut new_url = self.0.clone();
        if !new_url.username().is_empty() {
            new_url
                .set_username("********")
                .expect("set_username should succeed if username is present");
        }
        if new_url.password().is_some() {
            new_url
                .set_password(Some("********"))
                .expect("set_password should succeed if password is present");
        }

        f.debug_tuple("DatabaseUrl")
            .field(&new_url.as_str())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_toml_valid() {
        let toml_content = r#"
            debug = true
            register_panic_hook = true
            secret_key = "123abc"
            fallback_secret_keys = ["456def", "789ghi"]
            auth_backend = { type = "none" }

            [static_files]
            url = "/assets/"
            rewrite = "none"
            cache_timeout = "1h"

            [middlewares]
            live_reload.enabled = true
            [middlewares.session]
            secure = false
            http_only = false
            domain = "localhost"
            path = "/some/path"
            always_save = true
            name = "some.sid"
        "#;

        let config = ProjectConfig::from_toml(toml_content).unwrap();

        assert!(config.debug);
        assert!(config.register_panic_hook);
        assert_eq!(config.secret_key.as_bytes(), b"123abc");
        assert_eq!(config.fallback_secret_keys.len(), 2);
        assert_eq!(config.fallback_secret_keys[0].as_bytes(), b"456def");
        assert_eq!(config.fallback_secret_keys[1].as_bytes(), b"789ghi");
        assert_eq!(config.auth_backend, AuthBackendConfig::None);
        assert_eq!(config.static_files.url, "/assets/");
        assert_eq!(
            config.static_files.rewrite,
            StaticFilesPathRewriteMode::None
        );
        assert_eq!(
            config.static_files.cache_timeout,
            Some(Duration::from_secs(3600))
        );
        assert!(config.middlewares.live_reload.enabled);
        assert!(!config.middlewares.session.secure);
        assert!(!config.middlewares.session.http_only);
        assert_eq!(
            config.middlewares.session.domain,
            Some(String::from("localhost"))
        );
        assert!(config.middlewares.session.always_save);
        assert_eq!(config.middlewares.session.name, String::from("some.sid"));
        assert_eq!(config.middlewares.session.path, String::from("/some/path"));
    }

    #[test]
    fn same_site_from_valid_toml() {
        let same_site_options = [
            (
                "none",
                SameSite::None,
                tower_sessions::cookie::SameSite::None,
            ),
            ("lax", SameSite::Lax, tower_sessions::cookie::SameSite::Lax),
            (
                "strict",
                SameSite::Strict,
                tower_sessions::cookie::SameSite::Strict,
            ),
        ];
        for (value, expected, tower_sessions_expected) in same_site_options {
            let toml_content = format!(
                r#"
            [middlewares.session]
            same_site = "{value}"
        "#
            );
            let config = ProjectConfig::from_toml(&toml_content).unwrap();
            let actual = config.middlewares.session.same_site;
            assert_eq!(actual, expected);
            assert_eq!(
                tower_sessions::cookie::SameSite::from(actual),
                tower_sessions_expected
            );
        }
    }

    #[test]
    fn expiry_from_valid_toml() {
        let expiry_opts = [
            (
                "2h",
                Expiry::OnInactivity(Duration::from_secs(7200)),
                tower_sessions::Expiry::OnInactivity(time::Duration::seconds(7200)),
            ),
            (
                "2025-12-31T23:59:59+00:00",
                Expiry::AtDateTime(
                    DateTime::parse_from_rfc3339("2025-12-31T23:59:59+00:00").unwrap(),
                ),
                tower_sessions::Expiry::AtDateTime(OffsetDateTime::new_utc(
                    time::Date::from_calendar_date(2025, time::Month::December, 31).unwrap(),
                    time::Time::from_hms(23, 59, 59).unwrap(),
                )),
            ),
        ];
        for (value, expected, tower_session_expected) in expiry_opts {
            let toml_content = format!(
                r#"
            [middlewares.session]
            expiry = "{value}"
        "#
            );
            let config = ProjectConfig::from_toml(&toml_content).unwrap();
            let actual = config.middlewares.session.expiry;
            assert_eq!(actual, expected);
            assert_eq!(tower_sessions::Expiry::from(actual), tower_session_expected);
        }
    }

    #[test]
    fn expiry_from_invalid_toml() {
        let toml_content = r#"
            [middlewares.session]
            expiry = "invalid time"
        "#
        .to_string();

        let config = ProjectConfig::from_toml(&toml_content);
        assert!(config.is_err());
    }

    #[test]
    fn from_toml_invalid() {
        let toml_content = r"
            debug = true
            secret_key = 123abc
        ";

        let result = ProjectConfig::from_toml(toml_content);
        assert!(result.is_err());
    }

    #[test]
    fn from_toml_missing_fields() {
        let toml_content = r#"
            secret_key = "123abc"

            [static_files]
            rewrite = "query_param"
        "#;

        let config = ProjectConfig::from_toml(toml_content).unwrap();
        assert_eq!(config.debug, cfg!(debug_assertions));
        assert_eq!(config.secret_key.as_bytes(), b"123abc");

        assert_eq!(config.static_files.url, "/static/");
        assert_eq!(
            config.static_files.rewrite,
            StaticFilesPathRewriteMode::QueryParam
        );
    }
}
