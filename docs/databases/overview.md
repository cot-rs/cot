---
title: Overview
---

Cot comes with its own ORM (Object-Relational Mapping) system, which is a layer of abstraction that allows you to interact with your database using objects instead of raw SQL queries. This makes it easier to work with your database and allows you to write more maintainable code. It abstracts over the specific database engine that you are using, so you can switch between different databases without changing your code. The Cot ORM is also capable of automatically creating migrations for you, so you can easily update your database schema as your application evolves, just by modifying the corresponding Rust structures.

## Defining models
To define a model in Cot, create a Rust struct and annotate it with the [`model`](attr@cot::db::model) attribute macro. Cot will automatically implement the  [`Model`](trait@cot::db::Model) trait for the model, making the struct recognizable to the ORM and mapping it to a database table. Here's an example of a simple model that represents a link in a link shortener service:

```rust
use cot::db::{model, Auto, LimitedString};
use cot::common_types::Url;

#[model]
pub struct Link {
    #[model(primary_key)]
    id: Auto<i64>,
    #[model(unique)]
    slug: LimitedString<32>,
    url: Url,
}
```

There's some invaluable stuff going on here, so let's break it down:

* The [`#[model]`](attr@cot::db::model) attribute macro is used to automatically implement the [`Model`](trait@cot::db::Model) trait for the structure. This is required for the Cot ORM to recognize it as a database model.
* The `id` field is a typical database primary key, which means that it uniquely identifies each row in the table. It's of type `i64`, which is a 64-bit signed integer. [`Auto`](enum@cot::db::Auto) wrapper is used to automatically generate a new value for this field when a new row is inserted into the table (`AUTOINCREMENT` or `SERIAL` value in the database nomenclature).
* The `slug` field is marked as [`unique`](attr@cot::db::model), which means that each value in this field must be unique across all rows in the table. It's of type [`LimitedString<32>`](struct@cot::db::LimitedString), which is a string with a maximum length of `32` characters. This is a custom type provided by Cot that ensures that the string is not longer than the specified length at the time of constructing an instance of the structure.
* The `url` field is of type [`Url`](struct@cot::common_types::Url), which is a custom type provided by Cot that represents a URL.

After putting this structure in your project, you can use it to interact with the database. Before you do that though, it's necessary to create the table in the database that corresponds to this model. Cot CLI has got you covered and can automatically create migrations for you – just run the following command:

```bash
cot migration make
```

This will create a new file in your `migrations` directory in the crate's src directory. We will come back to the contents of this file later in this guide, but for now, let's focus on how to use the model to interact with the database.

## Model Fields Options
Cot provides specific field-level attributes that provide special meaning to fields in a model. The most common ones are listed below:

### `primary_key`
This is used to mark a field as the primary key of the table. This is a required field for every model.

```rust
#[model]
pub struct Post {
    #[model(primary_key)]
    id: Auto<i64>,
    title: String,
    content: String,
}
```

### `unique`
This is used to mark a field as unique, which means that each value in this field must be unique across all rows in the table. For more information see the [model field reference](attr@cot::db::model).

```rust
#[model]
pub struct User {
    #[model(primary_key)]
    id: Auto<i64>,
    #[model(unique)]
    username: String,
}
```

### `field_name`
This is used to specify a custom column name for a field in the database table.

```rust
#[model]
pub struct Post {
    #[model(primary_key)]
    id: Auto<i64>,
    #[model(field_name = "post_title")]
    title: String,
    content: String,
}
```

