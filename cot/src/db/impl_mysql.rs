//! Database interface implementation – MySQL backend.

use sea_query::{ExprTrait, SimpleExpr};

use crate::db::ColumnType;
use crate::db::query::QueryBuildingError;
use crate::db::query::expr::like::{CaseSensitivity, LikeDialect, to_sql_like};
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

impl LikeDialect for DatabaseMySql {
    fn like_expr(
        &self,
        lhs: SimpleExpr,
        glob_pattern: &str,
        case_sensitivity: CaseSensitivity,
    ) -> Result<SimpleExpr, QueryBuildingError> {
        let glob = to_sql_like(glob_pattern);

        match case_sensitivity {
            CaseSensitivity::Sensitive => {
                let expr = lhs
                    .binary(
                        sea_query::BinOper::Custom("COLLATE"),
                        sea_query::Expr::cust("utf8mb4_bin"),
                    )
                    .like(glob);
                Ok(expr)
            }
            CaseSensitivity::Insensitive => {
                Ok(sea_query::Func::lower(lhs).like(glob.to_lowercase()))
            }
        }
    }
}
