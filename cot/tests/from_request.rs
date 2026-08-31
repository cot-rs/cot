use cot::http::Request;
use cot::request::RequestHead;
use cot::request::extractors::FromRequestHead;

#[derive(FromRequestHead)]
#[expect(dead_code)]
struct MyStruct {
    user_id: DummyExtractor,
    session_id: DummyExtractor,
}

#[derive(FromRequestHead)]
struct MyUnitStruct;

#[derive(FromRequestHead)]
struct MyTupleStruct(DummyExtractor, DummyExtractor);

struct DummyExtractor;

impl FromRequestHead for DummyExtractor {
    fn from_request_head(_head: &RequestHead) -> impl Future<Output = cot::Result<Self>> + Send {
        core::future::ready(Ok(Self))
    }
}

#[cot::test]
async fn test_named_struct() {
    let req = Request::builder().uri("/").body(()).unwrap();
    let (head, ()) = req.into_parts();
    let _ = MyStruct::from_request_head(&head).await.unwrap();
}

#[cot::test]
async fn test_unit_struct() {
    let req = Request::builder().uri("/").body(()).unwrap();
    let (head, ()) = req.into_parts();
    let _ = MyUnitStruct::from_request_head(&head).await.unwrap();
}

#[cot::test]
async fn test_tuple_struct() {
    let req = Request::builder().uri("/").body(()).unwrap();
    let (head, ()) = req.into_parts();
    let _ = MyTupleStruct::from_request_head(&head).await.unwrap();
}
