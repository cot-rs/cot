//! `DatabaseField` implementations for [`jiff`] types.
//!
//! `sqlx` doesn't support `jiff` types natively yet, so the values are passed
//! to the database as `time` types (PostgreSQL and MySQL) or as text (SQLite).
//! The conversion happens in the `prepare_values` function of each database
//! backend, right before the values are bound to the query.

use jiff::civil;

use crate::db::fields::impl_to_db_value_default;
#[cfg(feature = "mysql")]
use crate::db::impl_mysql::MySqlValueRef;
#[cfg(feature = "postgres")]
use crate::db::impl_postgres::PostgresValueRef;
#[cfg(feature = "sqlite")]
use crate::db::impl_sqlite::SqliteValueRef;
use crate::db::{
    ColumnType, DatabaseError, DatabaseField, DbValue, FromDbValue, Result, SqlxValueRef, ToDbValue,
};
use crate::utils::jiff::{
    OutOfRangeError, date_from_time, datetime_from_time, time_from_time, timestamp_from_time,
};

fn map_out_of_range<T>(result: std::result::Result<T, OutOfRangeError>) -> Result<T> {
    result.map_err(DatabaseError::value_decode)
}

#[expect(clippy::unnecessary_wraps, reason = "to have a unified interface")]
fn civil_time_from_time(time: time::Time) -> std::result::Result<civil::Time, OutOfRangeError> {
    Ok(time_from_time(time))
}

macro_rules! impl_jiff_db_field {
    ($ty:ty, $time_ty:ty, $column_type:ident, $from_time:path) => {
        impl DatabaseField for $ty {
            const TYPE: ColumnType = ColumnType::$column_type;
        }

        impl FromDbValue for $ty {
            #[cfg(feature = "sqlite")]
            fn from_sqlite(value: SqliteValueRef<'_>) -> Result<Self> {
                map_out_of_range($from_time(value.get::<$time_ty>()?))
            }

            #[cfg(feature = "postgres")]
            fn from_postgres(value: PostgresValueRef<'_>) -> Result<Self> {
                map_out_of_range($from_time(value.get::<$time_ty>()?))
            }

            #[cfg(feature = "mysql")]
            fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
                map_out_of_range($from_time(value.get::<$time_ty>()?))
            }
        }

        impl FromDbValue for Option<$ty> {
            #[cfg(feature = "sqlite")]
            fn from_sqlite(value: SqliteValueRef<'_>) -> Result<Self> {
                map_out_of_range(value.get::<Option<$time_ty>>()?.map($from_time).transpose())
            }

            #[cfg(feature = "postgres")]
            fn from_postgres(value: PostgresValueRef<'_>) -> Result<Self> {
                map_out_of_range(value.get::<Option<$time_ty>>()?.map($from_time).transpose())
            }

            #[cfg(feature = "mysql")]
            fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
                map_out_of_range(value.get::<Option<$time_ty>>()?.map($from_time).transpose())
            }
        }

        impl_to_db_value_default!($ty);
    };
}

impl_jiff_db_field!(
    jiff::Timestamp,
    time::OffsetDateTime,
    DateTimeWithTimeZone,
    timestamp_from_time
);
impl_jiff_db_field!(civil::Date, time::Date, Date, date_from_time);
impl_jiff_db_field!(civil::Time, time::Time, Time, civil_time_from_time);
impl_jiff_db_field!(
    civil::DateTime,
    time::PrimitiveDateTime,
    DateTime,
    datetime_from_time
);

fn weekday_from_db(value: u8) -> Result<civil::Weekday> {
    i8::try_from(value)
        .ok()
        .and_then(|value| civil::Weekday::from_monday_zero_offset(value).ok())
        .ok_or_else(|| DatabaseError::ValueDecode(format!("invalid weekday value: {value}").into()))
}

