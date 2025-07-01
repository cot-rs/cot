use cot_macros::SelectChoice;

#[derive(SelectChoice)]
struct NotAnEnum {
    x: u8,
    y: u8,
}

fn main() {}
