use cot_macros::SelectAsFormField;

#[derive(SelectAsFormField)]
struct NotAnEnum {
    x: u8,
}

fn main() {}
