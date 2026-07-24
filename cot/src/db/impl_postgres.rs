//! Database interface implementation – PostgreSQL backend.

use cot::db::query::QueryBuildingError;
use sea_query::extension::postgres::PgExpr;
use sea_query::{ExprTrait, LikeExpr, SimpleExpr};

use crate::db::query::expr::like::{CaseSensitivity, LikeExprBuilder, to_sql_like};
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

impl LikeExprBuilder for DatabasePostgres {
    fn like_expr(
        &self,
        lhs: SimpleExpr,
        glob_pattern: &str,
        case_sensitivity: CaseSensitivity,
    ) -> Result<SimpleExpr, QueryBuildingError> {
        let glob = LikeExpr::new(to_sql_like(glob_pattern));

        match case_sensitivity {
            CaseSensitivity::Sensitive => Ok(lhs.like(glob)),
            CaseSensitivity::Insensitive => Ok(lhs.ilike(glob)),
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_query::{Alias, Asterisk, PostgresQueryBuilder, Query};

    use super::*;
    use crate::test::DEFAULT_POSTGRES_TEST_URL;

    #[expect(clippy::unused_async)]
    async fn test_db() -> DatabasePostgres {
        let db_url =
            std::env::var("POSTGRES_URL").unwrap_or_else(|_| DEFAULT_POSTGRES_TEST_URL.to_string());
        let db_connection = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy(&format!("{db_url}/postgres"))
            .expect("lazy pool creation should not fail");
        DatabasePostgres { db_connection }
    }

    fn col_expr() -> SimpleExpr {
        sea_query::Expr::col(Alias::new("name"))
    }

    fn render(expr: SimpleExpr) -> String {
        Query::select()
            .column(Asterisk)
            .from(Alias::new("t"))
            .and_where(expr)
            .to_string(PostgresQueryBuilder)
    }

    fn assert_where(expr: SimpleExpr, expected_where: &str) {
        let sql = render(expr);
        let expected = format!("SELECT * FROM \"t\" WHERE {expected_where}");
        assert_eq!(sql, expected);
    }

    #[ignore = "Tests that use PostgreSQL are ignored by default"]
    #[cot::test]
    async fn case_sensitive_uses_like_not_ilike() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "foo*", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(expr, "\"name\" LIKE 'foo%'");
    }

    #[ignore = "Tests that use PostgreSQL are ignored by default"]
    #[cot::test]
    async fn case_insensitive_uses_ilike_and_preserves_pattern_case() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "Foo*", CaseSensitivity::Insensitive)
            .unwrap();
        assert_where(expr, "\"name\" ILIKE 'Foo%'");
    }

    #[ignore = "Tests that use PostgreSQL are ignored by default"]
    #[cot::test]
    async fn glob_wildcards_translate_to_sql_wildcards() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "f??o", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(expr, "\"name\" LIKE 'f__o'");
    }

    #[ignore = "Tests that use PostgreSQL are ignored by default"]
    #[cot::test]
    async fn literal_percent_and_underscore_are_escaped() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "100%off_sale", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(expr, "\"name\" LIKE E'100\\\\%off\\\\_sale'");
    }
}
