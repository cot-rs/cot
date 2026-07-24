//! Database interface implementation – SQLite backend.

use sea_query::extension::sqlite::SqliteExpr;
use sea_query::{ExprTrait, LikeExpr, SimpleExpr};
use sea_query_sqlx::SqlxValues;

use crate::db::query::QueryBuildingError;
use crate::db::query::expr::like::{
    CaseSensitivity, LIKE_ESCAPE_CHAR, LikeExprBuilder, to_sql_like,
};
use crate::db::sea_query_db::impl_sea_query_db_backend;

impl_sea_query_db_backend!(DatabaseSqlite: sqlx::sqlite::Sqlite, sqlx::sqlite::SqlitePool, SqliteRow, SqliteValueRef, sea_query::SqliteQueryBuilder);

impl DatabaseSqlite {
    async fn init(&self) -> crate::db::Result<()> {
        self.raw("PRAGMA foreign_keys = ON").await?;
        Ok(())
    }

    async fn raw(&self, sql: &str) -> crate::db::Result<crate::db::StatementResult> {
        self.raw_with(sql, SqlxValues(sea_query::Values(Vec::new())))
            .await
    }

    fn prepare_values(_values: &mut SqlxValues) {
        // No changes are needed for SQLite
    }

    #[expect(clippy::unnecessary_wraps)] // to have a unified interface between database impls
    fn last_inserted_row_id_for(result: &sqlx::sqlite::SqliteQueryResult) -> Option<u64> {
        #[expect(clippy::cast_sign_loss)]
        Some(result.last_insert_rowid() as u64)
    }

    #[expect(clippy::unused_self)] // to have a unified interface between database impls
    pub(super) fn sea_query_column_type_for(
        &self,
        column_type: crate::db::ColumnType,
    ) -> sea_query::ColumnType {
        sea_query::ColumnType::from(column_type)
    }
}

impl LikeExprBuilder for DatabaseSqlite {
    fn like_expr(
        &self,
        lhs: SimpleExpr,
        glob_pattern: &str,
        case_sensitivity: CaseSensitivity,
    ) -> Result<SimpleExpr, QueryBuildingError> {
        match case_sensitivity {
            CaseSensitivity::Sensitive => {
                let glob = to_sqlite_glob(glob_pattern);
                Ok(lhs.glob(glob))
            }
            CaseSensitivity::Insensitive => {
                let like = LikeExpr::new(to_sql_like(&glob_pattern.to_lowercase()))
                    .escape(LIKE_ESCAPE_CHAR);
                Ok(sea_query::Func::lower(lhs).like(like))
            }
        }
    }
}

fn to_sqlite_glob(glob: &str) -> String {
    let mut escaped = String::with_capacity(glob.len());
    let mut chars = glob.chars();
    while let Some(ch) = chars.next() {
        match ch {
            LIKE_ESCAPE_CHAR => {
                if let Some(ch) = chars.next() {
                    push_glob_literal(&mut escaped, ch);
                }
            }
            '*' => escaped.push('*'),
            '?' => escaped.push('?'),
            other => push_glob_literal(&mut escaped, other),
        }
    }
    escaped
}

fn push_glob_literal(out: &mut String, ch: char) {
    if matches!(ch, '*' | '?' | '[') {
        out.push('[');
        out.push(ch);
        out.push(']');
    } else {
        out.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use sea_query::{Alias, Asterisk, Query, SqliteQueryBuilder};

    use super::*;
    use crate::test::TestDatabase;

    #[test]
    fn test_to_sqlite_glob() {
        assert_eq!(to_sqlite_glob("*foo*"), "*foo*");
        assert_eq!(to_sqlite_glob("f?o"), "f?o");
        assert_eq!(to_sqlite_glob("a\\*b"), "a[*]b");
        assert_eq!(to_sqlite_glob("a\\?b"), "a[?]b");
        assert_eq!(to_sqlite_glob("100[percent]"), "100[[]percent]");
        assert_eq!(to_sqlite_glob("[abc]"), "[[]abc]");
        assert_eq!(to_sqlite_glob("hello world"), "hello world");
        assert_eq!(to_sqlite_glob(""), "");
        assert_eq!(to_sqlite_glob("foo\\"), "foo");
        assert_eq!(to_sqlite_glob("C:\\\\Users"), "C:\\Users");
        assert_eq!(to_sqlite_glob("*foo\\*bar?baz*"), "*foo[*]bar?baz*");
        assert_eq!(to_sqlite_glob("\\*\\?\\["), "[*][?][[]");
        assert_eq!(to_sqlite_glob("café*"), "café*");
        assert_eq!(to_sqlite_glob("日本語?"), "日本語?");
        assert_eq!(to_sqlite_glob("***"), "***");
        assert_eq!(to_sqlite_glob("???"), "???");
    }

    fn assert_where(expr: SimpleExpr, expected_where: &str) {
        let sql = render(expr);
        let expected = format!("SELECT * FROM \"t\" WHERE {expected_where}");
        assert_eq!(sql, expected);
    }

    async fn test_db() -> TestDatabase {
        TestDatabase::new_sqlite().await.unwrap()
    }

    fn col_expr() -> SimpleExpr {
        sea_query::Expr::col(Alias::new("name"))
    }

    fn render(expr: SimpleExpr) -> String {
        Query::select()
            .column(Asterisk)
            .from(Alias::new("t"))
            .and_where(expr)
            .to_string(SqliteQueryBuilder)
    }

    #[cot::test]
    async fn case_sensitive_uses_glob() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "foo*", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(expr, "\"name\" GLOB 'foo*'");
    }

    #[cot::test]
    async fn case_sensitive_positional_pattern_translates_question_marks() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "f??o", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(expr, "\"name\" GLOB 'f??o'");
    }

    #[cot::test]
    async fn case_sensitive_escapes_literal_wildcard_chars_for_glob() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "50\\*off", CaseSensitivity::Sensitive)
            .unwrap();
        assert_where(expr, "\"name\" GLOB '50[*]off'");
    }

    #[cot::test]
    async fn case_insensitive_uses_lower_and_like() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "Foo*", CaseSensitivity::Insensitive)
            .unwrap();
        assert_where(expr, "LOWER(\"name\") LIKE 'foo%' ESCAPE '\\'");
    }

    #[cot::test]
    async fn case_insensitive_pattern_is_lowercased_before_conversion() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "README", CaseSensitivity::Insensitive)
            .unwrap();
        assert_where(expr, "LOWER(\"name\") LIKE 'readme' ESCAPE '\\'");
    }

    #[cot::test]
    async fn case_insensitive_includes_escape_clause() {
        let db = test_db().await;
        let expr = db
            .like_expr(col_expr(), "100\\%off", CaseSensitivity::Insensitive)
            .unwrap();
        assert_where(expr, "LOWER(\"name\") LIKE '100\\%off' ESCAPE '\\'");
    }
}
