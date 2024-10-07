//! Generated by flareon CLI 0.1.0 on 2024-10-04 19:55:15+00:00

use crate::auth::PasswordHash;

#[derive(Debug, Copy, Clone)]
pub(super) struct Migration;
impl ::flareon::db::migrations::Migration for Migration {
    const APP_NAME: &'static str = "flareon_auth";
    const MIGRATION_NAME: &'static str = "m_0001_initial";
    const OPERATIONS: &'static [::flareon::db::migrations::Operation] =
        &[::flareon::db::migrations::Operation::create_model()
            .table_name(::flareon::db::Identifier::new("database_user"))
            .fields(&[
                ::flareon::db::migrations::Field::new(
                    ::flareon::db::Identifier::new("id"),
                    <i64 as ::flareon::db::DatabaseField>::TYPE,
                )
                .auto()
                .primary_key(),
                ::flareon::db::migrations::Field::new(
                    ::flareon::db::Identifier::new("username"),
                    <String as ::flareon::db::DatabaseField>::TYPE,
                ),
                ::flareon::db::migrations::Field::new(
                    ::flareon::db::Identifier::new("password"),
                    <PasswordHash as ::flareon::db::DatabaseField>::TYPE,
                ),
            ])
            .build()];
}

#[derive(::core::fmt::Debug)]
#[::flareon::db::model(model_type = "migration")]
struct _DatabaseUser {
    id: i64,
    username: String,
    password: PasswordHash,
}
