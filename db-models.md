---
title: Database models
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

* The `#[model]` attribute is used to mark the structure as a model. This is required for the Cot ORM to recognize it as such.
* The `id` field is a typical database primary key, which means that it uniquely identifies each row in the table. It's of type `i64`, which is a 64-bit signed integer. `Auto` wrapper is used to automatically generate a new value for this field when a new row is inserted into the table (`AUTOINCREMENT` or `SERIAL` value in the database nomenclature).
* The `slug` field is marked as `unique`, which means that each value in this field must be unique across all rows in the table. It's of type `LimitedString<32>`, which is a string with a maximum length of `32` characters. This is a custom type provided by Cot that ensures that the string is not longer than the specified length at the time of constructing an instance of the structure.

After putting this structure in your project, you can use it to interact with the database. Before you do that though, it's necessary to create the table in the database that corresponds to this model. Cot CLI has got you covered and can automatically create migrations for you – just run the following command:

```bash
cot migration make
```

This will create a new file in your `migrations` directory in the crate's src directory. We will come back to the contents of this file later in this guide, but for now, let's focus on how to use the model to interact with the database.

## Common operations

### Saving models

In order to write a model instance to the database, you can use the `save` method. Note that you need to have an instance of the `Database` structure to do this – typically you can get it from the request object in your view. Here's an example of how you can save a new link to the database inside a view:

```rust
use cot::request::extractors::RequestDb;

async fn create_link(RequestDb(db): RequestDb) -> cot::Result<Html> {
    let mut link = Link {
        id: Auto::default(),
        slug: LimitedString::new("slug").unwrap(),
        url: "https://example.com".to_string(),
    };
    link.save(db).await?;

    // ...
}
```

### Updating models

Updating a model is similar to saving a new one, but you need to have an existing instance of the model that you want to update, or another instance with the same primary key. Here's an example of how you can update an existing link in the database:

```rust
link.url = "https://example.org".to_string();
link.save(db).await?;
```

Note that `.save()` is a convenient method that can be used for both creating new rows and updating existing ones. If the primary key of the model is set to `Auto`, the method will always create a new row in the database. If the primary key is set to a specific value, the method will update the row with that primary key, or create a new one if it doesn't exist.

If you specifically want to update a row in the database for given primary key, you can use the `update` method:

```rust
link.url = "https://example.org".to_string();
link.update(db).await?;
```

Similarly, if you want to insert a new row in the database and cause an error if a row with the same primary key already exists, you can use the `insert` method:

```rust
let mut link = Link {
    id: Auto::default(),
    slug: LimitedString::new("slug").unwrap(),
    url: "https://example.com".to_string(),
};
link.insert(db).await?;
```

### Retrieving models

The basis for retrieving models from the database is the `Query` structure. It contains information about which model you want to retrieve and allows you to filter, sort, and limit the results.

The easiest way to work with the `Query` structure is the `query!` macro, which allows you to write complicated queries in readable way using Rusty syntax. For example, to retrieve the link which has slug "cot" from the database, you can write:

```rust
use cot::db::query;

let link = query!(Link, $slug == LimitedString::new("cot").unwrap())
    .get(db)
    .await?;
```

As you can see, the `query!` macro takes the model type as the first argument, followed by the filter expression. The filter expression supports many of the common comparison operators, such as `==`, `!=`, `>`, `<`, `>=`, and `<=`. You can also use logical operators like `&&` and `||` to combine multiple conditions. The `$` sign is used to access the fields of the model in the filter expression—this is needed so that the macro can differentiate between fields of the model and other variables. What's nice about the filter expression is that it's type-checked at compile time, so not only you won't be able to filter using a non-existent field, but also you won't be able to compare fields of different types.

### Deleting models

To delete a model from the database, you can use the `delete` method of the `Query` object returned by the `query!` macro. Here's an example of how you can delete a link from the database:

```rust
query!(Link, $slug == LimitedString::new("cot").unwrap()).delete(db).await?;
```

### Bulk operations

If you need to insert multiple rows at once, you can use the `bulk_insert` method. This is much more efficient than calling `save` or `insert` for each row individually, as it performs the operation in a single database query.

```rust
let mut links = vec![
    Link {
        id: Auto::default(),
        slug: LimitedString::new("cot").unwrap(),
        url: "https://cot.rs".to_string(),
        user: ForeignKey::new(1),
    },
    Link {
        id: Auto::default(),
        slug: LimitedString::new("rust").unwrap(),
        url: "https://rust-lang.org".to_string(),
        user: ForeignKey::new(1),
    },
];

Link::bulk_insert(db, &mut links).await?;
```

Note that `bulk_insert` takes a mutable slice of models, because it needs to update the primary keys of the inserted models with the values generated by the database.

Similarly, there is also `bulk_insert_or_update` method, which works like `bulk_insert`, but updates the existing rows if they conflict with the new ones.

```rust
Link::bulk_insert_or_update(db, &mut links).await?;
```

## Foreign keys

To define a foreign key relationship between two models, you can use the `ForeignKey` type. Here's an example of how you can define a foreign key relationship between a `Link` model and some other `User` model:

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

Cot tries to be as consistent as possible when it comes to the database engine you are using. This means that you can use SQLite for development and testing, and then switch to PostgreSQL or MySQL for production without changing your code. The only thing you need to do is to change the `url` value in the configuration file!

## Summary

In this chapter you learned how to define your own models in Cot, how to interact with the database using these models, and how to define foreign key relationships between models. In the next chapter, we'll try to register these models in the admin panel so that you can manage them through an easy-to-use web interface.
