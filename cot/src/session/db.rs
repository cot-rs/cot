pub mod migrations;

use cot::db::migrations::SyncDynMigration;

use crate::App;
use crate::db::{Auto, Model, model};

#[derive(Debug, Clone)]
#[model]
pub struct Session {
    #[model(primary_key)]
    pub id: Auto<i32>,
    pub key: String,
    pub data: String,
}

pub struct SessionApp;

impl App for SessionApp {
    fn name(&self) -> &'static str {
        "cot_session"
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }
}

impl SessionApp {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}
