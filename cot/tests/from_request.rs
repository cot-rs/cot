use cot::http::Request;
use cot::http::request::Parts;
use cot::request::extractors::FromRequestParts;

#[derive(FromRequestParts)]
#[allow(dead_code)]
struct MyStruct {
    user_id: DummyExtractor,
    session_id: DummyExtractor,
}

#[derive(FromRequestParts)]
struct MyUnitStruct;

#[derive(FromRequestParts)]
struct MyTupleStruct(DummyExtractor, DummyExtractor);

struct DummyExtractor;

impl FromRequestParts for DummyExtractor {
    async fn from_request_parts(_parts: &mut Parts) -> cot::Result<Self> {
        Ok(Self)
    }
}

#[cot::test]
async fn test_named_struct() {
    let req = Request::builder().uri("/").body(()).unwrap();
    let (mut parts, ()) = req.into_parts();
    let _ = MyStruct::from_request_parts(&mut parts).await.unwrap();
}

#[cot::test]
async fn test_unit_struct() {
    let req = Request::builder().uri("/").body(()).unwrap();
    let (mut parts, ()) = req.into_parts();
    let _ = MyUnitStruct::from_request_parts(&mut parts).await.unwrap();
}

#[cot::test]
async fn test_tuple_struct() {
    let req = Request::builder().uri("/").body(()).unwrap();
    let (mut parts, ()) = req.into_parts();
    let _ = MyTupleStruct::from_request_parts(&mut parts).await.unwrap();
}
