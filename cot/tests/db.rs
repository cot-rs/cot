#![cfg(feature = "fake")]
#![cfg_attr(miri, ignore)]

use bytes::Bytes;
use cot::auth::PasswordHash;
use cot::common_types::{Email, Password, Url};
use cot::db::migrations::{Field, Operation};
use cot::db::query::expr::ExprEq;
use cot::db::{
    Auto, Database, DatabaseError, DatabaseField, ForeignKey, ForeignKeyOnDeletePolicy,
    ForeignKeyOnUpdatePolicy, Identifier, LimitedString, Model, model, query,
};
use cot::test::TestDatabase;
use fake::rand::rngs::StdRng;
use fake::rand::{RngExt, SeedableRng};
use fake::{Dummy, Fake, Faker};

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

#[derive(Debug, PartialEq)]
#[model]
struct TestModel {
    #[model(primary_key)]
    id: Auto<i32>,
    name: String,
}

#[cot_macros::dbtest]
async fn model_crud(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    assert_eq!(TestModel::objects().all(&**test_db).await.unwrap(), vec![]);

    // Create
    let mut model = TestModel {
        id: Auto::fixed(1),
        name: "test".to_owned(),
    };
    model.save(&**test_db).await.unwrap();

    // Read
    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "test");

    // Update (& read again)
    model.name = "test2".to_owned();
    model.save(&**test_db).await.unwrap();
    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "test2");

    // Delete
    TestModel::objects()
        .filter(<TestModel as Model>::Fields::id.eq(1))
        .delete(&**test_db)
        .await
        .unwrap();

    assert_eq!(TestModel::objects().all(&**test_db).await.unwrap(), vec![]);
}

#[cot_macros::dbtest]
async fn model_insert(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    // Insert
    let mut model = TestModel {
        id: Auto::fixed(1),
        name: "test".to_owned(),
    };
    let result = model.insert(&**test_db).await;
    assert!(result.is_ok());

    // Can't insert the same model instance again
    let result = model.insert(&**test_db).await;
    assert!(result.is_err());

    // Read the model from the database
    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "test");
}

#[cot_macros::dbtest]
async fn model_update(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    // Insert
    let mut model = TestModel {
        id: Auto::fixed(1),
        name: "test".to_owned(),
    };
    let result = model.insert(&**test_db).await;
    assert!(result.is_ok());

    // Update
    model.name = "test2".to_owned();
    let result = model.update(&**test_db).await;
    assert!(result.is_ok());

    // Can't update non-existing object
    let mut model = TestModel {
        id: Auto::fixed(2),
        name: "test3".to_owned(),
    };
    let result = model.update(&**test_db).await;
    assert!(result.is_err());

    // Read the model from the database
    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "test2");
}

#[cot_macros::dbtest]
async fn model_macro_filtering(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    assert_eq!(TestModel::objects().all(&**test_db).await.unwrap(), vec![]);

    let mut model = TestModel {
        id: Auto::auto(),
        name: "test".to_owned(),
    };
    model.save(&**test_db).await.unwrap();
    let objects = query!(TestModel, $name == "test")
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "test");

    let objects = query!(TestModel, $name == "t")
        .all(&**test_db)
        .await
        .unwrap();
    assert!(objects.is_empty());
}

async fn migrate_test_model(db: &Database) {
    CREATE_TEST_MODEL.forwards(db).await.unwrap();
}

const CREATE_TEST_MODEL: Operation = Operation::create_model()
    .table_name(Identifier::new("cot__test_model"))
    .fields(&[
        Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
            .primary_key()
            .auto(),
        Field::new(Identifier::new("name"), <String as DatabaseField>::TYPE),
    ])
    .build();

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

