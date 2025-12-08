#[allow(
    clippy::allow_attributes,
    reason = "Only happens when hot-patching is enabled"
)]
#[allow(
    clippy::future_not_send,
    reason = "Send not needed; serve/Bootstrapper is run async in a single thread"
)]
#[doc(hidden)]
pub async fn serve<O, F>(callback: impl FnMut() -> F)
where
    F: Future<Output = O> + 'static,
{
    #[cfg(feature = "hot-patching")]
    {
        dioxus_devtools::serve_subsecond(callback).await;
    }

    #[cfg(not(feature = "hot-patching"))]
    {
        let mut callback = callback; // avoid "variable does not need to be mutable" warnings
        callback().await;
    }
}

#[doc(hidden)]
pub fn call_hot<F, A, R>(func: F, args: A) -> R
where
    F: FnMut(A) -> R,
{
    #[cfg(feature = "hot-patching")]
    {
        let mut hot_fn = subsecond::HotFn::current(func);
        hot_fn.call((args,))
    }

    #[cfg(not(feature = "hot-patching"))]
    {
        let mut func = func; // avoid "variable does not need to be mutable" warnings
        func(args)
    }
}
