use cot::db::query::expr::ExprEq;
use cot::db::{Auto, Model};
use cot::test::TestDatabase;
use cot_macros::query;

use crate::db_testing::query::{TestModel, migrate_test_model};

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
