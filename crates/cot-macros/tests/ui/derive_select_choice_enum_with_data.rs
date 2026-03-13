use cot_macros::SelectChoice;

#[derive(SelectChoice)]
enum EnumWithData {
    Foo(u32),
    Bar { x: String },
    Unit,
}

fn main() {}
