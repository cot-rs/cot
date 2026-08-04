use cot::db::migrations::{Field, Operation};
use cot::db::query::expr::ExprEq;
use cot::db::{Auto, Database, DatabaseError, DatabaseField, Identifier, Model};
use cot::test::TestDatabase;
use cot_macros::{model, query};

#[derive(Debug, PartialEq)]
#[model]
struct TestModel {
    #[model(primary_key)]
    id: Auto<i32>,
    name: String,
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

#[cot_macros::dbtest]
async fn raw_as_maps_rows_to_model(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let mut model1 = TestModel {
        id: Auto::fixed(1),
        name: "test1".to_owned(),
    };
    model1.save(&**test_db).await.unwrap();
    let mut model2 = TestModel {
        id: Auto::fixed(2),
        name: "test2".to_owned(),
    };
    model2.save(&**test_db).await.unwrap();

    let mut objects = test_db
        .raw_as::<TestModel>("SELECT * FROM cot__test_model")
        .await
        .unwrap();
    objects.sort_by_key(|o| o.id);
    assert_eq!(objects, vec![model1, model2]);
}

#[cot_macros::dbtest]
async fn raw_executes_statement(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let mut model = TestModel {
        id: Auto::fixed(1),
        name: "test".to_owned(),
    };
    model.save(&**test_db).await.unwrap();

    let result = test_db
        .raw("UPDATE cot__test_model SET name = 'updated'")
        .await
        .unwrap();
    assert_eq!(result.rows_affected().0, 1);

    let objects = TestModel::objects().all(&**test_db).await.unwrap();
    assert_eq!(objects[0].name, "updated");
}

#[cot_macros::dbtest]
async fn raw_returns_error_for_invalid_sql(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;

    let result = test_db.raw("NOT A VALID SQL STATEMENT").await;
    assert!(result.is_err());
}

// `raw_with`/`raw_as_with` need bound-parameter placeholders in the SQL text
// itself (`?` on SQLite/MySQL vs. `$1, $2, ...` on PostgreSQL), so a single
// `dbtest` function body can't exercise all three backends. These are
// therefore SQLite-only.

#[cfg(feature = "sqlite")]
#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn raw_with_executes_parameterized_statement() {
    let db = TestDatabase::new_sqlite()
        .await
        .expect("failed to create SQLite test database");
    migrate_test_model(&db).await;

    let mut model = TestModel {
        id: Auto::fixed(1),
        name: "test".to_owned(),
    };
    model.save(&*db).await.unwrap();

    let params: &[&dyn cot::db::ToDbValue] = &[&"updated", &1_i32];
    let result = db
        .raw_with("UPDATE cot__test_model SET name = ? WHERE id = ?", params)
        .await
        .unwrap();
    assert_eq!(result.rows_affected().0, 1);

    let objects = TestModel::objects().all(&*db).await.unwrap();
    assert_eq!(objects[0].name, "updated");

    db.cleanup()
        .await
        .expect("failed to clean up SQLite test database");
}

#[cfg(feature = "sqlite")]
#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn raw_as_with_maps_parameterized_rows_to_model() {
    let db = TestDatabase::new_sqlite()
        .await
        .expect("failed to create SQLite test database");
    migrate_test_model(&db).await;

    let mut model1 = TestModel {
        id: Auto::fixed(1),
        name: "test1".to_owned(),
    };
    model1.save(&*db).await.unwrap();
    let mut model2 = TestModel {
        id: Auto::fixed(2),
        name: "test2".to_owned(),
    };
    model2.save(&*db).await.unwrap();

    let objects = db
        .raw_as_with::<TestModel>("SELECT * FROM cot__test_model WHERE name = ?", &[&"test1"])
        .await
        .unwrap();
    assert_eq!(objects, vec![model1]);

    let objects = db
        .raw_as_with::<TestModel>("SELECT * FROM cot__test_model WHERE name = ?", &[&"test2"])
        .await
        .unwrap();
    assert_eq!(objects, vec![model2]);

    db.cleanup()
        .await
        .expect("failed to clean up SQLite test database");
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

