use std::sync::Arc;

use async_trait::async_trait;
use cot::db::{Auto, Model, query};
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

use crate::db::Database;
use crate::session::db::Session;

#[derive(Clone, Debug)]
pub struct DbStore {
    connection: Arc<Database>,
}

impl DbStore {
    pub fn new(connection: Arc<Database>) -> DbStore {
        DbStore { connection }
    }
}

#[async_trait]
impl SessionStore for DbStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let key = record.id.to_string();

        let data = serde_json::to_string(&record).unwrap();
        let mut model = Session {
            id: Auto::auto(),
            key,
            data,
        };
        self.connection
            .insert(&mut model)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let key = record.id.to_string();
        let data = serde_json::to_string(&record).unwrap();

        let query = query!(Session, $key ==key.clone())
            .get(&self.connection)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        if let Some(mut model) = query {
            model.data = data;
            model
                .update(&self.connection)
                .await
                .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        } else {
            let mut record = record.clone();
            self.create(&mut record).await?;
        }
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let key = session_id.to_string();
        let query = query!(Session, $key ==key)
            .get(&self.connection)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        if let Some(data) = query {
            let j = serde_json::from_str::<Record>(&data.data);
            let rec = serde_json::from_str::<Record>(&data.data)
                .map_err(|err| session_store::Error::Backend(err.to_string()))?;
            Ok(Some(rec))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let key = session_id.to_string();
        query!(Session, $key ==key)
            .delete(&self.connection)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
