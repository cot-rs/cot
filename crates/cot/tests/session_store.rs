use std::collections::HashMap;

use cot::App;
use cot::session::db::SessionApp;
use cot::session::store::db::DbStore;
use cot::test::TestDatabase;
use time::{Duration, OffsetDateTime};
use tower_sessions::SessionStore;
use tower_sessions::session::{Id, Record};

async fn make_db_store(test_db: &mut TestDatabase) -> DbStore {
    let session_app = SessionApp::new();
    test_db.add_migrations(session_app.migrations());
    test_db.run_migrations().await;
    DbStore::new(test_db.database())
}

fn make_record() -> Record {
    Record {
        id: Id::default(),
        data: HashMap::default(),
        expiry_date: OffsetDateTime::now_utc() + Duration::minutes(30),
    }
}

fn truncate_record_expiry(record: &Record) -> Record {
    let mut record = record.clone();
    let exp = record.expiry_date;
    let exp = exp
        .replace_nanosecond(exp.microsecond() * 1_000)
        .expect("could not replace nano seconds.");

    record.expiry_date = exp;
    record
}

#[cot_macros::dbtest]
async fn test_create_and_load(test_db: &mut TestDatabase) {
    let store = make_db_store(test_db).await;
    let mut rec = make_record();
    store.create(&mut rec).await.expect("create failed");
    let loaded = store.load(&rec.id).await.expect("load err");
    let expected = truncate_record_expiry(&rec);
    assert_eq!(Some(expected), loaded);
}

#[cot_macros::dbtest]
async fn test_save_overwrites(test_db: &mut TestDatabase) {
    let store = make_db_store(test_db).await;
    let mut rec = make_record();
    store.create(&mut rec).await.unwrap();

    let mut rec2 = rec.clone();
    rec2.data.insert("foo".into(), "bar".into());
    store.save(&rec2).await.expect("save failed");

    let loaded = store.load(&rec.id).await.unwrap().unwrap();
    assert_eq!(rec2.data, loaded.data);
}

#[cot_macros::dbtest]
async fn test_save_creates_if_missing(test_db: &mut TestDatabase) {
    let store = make_db_store(test_db).await;
    let rec = make_record();
    store.save(&rec).await.expect("save failed");
    let loaded = store.load(&rec.id).await.unwrap();
    let expected = truncate_record_expiry(&rec);
    assert_eq!(Some(expected), loaded);
}

#[cot_macros::dbtest]
async fn test_delete(test_db: &mut TestDatabase) {
    let store = make_db_store(test_db).await;
    let mut rec = make_record();
    store.create(&mut rec).await.unwrap();

    store.delete(&rec.id).await.expect("delete failed");
    let loaded = store.load(&rec.id).await.unwrap();
    assert!(loaded.is_none());

    store.delete(&rec.id).await.expect("second delete");
}

#[cot_macros::dbtest]
async fn test_create_id_collision(test_db: &mut TestDatabase) {
    let store = make_db_store(test_db).await;
    let expiry = OffsetDateTime::now_utc() + Duration::minutes(30);

    let mut r1 = Record {
        id: Id::default(),
        data: HashMap::default(),
        expiry_date: expiry,
    };
    store.create(&mut r1).await.unwrap();

    let mut r2 = Record {
        id: r1.id,
        data: HashMap::default(),
        expiry_date: expiry,
    };
    store.create(&mut r2).await.unwrap();

    assert_ne!(r1.id, r2.id, "ID collision not resolved");

    let loaded1 = store.load(&r1.id).await.unwrap();
    let loaded2 = store.load(&r2.id).await.unwrap();
    assert!(loaded1.is_some() && loaded2.is_some());
}