impl FromDbValue for civil::Weekday {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> Result<Self> {
        value.get::<u8>().and_then(weekday_from_db)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> Result<Self> {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        value.get::<i16>().and_then(|v| weekday_from_db(v as u8))
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> Result<Self> {
        value.get::<u8>().and_then(weekday_from_db)
    }
}

impl ToDbValue for civil::Weekday {
    fn to_db_value(&self) -> DbValue {
        u32::from(self.to_monday_zero_offset().cast_unsigned()).into()
    }
}

/// Converts `jiff` values into `time` values that `sqlx` can bind.
#[cfg(any(feature = "postgres", feature = "mysql"))]
pub(crate) fn jiff_to_time_value(value: &mut DbValue) {
    use crate::utils::jiff::{date_to_time, datetime_to_time, time_to_time, timestamp_to_time};

    let converted = match value {
        DbValue::JiffDate(date) => DbValue::TimeDate(date.map(date_to_time)),
        DbValue::JiffTime(time) => DbValue::TimeTime(time.map(time_to_time)),
        DbValue::JiffDateTime(datetime) => {
            DbValue::TimeDateTime(datetime.as_deref().copied().map(datetime_to_time))
        }
        DbValue::JiffTimestamp(timestamp) => {
            DbValue::TimeDateTimeWithTimeZone(timestamp.as_deref().copied().map(timestamp_to_time))
        }
        _ => return,
    };
    *value = converted;
}

/// Converts `jiff` values into text in the format used for storing date/time
/// values in SQLite.
///
/// The format is the same as the one used by `sqlx` for `chrono` types, which
/// is what previous versions of Cot used. Since SQLite compares the values
/// as text, keeping the format the same ensures that the newly written values
/// can be compared with the existing ones.
#[cfg(feature = "sqlite")]
pub(crate) fn jiff_to_sqlite_text_value(value: &mut DbValue) {
    let text = match value {
        DbValue::JiffDate(date) => date.map(sqlite_date),
        DbValue::JiffTime(time) => time.map(sqlite_time),
        DbValue::JiffDateTime(datetime) => datetime.as_deref().copied().map(sqlite_datetime),
        DbValue::JiffTimestamp(timestamp) => timestamp.as_deref().copied().map(sqlite_timestamp),
        _ => return,
    };
    *value = DbValue::String(text);
}

/// `%F` in `chrono`'s format syntax.
#[cfg(feature = "sqlite")]
fn sqlite_date(date: civil::Date) -> String {
    let (year, month, day) = (date.year(), date.month(), date.day());
    if year < 0 {
        // the sign counts towards the width
        format!("{year:05}-{month:02}-{day:02}")
    } else {
        format!("{year:04}-{month:02}-{day:02}")
    }
}

/// `%T%.f` in `chrono`'s format syntax.
#[cfg(feature = "sqlite")]
fn sqlite_time(time: civil::Time) -> String {
    let nanos = time.subsec_nanosecond();
    let fraction = if nanos == 0 {
        String::new()
    } else if nanos % 1_000_000 == 0 {
        format!(".{:03}", nanos / 1_000_000)
    } else if nanos % 1_000 == 0 {
        format!(".{:06}", nanos / 1_000)
    } else {
        format!(".{nanos:09}")
    };

    format!(
        "{:02}:{:02}:{:02}{fraction}",
        time.hour(),
        time.minute(),
        time.second()
    )
}

/// `%F %T%.f` in `chrono`'s format syntax.
#[cfg(feature = "sqlite")]
fn sqlite_datetime(datetime: civil::DateTime) -> String {
    format!(
        "{} {}",
        sqlite_date(datetime.date()),
        sqlite_time(datetime.time())
    )
}

/// RFC 3339 with the UTC offset written as `+00:00` (as opposed to `Z`).
#[cfg(feature = "sqlite")]
fn sqlite_timestamp(timestamp: jiff::Timestamp) -> String {
    let datetime = jiff::tz::Offset::UTC.to_datetime(timestamp);
    format!(
        "{}T{}+00:00",
        sqlite_date(datetime.date()),
        sqlite_time(datetime.time())
    )
}

#[cfg(test)]
mod tests {
    use jiff::civil::{Weekday, date};

    use super::*;

    #[test]
    fn column_types() {
        assert_eq!(
            <jiff::Timestamp as DatabaseField>::TYPE,
            ColumnType::DateTimeWithTimeZone
        );
        assert_eq!(<civil::Date as DatabaseField>::TYPE, ColumnType::Date);
        assert_eq!(<civil::Time as DatabaseField>::TYPE, ColumnType::Time);
        assert_eq!(
            <civil::DateTime as DatabaseField>::TYPE,
            ColumnType::DateTime
        );
        const {
            assert!(!<jiff::Timestamp as DatabaseField>::NULLABLE);
            assert!(<Option<jiff::Timestamp> as DatabaseField>::NULLABLE);
        }
    }

