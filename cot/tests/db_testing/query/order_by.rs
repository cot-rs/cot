use cot::db::migrations::{Field, Operation};
use cot::db::query::expr::ExprSort;
use cot::db::{Auto, Database, DatabaseField, Identifier, Model};
use cot::test::TestDatabase;
use cot_macros::{model, query};

#[derive(Debug, PartialEq, Clone)]
#[model]
struct OrderTestModel {
    #[model(primary_key)]
    id: Auto<i32>,
    category: String,
    priority: i32,
    x: i32,
    y: i32,
    score: Option<i32>,
}

async fn migrate_order_test_model(db: &Database) {
    CREATE_ORDER_TEST_MODEL.forwards(db).await.unwrap();
}

const CREATE_ORDER_TEST_MODEL: Operation = Operation::create_model()
    .table_name(Identifier::new("cot__order_test_model"))
    .fields(&[
        Field::new(Identifier::new("id"), <Auto<i32> as DatabaseField>::TYPE)
            .primary_key()
            .auto(),
        Field::new(Identifier::new("category"), <String as DatabaseField>::TYPE),
        Field::new(Identifier::new("priority"), <i32 as DatabaseField>::TYPE),
        Field::new(Identifier::new("x"), <i32 as DatabaseField>::TYPE),
        Field::new(Identifier::new("y"), <i32 as DatabaseField>::TYPE),
        Field::new(
            Identifier::new("score"),
            <Option<i32> as DatabaseField>::TYPE,
        )
        .null(),
    ])
    .build();

async fn seed_order_test_model(
    test_db: &TestDatabase,
    rows: &[(&str, i32, i32, i32, Option<i32>)],
) {
    let mut models: Vec<OrderTestModel> = rows
        .iter()
        .map(|(category, priority, x, y, score)| OrderTestModel {
            id: Auto::auto(),
            category: (*category).to_owned(),
            priority: *priority,
            x: *x,
            y: *y,
            score: *score,
        })
        .collect();
    OrderTestModel::bulk_insert(&**test_db, &mut models)
        .await
        .unwrap();
}

fn categories_of(objects: &[OrderTestModel]) -> Vec<&str> {
    objects.iter().map(|o| o.category.as_str()).collect()
}

#[cot_macros::dbtest]
async fn order_by_single_field_ascending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("banana", 1, 0, 0, None),
            ("apple", 1, 0, 0, None),
            ("cherry", 1, 0, 0, None),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::category.asc()])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["apple", "banana", "cherry"]);
}

#[cot_macros::dbtest]
async fn order_by_single_field_descending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("banana", 1, 0, 0, None),
            ("apple", 1, 0, 0, None),
            ("cherry", 1, 0, 0, None),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::category.desc()])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["cherry", "banana", "apple"]);
}

#[cot_macros::dbtest]
async fn order_by_bare_field_defaults_to_ascending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[("banana", 1, 0, 0, None), ("apple", 1, 0, 0, None)],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::category])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["apple", "banana"]);
}

#[cot_macros::dbtest]
async fn order_by_multiple_columns_breaks_ties(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("fruit", 2, 0, 0, None),
            ("fruit", 1, 0, 0, None),
            ("veg", 1, 0, 0, None),
            ("fruit", 3, 0, 0, None),
        ],
    )
    .await;

    // category ASC, then priority DESC within each category.
    let objects = OrderTestModel::objects()
        .order_by([
            <OrderTestModel as Model>::Fields::category.asc(),
            <OrderTestModel as Model>::Fields::priority.desc(),
        ])
        .all(&**test_db)
        .await
        .unwrap();

    let got: Vec<_> = objects
        .iter()
        .map(|o| (o.category.as_str(), o.priority))
        .collect();
    assert_eq!(
        got,
        vec![("fruit", 3), ("fruit", 2), ("fruit", 1), ("veg", 1)]
    );
}

