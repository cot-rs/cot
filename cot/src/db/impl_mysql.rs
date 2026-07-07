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
                let expr = lhs
                    .binary(
                        sea_query::BinOper::Custom("COLLATE"),
                        // We assume that the database is using utf8mb4 character set, which is the
                        // default in MySQL 8.0. See https://dev.mysql.com/doc/refman/9.7/en/charset.html
                        // TODO: Allow users to change collation in the future if needed.
                        sea_query::Expr::cust("utf8mb4_bin"),
                    )
                    .like(sql_pattern);
                Ok(expr)
            }
            CaseSensitivity::Insensitive => {
                let like = LikeExpr::new(sql_pattern.to_lowercase()).escape(LIKE_ESCAPE_CHAR);
                Ok(sea_query::Func::lower(lhs).like(like))
            }
        }
    }
}
