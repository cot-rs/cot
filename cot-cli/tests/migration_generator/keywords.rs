use cot::db::{model, Auto, ForeignKey};

#[model]
struct Keywords {
    #[model(primary_key)]
    id: Auto<i32>,
    r#abstract: String,
    r#type: i32,
}