#[cot_macros::dbtest]
async fn order_by_expression_ascending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 1, 5, 5, None), // sum = 10
            ("b", 1, 1, 1, None), // sum = 2
            ("c", 1, 3, 3, None), // sum = 6
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([
            (<OrderTestModel as Model>::Fields::x + <OrderTestModel as Model>::Fields::y).asc(),
        ])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["b", "c", "a"]);
}

#[cot_macros::dbtest]
async fn order_by_expression_descending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 1, 5, 5, None),
            ("b", 1, 1, 1, None),
            ("c", 1, 3, 3, None),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([
            (<OrderTestModel as Model>::Fields::x + <OrderTestModel as Model>::Fields::y).desc(),
        ])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["a", "c", "b"]);
}

#[cot_macros::dbtest]
async fn order_by_bare_expression_defaults_to_ascending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(test_db, &[("a", 1, 5, 5, None), ("b", 1, 1, 1, None)]).await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::x + <OrderTestModel as Model>::Fields::y])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["b", "a"]);
}

#[cot_macros::dbtest]
async fn order_by_mixed_column_and_expression_terms(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("fruit", 1, 2, 2, None), // sum = 4
            ("fruit", 1, 1, 1, None), // sum = 2
            ("veg", 1, 0, 0, None),   // sum = 0
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([
            <OrderTestModel as Model>::Fields::category.asc(),
            (<OrderTestModel as Model>::Fields::x + <OrderTestModel as Model>::Fields::y).desc(),
        ])
        .all(&**test_db)
        .await
        .unwrap();

    let got: Vec<_> = objects
        .iter()
        .map(|o| (o.category.as_str(), o.x + o.y))
        .collect();
    assert_eq!(got, vec![("fruit", 4), ("fruit", 2), ("veg", 0)]);
}

#[cot_macros::dbtest]
async fn order_by_nulls_first_with_ascending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 1, 0, 0, Some(2)),
            ("b", 1, 0, 0, None),
            ("c", 1, 0, 0, Some(1)),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::score.asc().nulls_first()])
        .all(&**test_db)
        .await
        .unwrap();

    let scores: Vec<_> = objects.iter().map(|o| o.score).collect();
    assert_eq!(scores, vec![None, Some(1), Some(2)]);
}

#[cot_macros::dbtest]
async fn order_by_nulls_last_with_ascending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 1, 0, 0, Some(2)),
            ("b", 1, 0, 0, None),
            ("c", 1, 0, 0, Some(1)),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::score.asc().nulls_last()])
        .all(&**test_db)
        .await
        .unwrap();

    let scores: Vec<_> = objects.iter().map(|o| o.score).collect();
    assert_eq!(scores, vec![Some(1), Some(2), None]);
}

#[cot_macros::dbtest]
async fn order_by_nulls_first_with_descending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 1, 0, 0, Some(2)),
            ("b", 1, 0, 0, None),
            ("c", 1, 0, 0, Some(1)),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::score
            .desc()
            .nulls_first()])
        .all(&**test_db)
        .await
        .unwrap();

    let scores: Vec<_> = objects.iter().map(|o| o.score).collect();
    assert_eq!(scores, vec![None, Some(2), Some(1)]);
}

#[cot_macros::dbtest]
async fn order_by_nulls_last_with_descending(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 1, 0, 0, Some(2)),
            ("b", 1, 0, 0, None),
            ("c", 1, 0, 0, Some(1)),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::score.desc().nulls_last()])
        .all(&**test_db)
        .await
        .unwrap();

    let scores: Vec<_> = objects.iter().map(|o| o.score).collect();
    assert_eq!(scores, vec![Some(2), Some(1), None]);
}

