use bytes::Bytes;
use cot::auth::PasswordHash;
use cot::common_types::{Email, Password, Url};
use cot::db::migrations::{Field, Operation};
use cot::db::{Auto, Database, DatabaseField, Identifier, LimitedString, Model};
use cot::test::TestDatabase;
use cot_macros::{model, query};
use fake::rand::rngs::StdRng;
use fake::rand::{RngExt, SeedableRng};
use fake::{Dummy, Fake, Faker};

use crate::db_testing::run_migrations;

struct WeekdaySetFaker;

impl Dummy<WeekdaySetFaker> for chrono::WeekdaySet {
    fn dummy_with_rng<R: fake::rand::Rng + ?Sized>(_: &WeekdaySetFaker, rng: &mut R) -> Self {
        use chrono::Weekday;

        let mut set = chrono::WeekdaySet::EMPTY;
        let weekdays = [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ];

        for weekday in weekdays {
            if rng.random_bool(0.5) {
                set.insert(weekday);
            }
        }

        set
    }
}

struct EmailFaker;

impl Dummy<EmailFaker> for Email {
    fn dummy_with_rng<R: fake::rand::Rng + ?Sized>(_: &EmailFaker, rng: &mut R) -> Self {
        let username: String = (0..10)
            .map(|_| (0x61u8 + (rng.next_u32() % 26) as u8) as char)
            .collect();
        let domain: String = (0..10)
            .map(|_| (0x61u8 + (rng.next_u32() % 26) as u8) as char)
            .collect();
        Email::new(format!("{username}@{domain}.com")).expect("Generated email should be valid")
    }
}

struct UrlFaker;

impl Dummy<UrlFaker> for Url {
    fn dummy_with_rng<R: RngExt + ?Sized>(_config: &UrlFaker, rng: &mut R) -> Self {
        let domain: String = (0..10)
            .map(|_| (0x61u8 + (rng.next_u32() % 26) as u8) as char)
            .collect();
        Url::new(format!("https://{domain}.com")).expect("Generated URL should be valid")
    }
}

#[derive(Debug, PartialEq, Dummy)]
#[model]
struct AllFieldsModel {
    #[dummy(expr = "Auto::auto()")]
    #[model(primary_key)]
    id: Auto<i32>,
    field_bool: bool,
    field_i8: i8,
    field_i16: i16,
    field_i32: i32,
    field_i64: i64,
    field_u8: u8,
    field_u16: u16,
    field_u32: u32,
    // SQLite only allows us to store signed integers, so we're generating numbers that do not
    // exceed i64::MAX
    #[dummy(faker = "0..i64::MAX as u64")]
    field_u64: u64,
    field_f32: f32,
    field_f64: f64,
    field_date: chrono::NaiveDate,
    field_time: chrono::NaiveTime,
    #[dummy(faker = "fake::chrono::Precision::<6>")]
    field_datetime: chrono::NaiveDateTime,
    #[dummy(faker = "fake::chrono::Precision::<6>")]
    field_datetime_timezone: chrono::DateTime<chrono::FixedOffset>,
    field_string: String,
    field_blob: Vec<u8>,
    #[dummy(expr = "Bytes::from_static(b\"test bytes\")")]
    field_bytes: Bytes,
    field_option: Option<String>,
    field_limited_string: LimitedString<10>,
    field_option_limited_string: Option<LimitedString<10>>,
    #[dummy(faker = "WeekdaySetFaker")]
    field_weekday_set: chrono::WeekdaySet,
    #[dummy(faker = "EmailFaker")]
    field_email: Email,
    #[dummy(faker = "EmailFaker")]
    field_option_email: Option<Email>,
    #[dummy(faker = "UrlFaker")]
    field_url: Url,
    #[dummy(faker = "UrlFaker")]
    field_option_url: Option<Url>,
}

async fn migrate_all_fields_model(db: &Database) {
    CREATE_ALL_FIELDS_MODEL.forwards(db).await.unwrap();
}

macro_rules! all_fields_migration_field {
    ($name:ident, $ty:ty) => {
        Field::new(
            Identifier::new(concat!("field_", stringify!($name))),
            <$ty as DatabaseField>::TYPE,
        )
        .set_null(<$ty as DatabaseField>::NULLABLE)
    };
    ($ty:ty) => {
        Field::new(
            Identifier::new(concat!("field_", stringify!($ty))),
            <$ty as DatabaseField>::TYPE,
        )
        .set_null(<$ty as DatabaseField>::NULLABLE)
    };
}