#[cot_macros::dbtest]
async fn transaction_commit(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut model = TestModel {
        id: Auto::auto(),
        name: "test".to_string(),
    };
    model.insert(&mut transaction).await.unwrap();
    transaction.commit().await.unwrap();

    let exists = TestModel::objects()
        .filter(<TestModel as Model>::Fields::name.eq("test"))
        .exists(db)
        .await
        .unwrap();
    assert!(exists);
}

#[cot_macros::dbtest]
async fn transaction_rollback(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut model = TestModel {
        id: Auto::auto(),
        name: "test_rollback".to_string(),
    };
    model.insert(&mut transaction).await.unwrap();
    transaction.rollback().await.unwrap();

    let exists = TestModel::objects()
        .filter(<TestModel as Model>::Fields::name.eq("test_rollback"))
        .exists(db)
        .await
        .unwrap();
    assert!(!exists);
}

#[cot_macros::dbtest]
async fn transaction_nested(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut outer_model = TestModel {
        id: Auto::auto(),
        name: "outer".to_string(),
    };
    outer_model.insert(&mut transaction).await.unwrap();

    let mut nested = transaction.begin().await.unwrap();
    let mut inner_model = TestModel {
        id: Auto::auto(),
        name: "inner".to_string(),
    };
    inner_model.insert(&mut nested).await.unwrap();
    nested.rollback().await.unwrap();

    transaction.commit().await.unwrap();

    let outer_exists = TestModel::objects()
        .filter(<TestModel as Model>::Fields::name.eq("outer"))
        .exists(db)
        .await
        .unwrap();
    assert!(outer_exists);

    let inner_exists = TestModel::objects()
        .filter(<TestModel as Model>::Fields::name.eq("inner"))
        .exists(db)
        .await
        .unwrap();
    assert!(!inner_exists);
}

#[cot_macros::dbtest]
async fn transaction_nested_commit(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut outer_model = TestModel {
        id: Auto::auto(),
        name: "outer".to_string(),
    };
    outer_model.insert(&mut transaction).await.unwrap();

    let mut nested = transaction.begin().await.unwrap();
    let mut inner_model = TestModel {
        id: Auto::auto(),
        name: "inner".to_string(),
    };
    inner_model.insert(&mut nested).await.unwrap();
    // Committing the savepoint releases it into the enclosing transaction.
    nested.commit().await.unwrap();

    // Both rows are visible within the still-open outer transaction.
    assert_eq!(
        TestModel::objects().count(&mut transaction).await.unwrap(),
        2
    );

    transaction.commit().await.unwrap();

    // After committing the outer transaction, both rows are persisted.
    for name in ["outer", "inner"] {
        assert!(
            TestModel::objects()
                .filter(<TestModel as Model>::Fields::name.eq(name))
                .exists(db)
                .await
                .unwrap()
        );
    }
}

#[cot_macros::dbtest]
async fn transaction_nested_outer_rollback(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut outer_model = TestModel {
        id: Auto::auto(),
        name: "outer".to_string(),
    };
    outer_model.insert(&mut transaction).await.unwrap();

    let mut nested = transaction.begin().await.unwrap();
    let mut inner_model = TestModel {
        id: Auto::auto(),
        name: "inner".to_string(),
    };
    inner_model.insert(&mut nested).await.unwrap();
    // Releasing the savepoint doesn't durably persist the nested work; it only
    // hands it up to the enclosing transaction.
    nested.commit().await.unwrap();

    // Rolling back the outer transaction discards everything, including the
    // work from the already-committed nested transaction.
    transaction.rollback().await.unwrap();

    for name in ["outer", "inner"] {
        assert!(
            !TestModel::objects()
                .filter(<TestModel as Model>::Fields::name.eq(name))
                .exists(db)
                .await
                .unwrap()
        );
    }
}

