use std::borrow::Cow;

use cot::auth::db::{DatabaseUser, DatabaseUserCredentials};
use cot::auth::{Auth, Password};
use cot::request::RequestExt;
use cot::test::{TestDatabase, TestRequestBuilder};

#[cot_macros::dbtest]
async fn database_user(test_db: &mut TestDatabase) {
    test_db.with_auth().run_migrations().await;
    let mut request_builder = TestRequestBuilder::get("/");
    request_builder.with_db_auth(test_db.database()).await;

    let mut request = request_builder.clone().with_session().build();
    let auth: Auth = request.extract_parts().await.unwrap();

    // Anonymous user
    assert!(!auth.user().is_authenticated());

    // Authenticated user
    DatabaseUser::create_user(
        &**test_db,
        "testuser".to_string(),
        &Password::new("password123"),
    )
    .await
    .unwrap();

    let user = auth
        .authenticate(&DatabaseUserCredentials::new(
            "testuser".to_string(),
            Password::new("password123"),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(user.is_authenticated());
    assert_eq!(user.username(), Some(Cow::from("testuser")));

    // Log in
    auth.login(user).await.unwrap();
    let user = auth.user();
    assert!(user.is_authenticated());
    assert_eq!(user.username(), Some(Cow::from("testuser")));

    // Invalid credentials
    let user = auth
        .authenticate(&DatabaseUserCredentials::new(
            "testuser".to_string(),
            Password::new("wrongpassword"),
        ))
        .await
        .unwrap();
    assert!(user.is_none());

    // User persists between requests
    let mut request = request_builder.clone().with_session_from(&request).build();
    let auth: Auth = request.extract_parts().await.unwrap();

    let user = auth.user();
    assert!(user.is_authenticated());
    assert_eq!(user.username(), Some(Cow::from("testuser")));

    // Log out
    auth.logout().await.unwrap();
    assert!(!auth.user().is_authenticated());
}
