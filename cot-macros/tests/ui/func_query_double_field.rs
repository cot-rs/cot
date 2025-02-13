use cot::db::{model, query};

#[derive(Debug)]
#[model]
struct MyModel {
    #[model(primary_key)]
    id: i32,
    name: std::string::String,
    description: String,
    visits: i32,
}

fn main() {
    query!(
        MyModel,
        $name $name
    );
}
