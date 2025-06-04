use chrono::Weekday;

#[cfg(feature = "mysql")]
use crate::db::impl_mysql::MySqlValueRef;
#[cfg(feature = "postgres")]
use crate::db::impl_postgres::PostgresValueRef;
#[cfg(feature = "sqlite")]
use crate::db::impl_sqlite::SqliteValueRef;
use crate::db::{DatabaseError, DbValue, FromDbValue, SqlxValueRef, ToDbValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeekdaySet(u8);

impl WeekdaySet {
    pub const EMPTY: WeekdaySet = WeekdaySet(0);
    pub(crate) const fn single(weekday: Weekday) -> Self {
        match weekday {
            Weekday::Mon => Self(0b000_0001),
            Weekday::Tue => Self(0b000_0010),
            Weekday::Wed => Self(0b000_0100),
            Weekday::Thu => Self(0b000_1000),
            Weekday::Fri => Self(0b001_0000),
            Weekday::Sat => Self(0b010_0000),
            Weekday::Sun => Self(0b100_0000),
        }
    }

    pub(crate) const fn contains(self, day: Weekday) -> bool {
        self.0 & Self::single(day).0 != 0
    }

    pub(crate) fn insert(&mut self, day: Weekday) -> bool {
        if self.contains(day) {
            return false;
        }
        self.0 |= Self::single(day).0;
        true
    }

    pub(crate) fn weekdays(self) -> Vec<Weekday> {
        let mut weekdays = Vec::new();
        for weekday in [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ] {
            if self.contains(weekday) {
                weekdays.push(weekday);
            }
        }
        weekdays
    }
}

impl From<chrono::WeekdaySet> for WeekdaySet {
    fn from(set: chrono::WeekdaySet) -> Self {
        let mut new_set = WeekdaySet::EMPTY;

        for weekday in set.iter(Weekday::Mon) {
            new_set.insert(weekday);
        }

        new_set
    }
}

impl From<WeekdaySet> for chrono::WeekdaySet {
    fn from(set: WeekdaySet) -> Self {
        let mut new_set = chrono::WeekdaySet::EMPTY;

        for weekday in set.weekdays() {
            new_set.insert(weekday);
        }

        new_set
    }
}
impl From<WeekdaySet> for u8 {
    fn from(set: WeekdaySet) -> Self {
        set.0
    }
}

impl From<u8> for WeekdaySet {
    fn from(value: u8) -> Self {
        WeekdaySet(value)
    }
}

impl FromDbValue for WeekdaySet {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<u8>().map(WeekdaySet::from)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        value.get::<i16>().map(|v| WeekdaySet::from(v as u8))
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<u8>().map(WeekdaySet::from)
    }
}

impl FromDbValue for Option<WeekdaySet> {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<Option<u8>>().map(|v| v.map(WeekdaySet::from))
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        value
            .get::<Option<i16>>()
            .map(|v| v.map(|v| WeekdaySet::from(v as u8)))
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<Option<u8>>().map(|v| v.map(WeekdaySet::from))
    }
}

impl ToDbValue for WeekdaySet {
    fn to_db_value(&self) -> DbValue {
        self.0.to_db_value()
    }
}

