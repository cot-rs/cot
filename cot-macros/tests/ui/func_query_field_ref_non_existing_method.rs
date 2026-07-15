use cot::db::{model, query};

#[model]
struct MyModel {
    #[model(primary_key)]
    id: i32,
    name: String,
}

fn main() {
    query!(MyModel, $name.non_existing_method());
}