#[cot_macros::dbtest]
async fn order_by_custom_ranking(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("apple", 1, 0, 0, None),
            ("banana", 1, 0, 0, None),
            ("cherry", 1, 0, 0, None),
        ],
    )
    .await;

    // Rank explicitly as cherry, apple, banana regardless of alphabetic
    // or insertion order.
    let objects = OrderTestModel::objects()
        .order_by([
            <OrderTestModel as Model>::Fields::category.custom(["cherry", "apple", "banana"])
        ])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(categories_of(&objects), vec!["cherry", "apple", "banana"]);
}

#[cot_macros::dbtest]
async fn order_by_custom_ranking_partial_list_keeps_remaining_rows(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("apple", 1, 0, 0, None),
            ("banana", 1, 0, 0, None),
            ("cherry", 1, 0, 0, None),
        ],
    )
    .await;

    // Only rank "banana" explicitly. the rest keep arbitrary (but present)
    // positions after it.
    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::category.custom(["banana"])])
        .all(&**test_db)
        .await
        .unwrap();

    assert_eq!(objects.len(), 3);
    let mut got = categories_of(&objects);
    got.sort_unstable();
    assert_eq!(got, vec!["apple", "banana", "cherry"]);
}

#[cot_macros::dbtest]
async fn order_by_combined_with_filter(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("fruit", 3, 0, 0, None),
            ("fruit", 1, 0, 0, None),
            ("veg", 5, 0, 0, None),
            ("fruit", 2, 0, 0, None),
        ],
    )
    .await;

    let objects = query!(OrderTestModel, $category == "fruit")
        .order_by([<OrderTestModel as Model>::Fields::priority.desc()])
        .all(&**test_db)
        .await
        .unwrap();

    let priorities: Vec<_> = objects.iter().map(|o| o.priority).collect();
    assert_eq!(priorities, vec![3, 2, 1]);
}

#[cot_macros::dbtest]
async fn order_by_combined_with_limit_and_offset(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    seed_order_test_model(
        test_db,
        &[
            ("a", 5, 0, 0, None),
            ("b", 3, 0, 0, None),
            ("c", 4, 0, 0, None),
            ("d", 1, 0, 0, None),
            ("e", 2, 0, 0, None),
        ],
    )
    .await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::priority.asc()])
        .limit(2)
        .offset(1)
        .all(&**test_db)
        .await
        .unwrap();

    let priorities: Vec<_> = objects.iter().map(|o| o.priority).collect();
    assert_eq!(priorities, vec![2, 3]);
}

#[cot_macros::dbtest]
async fn order_by_within_uncommitted_transaction(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;
    let db = &**test_db;

    let mut transaction = db.begin().await.unwrap();
    let mut models = vec![
        OrderTestModel {
            id: Auto::auto(),
            category: "c".to_owned(),
            priority: 3,
            x: 0,
            y: 0,
            score: None,
        },
        OrderTestModel {
            id: Auto::auto(),
            category: "a".to_owned(),
            priority: 1,
            x: 0,
            y: 0,
            score: None,
        },
        OrderTestModel {
            id: Auto::auto(),
            category: "b".to_owned(),
            priority: 2,
            x: 0,
            y: 0,
            score: None,
        },
    ];
    OrderTestModel::bulk_insert(&mut transaction, &mut models)
        .await
        .unwrap();

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::category.asc()])
        .all(&mut transaction)
        .await
        .unwrap();
    assert_eq!(categories_of(&objects), vec!["a", "b", "c"]);

    transaction.commit().await.unwrap();

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::category.desc()])
        .all(db)
        .await
        .unwrap();
    assert_eq!(categories_of(&objects), vec!["c", "b", "a"]);
}

#[cot_macros::dbtest]
async fn order_by_empty_table_returns_empty(test_db: &mut TestDatabase) {
    migrate_order_test_model(&*test_db).await;

    let objects = OrderTestModel::objects()
        .order_by([<OrderTestModel as Model>::Fields::priority.asc()])
        .all(&**test_db)
        .await
        .unwrap();

    assert!(objects.is_empty());
}
