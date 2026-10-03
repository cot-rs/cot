//! Conversions between [`jiff`] and [`time`] types.
//!
//! Cot exposes `jiff` types in its public API, but some of its dependencies
//! (such as `sqlx` and `tower-sessions`) only work with `time` types. The
//! functions in this module bridge the gap.
//!
//! The conversions from `jiff` to `time` are infallible, because the range of
//! `jiff` types is a subset of the range of the corresponding `time` types. The
//! conversions in the other direction can fail for values at the edges of the
//! `time` range.

use thiserror::Error;

/// A `time` value can't be represented as a `jiff` value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Error)]
#[error("date/time value is out of the supported range")]
pub(crate) struct OutOfRangeError;

#[cfg(any(feature = "postgres", feature = "mysql", test))]
pub(crate) fn timestamp_to_time(timestamp: jiff::Timestamp) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp_nanos(timestamp.as_nanosecond())
        .expect("jiff::Timestamp range is a subset of time::OffsetDateTime range")
}

pub(crate) fn timestamp_from_time(
    datetime: time::OffsetDateTime,
) -> Result<jiff::Timestamp, OutOfRangeError> {
    // Not using `jiff::Timestamp::from_nanosecond`, because as of jiff 0.2.37
    // it doesn't check whether the value is within the supported range.
    jiff::Timestamp::new(
        datetime.unix_timestamp(),
        datetime.nanosecond().cast_signed(),
    )
    .map_err(|_| OutOfRangeError)
}

#[cfg(any(feature = "postgres", feature = "mysql", test))]
pub(crate) fn date_to_time(date: jiff::civil::Date) -> time::Date {
    let month = time::Month::try_from(date.month().cast_unsigned())
        .expect("jiff::civil::Date month is always in range");
    time::Date::from_calendar_date(i32::from(date.year()), month, date.day().cast_unsigned())
        .expect("jiff::civil::Date range is a subset of time::Date range")
}

pub(crate) fn date_from_time(date: time::Date) -> Result<jiff::civil::Date, OutOfRangeError> {
    let year = i16::try_from(date.year()).map_err(|_| OutOfRangeError)?;
    jiff::civil::Date::new(
        year,
        u8::from(date.month()).cast_signed(),
        date.day().cast_signed(),
    )
    .map_err(|_| OutOfRangeError)
}

#[cfg(any(feature = "postgres", feature = "mysql", test))]
pub(crate) fn time_to_time(time: jiff::civil::Time) -> time::Time {
    time::Time::from_hms_nano(
        time.hour().cast_unsigned(),
        time.minute().cast_unsigned(),
        time.second().cast_unsigned(),
        time.subsec_nanosecond().cast_unsigned(),
    )
    .expect("jiff::civil::Time components are always in range")
}

pub(crate) fn time_from_time(time: time::Time) -> jiff::civil::Time {
    jiff::civil::Time::new(
        time.hour().cast_signed(),
        time.minute().cast_signed(),
        time.second().cast_signed(),
        time.nanosecond().cast_signed(),
    )
    .expect("time::Time components are always in range")
}

#[cfg(any(feature = "postgres", feature = "mysql", test))]
pub(crate) fn datetime_to_time(datetime: jiff::civil::DateTime) -> time::PrimitiveDateTime {
    time::PrimitiveDateTime::new(date_to_time(datetime.date()), time_to_time(datetime.time()))
}

pub(crate) fn datetime_from_time(
    datetime: time::PrimitiveDateTime,
) -> Result<jiff::civil::DateTime, OutOfRangeError> {
    Ok(date_from_time(datetime.date())?.to_datetime(time_from_time(datetime.time())))
}

#[cfg(test)]
mod tests {
    use jiff::civil;

    use super::*;

    #[test]
    fn timestamp_roundtrip() {
        for timestamp in [
            jiff::Timestamp::UNIX_EPOCH,
            jiff::Timestamp::MIN,
            jiff::Timestamp::MAX,
            "2023-01-02T12:34:56.123456789Z".parse().unwrap(),
            "1900-01-02T12:34:56.000000001-05:00".parse().unwrap(),
        ] {
            let converted = timestamp_to_time(timestamp);
            assert_eq!(converted.unix_timestamp_nanos(), timestamp.as_nanosecond());
            assert_eq!(timestamp_from_time(converted), Ok(timestamp));
        }
    }

    #[test]
    fn timestamp_from_time_keeps_instant_for_non_utc_offset() {
        let datetime = time::OffsetDateTime::new_in_offset(
            time::Date::from_calendar_date(2023, time::Month::January, 2).unwrap(),
            time::Time::from_hms(12, 34, 56).unwrap(),
            time::UtcOffset::from_hms(-2, -30, 0).unwrap(),
        );

        assert_eq!(
            timestamp_from_time(datetime),
            Ok("2023-01-02T15:04:56Z".parse().unwrap())
        );
    }

    #[test]
    fn timestamp_from_time_out_of_range() {
        let datetime = time::Date::MIN.midnight().assume_utc();

        assert_eq!(timestamp_from_time(datetime), Err(OutOfRangeError));
    }

    #[test]
    fn date_roundtrip() {
        for date in [
            civil::Date::MIN,
            civil::Date::MAX,
            civil::date(2024, 2, 29),
            civil::date(-1, 12, 31),
        ] {
            let converted = date_to_time(date);
            assert_eq!(
                (
                    converted.year(),
                    u8::from(converted.month()),
                    converted.day()
                ),
                (
                    i32::from(date.year()),
                    date.month().cast_unsigned(),
                    date.day().cast_unsigned()
                )
            );
            assert_eq!(date_from_time(converted), Ok(date));
        }
    }

    #[test]
    fn time_roundtrip() {
        for time in [
            civil::Time::MIN,
            civil::Time::MAX,
            civil::time(12, 34, 56, 123_456_789),
        ] {
            let converted = time_to_time(time);
            assert_eq!(
                converted.as_hms_nano(),
                (
                    time.hour().cast_unsigned(),
                    time.minute().cast_unsigned(),
                    time.second().cast_unsigned(),
                    time.subsec_nanosecond().cast_unsigned()
                )
            );
            assert_eq!(time_from_time(converted), time);
        }
    }

    #[test]
    fn datetime_roundtrip() {
        for datetime in [
            civil::DateTime::MIN,
            civil::DateTime::MAX,
            civil::date(2023, 1, 2).at(12, 34, 56, 123_456_789),
        ] {
            let converted = datetime_to_time(datetime);
            assert_eq!(converted.date(), date_to_time(datetime.date()));
            assert_eq!(converted.time(), time_to_time(datetime.time()));
            assert_eq!(datetime_from_time(converted), Ok(datetime));
        }
    }
}
