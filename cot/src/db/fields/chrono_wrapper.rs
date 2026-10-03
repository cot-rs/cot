use chrono::Weekday;

use crate::common_types::WeekdaySet;
#[cfg(feature = "mysql")]
use crate::db::impl_mysql::MySqlValueRef;
#[cfg(feature = "postgres")]
use crate::db::impl_postgres::PostgresValueRef;
#[cfg(feature = "sqlite")]
use crate::db::impl_sqlite::SqliteValueRef;
use crate::db::{DatabaseError, DbValue, FromDbValue, SqlxValueRef, ToDbValue};

fn weekday_from_chrono(weekday: Weekday) -> jiff::civil::Weekday {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is always in 0..7"
    )]
    jiff::civil::Weekday::from_monday_zero_offset(weekday.num_days_from_monday() as i8)
        .expect("chrono::Weekday is always a valid weekday")
}

fn weekday_to_chrono(weekday: jiff::civil::Weekday) -> Weekday {
    Weekday::try_from(weekday.to_monday_zero_offset().cast_unsigned())
        .expect("jiff::civil::Weekday is always a valid weekday")
}

impl From<chrono::WeekdaySet> for WeekdaySet {
    fn from(set: chrono::WeekdaySet) -> Self {
        set.iter(Weekday::Mon).map(weekday_from_chrono).collect()
    }
}

impl From<WeekdaySet> for chrono::WeekdaySet {
    fn from(set: WeekdaySet) -> Self {
        let mut new_set = chrono::WeekdaySet::EMPTY;
        for weekday in set.iter() {
            new_set.insert(weekday_to_chrono(weekday));
        }
        new_set
    }
}

impl FromDbValue for Weekday {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<u8>()
            .and_then(|v| Weekday::try_from(v).map_err(|e| DatabaseError::ValueDecode(e.into())))
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        value.get::<i16>().and_then(|v| {
            Weekday::try_from(v as u8).map_err(|e| DatabaseError::ValueDecode(e.into()))
        })
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<u8>()
            .and_then(|v| Weekday::try_from(v).map_err(|e| DatabaseError::ValueDecode(e.into())))
    }
}
impl ToDbValue for Weekday {
    fn to_db_value(&self) -> DbValue {
        self.num_days_from_monday().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_conversion() {
        for (chrono_weekday, jiff_weekday) in [
            (Weekday::Mon, jiff::civil::Weekday::Monday),
            (Weekday::Wed, jiff::civil::Weekday::Wednesday),
            (Weekday::Sun, jiff::civil::Weekday::Sunday),
        ] {
            assert_eq!(weekday_from_chrono(chrono_weekday), jiff_weekday);
            assert_eq!(weekday_to_chrono(jiff_weekday), chrono_weekday);
        }
    }

    #[test]
    fn weekday_set_chrono_conversion() {
        let mut chrono_set = chrono::WeekdaySet::EMPTY;
        chrono_set.insert(Weekday::Mon);
        chrono_set.insert(Weekday::Sun);

        let set = WeekdaySet::from(chrono_set);
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [jiff::civil::Weekday::Monday, jiff::civil::Weekday::Sunday]
        );

        assert_eq!(chrono::WeekdaySet::from(set), chrono_set);
    }

    #[test]
    fn chrono_weekday_set_to_db_value() {
        let mut set = chrono::WeekdaySet::EMPTY;
        set.insert(Weekday::Mon);
        set.insert(Weekday::Fri);

        assert_eq!(set.to_db_value(), DbValue::TinyUnsigned(Some(0b001_0001)));
        assert_eq!(
            None::<chrono::WeekdaySet>.to_db_value(),
            DbValue::TinyUnsigned(None)
        );
    }
}