macro_rules! run_migrations {
    ( $db:ident, $( $operations:ident ),* ) => {
        struct TestMigration;

        impl cot::db::migrations::Migration for TestMigration {
            const APP_NAME: &'static str = "cot";
            const DEPENDENCIES: &'static [cot::db::migrations::MigrationDependency] = &[];
            const MIGRATION_NAME: &'static str = "test_migration";
            const OPERATIONS: &'static [Operation] = &[ $($operations),* ];
        }

        cot::db::migrations::MigrationEngine::new(
            cot::db::migrations::wrap_migrations(&[&TestMigration])
        )
            .unwrap()
            .run(&**$db)
            .await
            .unwrap();
    };
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
async fn foreign_keys(db: &mut TestDatabase) {
    #[derive(Debug, Clone, PartialEq)]
    #[model]
    struct Artist {
        #[model(primary_key)]
        id: Auto<i32>,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[model]
    struct Track {
        #[model(primary_key)]
        id: Auto<i32>,
        artist: ForeignKey<Artist>,
        name: String,
    }

    const CREATE_ARTIST: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__artist"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(Identifier::new("name"), <String as DatabaseField>::TYPE),
        ])
        .build();
    const CREATE_TRACK: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__track"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(
                Identifier::new("artist"),
                <ForeignKey<Artist> as DatabaseField>::TYPE,
            )
            .foreign_key(
                <Artist as Model>::TABLE_NAME,
                <Artist as Model>::PRIMARY_KEY_NAME,
                ForeignKeyOnDeletePolicy::Restrict,
                ForeignKeyOnUpdatePolicy::Restrict,
            ),
            Field::new(Identifier::new("name"), <String as DatabaseField>::TYPE),
        ])
        .build();

    run_migrations!(db, CREATE_ARTIST, CREATE_TRACK);

    let mut artist = Artist {
        id: Auto::auto(),
        name: "artist".to_owned(),
    };
    artist.save(&**db).await.unwrap();

    let mut track = Track {
        id: Auto::auto(),
        artist: ForeignKey::from(&artist),
        name: "track".to_owned(),
    };
    track.save(&**db).await.unwrap();

    let mut track = Track::objects().all(&**db).await.unwrap()[0].clone();
    let artist_from_db = track.artist.get(&**db).await.unwrap();
    assert_eq!(artist_from_db, &artist);

    let error = query!(Artist, $id == artist.id)
        .delete(&**db)
        .await
        .unwrap_err();
    // expected foreign key violation
    assert!(matches!(error, DatabaseError::DatabaseEngineError(_)));

    query!(Track, $artist == &artist)
        .delete(&**db)
        .await
        .unwrap();
    query!(Artist, $id == artist.id)
        .delete(&**db)
        .await
        .unwrap();
    // no error should be thrown
}

#[cot_macros::dbtest]
async fn foreign_keys_option(db: &mut TestDatabase) {
    #[derive(Debug, Clone, PartialEq)]
    #[model]
    struct Parent {
        #[model(primary_key)]
        id: Auto<i32>,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[model]
    struct Child {
        #[model(primary_key)]
        id: Auto<i32>,
        parent: Option<ForeignKey<Parent>>,
    }

    const CREATE_PARENT: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__parent"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build();
    const CREATE_CHILD: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__child"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(
                Identifier::new("parent"),
                <Option<ForeignKey<Parent>> as DatabaseField>::TYPE,
            )
            .set_null(<Option<ForeignKey<Parent>> as DatabaseField>::NULLABLE)
            .foreign_key(
                <Parent as Model>::TABLE_NAME,
                <Parent as Model>::PRIMARY_KEY_NAME,
                ForeignKeyOnDeletePolicy::SetNone,
                ForeignKeyOnUpdatePolicy::SetNone,
            ),
        ])
        .build();

    run_migrations!(db, CREATE_PARENT, CREATE_CHILD);

    // Test child with `None` parent
    let mut child = Child {
        id: Auto::auto(),
        parent: None,
    };
    child.save(&**db).await.unwrap();

    let child = Child::objects().all(&**db).await.unwrap()[0].clone();
    assert_eq!(child.parent, None);

    query!(Child, $id == child.id).delete(&**db).await.unwrap();

    // Test child with `Some` parent
    let mut parent = Parent { id: Auto::auto() };
    parent.save(&**db).await.unwrap();

    let mut child = Child {
        id: Auto::auto(),
        parent: Some(ForeignKey::from(&parent)),
    };
    child.save(&**db).await.unwrap();

    let child = Child::objects().all(&**db).await.unwrap()[0].clone();
    let mut parent_fk = child.parent.unwrap();
    let parent_from_db = parent_fk.get(&**db).await.unwrap();
    assert_eq!(parent_from_db, &parent);

    // Check none policy
    query!(Parent, $id == parent.id)
        .delete(&**db)
        .await
        .unwrap();
    let child = Child::objects().all(&**db).await.unwrap()[0].clone();
    assert_eq!(child.parent, None);
}

