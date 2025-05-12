pub mod migrations;

use crate::db::{Auto, Model, model};

#[derive(Debug, Clone)]
#[model]
pub struct Session {
    #[model(primary_key)]
    pub id: Auto<i32>,
    pub session_key: String,
    pub session_data: String,
    // pub session_expiration: i64,
}