impl ToDbValue for Option<WeekdaySet> {
    fn to_db_value(&self) -> DbValue {
        self.map(|val| val.0).to_db_value()
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
    use crate::db::DbValue;

    #[test]
    fn weekday_set_empty() {
        let set = WeekdaySet::EMPTY;
        assert_eq!(set.0, 0);
        assert!(set.weekdays().is_empty());
    }

    #[test]
    fn weekday_set_single() {
        let monday = WeekdaySet::single(Weekday::Mon);
        assert_eq!(monday.0, 0b000_0001);
        assert!(monday.contains(Weekday::Mon));
        assert!(!monday.contains(Weekday::Tue));

        let friday = WeekdaySet::single(Weekday::Fri);
        assert_eq!(friday.0, 0b001_0000);
        assert!(friday.contains(Weekday::Fri));
        assert!(!friday.contains(Weekday::Mon));
    }

    #[test]
    fn weekday_set_insert() {
        let mut set = WeekdaySet::EMPTY;

        assert!(set.insert(Weekday::Mon));
        assert!(set.contains(Weekday::Mon));
        assert!(!set.insert(Weekday::Mon));

        assert!(set.insert(Weekday::Fri));
        assert!(set.contains(Weekday::Fri));
        assert!(set.contains(Weekday::Mon));
    }

    #[test]
    fn weekday_set_weekdays() {
        let mut set = WeekdaySet::EMPTY;
        set.insert(Weekday::Mon);
        set.insert(Weekday::Wed);
        set.insert(Weekday::Fri);

        let weekdays = set.weekdays();
        assert_eq!(weekdays, vec![Weekday::Mon, Weekday::Wed, Weekday::Fri]);
    }

    #[test]
    fn weekday_set_chrono_conversion() {
        let mut chrono_set = chrono::WeekdaySet::EMPTY;
        chrono_set.insert(Weekday::Mon);
        chrono_set.insert(Weekday::Wed);

        // Test conversion from chrono to our type
        let our_set = WeekdaySet::from(chrono_set);
        assert!(our_set.contains(Weekday::Mon));
        assert!(our_set.contains(Weekday::Wed));
        assert!(!our_set.contains(Weekday::Tue));

        // Test conversion back to chrono
        let back_to_chrono = chrono::WeekdaySet::from(our_set);
        assert!(back_to_chrono.contains(Weekday::Mon));
        assert!(back_to_chrono.contains(Weekday::Wed));
        assert!(!back_to_chrono.contains(Weekday::Tue));

        assert_eq!(chrono_set, back_to_chrono);
    }

    #[test]
    fn weekday_set_from_u8() {
        let set = WeekdaySet::from(0b101_0101);
        assert!(set.contains(Weekday::Mon));
        assert!(!set.contains(Weekday::Tue));
        assert!(set.contains(Weekday::Wed));
        assert!(!set.contains(Weekday::Thu));
        assert!(set.contains(Weekday::Fri));
        assert!(!set.contains(Weekday::Sat));
        assert!(set.contains(Weekday::Sun));
    }

    #[test]
    fn weekday_set_to_u8() {
        let mut set = WeekdaySet::EMPTY;
        set.insert(Weekday::Mon);
        set.insert(Weekday::Wed);
        set.insert(Weekday::Fri);
        set.insert(Weekday::Sun);

        let value: u8 = set.into();
        assert_eq!(value, 0b101_0101);
    }

    #[test]
    fn weekday_set_all_weekdays() {
        let all_weekdays = 0b111_1111u8;
        let set = WeekdaySet::from(all_weekdays);

        for weekday in [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ] {
            assert!(set.contains(weekday));
        }

        assert_eq!(set.weekdays().len(), 7);
    }

    #[test]
    fn weekday_set_to_db_value() {
        let mut set = WeekdaySet::EMPTY;
        set.insert(Weekday::Mon);
        set.insert(Weekday::Fri);

        let db_value = set.to_db_value();
        assert_eq!(db_value, DbValue::TinyUnsigned(Some(0b001_0001)));
    }

    #[test]
    fn weekday_set_option_to_db_value() {
        let mut set = WeekdaySet::EMPTY;
        set.insert(Weekday::Tue);
        set.insert(Weekday::Thu);

        let some_set = Some(set);
        let db_value = some_set.to_db_value();
        assert_eq!(db_value, DbValue::TinyUnsigned(Some(0b000_1010)));

        let none_set: Option<WeekdaySet> = None;
        let db_value = none_set.to_db_value();
        assert_eq!(db_value, DbValue::TinyUnsigned(None));
    }
}
