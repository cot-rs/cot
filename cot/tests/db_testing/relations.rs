use cot::db::migrations::{Field, Operation};
use cot::db::{
    Auto, DatabaseError, DatabaseField, ForeignKey, ForeignKeyOnDeletePolicy,
    ForeignKeyOnUpdatePolicy, Identifier, Model,
};
use cot::test::TestDatabase;
use cot_macros::{model, query};

use crate::db_testing::run_migrations;

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