#[cot_macros::dbtest]
async fn foreign_keys_cascade(db: &mut TestDatabase) {
    #[derive(Debug, Clone, PartialEq)]
    #[model]
    struct Parent {
        #[model(primary_key)]
        id: Auto<i32>,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[model]
    struct Child {
        #[model(primary_key)]
        id: Auto<i32>,
        parent: Option<ForeignKey<Parent>>,
    }

    const CREATE_PARENT: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__parent"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build();
    const CREATE_CHILD: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__child"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
            Field::new(
                Identifier::new("parent"),
                <Option<ForeignKey<Parent>> as DatabaseField>::TYPE,
            )
            .set_null(<Option<ForeignKey<Parent>> as DatabaseField>::NULLABLE)
            .foreign_key(
                <Parent as Model>::TABLE_NAME,
                <Parent as Model>::PRIMARY_KEY_NAME,
                ForeignKeyOnDeletePolicy::Cascade,
                ForeignKeyOnUpdatePolicy::Cascade,
            ),
        ])
        .build();

    run_migrations!(db, CREATE_PARENT, CREATE_CHILD);

    // with parent
    let mut parent = Parent { id: Auto::auto() };
    parent.save(&**db).await.unwrap();

    let mut child = Child {
        id: Auto::auto(),
        parent: Some(ForeignKey::from(&parent)),
    };
    child.save(&**db).await.unwrap();

    let child = Child::objects().all(&**db).await.unwrap()[0].clone();
    let mut parent_fk = child.parent.unwrap();
    let parent_from_db = parent_fk.get(&**db).await.unwrap();
    assert_eq!(parent_from_db, &parent);

    // Check cascade policy
    query!(Parent, $id == parent.id)
        .delete(&**db)
        .await
        .unwrap();
    assert!(Child::objects().all(&**db).await.unwrap().is_empty());
}

// Check different types for the primary key
#[derive(Debug, PartialEq)]
#[model]
struct TestModelu32Key {
    #[model(primary_key)]
    id: Auto<u32>,
    name: String,
}

#[derive(Debug, PartialEq)]
#[model]
struct TestModelu64Key {
    #[model(primary_key)]
    id: Auto<u64>,
    name: String,
}

#[derive(Debug, PartialEq)]
#[model]
struct TestModeli64Key {
    #[model(primary_key)]
    id: Auto<i64>,
    name: String,
}

#[derive(Debug, PartialEq)]
#[model]
struct TestModelStringKey {
    #[model(primary_key)]
    id: String,
    name: String,
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

#[cot_macros::dbtest]
async fn bulk_insert_basic(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let mut models = vec![
        TestModel {
            id: Auto::auto(),
            name: "test1".to_owned(),
        },
        TestModel {
            id: Auto::auto(),
            name: "test2".to_owned(),
        },
        TestModel {
            id: Auto::auto(),
            name: "test3".to_owned(),
        },
    ];

    TestModel::bulk_insert(&**test_db, &mut models)
        .await
        .unwrap();

    assert!(matches!(models[0].id, Auto::Fixed(_)));
    assert!(matches!(models[1].id, Auto::Fixed(_)));
    assert!(matches!(models[2].id, Auto::Fixed(_)));

    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 3);

    let names: Vec<_> = objects.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"test1"));
    assert!(names.contains(&"test2"));
    assert!(names.contains(&"test3"));

    // Verify IDs match between models and database
    for model in &models {
        if let Auto::Fixed(_) = model.id {
            let db_model = TestModel::get_by_primary_key(&**test_db, model.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(db_model.name, model.name);
        }
    }
}

#[cot_macros::dbtest]
async fn bulk_insert_or_update(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let mut models = vec![
        TestModel {
            id: Auto::auto(),
            name: "test1".to_owned(),
        },
        TestModel {
            id: Auto::auto(),
            name: "test2".to_owned(),
        },
        TestModel {
            id: Auto::auto(),
            name: "test3".to_owned(),
        },
    ];
    TestModel::bulk_insert(&**test_db, &mut models)
        .await
        .unwrap();

    let mut models = vec![
        TestModel {
            id: models[0].id,
            name: "test1_updated".to_owned(),
        },
        TestModel {
            id: models[2].id,
            name: "test3_updated".to_owned(),
        },
    ];
    TestModel::bulk_insert_or_update(&**test_db, &mut models)
        .await
        .unwrap();

    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 3);

    let names: Vec<_> = objects.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"test1_updated"));
    assert!(names.contains(&"test2"));
    assert!(names.contains(&"test3_updated"));
}

#[cot_macros::dbtest]
async fn bulk_insert_empty(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let mut models: Vec<TestModel> = vec![];
    let result = TestModel::bulk_insert(&**test_db, &mut models).await;

    assert!(result.is_ok());
    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), 0);
}

