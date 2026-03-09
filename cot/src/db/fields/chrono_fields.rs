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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, Utc};

    use crate::db::{ColumnType, DatabaseField, DbValue, ToDbValue};

    #[test]
    fn test_datetime_fixed_offset_column_type() {
        assert_eq!(
            <DateTime<FixedOffset> as DatabaseField>::TYPE,
            ColumnType::DateTimeWithTimeZone
        );
        const {
            assert!(!<DateTime<FixedOffset> as DatabaseField>::NULLABLE);
        }
    }

    #[test]
    fn test_datetime_utc_column_type() {
        assert_eq!(
            <DateTime<Utc> as DatabaseField>::TYPE,
            ColumnType::DateTimeWithTimeZone
        );
        const {
            assert!(!<DateTime<Utc> as DatabaseField>::NULLABLE);
        }
    }

    #[test]
    fn test_option_datetime_column_type() {
        assert_eq!(
            <Option<DateTime<FixedOffset>> as DatabaseField>::TYPE,
            ColumnType::DateTimeWithTimeZone
        );
        const {
            assert!(<Option<DateTime<FixedOffset>> as DatabaseField>::NULLABLE);
        }

        assert_eq!(
            <Option<DateTime<Utc>> as DatabaseField>::TYPE,
            ColumnType::DateTimeWithTimeZone
        );
        const {
            assert!(<Option<DateTime<Utc>> as DatabaseField>::NULLABLE);
        }
    }

    #[test]
    fn test_datetime_fixed_offset_to_db_value() {
        let dt = DateTime::parse_from_rfc3339("2023-01-01T12:00:00+01:00").unwrap();
        let db_value = dt.to_db_value();

        match db_value {
            DbValue::ChronoDateTimeWithTimeZone(Some(v)) => assert_eq!(*v, dt),
            _ => panic!("Expected DbValue::ChronoDateTimeWithTimeZone, got {db_value:?}"),
        }
    }

    #[test]
    fn test_datetime_utc_to_db_value() {
        let dt = Utc::now();
        let db_value = dt.to_db_value();

        match db_value {
            DbValue::ChronoDateTimeUtc(Some(v)) => assert_eq!(*v, dt),
            _ => panic!("Expected DbValue::ChronoDateTimeUtc, got {db_value:?}"),
        }
    }

    #[test]
    fn test_option_datetime_to_db_value() {
        let dt = DateTime::parse_from_rfc3339("2023-01-01T12:00:00+01:00").unwrap();
        let some_dt = Some(dt);
        let none_dt: Option<DateTime<FixedOffset>> = None;

        match some_dt.to_db_value() {
            DbValue::ChronoDateTimeWithTimeZone(Some(v)) => assert_eq!(*v, dt),
            _ => panic!(
                "Expected DbValue::ChronoDateTimeWithTimeZone(Some), got {:?}",
                some_dt.to_db_value()
            ),
        }

        assert_eq!(
            none_dt.to_db_value(),
            DbValue::ChronoDateTimeWithTimeZone(None)
        );

        let dt_utc = Utc::now();
        let some_dt_utc = Some(dt_utc);
        let none_dt_utc: Option<DateTime<Utc>> = None;

        match some_dt_utc.to_db_value() {
            DbValue::ChronoDateTimeUtc(Some(v)) => assert_eq!(*v, dt_utc),
            _ => panic!(
                "Expected DbValue::ChronoDateTimeUtc(Some), got {:?}",
                some_dt_utc.to_db_value()
            ),
        }

        assert_eq!(none_dt_utc.to_db_value(), DbValue::ChronoDateTimeUtc(None));
    }
}
