//! Tests pinning the on-disk representation of date and time values.
//!
//! SQLite has no native date/time types, so the values are stored as text.
//! Since SQLite compares text values lexicographically, the exact format
//! matters for filtering and ordering: switching to a different format would
//! make the existing data incomparable with newly written data.

use cot::db::migrations::{Field, Operation};
use cot::db::{Auto, DatabaseField, Identifier, Model};
use cot::test::TestDatabase;
use cot_macros::model;

use crate::db_testing::run_migrations;

const TABLE_NAME: &str = "cot__datetime_storage_model";

#[derive(Debug, Clone, PartialEq)]
#[model(table_name = "datetime_storage_model")]
struct ChronoModel {
    #[model(primary_key)]
    id: Auto<i32>,
    date: chrono::NaiveDate,
    time: chrono::NaiveTime,
    datetime: chrono::NaiveDateTime,
    datetime_fixed: chrono::DateTime<chrono::FixedOffset>,
    datetime_utc: chrono::DateTime<chrono::Utc>,
}

/// The same table as [`ChronoModel`], but with all the columns read as text.
#[derive(Debug, Clone, PartialEq)]
#[model(table_name = "datetime_storage_model")]
struct TextModel {
    #[model(primary_key)]
    id: Auto<i32>,
    date: String,
    time: String,
    datetime: String,
    datetime_fixed: String,
    datetime_utc: String,
}

const CREATE_MODEL: Operation = Operation::create_model()
    .table_name(Identifier::new(TABLE_NAME))
    .fields(&[
        Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
            .primary_key()
            .auto(),
        Field::new(
            Identifier::new("date"),
            <chrono::NaiveDate as DatabaseField>::TYPE,
        ),
        Field::new(
            Identifier::new("time"),
            <chrono::NaiveTime as DatabaseField>::TYPE,
        ),
        Field::new(
            Identifier::new("datetime"),
            <chrono::NaiveDateTime as DatabaseField>::TYPE,
        ),
        Field::new(
            Identifier::new("datetime_fixed"),
            <chrono::DateTime<chrono::FixedOffset> as DatabaseField>::TYPE,
        ),
        Field::new(
            Identifier::new("datetime_utc"),
            <chrono::DateTime<chrono::Utc> as DatabaseField>::TYPE,
        ),
    ])
    .build();

fn chrono_model(nanos: u32, offset_secs: i32) -> ChronoModel {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    let time = chrono::NaiveTime::from_hms_nano_opt(12, 34, 56, nanos).unwrap();
    let datetime = date.and_time(time);
    let offset = chrono::FixedOffset::east_opt(offset_secs).unwrap();

    ChronoModel {
        id: Auto::auto(),
        date,
        time,
        datetime,
        datetime_fixed: datetime.and_local_timezone(offset).unwrap(),
        datetime_utc: datetime.and_utc(),
    }
}

fn text_model(
    date: &str,
    time: &str,
    datetime: &str,
    datetime_fixed: &str,
    datetime_utc: &str,
) -> TextModel {
    TextModel {
        id: Auto::auto(),
        date: date.to_owned(),
        time: time.to_owned(),
        datetime: datetime.to_owned(),
        datetime_fixed: datetime_fixed.to_owned(),
        datetime_utc: datetime_utc.to_owned(),
    }
}

fn expected_text_models() -> Vec<TextModel> {
    vec![
        text_model(
            "2026-10-04",
            "12:34:56",
            "2026-10-04 12:34:56",
            "2026-10-04T12:34:56+00:00",
            "2026-10-04T12:34:56+00:00",
        ),
        text_model(
            "2026-10-04",
            "12:34:56.789",
            "2026-10-04 12:34:56.789",
            "2026-10-04T12:34:56.789+01:00",
            "2026-10-04T12:34:56.789+00:00",
        ),
        text_model(
            "2026-10-04",
            "12:34:56.123456",
            "2026-10-04 12:34:56.123456",
            "2026-10-04T12:34:56.123456-02:30",
            "2026-10-04T12:34:56.123456+00:00",
        ),
        text_model(
            "2026-10-04",
            "12:34:56.123456789",
            "2026-10-04 12:34:56.123456789",
            "2026-10-04T12:34:56.123456789+05:45",
            "2026-10-04T12:34:56.123456789+00:00",
        ),
    ]
}

fn chrono_models() -> Vec<ChronoModel> {
    vec![
        chrono_model(0, 0),
        chrono_model(789_000_000, 3600),
        chrono_model(123_456_000, -9000),
        chrono_model(123_456_789, 20_700),
    ]
}

fn without_ids<T: Clone>(models: &[T], clear_id: impl Fn(&mut T)) -> Vec<T> {
    models
        .iter()
        .cloned()
        .map(|mut model| {
            clear_id(&mut model);
            model
        })
        .collect()
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn sqlite_chrono_text_format() {
    let db = &mut TestDatabase::new_sqlite().await.unwrap();
    run_migrations!(db, CREATE_MODEL);

    for mut model in chrono_models() {
        model.save(&**db).await.unwrap();
    }

    let text_models = TextModel::objects()
        .order_by([<TextModel as Model>::Fields::id])
        .all(&**db)
        .await
        .unwrap();
    assert_eq!(
        without_ids(&text_models, |model| model.id = Auto::auto()),
        expected_text_models()
    );

    db.cleanup().await.unwrap();
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn sqlite_chrono_reads_text_format() {
    let db = &mut TestDatabase::new_sqlite().await.unwrap();
    run_migrations!(db, CREATE_MODEL);

    for mut model in expected_text_models() {
        model.save(&**db).await.unwrap();
    }

    let chrono_models_from_db = ChronoModel::objects()
        .order_by([<ChronoModel as Model>::Fields::id])
        .all(&**db)
        .await
        .unwrap();
    let expected = chrono_models();
    assert_eq!(
        without_ids(&chrono_models_from_db, |model| model.id = Auto::auto()),
        expected
    );
    for (actual, expected) in chrono_models_from_db.iter().zip(&expected) {
        assert_eq!(
            actual.datetime_fixed.offset(),
            expected.datetime_fixed.offset()
        );
    }

    db.cleanup().await.unwrap();
}