#[cot_macros::dbtest]
async fn transaction_insert_or_update(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();

    // insert_or_update on a new primary key takes the insert path.
    let mut model = TestModel {
        id: Auto::fixed(1),
        name: "inserted".to_string(),
    };
    model.save(&mut transaction).await.unwrap();
    assert_eq!(
        TestModel::get_by_primary_key(&mut transaction, model.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "inserted"
    );

    // insert_or_update on an existing primary key takes the update path.
    model.name = "updated".to_string();
    model.save(&mut transaction).await.unwrap();
    assert_eq!(
        TestModel::get_by_primary_key(&mut transaction, model.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "updated"
    );

    transaction.commit().await.unwrap();

    let saved = TestModel::get_by_primary_key(db, model.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.name, "updated");
}

#[cot_macros::dbtest]
async fn transaction_bulk_insert(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut models = vec![
        TestModel {
            id: Auto::auto(),
            name: "bulk1".to_string(),
        },
        TestModel {
            id: Auto::auto(),
            name: "bulk2".to_string(),
        },
    ];
    TestModel::bulk_insert(&mut transaction, &mut models)
        .await
        .unwrap();
    assert!(matches!(models[0].id, Auto::Fixed(_)));
    assert!(matches!(models[1].id, Auto::Fixed(_)));

    let count_in_transaction = TestModel::objects().count(&mut transaction).await.unwrap();
    assert_eq!(count_in_transaction, 2);

    transaction.commit().await.unwrap();

    let objects = TestModel::objects().all(db).await.unwrap();
    let names: Vec<_> = objects.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"bulk1"));
    assert!(names.contains(&"bulk2"));
}

#[cot_macros::dbtest]
async fn transaction_bulk_insert_or_update(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut initial = vec![
        TestModel {
            id: Auto::auto(),
            name: "initial1".to_string(),
        },
        TestModel {
            id: Auto::auto(),
            name: "initial2".to_string(),
        },
    ];
    TestModel::bulk_insert(db, &mut initial).await.unwrap();

    let mut transaction = db.begin().await.unwrap();
    let mut updates = vec![
        TestModel {
            id: initial[0].id,
            name: "initial1_updated".to_string(),
        },
        TestModel {
            id: Auto::fixed(9999),
            name: "new".to_string(),
        },
    ];
    TestModel::bulk_insert_or_update(&mut transaction, &mut updates)
        .await
        .unwrap();

    let names_in_transaction: Vec<_> = TestModel::objects()
        .all(&mut transaction)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert!(names_in_transaction.contains(&"initial1_updated".to_string()));
    assert!(names_in_transaction.contains(&"initial2".to_string()));
    assert!(names_in_transaction.contains(&"new".to_string()));

    transaction.commit().await.unwrap();

    let names: Vec<_> = TestModel::objects()
        .all(db)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert!(names.contains(&"initial1_updated".to_string()));
    assert!(names.contains(&"initial2".to_string()));
    assert!(names.contains(&"new".to_string()));
}

#[cot_macros::dbtest]
async fn transaction_query(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut model = TestModel {
        id: Auto::auto(),
        name: "queried".to_string(),
    };
    model.insert(&mut transaction).await.unwrap();

    // The insert isn't committed yet, so it's only visible through the
    // transaction that created it.
    let objects = query!(TestModel, $name == "queried")
        .all(&mut transaction)
        .await
        .unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "queried");

    transaction.rollback().await.unwrap();

    let objects = query!(TestModel, $name == "queried").all(db).await.unwrap();
    assert!(objects.is_empty());
}

#[cot_macros::dbtest]
async fn transaction_exists(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut model = TestModel {
        id: Auto::auto(),
        name: "exists_check".to_string(),
    };
    model.insert(&mut transaction).await.unwrap();

    assert!(
        TestModel::objects()
            .filter(<TestModel as Model>::Fields::name.eq("exists_check"))
            .exists(&mut transaction)
            .await
            .unwrap()
    );
    assert!(
        !TestModel::objects()
            .filter(<TestModel as Model>::Fields::name.eq("does_not_exist"))
            .exists(&mut transaction)
            .await
            .unwrap()
    );

    transaction.commit().await.unwrap();

    assert!(
        TestModel::objects()
            .filter(<TestModel as Model>::Fields::name.eq("exists_check"))
            .exists(db)
            .await
            .unwrap()
    );
}

