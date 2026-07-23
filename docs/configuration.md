---
title: Configuration
---

<!--
This file is generated from `cot::config::ProjectConfig`'s type definition.
Do not edit it by hand -- run `just generate-config-docs` instead.
-->

The configuration for a project.

Cot projects are configured via a TOML file (typically `config/dev.toml` and `config/prod.toml`, loaded with
[`ProjectConfig::from_toml`](https://docs.rs/cot/latest/cot/config/struct.ProjectConfig.html#method.from_toml)).
This page lists every table and key that `ProjectConfig` understands.

Any top-level table not listed below is preserved as-is and made available to your application through `ProjectConfig::extra`, for app-specific configuration.

## Top-level keys

| Key | Type | Default | Description |
|---|---|---|---|
| `debug` | boolean | `true` | Debug mode flag. |
| `register_panic_hook` | boolean | `true` | Whether to register a panic hook. |
| `secret_key` | string | — | The secret key used for signing cookies and other sensitive data. This is a cryptographic key, should be kept secret, and should be set to a random and unique value for each project. |
| `fallback_secret_keys` | array of strings | `[]` | Fallback secret keys that can be used to verify old sessions. |
| `auth_backend` | table | *(see below)* | The authentication backend to use. |
| `database` | table | *(see below)* | Database configuration. |
| `cache` | table | *(see below)* | Cache subsystem configuration. |
| `static_files` | table | *(see below)* | Static files configuration. |
| `middlewares` | table | *(see below)* | Middleware configuration. |
| `email` | table | *(see below)* | Email backend configuration. |

## `[auth_backend]`

Select the variant with the `type` key:

### `type = "none"`

No authentication backend.

### `type = "database"`

Database authentication backend.

## `[database]`

| Key | Type | Default | Description |
|---|---|---|---|
| `url` | string | — | The URL of the database, possibly with username, password, and other options. |

## `[cache]`

| Key | Type | Default | Description |
|---|---|---|---|
| `max_retries` | integer | `3` | Maximum number of retries for cache operations. |
| `timeout` | string | `"5m"` | Timeout for cache operations. |
| `prefix` | string | — | Prefix for cache keys. |
| `store` | table | *(see below)* | The cache store configuration. |

### `[cache.store]`

Select the variant with the `type` key:

#### `type = "memory"`

In-memory cache store.

#### `type = "redis"`

Redis cache store. This stores cache data in a Redis instance. The URL to the Redis server must be specified, and additional Redis-specific options can be configured.

| Key | Type | Default | Description |
|---|---|---|---|
| `url` | string | — | The URL of the Redis server. |
| `pool_size` | integer | — | Connection pool size for Redis connections. This controls how many connections to maintain in the connection pool. When not specified, a default pool size of `10` is used. |

#### `type = "file"`

File-based cache store. This stores cache data in files on the local filesystem. The path to the directory where the cache files will be stored must be specified.

| Key | Type | Default | Description |
|---|---|---|---|
| `path` | string | — | The path to the directory where cache files will be stored. |

## `[static_files]`

| Key | Type | Default | Description |
|---|---|---|---|
| `url` | string | `"/static/"` | The URL prefix for the static files to be served at (which should typically end with a slash). |
| `rewrite` | `"none"`, `"query_param"` | `"none"` | The URL rewriting mode for the static files. This is useful to allow long-lived caching of static files, while still allowing to invalidate the cache when the file changes. |
| `cache_timeout` | string | — | The duration for which static files should be cached by browsers. |

## `[middlewares]`

| Key | Type | Default | Description |
|---|---|---|---|
| `live_reload` | table | *(see below)* | The configuration for the live reload middleware. |
| `session` | table | *(see below)* | The configuration for the session middleware. |

### `[middlewares.live_reload]`

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `false` | Whether the live reload middleware is enabled. |

### `[middlewares.session]`

| Key | Type | Default | Description |
|---|---|---|---|
| `secure` | boolean | `true` | The [`Secure`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#block_access_to_your_cookies) of the cookie determines whether the session middleware is secure. |
| `http_only` | boolean | `true` | The [`HttpOnly`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#block_access_to_your_cookies) of the cookie used for the session. It is set to `true` by default. |
| `same_site` | `"strict"`, `"lax"`, `"none"` | `"strict"` | The [`SameSite`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#controlling_third-party_cookies_with_samesite) attribute of the cookie used for the session. |
| `domain` | string | — | The [`Domain`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#define_where_cookies_are_sent) attribute of the cookie used for the session. |
| `path` | string | `"/"` | The [`Path`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#define_where_cookies_are_sent) attribute of the cookie used for the session. |
| `name` | string | `"id"` | The name of the cookie used for the session. |
| `always_save` | boolean | `false` | Whether the unmodified session should be saved on read or not. If set to `true`, the session will be saved even if it was not modified. |
| `expiry` | string | — | The [`Expiry`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies#removal_defining_the_lifetime_of_a_cookie) behavior for session cookies. |
| `store` | table | *(see below)* | What session store to use. |

#### `[middlewares.session.store]`

Select the variant with the `type` key:

##### `type = "memory"`

In-memory session storage.

##### `type = "database"`

Database-backed session storage.

##### `type = "file"`

File-based session storage.

| Key | Type | Default | Description |
|---|---|---|---|
| `path` | string | — | The path to the directory where session files will be stored. |

##### `type = "cache"`

Cache-based session storage.

| Key | Type | Default | Description |
|---|---|---|---|
| `uri` | string | — | The URI to the cache service. |

## `[email]`

| Key | Type | Default | Description |
|---|---|---|---|
| `transport` | table | *(see below)* | The type of email transport backend to use. |

### `[email.transport]`

Select the variant with the `type` key:

#### `type = "console"`

Console email transport backend that prints the contents to the standard output.

#### `type = "smtp"`

SMTP email transport backend.

| Key | Type | Default | Description |
|---|---|---|---|
| `url` | string | — | The SMTP connection URL. |
| `mechanism` | `"plain"`, `"login"`, `"xoauth2"` | — | The authentication mechanism to use. |

## Full default configuration

This is a complete example with every key set explicitly to its default value (fields without a well-defined default, like `secret_key`, are omitted):

```toml
debug = true
register_panic_hook = true
fallback_secret_keys = []

[auth_backend]
type = "none"

[cache]
max_retries = 3
timeout = "5m"

[cache.store]
type = "memory"

[static_files]
url = "/static/"
rewrite = "none"

[middlewares.live_reload]
enabled = false

[middlewares.session]
secure = true
http_only = true
same_site = "strict"
path = "/"
name = "id"
always_save = false

[middlewares.session.store]
type = "memory"

[email.transport]
type = "console"
```
