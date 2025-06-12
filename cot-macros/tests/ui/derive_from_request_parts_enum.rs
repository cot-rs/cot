use cot::request::extractors::FromRequestParts;

#[derive(FromRequestParts)]
enum MyEnum {
    A,
    B,
}

fn main() {}
