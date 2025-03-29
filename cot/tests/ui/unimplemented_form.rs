use cot::form::{Form, FormContext};

struct MyForm {}

fn main() {
    let _ = <<MyForm as Form>::Context as FormContext>::new();
}
