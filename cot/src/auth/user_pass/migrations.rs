//! List of migrations for the current app.
//!
//! The migrations are generic over the canonical user of the project, as the
//! credentials reference the table of the user.

pub mod m_0002_password_user;

use crate::auth::canonical::CanonicalUser;
use crate::db::migrations::{SyncDynMigration, wrap_migrations};

/// Returns the migrations of [`PasswordUserApp<U>`](super::PasswordUserApp).
///
/// The first migration (`m_0001_initial`) is the one of
/// [`DatabaseUserApp`](crate::auth::db::DatabaseUserApp).
#[must_use]
pub fn migrations<U: CanonicalUser>() -> Vec<Box<SyncDynMigration>> {
    let mut migrations = wrap_migrations(crate::auth::db::migrations::MIGRATIONS);
    migrations.push(Box::new(m_0002_password_user::Migration::<U>::new()));
    migrations
}
