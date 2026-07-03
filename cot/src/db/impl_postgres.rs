//! Database interface implementation – PostgreSQL backend.

use cot::db::query::QueryBuildingError;
use sea_query::extension::postgres::PgExpr;
use sea_query::{ExprTrait, SimpleExpr};

use crate::db::query::expr::like::{CaseSensitivity, LikeDialect, to_sql_like};
use crate::db::sea_query_db::impl_sea_query_db_backend;

impl_sea_query_db_backend!(DatabasePostgres: sqlx::postgres::Postgres, sqlx::postgres::PgPool, PostgresRow, PostgresValueRef, sea_query::PostgresQueryBuilder);

impl DatabasePostgres {
    #[expect(clippy::unused_async)]
    async fn init(&self) -> crate::db::Result<()> {
        Ok(())
    }

    fn prepare_values(values: &mut sea_query_sqlx::SqlxValues) {
        for value in &mut values.0.0 {
            Self::tinyint_to_smallint(value);
        }
    }

    /// PostgreSQL does only support 2+ bytes integers, so we need to convert
    /// i8 to i16. Otherwise, sqlx will convert it internally to `char`
    /// and we'll get an error.
    ///
    /// Unsigned integers don't need this treatment: `sea-query-sqlx` already
    /// widens them to the next signed integer type to match the column types
    /// generated for them (see `ColumnType` rendering in `sea-query`'s
    /// PostgreSQL backend).
    fn tinyint_to_smallint(value: &mut sea_query::Value) {
        if let sea_query::Value::TinyInt(num) = value {
            *value = sea_query::Value::SmallInt(num.map(i16::from));
        }
    }

    fn last_inserted_row_id_for(_result: &sqlx::postgres::PgQueryResult) -> Option<u64> {
        None
    }

    #[expect(clippy::unused_self)] // to have a unified interface between database impls
    pub(super) fn sea_query_column_type_for(
        &self,
        column_type: crate::db::ColumnType,
    ) -> sea_query::ColumnType {
        sea_query::ColumnType::from(column_type)
    }
}

impl LikeDialect for DatabasePostgres {
    fn like_expr(
        &self,
        lhs: SimpleExpr,
        glob_pattern: &str,
        case_sensitivity: CaseSensitivity,
    ) -> Result<SimpleExpr, QueryBuildingError> {
        let glob = to_sql_like(glob_pattern);

        match case_sensitivity {
            CaseSensitivity::Sensitive => Ok(lhs.like(glob)),
            CaseSensitivity::Insensitive => Ok(lhs.ilike(glob)),
        }
    }
}