const CREATE_ALL_FIELDS_MODEL: Operation = Operation::create_model()
    .table_name(Identifier::new("cot__all_fields_model"))
    .fields(&[
        Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
            .primary_key()
            .auto(),
        all_fields_migration_field!(bool),
        all_fields_migration_field!(i8),
        all_fields_migration_field!(i16),
        all_fields_migration_field!(i32),
        all_fields_migration_field!(i64),
        all_fields_migration_field!(u8),
        all_fields_migration_field!(u16),
        all_fields_migration_field!(u32),
        all_fields_migration_field!(u64),
        all_fields_migration_field!(f32),
        all_fields_migration_field!(f64),
        all_fields_migration_field!(date, chrono::NaiveDate),
        all_fields_migration_field!(time, chrono::NaiveTime),
        all_fields_migration_field!(datetime, chrono::NaiveDateTime),
        all_fields_migration_field!(datetime_timezone, chrono::DateTime<chrono::FixedOffset>),
        all_fields_migration_field!(string, String),
        all_fields_migration_field!(blob, Vec<u8>),
        all_fields_migration_field!(bytes, Bytes),
        all_fields_migration_field!(option, Option<String>),
        all_fields_migration_field!(limited_string, LimitedString<10>),
        all_fields_migration_field!(option_limited_string, Option<LimitedString<10>>),
        all_fields_migration_field!(weekday_set, chrono::WeekdaySet),
        all_fields_migration_field!(email, Email),
        all_fields_migration_field!(option_email, Option<Email>),
        all_fields_migration_field!(url, Url),
        all_fields_migration_field!(option_url, Option<Url>),
        all_fields_migration_field!(option_password_hash, Option<PasswordHash>),
    ])
    .build();

#[cot_macros::dbtest]
async fn all_fields_model(db: &mut TestDatabase) {
    migrate_all_fields_model(db).await;

    assert_eq!(AllFieldsModel::objects().all(&**db).await.unwrap(), vec![]);

    let r = &mut StdRng::seed_from_u64(123_785);
    let mut models = (0..100)
        .map(|_| Faker.fake_with_rng(r))
        .collect::<Vec<AllFieldsModel>>();
    for model in &mut models {
        model.save(&**db).await.unwrap();
    }

    let mut models_from_db: Vec<_> = AllFieldsModel::objects().all(&**db).await.unwrap();
    normalize_datetimes(&mut models);
    normalize_datetimes(&mut models_from_db);

    assert_eq!(models.len(), models_from_db.len());
    for model in &models {
        assert!(
            models_from_db.contains(model),
            "Could not find model {model:?} in models_from_db: {models_from_db:?}",
        );
    }
}

/// Normalize the datetimes to UTC.
fn normalize_datetimes(data: &mut Vec<AllFieldsModel>) {
    for model in data {
        model.field_datetime_timezone = model.field_datetime_timezone.with_timezone(
            &chrono::FixedOffset::east_opt(0).expect("UTC timezone is always valid"),
        );
    }
}

#[cot_macros::dbtest]
async fn password_hash_field(db: &TestDatabase) {
    #[derive(Debug, Clone)]
    #[model]
    struct PasswordHashModel {
        #[model(primary_key)]
        id: Auto<i32>,
        password: PasswordHash,
    }

    const CREATE_OPTIONAL_PASSWORD_HASH_MODEL: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__password_hash_model"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(
                Identifier::new("password"),
                <PasswordHash as DatabaseField>::TYPE,
            ),
        ])
        .build();

    run_migrations!(db, CREATE_OPTIONAL_PASSWORD_HASH_MODEL);

    let generated_password: String = Faker.fake();
    let mut password_model = PasswordHashModel {
        id: Auto::auto(),
        password: PasswordHash::from_password(&Password::new(&generated_password)),
    };
    password_model.save(&**db).await.unwrap();

    let models = PasswordHashModel::objects().all(&**db).await.unwrap();

    assert_eq!(models.len(), 1);
    assert_eq!(
        models[0].password.as_str(),
        password_model.password.as_str()
    );
}

#[cot_macros::dbtest]
async fn password_hash_option(db: &TestDatabase) {
    #[derive(Debug, Clone)]
    #[model]
    struct PasswordHashModel {
        #[model(primary_key)]
        id: Auto<i32>,
        password: Option<PasswordHash>,
    }

    const CREATE_OPTIONAL_PASSWORD_HASH_MODEL: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__password_hash_model"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(
                Identifier::new("password"),
                <Option<PasswordHash> as DatabaseField>::TYPE,
            )
            .set_null(<Option<PasswordHash> as DatabaseField>::NULLABLE),
        ])
        .build();

    run_migrations!(db, CREATE_OPTIONAL_PASSWORD_HASH_MODEL);

    let generated_password: String = Faker.fake();
    let mut with_password = PasswordHashModel {
        id: Auto::auto(),
        password: Some(PasswordHash::from_password(&Password::new(
            &generated_password,
        ))),
    };
    with_password.save(&**db).await.unwrap();

    let mut without_password = PasswordHashModel {
        id: Auto::auto(),
        password: None,
    };
    without_password.save(&**db).await.unwrap();

    let models = PasswordHashModel::objects().all(&**db).await.unwrap();

    assert_eq!(models.len(), 2);
    assert_eq!(
        models[0].password.as_ref().unwrap().as_str(),
        with_password.password.as_ref().unwrap().as_str()
    );
    assert!(models[1].password.is_none());
}

