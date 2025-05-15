use std::fmt::Display;

use cot::admin::AdminModel;
use cot::db::{Model, model};
use cot::form::Form;

#[model]
#[derive(Debug, Form, AdminModel)]
struct MyModel {
    #[model(primary_key)]
    id: i32,
    name: std::string::String,
}

impl Display for MyModel {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unimplemented!()
    }
}

fn main() {
    println!("{:?}", MyModel::TABLE_NAME);
}
