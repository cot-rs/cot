use cot_macros::SelectChoice;

#[derive(SelectChoice)]
enum EnumWithData {
    Foo(u32),
    Bar { x: String },
    Unit,
}

#[derive(SelectChoice)]
struct NotAnEnum {
    x: u8,
    y: u8,
}

#[derive(SelectChoice)]
enum EmptyEnum {}

// #[derive(SelectChoice)]
// enum WrongAttr {
//     #[select(id = "bad")]
//     Variant,
//     Ok,
// }

fn main() {}
