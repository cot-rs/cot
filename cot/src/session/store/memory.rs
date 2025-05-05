use async_trait::async_trait;
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

#[derive(Debug)]
pub struct MemoryStore(tower_sessions::MemoryStore);

impl MemoryStore {
    #[must_use]
    pub fn new() -> Self {
        Self(tower_sessions::MemoryStore::default())
    }
}

#[async_trait]
impl SessionStore for MemoryStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        self.0.create(session_record).await
    }

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
