use cot::request::Request;
use cot::response::Response;

pub trait ErrorPageHandler {
    fn handle(&self, request: Request) -> impl Future<Output = cot::Result<Response>> + Send;
}

macro_rules! impl_request_handler {
    ($($ty:ident),*) => {
        impl<Func, $($ty,)* Fut, R> ErrorPageHandler for Func
        where
            Func: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            $($ty: FromRequestParts + Send,)*
            Fut: Future<Output = R> + Send,
            R: IntoResponse,
        {
            #[allow(non_snake_case)]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(unused_variables, unused_mut)] // for the case where there are no params
                let (mut parts, _body) = request.into_parts();

                $(
                    let $ty = $ty::from_request_parts(&mut parts).await?;
                )*

                self.clone()($($ty,)*).await.into_response()
            }
        }
    };
}
