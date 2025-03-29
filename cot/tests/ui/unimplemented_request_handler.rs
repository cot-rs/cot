use cot::router::{Route, Router};

async fn test(_invalid: ()) -> cot::Result<cot::response::Response> {
    unimplemented!()
}

fn main() {
    let _ = Router::with_urls([Route::with_handler("/", test)]);
}
