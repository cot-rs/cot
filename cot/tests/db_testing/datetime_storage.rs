//! Tests pinning the on-disk representation of date and time values.
//!
//! SQLite has no native date/time types, so the values are stored as text.
//! Since SQLite compares text values lexicographically, the exact format
//! matters for filtering and ordering: switching to a different format would
//! make the existing data incomparable with newly written data.

use cot::db::migrations::{Field, Operation};
use cot::db::query::expr::ExprSort;
use cot::db::{Auto, DatabaseField, Identifier, Model};
use cot::test::TestDatabase;
use cot_macros::{model, query};

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

/// The same table as [`ChronoModel`], but with `jiff` types.
#[derive(Debug, Clone, PartialEq)]
#[model(table_name = "datetime_storage_model")]
struct JiffModel {
    #[model(primary_key)]
    id: Auto<i32>,
    date: jiff::civil::Date,
    time: jiff::civil::Time,
    datetime: jiff::civil::DateTime,
    datetime_fixed: jiff::Timestamp,
    datetime_utc: jiff::Timestamp,
}

impl From<ChronoModel> for JiffModel {
    fn from(model: ChronoModel) -> Self {
        let timestamp = |nanos: i64| jiff::Timestamp::from_nanosecond(i128::from(nanos)).unwrap();

        Self {
            id: model.id,
            date: model.date.to_string().parse().unwrap(),
            time: model.time.to_string().parse().unwrap(),
            datetime: model.datetime.to_string().parse().unwrap(),
            datetime_fixed: timestamp(model.datetime_fixed.timestamp_nanos_opt().unwrap()),
            datetime_utc: timestamp(model.datetime_utc.timestamp_nanos_opt().unwrap()),
        }
    }
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn sqlite_jiff_text_format() {
    let db = &mut TestDatabase::new_sqlite().await.unwrap();
    run_migrations!(db, CREATE_MODEL);

    for model in chrono_models() {
        JiffModel::from(model).save(&**db).await.unwrap();
    }

    // `jiff::Timestamp` doesn't store the offset, so the values are always
    // written in UTC; other than that, the format is the same as for `chrono`.
    let mut expected = expected_text_models();
    expected[1].datetime_fixed = "2023-01-02T11:34:56.789+00:00".to_owned();
    expected[2].datetime_fixed = "2023-01-02T15:04:56.123456+00:00".to_owned();
    expected[3].datetime_fixed = "2023-01-02T06:49:56.123456789+00:00".to_owned();

    let text_models = TextModel::objects().all(&**db).await.unwrap();
    assert_eq!(
        without_ids(&text_models, |model| model.id = Auto::auto()),
        expected
    );

    db.cleanup().await.unwrap();
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn sqlite_jiff_reads_text_format() {
    let db = &mut TestDatabase::new_sqlite().await.unwrap();
    run_migrations!(db, CREATE_MODEL);

    for mut model in expected_text_models() {
        model.save(&**db).await.unwrap();
    }

    let jiff_models = JiffModel::objects().all(&**db).await.unwrap();
    assert_eq!(
        without_ids(&jiff_models, |model| model.id = Auto::auto()),
        chrono_models()
            .into_iter()
            .map(JiffModel::from)
            .collect::<Vec<_>>()
    );

    db.cleanup().await.unwrap();
}

/// The models that can be stored in all the databases without losing
/// precision (PostgreSQL and MySQL only support microsecond precision, and
/// MySQL doesn't store fractional seconds in `TIME` columns).
fn portable_chrono_models() -> Vec<ChronoModel> {
    use chrono::Timelike;

    [
        chrono_model(0, 0),
        chrono_model(789_000_000, 3600),
        chrono_model(123_456_000, -9000),
    ]
    .into_iter()
    .map(|mut model| {
        model.time = model.time.with_nanosecond(0).unwrap();
        model
    })
    .collect()
}

#[cot_macros::dbtest]
async fn jiff_reads_values_written_by_chrono(db: &mut TestDatabase) {
    run_migrations!(db, CREATE_MODEL);

    for mut model in portable_chrono_models() {
        model.save(&**db).await.unwrap();
    }

    let mut jiff_models = JiffModel::objects().all(&**db).await.unwrap();
    jiff_models.sort_by_key(|model| model.id.unwrap());
    assert_eq!(
        without_ids(&jiff_models, |model| model.id = Auto::auto()),
        portable_chrono_models()
            .into_iter()
            .map(JiffModel::from)
            .collect::<Vec<_>>()
    );
}

#[cot_macros::dbtest]
async fn chrono_reads_values_written_by_jiff(db: &mut TestDatabase) {
    run_migrations!(db, CREATE_MODEL);

    for model in portable_chrono_models() {
        JiffModel::from(model).save(&**db).await.unwrap();
    }

    let mut chrono_models = ChronoModel::objects().all(&**db).await.unwrap();
    chrono_models.sort_by_key(|model| model.id.unwrap());
    let chrono_models: Vec<_> = without_ids(&chrono_models, |model| model.id = Auto::auto())
        .into_iter()
        .map(JiffModel::from)
        .collect();
    assert_eq!(
        chrono_models,
        portable_chrono_models()
            .into_iter()
            .map(JiffModel::from)
            .collect::<Vec<_>>()
    );
}

#[cot_macros::dbtest]
async fn jiff_filter_and_order(db: &mut TestDatabase) {
    run_migrations!(db, CREATE_MODEL);

    // mix the values written by both libraries to make sure they can be
    // compared
    let models = portable_chrono_models();
    let mut first = models[0].clone();
    first.save(&**db).await.unwrap();
    JiffModel::from(models[1].clone())
        .save(&**db)
        .await
        .unwrap();
    let mut third = models[2].clone();
    third.save(&**db).await.unwrap();

    let threshold: jiff::Timestamp = "2023-01-02T12:34:56.5Z".parse().unwrap();
    let later = query!(JiffModel, $datetime_utc > threshold)
        .order_by([<JiffModel as Model>::Fields::datetime_utc.asc()])
        .all(&**db)
        .await
        .unwrap();
    assert_eq!(
        later
            .iter()
            .map(|model| model.datetime_utc)
            .collect::<Vec<_>>(),
        [JiffModel::from(models[1].clone()).datetime_utc,]
    );

    let datetime_threshold = jiff::civil::date(2023, 1, 2).at(12, 34, 56, 500_000_000);
    let earlier = query!(JiffModel, $datetime < datetime_threshold)
        .order_by([<JiffModel as Model>::Fields::datetime.asc()])
        .all(&**db)
        .await
        .unwrap();
    assert_eq!(
        earlier
            .iter()
            .map(|model| model.datetime)
            .collect::<Vec<_>>(),
        [
            jiff::civil::date(2023, 1, 2).at(12, 34, 56, 0),
            jiff::civil::date(2023, 1, 2).at(12, 34, 56, 123_456_000),
        ]
    );
}
