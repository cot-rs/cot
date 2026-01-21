---
title: Caching
---

Cot provides a flexible caching system that allows you to store and retrieve data quickly, reducing the load on your database and improving response times. The caching system is designed to be pluggable, meaning you can switch between different cache backends without changing your application code.

## Configuration

To use the caching system, you first need to configure it in your `ProjectConfig` (or via configuration files).

### Configuration via TOML

You can configure the cache in your `config/*.toml` files:

```toml
[cache]
prefix = "myapp" # Optional: prefix for all cache keys
max_retries = 3 # Optional: max retries for cache operations (default: 3)
timeout = "5s"  # Optional: timeout for cache operations (default: 5s)

[cache.store]
type = "memory" # Options: "memory", "redis", "file" (if enabled)
```

For Redis:

```toml
[cache.store]
type = "redis"
url = "redis://127.0.0.1:6379"
pool_size = 20 # Optional: connection pool size
```

## Usage

You can access the cache by using the `Cache` extractor. The cache interface provides standard methods like `get`, `insert`, `remove`, etc.

```rust
use cot::cache::Cache;
use cot::html::Html;

async fn cache_example(cache: Cache) -> cot::Result<Html> {
    // Insert a value (uses default expiration if set in config, or infinite)
    cache.insert("user_1_name", "Alice").await?;

    // Get a value
    let name: Option<String> = cache.get("user_1_name").await?;

    if let Some(n) = name {
        println!("Found user: {}", n);
    }

    Ok(Html::new("OK"))
}
```

### Expiration

You can set an expiration time for specific keys:

```rust
use std::time::Duration;
use cot::config::Timeout;

// Cache for 60 seconds
cache.insert_expiring(
    "temp_key",
    "temp_value",
    Timeout::After(Duration::from_secs(60))
).await?;
```

## Advanced Topics

#### Lazy Computation

You can use `get_or_insert_with` to lazily compute and cache values:

```rust
let value: String = cache.get_or_insert_with("expensive_key", || async {
    // Perform expensive computation
    Ok("expensive_result".to_string())
}).await?;
```

#### Prefix

Sharing a cache instance between different environments (e.g., production and dev), or between different versions of the same
application can cause data collisions and bugs. To prevent this, you can specify a prefix for the cache keys. When a prefix
is set, all keys will be formatted as `{prefix}:{key}`, ensuring each server instance has its own isolated namespace.

The prefix can be set in the configuration file:

```toml
[cache]
prefix = "v1"
```

## Cache Backends

Cot supports the following cache backends:

- **Memory**: Stores data in memory. Fast, but data is lost when the server restarts. Good for development or short-lived cache.
- **Redis**: Stores data in a Redis instance. Persistent and shared across multiple server instances. Requires the `redis` feature.
- **File**: Stores data in files. Persistent but slower than memory/Redis. Requires configuring a path.

To use Redis, make sure to enable the `redis` feature in your `Cargo.toml`:

```toml
[dependencies]
cot = { version = "0.5", features = ["redis"] }
```
