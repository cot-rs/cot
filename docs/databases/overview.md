---
title: Overview
---

Cot comes with its own ORM (Object-Relational Mapping) system, which is a layer of abstraction that allows you to interact with your database using objects instead of raw SQL queries. This makes it easier to work with your database and allows you to write more maintainable code. It abstracts over the specific database engine that you are using, so you can switch between different databases without changing your code. The Cot ORM is also capable of automatically creating migrations for you, so you can easily update your database schema as your application evolves, just by modifying the corresponding Rust structures.

## Defining models

To define a model in Cot, you need to create a new Rust structure that implements the [`Model`](trait@cot::db::Model) trait. This trait requires you to define the name of the table that the model corresponds to, as well as the fields that the table should have. Here's an example of a simple model that represents a link in a link shortener service:

```rust
use cot::db::{model, Auto, LimitedString};

#[model]
pub struct Link {
    #[model(primary_key)]
    id: Auto<i64>,
    #[model(unique)]
    slug: LimitedString<32>,
    url: String,
}
```

There's some very useful stuff going on here, so let's break it down:

* The [`#[model]`](attr@cot::db::model) attribute is used to mark the structure as a model. This is required for the Cot ORM to recognize it as such.
* The `id` field is a typical database primary key, which means that it uniquely identifies each row in the table. It's of type `i64`, which is a 64-bit signed integer. [`Auto`](enum@cot::db::Auto) wrapper is used to automatically generate a new value for this field when a new row is inserted into the table (`AUTOINCREMENT` or `SERIAL` value in the database nomenclature).
* The `slug` field is marked as [`unique`](attr@cot::db::model), which means that each value in this field must be unique across all rows in the table. It's of type [`LimitedString<32>`](struct@cot::db::LimitedString), which is a string with a maximum length of `32` characters. This is a custom type provided by Cot that ensures that the string is not longer than the specified length at the time of constructing an instance of the structure.

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
This is used to mark a field as unique, which means that each value in this field must be unique across all rows in the table. For more information see the [model field reference](https://docs.rs/cot_macros/0.5.0/cot_macros/attr.model.html).

```rust
#[model]
pub struct User {
    #[model(primary_key)]
    id: Auto<i64>,
    #[model(unique)]
    username: String,
}
```

## Field Types

Cot

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

## Summary

In this chapter you learned how to define your own models in Cot, how to interact with the database using these models, and how to define foreign key relationships between models. In the next chapter, we'll try to register these models in the admin panel so that you can manage them through an easy-to-use web interface.
