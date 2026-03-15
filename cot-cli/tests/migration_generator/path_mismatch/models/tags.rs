use cot::db::{Auto, model};

#[model]
pub struct Tags {
    #[model(primary_key)]
    pub id: Auto<i64>,
}