## Field Types
To use a type in a model, it **must** implement the [`ToDbValue`](trait@cot::db::ToDbValue) and [`FromDbValue`](trait@cot::db::FromDbValue) traits. The [`ToDbValue`](trait@cot::db::ToDbValue) trait tells Cot how to serialize the field value into a format that can be stored in the database (e.g. a string, a number, a boolean, etc.) while the [`FromDbValue`](trait@cot::db::FromDbValue) trait tells Cot how to deserialize the field value from the database format back into the Rust type.
Cot provides implementations of these traits for many common types on a best-effort basis. Refer to the [implementations](trait@cot::db::FromDbValue#foreign-impls) and [implementors](trait@cot::db::FromDbValue#implementors) section of the docs for a complete list of the supported types.

In the example below, we show how to use a custom type as a field in a model:

```rust
use cot::db::{Auto, ColumnType, DatabaseField, DbFieldValue, FromDbValue, model, SqlxValueRef, ToDbFieldValue};
use cot::db::impl_mysql::MySqlValueRef;
use cot::db::impl_postgres::PostgresValueRef;
use cot::db::impl_sqlite::SqliteValueRef;

#[derive(Debug, Clone)]
struct NewType(i32);

impl FromDbValue for NewType {
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized
    {
        Ok(NewType(value.get::<i32>()?))
    }

    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized
    {
        Ok(NewType(value.get::<i32>()?))
    }

    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized
    {
        Ok(NewType(value.get::<i32>()?))
    }
}

impl ToDbFieldValue for NewType {
    fn to_db_field_value(&self) -> DbFieldValue {
        self.0.clone().into()
    }
}

#[model]
#[derive(Debug, Clone)]
pub struct Post {
    #[model(primary_key)]
    id: Auto<i64>,
    new_type: NewType,
}

```

## Relationships
Relational databases are all about relationships between tables, and Cot provides a convenient way to define database relationships between models.

### Foreign keys
To define a foreign key relationship between two models, you can use the [`ForeignKey`](https://docs.rs/cot/latest/cot/db/enum.ForeignKey.html) type. Here's an example of how you can define a foreign key relationship between a `Link` model and some other `User` model:

```rust
use cot::db::ForeignKey;

#[model]
pub struct Link {
    #[model(primary_key)]
    id: Auto<i64>,
    #[model(unique)]
    slug: LimitedString<32>,
    url: String,
    user: ForeignKey<User>,
}

#[model]
pub struct User {
    #[model(primary_key)]
    id: Auto<i64>,
    name: String,
}
```

When you define a foreign key relationship, Cot will automatically create a foreign key constraint in the database. This constraint will ensure that the value in the `user_id` field of the `Link` model corresponds to a valid primary key in the `User` model.

When you retrieve a model that has a foreign key relationship, Cot will not automatically fetch the related model and populate the foreign key field with the corresponding value. Instead, you need to explicitly fetch the related model using the `get` method of the `ForeignKey` object. Here's an example of how you can fetch the related user for a link:

```rust
let mut link = query!(Link, $slug == LimitedString::new("cot").unwrap())
    .get(db)
    .await?
    .expect("Link not found");

let user = link.user.get(db).await?;
```

## Database Configuration
Configure your database connection in the configuration files inside your `config` directory:

```toml
[database]
# SQLite
url = "sqlite://db.sqlite3?mode=rwc"

# Or PostgreSQL
url = "postgresql://user:password@localhost/dbname"

# Or MySQL
url = "mysql://user:password@localhost/dbname"
```

Cot tries to be as consistent as possible when it comes to the database engine you are using. This means that you can use SQLite for development and testing, and then switch to PostgreSQL or MySQL for production without changing your code. The only thing you need to do is to change the [`url`](struct@cot::config::DatabaseConfig#structfield.url) value in the configuration file!

Currently, Cot supports the following database engines:
* SQLite
* PostgreSQL
* MySQL

As an alternative to setting the database configuration in the `TOML` file, you can also set it programmatically in the [`config`](trait@cot::project::Project#method.config) method of your project.
Note that when you do this, the config values from the `TOML` file will be entirely ignored. Here's an example of how you can do that:

```rust
use cot::config::DatabaseConfig;

struct MyProject;

impl Project for MyProject {
    fn config(&self) -> cot::Result<ProjectConfig> {
        Ok(
            ProjectConfig::builder()
                .database(
                    DatabaseConfig::builder()
                        .url("sqlite://db.sqlite3?mode=rwc")
                        .build(),
                )
                .build()
        )
    }
}

```

## Summary

In this chapter you learned about the Cot ORM and how to define models, fields, and relationships between models. You also learned how to configure your database connection and how to use the models to interact with the database. In the next chapter, we will dive deeper into how to perform various database operations using the Cot ORM.
