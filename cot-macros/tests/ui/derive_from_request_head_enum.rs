use cot::request::extractors::FromRequestHead;

#[derive(FromRequestHead)]
enum MyEnum {
    A,
    B,
}

fn main() {}
