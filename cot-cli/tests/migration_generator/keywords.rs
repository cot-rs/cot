use cot::db::{Auto, model};

#[model]
struct r#const {
    #[model(primary_key)]
    id: Auto<i32>,
    r#abstract: String,
    r#type: i32,
}
