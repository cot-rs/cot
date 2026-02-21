use cot::db::{model, Auto};

#[model]
struct r#Type {
    #[model(primary_key)]
    id: Auto<i32>,
    name: String,
}
