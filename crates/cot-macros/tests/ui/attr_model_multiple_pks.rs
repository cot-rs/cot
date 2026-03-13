use cot::db::model;

#[model]
struct MyModel {
    #[model(primary_key)]
    id: i64,
    #[model(primary_key)]
    id_2: i64,
    name: String,
}

fn main() {}