#[cot_macros::dbtest]
async fn bulk_insert_large_batch(test_db: &mut TestDatabase) {
    const BATCH_SIZE: usize = 100_000;

    migrate_test_model(&*test_db).await;

    let mut models: Vec<TestModel> = (0..BATCH_SIZE)
        .map(|i| TestModel {
            id: Auto::auto(),
            name: format!("test{i}"),
        })
        .collect();

    TestModel::bulk_insert(&**test_db, &mut models)
        .await
        .unwrap();

    for model in &models {
        assert!(matches!(model.id, Auto::Fixed(_)));
    }

    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects.len(), BATCH_SIZE);
}

#[cot_macros::dbtest]
async fn bulk_insert_no_values(test_db: &mut TestDatabase) {
    #[derive(Debug, PartialEq)]
    #[model]
    struct PkOnlyModel {
        #[model(primary_key)]
        id: Auto<i32>,
    }

    const CREATE_PK_ONLY_MODEL: Operation = Operation::create_model()
        .table_name(Identifier::new("cot__pk_only_model"))
        .fields(&[
            Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build();

    async fn migrate_pk_only_model(db: &Database) {
        CREATE_PK_ONLY_MODEL.forwards(db).await.unwrap();
    }

    const BATCH_SIZE: usize = 17;

    migrate_pk_only_model(&*test_db).await;

    let mut models: Vec<PkOnlyModel> = (0..BATCH_SIZE)
        .map(|_| PkOnlyModel { id: Auto::auto() })
        .collect();

    let result = PkOnlyModel::bulk_insert(&**test_db, &mut models).await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        DatabaseError::BulkInsertNoValueColumns
    ));
}

#[cot_macros::dbtest]
async fn bulk_insert_with_fixed_pk(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let mut models = vec![
        TestModel {
            id: Auto::fixed(100),
            name: "test100".to_owned(),
        },
        TestModel {
            id: Auto::fixed(200),
            name: "test200".to_owned(),
        },
        TestModel {
            id: Auto::fixed(300),
            name: "test300".to_owned(),
        },
    ];

    TestModel::bulk_insert(&**test_db, &mut models)
        .await
        .unwrap();

    let model100 = TestModel::get_by_primary_key(&**test_db, Auto::fixed(100))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(model100.name, "test100");

    let model200 = TestModel::get_by_primary_key(&**test_db, Auto::fixed(200))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(model200.name, "test200");

    let model300 = TestModel::get_by_primary_key(&**test_db, Auto::fixed(300))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(model300.name, "test300");
}

async fn seed(test_db: &TestDatabase, names: &[&str]) {
    let mut models: Vec<TestModel> = names
        .iter()
        .map(|n| TestModel {
            id: Auto::auto(),
            name: (*n).to_owned(),
        })
        .collect();
    TestModel::bulk_insert(&**test_db, &mut models)
        .await
        .unwrap();
}

fn names_of(objects: &[TestModel]) -> Vec<&str> {
    objects.iter().map(|o| o.name.as_str()).collect()
}

