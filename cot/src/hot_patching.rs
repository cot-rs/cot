use std::panic::AssertUnwindSafe;
use std::pin::Pin;

use subsecond::{HotFn, HotFnPanic};

/// Runs given future with [`subsecond`], dropping the future and re-running it
/// when the code changes.
///
/// When the hot-patching feature is not enabled, the function just runs the
/// future once.
#[allow(
    clippy::allow_attributes,
    reason = "Only happens when hot-patching is enabled"
)]
#[allow(
    clippy::future_not_send,
    reason = "Send not needed; serve/Bootstrapper is run async in a single thread"
)]
pub async fn serve<O, F>(callback: impl FnMut() -> F)
where
    F: Future<Output = O> + 'static,
{
    println!("1");

    #[cfg(feature = "hot-patching")]
    {
        println!("2");
        // dioxus_devtools::serve_subsecond(callback).await;
        dioxus_devtools::connect_subsecond();
        let mut callback = callback;
        callback().await;
    }

    #[cfg(not(feature = "hot-patching"))]
    {
        let mut callback = callback; // avoid "variable does not need to be mutable" warnings
        callback().await;
    }
}

/// Calls the function using [`subsecond::HotFn`].
///
/// This causes the function passed to be hot-reloadable. If the hot-reloading
/// feature is not enabled, the function is called directly.
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

pub fn call_async<F, Fut, A, O>(f: F, args: A) -> Pin<Box<dyn Future<Output = O> + Send>>
where
    F: FnOnce(A) -> Fut,
    Fut: Future<Output = O> + Send + 'static,
{
    // return Box::pin(f(args));

    // For FnOnce, we need to handle this differently since we can only call it once
    // We'll store the closure in an Option and take it when needed
    let mut f_option = Some(f);

    // Create a wrapper function that boxes the future
    let wrapper = move |args| -> Pin<Box<dyn Future<Output = O> + Send>> {
        if let Some(closure) = f_option.take() {
            Box::pin(closure(args))
        } else {
            // This shouldn't happen in normal hot reload scenarios since each
            // hot reload creates a new call_async invocation
            panic!(
                "Hot reload closure already consumed - this indicates a problem with the hot reload system"
            )
        }
    };

    let mut hotfn = HotFn::current(wrapper);
    loop {
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| hotfn.call((args,))));

        // If the call succeeds just return the result, otherwise we try to handle the
        // panic if its our own.
        let err = match res {
            Ok(res) => return res,
            Err(err) => err,
        };

        // If this is our panic then let's handle it, otherwise we just resume unwinding
        let Some(_hot_payload) = err.downcast_ref::<HotFnPanic>() else {
            std::panic::resume_unwind(err);
        };

        // For hot reload with FnOnce, we can't retry with the same closure
        // The hot reload system should create a new function call entirely
        panic!(
            "Hot reload detected but cannot retry with FnOnce closure - hot reload should create new function instance"
        );
    }
}
