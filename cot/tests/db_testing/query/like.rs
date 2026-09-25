use cot::db::{Auto, Model};
use cot::test::TestDatabase;
use cot_macros::query;

use crate::db_testing::query::{TestModel, migrate_test_model};

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
