---
title: Configuration
---

<!--
This file is generated from `cot::config::ProjectConfig`'s type definition.
Do not edit it by hand -- run `just generate-config-docs` instead.
-->

The configuration for a project. This is all the project-specific configuration data that can (and makes sense to) be expressed in a TOML configuration file.

Cot projects are configured via a TOML file (typically `config/dev.toml` and `config/prod.toml`, loaded with
[`ProjectConfig::from_toml`](https://docs.rs/cot/latest/cot/config/struct.ProjectConfig.html#method.from_toml)).
This page lists every table and key that `ProjectConfig` understands.

Any top-level table not listed below is preserved as-is and made available to your application through `ProjectConfig::extra`, for app-specific configuration.

## Top-level keys

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `debug` | boolean | `true` in debug builds, `false` in release builds | Debug mode flag. This enables some expensive operations that are useful for debugging, such as logging additional information, and collecting some extra diagnostics for generating error pages. The debug flag also controls whether Cot displays nice error pages. All of this hurts the performance, so it should be disabled for production. |
| `register_panic_hook` | boolean | `true` | Whether to register a panic hook. The panic hook is used to display information about panics in the Cot error pages that are displayed in debug mode. |
| `secret_key` | string | — | The secret key used for signing cookies and other sensitive data. This is a cryptographic key, should be kept secret, and should be set to a random and unique value for each project. |
| `fallback_secret_keys` | array of strings | `[]` | Fallback secret keys that can be used to verify old sessions. This is useful for key rotation, where you can add a new key, gradually migrate sessions to the new key, and then remove the old key. |
| `auth_backend` | table | [`type = "none"`](#auth_backend) | The authentication backend to use. |
| `database` | table | [*(see below)*](#database) | Database configuration. |
| `cache` | table | [*(see below)*](#cache) | Cache subsystem configuration. |
| `static_files` | table | [*(see below)*](#static_files) | Static files configuration. |
| `middlewares` | table | [*(see below)*](#middlewares) | Middleware configuration. |
| `email` | table | [*(see below)*](#email) | Email backend configuration. |

## `[auth_backend]`

Select the variant with the `type` key:

### `type = "none"` (default)

No authentication backend. This enables `NoAuthBackend` to be used as the authentication backend, which effectively disables authentication.

### `type = "database"`

Database authentication backend. This enables `DatabaseUserBackend` to be used as the authentication backend.

## `[database]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `url` | string | *(unset)* | The URL of the database, possibly with username, password, and other options. |

## `[cache]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `max_retries` | integer | `3` | Maximum number of retries for cache operations. This controls how many times the cache will attempt to retry failed operations before giving up. |
| `timeout` | string | `"5m"` | Timeout for cache operations. This controls how long to wait for cache operations to complete before timing out. |
| `prefix` | string | *(unset)* | Prefix for cache keys. This prefix is added to all cache keys. It's useful for versioning or categorizing cache entries. When not specified, no prefix is used. |
| `store` | table | [`type = "memory"`](#cachestore) | The cache store configuration. This determines which type of cache backend to use and its specific configuration options. |

### `[cache.store]`

Select the variant with the `type` key:

#### `type = "memory"` (default)

In-memory cache store. This uses a simple in-memory store that does not persist data across application restarts. This is suitable for development or testing environments where persistence is not required.

#### `type = "redis"`

Redis cache store. This stores cache data in a Redis instance. The URL to the Redis server must be specified, and additional Redis-specific options can be configured.

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `url` | string | — | The URL of the Redis server. |
| `pool_size` | integer | `10` | Connection pool size for Redis connections. This controls how many connections to maintain in the connection pool. |

#### `type = "file"`

File-based cache store. This stores cache data in files on the local filesystem. The path to the directory where the cache files will be stored must be specified.

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `path` | string | — | The path to the directory where cache files will be stored. |

## `[static_files]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `url` | string | `"/static/"` | The URL prefix for the static files to be served at (which should typically end with a slash). This prefix is used to determine which requests should be handled by the static files middleware. For example, if set to `/assets/`, then requests to `/assets/style.css` will be served from the static files directory. |
| `rewrite` | `"none"`, `"query_param"` | `"none"` | The URL rewriting mode for the static files. This is useful to allow long-lived caching of static files, while still allowing to invalidate the cache when the file changes. |
| `cache_timeout` | string | *(unset)* | The duration for which static files should be cached by browsers. When set, this value is used to set the `Cache-Control` header for static files. This allows browsers to cache the files for the specified duration, improving performance by reducing the number of requests to the server. |

## `[middlewares]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `live_reload` | table | [*(see below)*](#middlewareslive_reload) | The configuration for the live reload middleware. |
| `session` | table | [*(see below)*](#middlewaressession) | The configuration for the session middleware. |

### `[middlewares.live_reload]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `false` | Whether the live reload middleware is enabled. |

### `[middlewares.session]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `secure` | boolean | `true` | The [`Secure`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#block_access_to_your_cookies) of the cookie determines whether the session middleware is secure. |
| `http_only` | boolean | `true` | The [`HttpOnly`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#block_access_to_your_cookies) of the cookie used for the session. It is set to `true` by default. |
| `same_site` | `"strict"`, `"lax"`, `"none"` | `"strict"` | The [`SameSite`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#controlling_third-party_cookies_with_samesite) attribute of the cookie used for the session. This lets you specify whether cookies are sent with cross-site requests. |
| `domain` | string | *(unset)* | The [`Domain`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#define_where_cookies_are_sent) attribute of the cookie used for the session. When not explicitly configured, it is set to `None` by default. |
| `path` | string | `"/"` | The [`Path`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#define_where_cookies_are_sent) attribute of the cookie used for the session. It is set to `/` by default. |
| `name` | string | `"id"` | The name of the cookie used for the session. It is set to "id" by default. |
| `always_save` | boolean | `false` | Whether the unmodified session should be saved on read or not. If set to `true`, the session will be saved even if it was not modified. |
| `expiry` | string | *(unset)* | The [`Expiry`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#removal_defining_the_lifetime_of_a_cookie) behavior for session cookies. This controls when the session cookie expires and how long it remains valid. The expiry behavior affects how the cookie's `max-age` and `expires` attributes are set in the HTTP response. |
| `store` | table | [`type = "memory"`](#middlewaressessionstore) | What session store to use. |

#### `[middlewares.session.store]`

Select the variant with the `type` key:

##### `type = "memory"` (default)

In-memory session storage. This uses a simple in-memory store that does not persist sessions across application restarts. This is the default, and is suitable for development or testing environments.

##### `type = "database"`

Database-backed session storage. This stores session data in the configured database. This requires the "db" and "json" features to be enabled.

##### `type = "file"`

File-based session storage. This stores session data in files on the local filesystem. The path to the directory where the session files will be stored must be specified.

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `path` | string | — | The path to the directory where session files will be stored. |

##### `type = "cache"`

Cache-based session storage. This stores session data in a cache service like Redis. The URI to the cache service must be specified.

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `uri` | string | — | The URI to the cache service. |

## `[email]`

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `transport` | table | [`type = "console"`](#emailtransport) | The type of email transport backend to use. This determines which type of email transport backend to use (`console` or `smtp`) along with its configuration options. |

### `[email.transport]`

Select the variant with the `type` key:

#### `type = "console"` (default)

Console email transport backend that prints the contents to the standard output. This is a convenient transport backend for development and testing that simply prints the email contents to the console instead of actually sending them.

#### `type = "smtp"`

SMTP email transport backend. This transport backend sends emails using the Simple Mail Transfer Protocol (SMTP). It requires authentication details and server configuration.

| Key | TOML Type | Default | Description |
|---|---|---|---|
| `url` | string | — | The SMTP connection URL. This specifies the protocol, credentials, host, port, and EHLO domain for connecting to the SMTP server. |
| `mechanism` | `"plain"`, `"login"`, `"xoauth2"` | — | The authentication mechanism to use. Supported mechanisms are `plain`, `login`, and `xoauth2`. |

## Full default configuration

This is a complete example with every key set explicitly to its default value. Keys that are unset by default, or whose default isn't a single value (like `debug`), are commented out. Keys without a default (like `secret_key`) are shown as `"..."` and must be set explicitly:

```toml,ignore
# debug = ...
register_panic_hook = true
secret_key = "..."
fallback_secret_keys = []

[auth_backend]
type = "none"

[database]
# url = ...

[cache]
max_retries = 3
timeout = "5m"
# prefix = ...

[cache.store]
type = "memory"

[static_files]
url = "/static/"
rewrite = "none"
# cache_timeout = ...

[middlewares.live_reload]
enabled = false

[middlewares.session]
secure = true
http_only = true
same_site = "strict"
# domain = ...
path = "/"
name = "id"
always_save = false
# expiry = ...

[middlewares.session.store]
type = "memory"

[email.transport]
type = "console"
```