#[cot_macros::dbtest]
async fn model_query_contains_case_sensitive(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["foo", "Foo", "fOO", "FOO", "bar"]).await;

    let objects = query!(TestModel, $name.contains("oo"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["foo", "Foo"]);

    let objects = query!(TestModel, $name.contains("fo"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["foo"]);

    let objects = query!(TestModel, $name.contains("bar"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["bar"]);

    let objects = query!(TestModel, $name.contains("xyz"))
        .all(&**test_db)
        .await
        .unwrap();
    assert!(objects.is_empty());

    let objects = query!(TestModel, $name.contains(""))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(objects.len(), 5);
}

#[cot_macros::dbtest]
async fn model_query_icontains(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["foo", "Foo", "fOO", "FOO", "bar"]).await;

    let mut objects = query!(TestModel, $name.icontains("OO"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    assert_eq!(names_of(&objects), vec!["foo", "Foo", "fOO", "FOO"]);

    let objects = query!(TestModel, $name.icontains("xyz"))
        .all(&**test_db)
        .await
        .unwrap();
    assert!(objects.is_empty());
}

#[cot_macros::dbtest]
async fn model_query_starts_with(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["foobar", "Foobar", "barfoo", "foo"]).await;

    let objects = query!(TestModel, $name.starts_with("foo"))
        .all(&**test_db)
        .await
        .unwrap();
    let mut got = names_of(&objects);
    got.sort_unstable();
    assert_eq!(got, vec!["foo", "foobar"]);

    let objects = query!(TestModel, $name.starts_with("bar"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["barfoo"]);

    let objects = query!(TestModel, $name.starts_with("foobarbaz"))
        .all(&**test_db)
        .await
        .unwrap();
    assert!(objects.is_empty());
}

#[cot_macros::dbtest]
async fn model_query_istarts_with(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["foobar", "Foobar", "barfoo"]).await;

    let mut objects = query!(TestModel, $name.istarts_with("FOO"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    assert_eq!(names_of(&objects), vec!["foobar", "Foobar"]);
}

#[cot_macros::dbtest]
async fn model_query_ends_with(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["report.pdf", "report.PDF", "archive.zip", "pdf"]).await;

    let objects = query!(TestModel, $name.ends_with(".pdf"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["report.pdf"]);

    let objects = query!(TestModel, $name.ends_with("report.pdf.pdf"))
        .all(&**test_db)
        .await
        .unwrap();
    assert!(objects.is_empty());
}

#[cot_macros::dbtest]
async fn model_query_iends_with(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["report.pdf", "report.PDF", "archive.zip"]).await;

    let mut objects = query!(TestModel, $name.iends_with(".PDF"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    assert_eq!(names_of(&objects), vec!["report.pdf", "report.PDF"]);
}

#[cot_macros::dbtest]
async fn model_query_raw_positional(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["faXo", "fooo", "fo", "faXYo", "f_o"]).await;

    let mut objects = query!(TestModel, $name.raw_like("f??o"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    let mut got = names_of(&objects);
    got.sort_unstable();
    assert_eq!(got, vec!["faXo", "fooo"]);
}

#[cot_macros::dbtest]
async fn model_query_raw_middle_wildcards(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(
        test_db,
        &[
            "foo_bar_baz",
            "foo bar baz extra",
            "foobarbaz",
            "bar_foo_baz", // wrong order, must not match
        ],
    )
    .await;

    let objects = query!(TestModel, $name.raw_like("*foo*bar*baz*"))
        .all(&**test_db)
        .await
        .unwrap();
    let mut got = names_of(&objects);
    got.sort_unstable();
    assert_eq!(got, vec!["foo bar baz extra", "foo_bar_baz", "foobarbaz"]);
}

#[cot_macros::dbtest]
async fn model_query_raw_escaped_wildcard(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["a*b", "aXb", "a?b"]).await;

    let objects = query!(TestModel, $name.raw_like("a\\*b"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["a*b"]);

    let mut objects = query!(TestModel, $name.raw_like("a?b"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    let mut got = names_of(&objects);
    got.sort_unstable();
    assert_eq!(got, vec!["a*b", "a?b", "aXb"]);
}

#[cot_macros::dbtest]
async fn model_query_iraw(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["README", "ReadMe", "readme", "READMEE", "REDME"]).await;

    let mut objects = query!(TestModel, $name.iraw_like("re?dme"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    let mut got = names_of(&objects);
    got.sort_unstable();
    assert_eq!(got, vec!["README", "ReadMe", "readme",]);
}

#[cot_macros::dbtest]
async fn model_query_literal_wildcard_characters_in_data(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["100% off", "under_score", "a*b", "aXb"]).await;

    let objects = query!(TestModel, $name.contains("100% off"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["100% off"]);

    let objects = query!(TestModel, $name.contains("_score"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["under_score"]);

    let objects = query!(TestModel, $name.contains("a*b"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["a*b"]);
}

#[cot_macros::dbtest]
async fn model_query_unicode_case_sensitive(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(
        test_db,
        &["café", "CAFÉ", "日本語のテスト", "🎉 party time", "naïve"],
    )
    .await;

    let objects = query!(TestModel, $name.contains("café"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["café"]);

    let objects = query!(TestModel, $name.starts_with("日本"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["日本語のテスト"]);

    let objects = query!(TestModel, $name.ends_with("time"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["🎉 party time"]);

    let objects = query!(TestModel, $name.raw_like("na?ve"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["naïve"]);
}

#[cot_macros::dbtest]
async fn model_query_contains_combined_with_boolean_ops(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    seed(test_db, &["apple pie", "apple tart", "banana split"]).await;

    let objects = query!(TestModel, $name.contains("apple") && $name.contains("pie"))
        .all(&**test_db)
        .await
        .unwrap();
    assert_eq!(names_of(&objects), vec!["apple pie"]);
    let mut objects = query!(TestModel, $name.starts_with("apple") || $name.ends_with("split"))
        .all(&**test_db)
        .await
        .unwrap();
    objects.sort_by_key(|a| a.id.unwrap());
    let mut got = names_of(&objects);
    got.sort_unstable();

    assert_eq!(got, vec!["apple pie", "apple tart", "banana split"]);
}
