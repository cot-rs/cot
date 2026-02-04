use crate::db::fields::{
    impl_from_postgres_default, impl_from_sqlite_default, impl_to_db_value_default,
};
#[cfg(feature = "mysql")]
use crate::db::impl_mysql::MySqlValueRef;
use crate::db::{ColumnType, DatabaseField, FromDbValue, Result, SqlxValueRef};

impl DatabaseField for chrono::DateTime<chrono::FixedOffset> {
    const TYPE: ColumnType = ColumnType::DateTimeWithTimeZone;
}

impl FromDbValue for chrono::DateTime<chrono::FixedOffset> {
    impl_from_sqlite_default!();

    impl_from_postgres_default!();

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
        Ok(value.get::<chrono::DateTime<chrono::Utc>>()?.fixed_offset())
    }
}
impl FromDbValue for Option<chrono::DateTime<chrono::FixedOffset>> {
    impl_from_sqlite_default!();

    impl_from_postgres_default!();

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
        Ok(value
            .get::<Option<chrono::DateTime<chrono::Utc>>>()?
            .map(|dt| dt.fixed_offset()))
    }
}

impl_to_db_value_default!(chrono::DateTime<chrono::FixedOffset>);

impl DatabaseField for chrono::DateTime<chrono::Utc> {
    const TYPE: ColumnType = ColumnType::DateTimeWithTimeZone;
}

impl FromDbValue for chrono::DateTime<chrono::Utc> {
    impl_from_sqlite_default!();

    impl_from_postgres_default!();

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
        value.get::<Self>()
    }
}
impl FromDbValue for Option<chrono::DateTime<chrono::Utc>> {
    impl_from_sqlite_default!();

    impl_from_postgres_default!();

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
        value.get::<Option<chrono::DateTime<chrono::Utc>>>()
    }
}

impl_to_db_value_default!(chrono::DateTime<chrono::Utc>);
