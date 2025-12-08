//! Running the project's `main` function, with optional hot-patching support
//! based on [`subsecond`].

use std::future::Future;

/// Runs the project's `main` function in a Tokio runtime.
///
/// If the `live-reload` feature is enabled and the project is run with
/// `dx serve --hot-patch`, the entire `main` function is restarted every time
/// a patch is applied. This way, everything built on startup (routes,
/// middleware, config, etc.) uses the new code.
pub fn run_main<F, Fut>(main: F)
where
    F: Fn() -> Fut + Copy + 'static,
    Fut: Future<Output = ()> + 'static,
{
    #[cfg(feature = "live-reload")]
    run_main_hot_patched(main);

    #[cfg(not(feature = "live-reload"))]
    build_runtime().block_on(main());
}

#[cfg(feature = "live-reload")]
fn run_main_hot_patched<F, Fut>(main: F)
where
    F: Fn() -> Fut + Copy + 'static,
    Fut: Future<Output = ()> + 'static,
{
    use std::pin::Pin;
    use std::sync::Arc;

    // `subsecond` treats pointer-sized functions as function pointers, so a
    // non-ZST `main` would never be patched
    const { assert!(size_of::<F>() == 0, "`main` must be zero-sized") };

    dioxus_devtools::connect_subsecond();
    let patched = Arc::new(tokio::sync::Notify::new());
    subsecond::register_handler(Arc::new({
        let patched = Arc::clone(&patched);
        move || patched.notify_one()
    }));

    loop {
        let runtime = build_runtime();
        let restart = runtime.block_on(async {
            // The future must be boxed inside the hot-patched function, so that
            // its vtable (and hence `poll`) comes from the new code
            let mut hot_main =
                subsecond::HotFn::current(move || -> Pin<Box<dyn Future<Output = ()>>> {
                    Box::pin(main())
                });
            tokio::select! {
                () = hot_main.call(()) => false,
                () = patched.notified() => true,
            }
        });
        // Dropping the runtime also drops all the tasks spawned by the old code
        // (such as open connections, which makes live reload refresh the page)
        drop(runtime);

        if !restart {
            break;
        }
        tracing::info!("hot-patch applied; restarting the project");
    }
}

#[expect(clippy::expect_used)]
fn build_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed building the Tokio runtime")
}
