pub mod memory;

use std::sync::Arc;

use async_trait::async_trait;
use cot::middleware::SessionStore;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store;

pub trait ToSessionStore {
    #[must_use]
    fn to_session_store(self) -> Result<Box<dyn SessionStore + Send + Sync>, session_store::Error>;
}

#[derive(Debug, Clone)]
pub struct SessionStoreWrapper(Arc<dyn SessionStore>);

impl SessionStoreWrapper {
    pub fn new(boxed: Arc<dyn SessionStore + Send + Sync>) -> Self {
        Self(boxed)
    }
}

#[async_trait]
impl SessionStore for SessionStoreWrapper {
    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        self.0.save(session_record).await
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        self.0.load(session_id).await
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        self.0.delete(session_id).await
    }
}