    #[test]
    fn to_db_value() {
        let timestamp: jiff::Timestamp = "2023-01-02T12:34:56Z".parse().unwrap();
        assert_eq!(
            timestamp.to_db_value(),
            DbValue::JiffTimestamp(Some(Box::new(timestamp)))
        );
        assert_eq!(
            None::<jiff::Timestamp>.to_db_value(),
            DbValue::JiffTimestamp(None)
        );
        assert_eq!(
            date(2023, 1, 2).to_db_value(),
            DbValue::JiffDate(Some(date(2023, 1, 2)))
        );
    }

    #[test]
    fn weekday_to_db_value() {
        assert_eq!(Weekday::Monday.to_db_value(), DbValue::Unsigned(Some(0)));
        assert_eq!(Weekday::Sunday.to_db_value(), DbValue::Unsigned(Some(6)));
    }

    #[test]
    fn weekday_from_db_value() {
        assert_eq!(weekday_from_db(0).unwrap(), Weekday::Monday);
        assert_eq!(weekday_from_db(6).unwrap(), Weekday::Sunday);
        assert!(weekday_from_db(7).is_err());
        assert!(weekday_from_db(u8::MAX).is_err());
    }

    #[cfg(any(feature = "postgres", feature = "mysql"))]
    #[test]
    fn jiff_to_time_value_converts() {
        let mut value = date(2023, 1, 2).at(12, 34, 56, 0).to_db_value();
        jiff_to_time_value(&mut value);
        assert_eq!(
            value,
            DbValue::TimeDateTime(Some(time::PrimitiveDateTime::new(
                time::Date::from_calendar_date(2023, time::Month::January, 2).unwrap(),
                time::Time::from_hms(12, 34, 56).unwrap(),
            )))
        );

        let mut value = None::<jiff::Timestamp>.to_db_value();
        jiff_to_time_value(&mut value);
        assert_eq!(value, DbValue::TimeDateTimeWithTimeZone(None));
    }

    #[cfg(any(feature = "postgres", feature = "mysql"))]
    #[test]
    fn jiff_to_time_value_ignores_other_values() {
        let mut value = DbValue::Int(Some(42));
        jiff_to_time_value(&mut value);
        assert_eq!(value, DbValue::Int(Some(42)));
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_text(value: &impl ToDbValue) -> DbValue {
        let mut value = value.to_db_value();
        jiff_to_sqlite_text_value(&mut value);
        value
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn jiff_to_sqlite_text_value_converts() {
        let string = |s: &str| DbValue::String(Some(s.to_owned()));

        assert_eq!(sqlite_text(&date(2023, 1, 2)), string("2023-01-02"));
        assert_eq!(sqlite_text(&date(-1, 1, 2)), string("-0001-01-02"));
        assert_eq!(sqlite_text(&date(12, 1, 2)), string("0012-01-02"));
        assert_eq!(sqlite_text(&civil::time(1, 2, 3, 0)), string("01:02:03"));
        assert_eq!(
            sqlite_text(&civil::time(1, 2, 3, 100_000_000)),
            string("01:02:03.100")
        );
        assert_eq!(
            sqlite_text(&civil::time(1, 2, 3, 120_000)),
            string("01:02:03.000120")
        );
        assert_eq!(
            sqlite_text(&civil::time(1, 2, 3, 1)),
            string("01:02:03.000000001")
        );
        assert_eq!(
            sqlite_text(&date(2023, 1, 2).at(12, 34, 56, 789_000_000)),
            string("2023-01-02 12:34:56.789")
        );
        assert_eq!(
            sqlite_text(
                &"2023-01-02T12:34:56.123456+01:00"
                    .parse::<jiff::Timestamp>()
                    .unwrap()
            ),
            string("2023-01-02T11:34:56.123456+00:00")
        );
        assert_eq!(sqlite_text(&None::<civil::Date>), DbValue::String(None));
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn jiff_to_sqlite_text_value_is_idempotent() {
        let mut value = date(2023, 1, 2).to_db_value();
        jiff_to_sqlite_text_value(&mut value);
        jiff_to_sqlite_text_value(&mut value);
        assert_eq!(value, DbValue::String(Some("2023-01-02".to_owned())));
    }
}