#[cot_macros::dbtest]
#[expect(clippy::too_many_lines)]
async fn weekday_set_field_functionality(db: &mut TestDatabase) {
    use chrono::Weekday;

    #[derive(Debug, PartialEq)]
    #[model]
    struct WeekdaySetModel {
        #[model(primary_key)]
        id: Auto<i32>,
        schedule: chrono::WeekdaySet,
        optional_schedule: Option<chrono::WeekdaySet>,
    }

    const CREATE_WEEKDAY_SET_MODEL: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__weekday_set_model"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(
                Identifier::new("schedule"),
                <chrono::WeekdaySet as DatabaseField>::TYPE,
            ),
            Field::new(
                Identifier::new("optional_schedule"),
                <Option<chrono::WeekdaySet> as DatabaseField>::TYPE,
            )
            .set_null(<Option<chrono::WeekdaySet> as DatabaseField>::NULLABLE),
        ])
        .build();

    run_migrations!(db, CREATE_WEEKDAY_SET_MODEL);

    // Test empty WeekdaySet
    let mut model1 = WeekdaySetModel {
        id: Auto::auto(),
        schedule: chrono::WeekdaySet::EMPTY,
        optional_schedule: None,
    };
    model1.save(&**db).await.unwrap();

    // Test WeekdaySet with all weekdays
    let mut all_days = chrono::WeekdaySet::EMPTY;
    for day in [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ] {
        all_days.insert(day);
    }
    let mut model2 = WeekdaySetModel {
        id: Auto::auto(),
        schedule: all_days,
        optional_schedule: Some(chrono::WeekdaySet::EMPTY),
    };
    model2.save(&**db).await.unwrap();

    // Test WeekdaySet with specific weekdays (weekdays only)
    let mut weekdays_only = chrono::WeekdaySet::EMPTY;
    for day in [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
    ] {
        weekdays_only.insert(day);
    }
    let mut model3 = WeekdaySetModel {
        id: Auto::auto(),
        schedule: weekdays_only,
        optional_schedule: Some(weekdays_only),
    };
    model3.save(&**db).await.unwrap();

    // Test WeekdaySet with weekend only
    let mut weekend_only = chrono::WeekdaySet::EMPTY;
    weekend_only.insert(Weekday::Sat);
    weekend_only.insert(Weekday::Sun);
    let mut model4 = WeekdaySetModel {
        id: Auto::auto(),
        schedule: weekend_only,
        optional_schedule: Some(all_days),
    };
    model4.save(&**db).await.unwrap();

    // Retrieve all models and verify they match
    let models_from_db = WeekdaySetModel::objects().all(&**db).await.unwrap();
    assert_eq!(models_from_db.len(), 4);

    // Find and verify each model
    let db_model1 = models_from_db.iter().find(|m| m.id == model1.id).unwrap();
    assert_eq!(db_model1.schedule, chrono::WeekdaySet::EMPTY);
    assert_eq!(db_model1.optional_schedule, None);

    let db_model2 = models_from_db.iter().find(|m| m.id == model2.id).unwrap();
    assert_eq!(db_model2.schedule, all_days);
    assert_eq!(db_model2.optional_schedule, Some(chrono::WeekdaySet::EMPTY));

    let db_model3 = models_from_db.iter().find(|m| m.id == model3.id).unwrap();
    assert_eq!(db_model3.schedule, weekdays_only);
    assert_eq!(db_model3.optional_schedule, Some(weekdays_only));

    let db_model4 = models_from_db.iter().find(|m| m.id == model4.id).unwrap();
    assert_eq!(db_model4.schedule, weekend_only);
    assert_eq!(db_model4.optional_schedule, Some(all_days));

    // Test querying by WeekdaySet
    let weekend_models = query!(WeekdaySetModel, $schedule == weekend_only)
        .all(&**db)
        .await
        .unwrap();
    assert_eq!(weekend_models.len(), 1);
    assert_eq!(weekend_models[0].id, model4.id);

    // Test updating WeekdaySet
    let mut model_to_update = models_from_db
        .into_iter()
        .find(|m| m.id == model1.id)
        .unwrap();
    model_to_update.schedule = weekdays_only;
    model_to_update.optional_schedule = Some(weekend_only);
    model_to_update.save(&**db).await.unwrap();

    let updated_model = WeekdaySetModel::get_by_primary_key(&**db, model_to_update.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_model.schedule, weekdays_only);
    assert_eq!(updated_model.optional_schedule, Some(weekend_only));
}
