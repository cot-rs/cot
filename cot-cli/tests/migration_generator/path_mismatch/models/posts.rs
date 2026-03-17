use cot::db::{Auto, ForeignKey, model};

use crate::tags::Tags;

#[model]
pub struct Posts {
    #[model(primary_key)]
    pub id: Auto<i64>,
    pub tag: ForeignKey<Tags>,
}
