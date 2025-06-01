use chrono::Weekday;

use crate::db::impl_mysql::MySqlValueRef;
use crate::db::impl_postgres::PostgresValueRef;
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

    pub(crate) const fn contains(&self, day: Weekday) -> bool {
        self.0 & Self::single(day).0 != 0
    }

    pub(crate) fn insert(&mut self, day: Weekday) -> bool {
        if self.contains(day) {
            return false;
        }
        self.0 |= Self::single(day).0;
        true
    }

    pub(crate) fn weekdays(&self) -> Vec<Weekday> {
        let mut weekdays = Vec::new();
        for i in 0..=6 {
            if self.0 & (1 << i) != 0 {
                weekdays.push(
                    Weekday::try_from(i).expect("weekday can always be created from numbers 0-6"),
                );
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
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<u8>().map(|v| WeekdaySet::from(v)).into()
    }

    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<i16>().map(|v| WeekdaySet::from(v as u8)).into()
    }

    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value.get::<u8>().map(|v| WeekdaySet::from(v)).into()
    }
}

impl FromDbValue for Option<WeekdaySet> {
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<u8>>()
            .map(|v| v.map(|v| WeekdaySet::from(v)))
            .into()
    }

    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<i16>>()
            .map(|v| v.map(|v| WeekdaySet::from(v as u8)))
            .into()
    }

    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<u8>>()
            .map(|v| v.map(|v| WeekdaySet::from(v)))
            .into()
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
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<u8>()
            .and_then(|v| Weekday::try_from(v).map_err(|e| DatabaseError::ValueDecode(e.into())))
            .into()
    }

    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<i16>()
            .and_then(|v| {
                Weekday::try_from(v as u8).map_err(|e| DatabaseError::ValueDecode(e.into()))
            })
            .into()
    }

    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<u8>()
            .and_then(|v| Weekday::try_from(v).map_err(|e| DatabaseError::ValueDecode(e.into())))
            .into()
    }
}
impl ToDbValue for Weekday {
    fn to_db_value(&self) -> DbValue {
        self.num_days_from_monday().into()
    }
}
