use cot::response::IntoResponse;

#[derive(IntoResponse)]
enum MyResponse {
    A(DummyA),
    B(DummyB),
}

struct DummyA;

impl IntoResponse for DummyA {
    fn into_response(self) -> cot::Result<cot::response::Response> {
        unimplemented!()
    }
}

struct DummyB;

impl IntoResponse for DummyB {
    fn into_response(self) -> cot::Result<cot::response::Response> {
        unimplemented!()
    }
}

fn main() {}
