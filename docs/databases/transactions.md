---
title: Transactions
---

A database transaction groups a series of operations into a single, atomic unit of work: either all of them are applied to the database, or none of them are. This is essential whenever a logical change spans more than one statement and you need to keep the database consistent even if something fails halfway through - the classic example being transferring money between two accounts, where the debit and the credit must either both happen or both be rolled back.

Cot exposes transactions through the [`Database::begin`](struct@cot::db::Database#method.begin) method, which returns a [`Transaction`](struct@cot::db::Transaction). The [`Transaction`](struct@cot::db::Transaction) implements the same [`DatabaseBackend`](trait@cot::db::DatabaseBackend) trait as [`Database`](struct@cot::db::Database), so every method you already use - [`insert`](trait@cot::db::Model#method.insert), [`save`](trait@cot::db::Model#method.save), the [`query!`](macro@cot::db::query) macro, and so on - works inside a transaction by simply passing the transaction in place of the database connection.

## Starting a transaction

Call [`begin`](struct@cot::db::Database#method.begin) to start a transaction, run your operations against it, and then finalize it with [`commit`](struct@cot::db::Transaction#method.commit). Nothing you do inside the transaction becomes visible to other connections until you commit.

```rust
use cot::db::Database;

#[model]
#[derive(Debug)]
struct Account {
    #[model(primary_key)]
    id: Auto<i64>,
    balance: i64,
}

async fn transfer(db: &Database) -> cot::Result<()> {
    let mut transaction = db.begin().await?;

    let mut alice = query!(Account, $id == 1).get(&mut transaction).await?.unwrap();
    let mut bob = query!(Account, $id == 2).get(&mut transaction).await?.unwrap();

    alice.balance -= 100;
    bob.balance += 100;
    alice.save(&mut transaction).await?;
    bob.save(&mut transaction).await?;

    // Both updates are applied together, or neither of them is.
    transaction.commit().await?;
#   Ok(())
}
```

## Committing and rolling back

A transaction ends in one of two ways:

* [`commit`](struct@cot::db::Transaction#method.commit) persists every change made inside the transaction.
* [`rollback`](struct@cot::db::Transaction#method.rollback) discards every change made inside the transaction, leaving the database exactly as it was before [`begin`](struct@cot::db::Database#method.begin) was called.

Both methods consume the [`Transaction`](struct@cot::db::Transaction), so it can no longer be used afterwards. If a [`Transaction`](struct@cot::db::Transaction) is dropped without being committed - for example because an error caused an early return - it is rolled back automatically, so a failed operation never leaves a half-applied change behind.

```rust
use cot::db::Database;

# #[model] #[derive(Debug)] struct Account { #[model(primary_key)] id: Auto<i64>, balance: i64 }
async fn withdraw(db: &Database, amount: i64) -> cot::Result<()> {
    let mut transaction = db.begin().await?;

    let mut account = query!(Account, $id == 1).get(&mut transaction).await?.unwrap();
    account.balance -= amount;
    account.save(&mut transaction).await?;

    if account.balance < 0 {
        // Not enough funds - undo the withdrawal.
        transaction.rollback().await?;
    } else {
        transaction.commit().await?;
    }
#   Ok(())
}
```

## Running queries in a transaction

Because [`Transaction`](struct@cot::db::Transaction) implements [`DatabaseBackend`](trait@cot::db::DatabaseBackend), you can pass a mutable reference to it anywhere a database backend is expected, including the [`query!`](macro@cot::db::query) macro and the [`Query`](struct@cot::db::query::Query) interface. Crucially, a query run through the transaction sees the transaction's own in-progress view of the data - including changes you made earlier in the same transaction that haven't been committed yet. This lets you write a row and then immediately query it (or re-check an invariant across the table) before deciding whether to commit.

```rust
use cot::db::Database;

# #[model] #[derive(Debug)] struct Account { #[model(primary_key)] id: Auto<i64>, balance: i64 }
async fn charge(db: &Database, account_id: i64, amount: i64) -> cot::Result<()> {
    let mut transaction = db.begin().await?;

    let mut account = query!(Account, $id == account_id).get(&mut transaction).await?.unwrap();
    account.balance -= amount;
    account.save(&mut transaction).await?;

    // This query runs against the transaction's own view, so it already counts
    // the balance we just wrote above - even though nothing has been committed.
    // The same query run directly on `db` would still see the old balance.
    let overdrawn = query!(Account, $balance < 0).count(&mut transaction).await?;
    if overdrawn > 0 {
        // The charge would overdraw an account, so undo it.
        transaction.rollback().await?;
    } else {
        transaction.commit().await?;
    }
#   Ok(())
}
```

## Nested transactions (savepoints)

Transactions can be nested. Calling [`begin`](struct@cot::db::Transaction#method.begin) on a [`Transaction`](struct@cot::db::Transaction) starts a nested transaction backed by a database *savepoint*. Committing the nested transaction releases the savepoint into the enclosing transaction, while rolling it back undoes only the work done since the savepoint was created - the outer transaction keeps going and can still be committed or rolled back independently.

This is useful when part of a larger operation is allowed to fail without aborting the whole thing.

```rust
use cot::db::Database;

# #[model] #[derive(Debug)] struct Account { #[model(primary_key)] id: Auto<i64>, balance: i64 }
async fn deposit_with_optional_bonus(db: &Database) -> cot::Result<()> {
    let mut transaction = db.begin().await?;

    let mut account = query!(Account, $id == 1).get(&mut transaction).await?.unwrap();
    account.balance += 100;
    account.save(&mut transaction).await?;

    // Try to apply a bonus in a nested transaction. If it fails, roll back
    // just the bonus while keeping the deposit above.
    let mut nested = transaction.begin().await?;
    account.balance += 10;
    match account.save(&mut nested).await {
        Ok(()) => nested.commit().await?,
        Err(_) => nested.rollback().await?,
    }

    transaction.commit().await?;
#   Ok(())
}
```

## Raw SQL in transactions

Just like [`Database`](struct@cot::db::Database), a [`Transaction`](struct@cot::db::Transaction) provides an escape hatch for running raw SQL when the [`query!`](macro@cot::db::query) macro and the [`Query`](struct@cot::db::query::Query) interface aren't enough. The [`raw`](struct@cot::db::Transaction#method.raw), [`raw_with`](struct@cot::db::Transaction#method.raw_with), [`raw_as`](struct@cot::db::Transaction#method.raw_as), and [`raw_as_with`](struct@cot::db::Transaction#method.raw_as_with) methods behave exactly like their [`Database`](struct@cot::db::Database) counterparts, except that the statements run within the transaction and are only persisted once it is committed.

> **Warning:** These methods execute the given SQL string as-is, without any sanitization. Never build the query string by interpolating untrusted input directly into it. Use the parameterized `raw_with`/`raw_as_with` variants whenever the query depends on external data.

```rust
use cot::db::Database;

async fn add_interest(db: &Database) -> cot::Result<()> {
    let mut transaction = db.begin().await?;

    transaction
        .raw("UPDATE account SET balance = balance + 10 WHERE balance > 0")
        .await?;

    transaction.commit().await?;
#   Ok(())
}
```
