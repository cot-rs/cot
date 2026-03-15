use chrono::{DateTime, FixedOffset, SubsecRound, TimeZone};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Error)]
pub(crate) enum DateTimeWithOffsetConversionError {
    #[error("nanoseconds out of range")]
    NanosecondsOutOfRange,
    #[error("offset not in valid range")]
    InvalidOffset,
    #[error("datetime out of range for conversion")]
    TimestampOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DateTimeWithOffsetAdapter(DateTime<FixedOffset>);

impl DateTimeWithOffsetAdapter {
    pub(crate) fn new(dt: DateTime<FixedOffset>) -> Self {
        Self(dt)
    }

    /// Return a chrono datetime truncated to microsecond precision,
    /// safe for databases(Postgres, Mysql) that only support up to 6 fractional
    /// digits.
    pub(crate) fn into_chrono_db_safe(self) -> DateTime<FixedOffset> {
        self.0.trunc_subsecs(6)
    }

    pub(crate) fn into_offsetdatetime(self) -> OffsetDateTime {
        self.try_into()
            .expect("could not convert DateTimeWithOffsetAdapter to OffsetDateTime")
    }
}

impl TryFrom<DateTimeWithOffsetAdapter> for OffsetDateTime {
    type Error = DateTimeWithOffsetConversionError;

    fn try_from(value: DateTimeWithOffsetAdapter) -> Result<Self, Self::Error> {
        let total_nanos = value
            .0
            .timestamp_nanos_opt()
            .ok_or(DateTimeWithOffsetConversionError::NanosecondsOutOfRange)?;

        let offset_secs = value.0.offset().local_minus_utc();
        let offset = UtcOffset::from_whole_seconds(offset_secs)
            .map_err(|_| DateTimeWithOffsetConversionError::InvalidOffset)?;

        let dt = OffsetDateTime::from_unix_timestamp_nanos(i128::from(total_nanos))
            .map_err(|_| DateTimeWithOffsetConversionError::TimestampOutOfRange)?
            .to_offset(offset);

        Ok(dt)
    }
}

impl TryFrom<OffsetDateTime> for DateTimeWithOffsetAdapter {
    type Error = DateTimeWithOffsetConversionError;

    fn try_from(value: OffsetDateTime) -> Result<Self, Self::Error> {
        let utc_time = value
            .checked_to_utc()
            .ok_or(DateTimeWithOffsetConversionError::TimestampOutOfRange)?;
        let secs = utc_time.unix_timestamp();
        let nsecs = utc_time.nanosecond();

        let offset_secs = value.offset().whole_seconds();
        let fixed_offset = FixedOffset::east_opt(offset_secs)
            .ok_or(DateTimeWithOffsetConversionError::InvalidOffset)?;

        let fixed_dt = fixed_offset
            .timestamp_opt(secs, nsecs)
            // OffsetDatetime has no ambiguity, so this should be fine.
            .single()
            .ok_or(DateTimeWithOffsetConversionError::TimestampOutOfRange)?;

        Ok(Self(fixed_dt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_both(s: &str) -> (DateTime<FixedOffset>, OffsetDateTime) {
        let chrono_dt = DateTime::parse_from_rfc3339(s).unwrap();
        let time_dt =
            OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).unwrap();
        (chrono_dt, time_dt)
    }

    #[cot::test]
    async fn test_into_offsetdatetime_roundtrip() {
        let (chrono_dt, time_dt) = parse_both("2025-08-01T12:34:56.789123456-04:00");
        let adapter = DateTimeWithOffsetAdapter::new(chrono_dt);
        let back: OffsetDateTime = adapter.into_offsetdatetime();
        assert_eq!(back, time_dt);
    }

    #[cot::test]
    async fn test_from_offsetdatetime_roundtrip() {
        let (_, time_dt) = parse_both("2021-12-31T23:59:59.999999999+00:00");
        let adapter: DateTimeWithOffsetAdapter = time_dt.try_into().unwrap();
        let back: OffsetDateTime = adapter.try_into().unwrap();
        assert_eq!(back, time_dt);
    }

    #[cot::test]
    async fn test_into_chrono_db_safe_truncates_to_micros() {
        let (chrono_dt, _) = parse_both("2025-08-01T12:34:56.789123456+02:00");
        let adapter = DateTimeWithOffsetAdapter::new(chrono_dt);

        let db_dt = adapter.into_chrono_db_safe();

        assert_eq!(format!("{db_dt:?}"), "2025-08-01T12:34:56.789123+02:00");
    }

    #[cot::test]
    async fn test_into_chrono_db_safe_idempotent_if_already_micros() {
        let (chrono_dt, _) = parse_both("2025-08-01T12:34:56.789123+02:00");
        let adapter = DateTimeWithOffsetAdapter::new(chrono_dt);

        let db_dt = adapter.into_chrono_db_safe();

        assert_eq!(format!("{db_dt:?}"), "2025-08-01T12:34:56.789123+02:00");
    }

    #[cot::test]
    async fn test_into_chrono_db_safe_roundtrip_to_offsetdatetime_has_only_micros() {
        let (chrono_dt, _) = parse_both("2025-08-01T12:34:56.789123456+02:00");
        let adapter = DateTimeWithOffsetAdapter::new(chrono_dt);

        let db_dt = adapter.into_chrono_db_safe();
        let db_adapter = DateTimeWithOffsetAdapter::new(db_dt);
        let back: OffsetDateTime = db_adapter.into_offsetdatetime();

        assert_eq!(format!("{db_dt:?}"), "2025-08-01T12:34:56.789123+02:00");

        assert_eq!(back.nanosecond(), db_dt.timestamp_subsec_nanos());
    }
}