#[cot_macros::dbtest]
async fn transaction_count(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    for name in ["count1", "count2", "count3"] {
        let mut model = TestModel {
            id: Auto::auto(),
            name: name.to_string(),
        };
        model.insert(&mut transaction).await.unwrap();
    }

    assert_eq!(
        TestModel::objects().count(&mut transaction).await.unwrap(),
        3
    );
    assert_eq!(
        TestModel::objects()
            .filter(<TestModel as Model>::Fields::name.eq("count2"))
            .count(&mut transaction)
            .await
            .unwrap(),
        1
    );

    transaction.commit().await.unwrap();

    assert_eq!(TestModel::objects().count(db).await.unwrap(), 3);
}

#[cot_macros::dbtest]
async fn transaction_delete(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut models = vec![
        TestModel {
            id: Auto::auto(),
            name: "keep".to_string(),
        },
        TestModel {
            id: Auto::auto(),
            name: "remove".to_string(),
        },
    ];
    TestModel::bulk_insert(db, &mut models).await.unwrap();

    let mut transaction = db.begin().await.unwrap();
    TestModel::objects()
        .filter(<TestModel as Model>::Fields::name.eq("remove"))
        .delete(&mut transaction)
        .await
        .unwrap();

    // The deletion isn't committed yet, but it's already visible through
    // the transaction that performed it.
    assert_eq!(
        TestModel::objects().count(&mut transaction).await.unwrap(),
        1
    );

    transaction.commit().await.unwrap();

    let names: Vec<_> = TestModel::objects()
        .all(db)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert_eq!(names, vec!["keep".to_string()]);
}

#[cot_macros::dbtest]
async fn transaction_raw(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let result = transaction
        .raw("INSERT INTO cot__test_model (name) VALUES ('raw')")
        .await
        .unwrap();
    assert_eq!(result.rows_affected().0, 1);

    // The insert is visible within the transaction that performed it.
    let objects = transaction
        .raw_as::<TestModel>("SELECT * FROM cot__test_model")
        .await
        .unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "raw");

    transaction.commit().await.unwrap();

    let objects = TestModel::objects().all(db).await.unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "raw");
}

#[cot_macros::dbtest]
async fn transaction_raw_rollback(test_db: &mut TestDatabase) {
    migrate_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    transaction
        .raw("INSERT INTO cot__test_model (name) VALUES ('raw_rollback')")
        .await
        .unwrap();
    transaction.rollback().await.unwrap();

    assert_eq!(TestModel::objects().count(db).await.unwrap(), 0);
}

// `raw_with`/`raw_as_with` need bound-parameter placeholders in the SQL text
// itself (`?` on SQLite/MySQL vs. `$1, $2, ...` on PostgreSQL), so a single
// `dbtest` function body can't exercise all three backends. These are
// therefore SQLite-only.

#[cfg(feature = "sqlite")]
#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn transaction_raw_with_parameterized() {
    let db = TestDatabase::new_sqlite()
        .await
        .expect("failed to create SQLite test database");
    migrate_test_model(&db).await;

    let mut transaction = db.begin().await.unwrap();
    let params: &[&dyn cot::db::ToDbValue] = &[&"raw_param"];
    let result = transaction
        .raw_with("INSERT INTO cot__test_model (name) VALUES (?)", params)
        .await
        .unwrap();
    assert_eq!(result.rows_affected().0, 1);

    let objects = transaction
        .raw_as_with::<TestModel>(
            "SELECT * FROM cot__test_model WHERE name = ?",
            &[&"raw_param"],
        )
        .await
        .unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "raw_param");

    transaction.commit().await.unwrap();

    let objects = TestModel::objects().all(&*db).await.unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "raw_param");

    db.cleanup()
        .await
        .expect("failed to clean up SQLite test database");
}
