//! Database interface implementation – MySQL backend.

use cot::db::query::expr::like::LIKE_ESCAPE_CHAR;
use sea_query::{ExprTrait, LikeExpr, SimpleExpr};

use crate::db::ColumnType;
use crate::db::query::QueryBuildingError;
use crate::db::query::expr::like::{CaseSensitivity, LikeExprBuilder, to_sql_like};
use crate::db::sea_query_db::impl_sea_query_db_backend;

impl_sea_query_db_backend!(DatabaseMySql: sqlx::mysql::MySql, sqlx::mysql::MySqlPool, MySqlRow, MySqlValueRef, sea_query::MysqlQueryBuilder);

impl DatabaseMySql {
    #[expect(clippy::unused_async)]
    async fn init(&self) -> crate::db::Result<()> {
        Ok(())
    }

    fn prepare_values(_values: &mut sea_query_sqlx::SqlxValues) {
        // No changes are needed for MySQL
    }

    #[expect(clippy::unnecessary_wraps)] // to have a unified interface between database impls
    fn last_inserted_row_id_for(result: &sqlx::mysql::MySqlQueryResult) -> Option<u64> {
        Some(result.last_insert_id())
    }

    #[expect(clippy::unused_self)] // to have a unified interface between database impls
    pub(super) fn sea_query_column_type_for(
        &self,
        column_type: ColumnType,
    ) -> sea_query::ColumnType {
        match column_type {
            ColumnType::DateTime | ColumnType::DateTimeWithTimeZone => {
                return sea_query::ColumnType::custom("DATETIME(6)");
            }
            _ => {}
        }

        sea_query::ColumnType::from(column_type)
    }
}

impl LikeExprBuilder for DatabaseMySql {
    fn like_expr(
        &self,
        lhs: SimpleExpr,
        glob_pattern: &str,
        case_sensitivity: CaseSensitivity,
    ) -> Result<SimpleExpr, QueryBuildingError> {
        let sql_pattern = to_sql_like(glob_pattern);

        match case_sensitivity {
            CaseSensitivity::Sensitive => {
                // We assume that the database is using utf8mb4 character set, which is the
                // default in MySQL 8.0. See https://dev.mysql.com/doc/refman/8.0/en/charset.html
                // TODO: Allow users to change collation in the future if needed.
                let collated_lhs = sea_query::Expr::cust_with_exprs("? COLLATE utf8mb4_bin", [lhs]);
                let like = LikeExpr::new(sql_pattern).escape(LIKE_ESCAPE_CHAR);
                Ok(collated_lhs.like(like))
            }
            CaseSensitivity::Insensitive => {
                let like = LikeExpr::new(sql_pattern.to_lowercase()).escape(LIKE_ESCAPE_CHAR);
                Ok(sea_query::Func::lower(lhs).like(like))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_query::{Alias, Asterisk, MysqlQueryBuilder, Query};

    use super::*;
    use crate::test::DEFAULT_MYSQL_TEST_URL;

    #[expect(clippy::unused_async)]
    async fn test_db() -> DatabaseMySql {
        let db_url =
            std::env::var("MYSQL_URL").unwrap_or_else(|_| DEFAULT_MYSQL_TEST_URL.to_string());
        let db_connection = sqlx::mysql::MySqlPoolOptions::new()
            .connect_lazy(&format!("{db_url}/mysql"))
            .expect("lazy pool creation should not fail");
        DatabaseMySql { db_connection }
    }

    fn col_expr() -> SimpleExpr {
        sea_query::Expr::col(Alias::new("name"))
    }

    fn render(expr: SimpleExpr) -> String {
        Query::select()
            .column(Asterisk)
            .from(Alias::new("t"))
            .and_where(expr)
            .to_string(MysqlQueryBuilder)
    }

    fn assert_where(expr: SimpleExpr, expected_where: &str) {
        let sql = render(expr);
        let expected = format!("SELECT * FROM `t` WHERE {expected_where}");
        assert_eq!(sql, expected);
    }

    #[ignore = "Tests that use MySQL are ignored by default"]
    #[cot::test]
    async fn case_sensitive_applies_binary_collation() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "foo%bar", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(
            expr,
            "(`name` COLLATE utf8mb4_bin) LIKE 'foo\\\\%bar' ESCAPE '\\\\'",
        );
    }

    #[ignore = "Tests that use MySQL are ignored by default"]
    #[cot::test]
    async fn case_sensitive_translates_positional_wildcards() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "f??o", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(
            expr,
            "(`name` COLLATE utf8mb4_bin) LIKE 'f__o' ESCAPE '\\\\'",
        );
    }

    #[ignore = "Tests that use MySQL are ignored by default"]
    #[cot::test]
    async fn case_insensitive_lowercases_column_and_pattern() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "FOO*", CaseSensitivity::Insensitive)
            .unwrap();
        assert_where(expr, "LOWER(`name`) LIKE 'foo%' ESCAPE '\\\\'");
    }

    #[ignore = "Tests that use MySQL are ignored by default"]
    #[cot::test]
    async fn case_insensitive_does_not_force_binary_collation() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "foo*", CaseSensitivity::Insensitive)
            .unwrap();
        let sql = render(expr);
        assert!(
            !sql.contains("COLLATE"),
            "insensitive path relies on LOWER, not collation: {sql}"
        );
    }
}
