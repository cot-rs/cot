use std::sync::Arc;

use async_trait::async_trait;
use cot::db::{Auto, LimitedString, Model, model, query};
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionStore, session_store};

use crate::db::Database;
use crate::session::db::Session as SessionModel;

#[derive(Clone, Debug)]
pub struct DbStore {
    connection: Arc<Database>,
    namespace: String,
}

impl DbStore {
    pub fn new(connection: Arc<Database>, namespace: impl Into<String>) -> DbStore {
        DbStore {
            connection,
            namespace: namespace.into(),
        }
    }

    fn generate_key(&self, key: impl Into<String>) -> String {
        format!("{}-{}", self.namespace, key.into())
    }
}

#[async_trait]
impl SessionStore for DbStore {
    async fn create(&self, session_record: &mut Record) -> session_store::Result<()> {
        let key = self.generate_key(session_record.id.to_string());
        let data = serde_json::to_string(&session_record.data).unwrap();
        let mut model = SessionModel {
            id: Auto::auto(),
            session_key: key,
            session_data: data,
        };
        let _: () = self
            .connection
            .insert(&mut model)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }

    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        let key = self.generate_key(session_record.id.to_string());
        let data = serde_json::to_string(&session_record.data).unwrap();
        // let mut model = SessionModel{
        //     id: Auto::auto(),
        //     session_key: key,
        //     session_data: data,
        // };
        println!("did we get here?");
        let query = query!(SessionModel, $session_data ==key.clone())
            .get(&self.connection)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        println!("Okay how about we get here?");
        if let Some(mut model) = query {
            model.session_data = data;
            model
                .update(&self.connection)
                .await
                .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        } else {
            println!("error error error!");
            let mut model = SessionModel {
                id: Auto::auto(),
                session_key: key,
                session_data: data,
            };
            let _: () = self
                .connection
                .insert(&mut model)
                .await
                .map_err(|err| session_store::Error::Backend(err.to_string()))?;

            // return Err(session_store::Error::Backend("Could not find key with
            // given data".to_string()));
        }
        println!("wheeeeeww!");
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        println!("load load load load");
        let key = self.generate_key(session_id.to_string());
        let query = query!(SessionModel, $session_data ==key)
            .get(&self.connection)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        if let Some(data) = query {
            println!("data here??");
            let rec = serde_json::from_str::<Record>(&data.session_data)
                .map_err(|err| session_store::Error::Backend(err.to_string()))?;
            Ok(Some(rec))
        } else {
            println!("nothing here");
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let key = self.generate_key(session_id.to_string());
        query!(SessionModel, $session_data ==key)
            .delete(&self.connection)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }
}
