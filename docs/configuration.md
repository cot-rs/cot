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
| `database` | table | *(see below)* | Configuration related to the database. |
| `cache` | table | *(see below)* | Configuration related to the cache. |
| `static_files` | table | *(see below)* | Configuration related to the static files. |
| `middlewares` | table | *(see below)* | Configuration related to the middlewares. |
| `email` | table | *(see below)* | Configuration related to the email backend. |

## `[auth_backend]`

Select the variant with the `type` key:

### `type = "none"`

No authentication backend.

No additional keys.

### `type = "database"`

Database authentication backend.

No additional keys.

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

No additional keys.

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
| `url` | string | `"/static/"` | The URL prefix for the static files to be served at (which should typically end with a slash). The default is `/static/`. |
| `rewrite` | string (one of: `"none"`, `"query_param"`) | `"none"` | The URL rewriting mode for the static files. This is useful to allow long-lived caching of static files, while still allowing to invalidate the cache when the file changes. |
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
| `secure` | boolean | `true` | The [`Secure`] of the cookie determines whether the session middleware is secure. |
| `http_only` | boolean | `true` | The [`HttpOnly`] of the cookie used for the session. It is set to `true` by default. |
| `same_site` | string (one of: `"strict"`, `"lax"`, `"none"`) | `"strict"` | The [`SameSite`] attribute of the cookie used for the session. The default value is [`SameSite::Strict`] |
| `domain` | string | — | The [`Domain`] attribute of the cookie used for the session. When not explicitly configured, it is set to `None` by default. |
| `path` | string | `"/"` | The [`Path`] attribute of the cookie used for the session. It is set to `/` by default. |
| `name` | string | `"id"` | The name of the cookie used for the session. It is set to "id" by default. |
| `always_save` | boolean | `false` | Whether the unmodified session should be saved on read or not. If set to `true`, the session will be saved even if it was not modified. It is set to `false` by default. |
| `expiry` | string | — | The [`Expiry`] behavior for session cookies. |
| `store` | table | *(see below)* | What session store to use. |

#### `[middlewares.session.store]`

Select the variant with the `type` key:

##### `type = "memory"`

In-memory session storage.

No additional keys.

##### `type = "database"`

Database-backed session storage.

No additional keys.

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

Console email transport backend.

No additional keys.

#### `type = "smtp"`

SMTP email transport backend.

| Key | Type | Default | Description |
|---|---|---|---|
| `url` | string | — | The SMTP connection URL. |
| `mechanism` | string (one of: `"plain"`, `"login"`, `"xoauth2"`) | — | The authentication mechanism to use. Supported mechanisms are `plain`, `login`, and `xoauth2`. |

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
