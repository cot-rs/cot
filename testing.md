---
title: Testing
---

Cot includes various built-in utilities to help you test your application. This guide will cover some of the most important ones.

## Why Test at All?

Testing is a critical part of any application development process. By writing and running tests, you can:
1. **Ensure Code Reliability** – Tests catch bugs and regressions before they reach production, increasing overall stability and confidence in your application.
2. **Document Your Code** – Tests serve as living documentation. They show how different parts of your application are supposed to work and can act as examples for future maintainers.
3. **Facilitate Refactoring** – With a robust test suite, you can safely modify or refactor your code. If something breaks, your tests will let you know right away.
4. **Encourage Good Design** – When code is easier to test, it often means it's well-structured and follows good design principles.

By employing Cot's testing utilities, you'll be able to verify that each piece of your application—from individual request handlers to full end-to-end processes—works correctly.

---

## General Overview

Cot provides several built-in utilities located in the [`cot::test` module](https://docs.rs/cot/0.1/cot/test/index.html) to help you create and run tests for your application.

Typical Rust projects keep their tests in:
- A dedicated `tests/` directory (for integration tests).
- A `mod tests` section in your source files (for unit tests).

You can run all your tests by executing:
```bash
cargo test
```

---

## Unit Testing

Unit tests focus on testing small, isolated pieces of your application, such as individual functions, request handlers, or utility methods. Cot's `TestRequestBuilder` utility helps you create HTTP request objects in a lightweight way, without spinning up a full HTTP server.

### Test Request Builder

The `TestRequestBuilder` offers a fluent API for constructing HTTP requests that can be dispatched to your request handlers directly:

```rust
// Create a GET request
let request = TestRequestBuilder::get("/").build();

// Create a POST request
let request = TestRequestBuilder::post("/").build();

// Add configuration and features
let request = TestRequestBuilder::get("/")
    .with_default_config()  // Add default configuration
    .with_session()         // Add session support
    .build();

// Add form data
let request = TestRequestBuilder::post("/")
    .form_data(&[("key", "value")])
    .build();

// Add JSON data
let request = TestRequestBuilder::post("/")
    .json(&your_data)
    .build();
```

#### When to Use `TestRequestBuilder`

- **Handler Testing**: Verify that individual handlers behave correctly given different inputs (e.g., form data, JSON bodies).
- **Config-dependent Testing**: Make sure your handlers behave as expected when certain configurations or features (like sessions) are enabled.

---

## Integration Testing

Integration tests check how multiple parts of your application work together. Cot provides a `Client` struct to help you simulate end-to-end HTTP interactions with a fully running instance of your application.

### Test Client

The `Client` struct lets you create a temporary instance of your Cot application and perform HTTP requests against it:

```rust
let project = CotProject::builder()
    .register_app_with_views(MyApp, "/app")
    .build().await?;

// Create a new test client
let mut client = Client::new(project);

// Make GET requests
let response = client.get("/").await?;

// Make custom requests
let request = http::Request::get("/").body(Body::empty())?;
let response = client.request(request).await?;
```

#### When to Use `Client`

- **Full Application Testing**: Confirm that routes, middlewares, and database integrations all work as intended.
- **Multi-Request Sequences**: Test flows that require multiple requests, like login/logout or multi-step forms.

---

## Test Database

Cot's testing utilities also include the `TestDatabase` struct, which helps you create temporary databases for your tests. This allows you to test how your application interacts with data storage without polluting your real database.

```rust
// Create SQLite test database (in-memory)
let mut test_db = TestDatabase::new_sqlite().await?;

// Create PostgreSQL test database
let mut test_db = TestDatabase::new_postgres("test_name").await?;

// Create MySQL test database
let mut test_db = TestDatabase::new_mysql("test_name").await?;

// Use the test database in requests
let request = TestRequestBuilder::get("/")
    .database(test_db.database())
    .build();

// Add authentication support
test_db.with_auth().run_migrations().await;

// Clean up after testing
test_db.cleanup().await?;
```

### Best Practices

1. **Always Clean Up Test Databases**
   ```rust
   let test_db = TestDatabase::new_sqlite().await?;
   // ... run your tests ...
   test_db.cleanup().await?;
   ```
   Cleaning up helps ensure that each test runs in isolation and that temporary databases don't accumulate. Note that if a test panics, the database will **not** be cleaned up, which might be useful for debugging. On the next test run, the database will be removed automatically.

2. **Use Unique Test Names for PostgreSQL/MySQL**
   ```rust
   let test_db = TestDatabase::new_postgres("unique_test_name").await?;
   ```
   This prevents naming collisions when running multiple tests or suites simultaneously.

3. **Add Migrations and Auth Support If Required**
   ```rust
   let mut test_db = TestDatabase::new_sqlite().await?;
   test_db.with_auth().run_migrations().await;
   let request = TestRequestBuilder::get("/")
       .with_db_auth(test_db.database())
       .build();
   ```
   This ensures that your tests have all necessary schema and authentication information set up.

### Environment Variables

- `POSTGRES_URL` \
  The connection URL for PostgreSQL (default: `postgresql://cot:cot@localhost`).

- `MYSQL_URL` \
  The connection URL for MySQL (default: `mysql://root:@localhost`).

### Important Notes

- PostgreSQL and MySQL test databases are created with the prefix `test_cot__`.
- The SQLite database is in-memory by default.
- Form data is currently only supported with POST requests.
- Custom migrations can be added using the `add_migrations` method on `TestDatabase`.

---

## Summary

Cot's testing framework provides a robust and flexible approach to ensuring the quality of your application.

- **Unit tests** with `TestRequestBuilder` help you verify that individual components behave as expected.
- **Integration tests** with `Client` let you test your entire application in a near-production environment, while `TestDatabase` give you confidence that your data layer is functioning correctly, whether you're using SQLite, PostgreSQL, or MySQL.

By integrating these testing tools into your workflow, you can deploy your Cot applications with greater confidence. Happy testing!
