use cot::db::migrations::{Field, Operation};
use cot::db::query::ExprEq;
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
