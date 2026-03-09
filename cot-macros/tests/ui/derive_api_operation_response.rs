use cot::openapi::ApiOperationResponse;
use cot::response::IntoResponse;

#[derive(IntoResponse, ApiOperationResponse)]
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

impl ApiOperationResponse for DummyA {}

struct DummyB;

impl IntoResponse for DummyB {
    fn into_response(self) -> cot::Result<cot::response::Response> {
        unimplemented!()
    }
}

impl ApiOperationResponse for DummyB {}

fn main() {}
