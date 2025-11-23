use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ItemFn;

pub(super) fn fn_to_cache_test(test_fn: &ItemFn) -> TokenStream {
    let test_fn_name = &test_fn.sig.ident;
    let memory_ident = format_ident!("{}_memory", test_fn_name);
    let redis_ident = format_ident!("{}_redis", test_fn_name);

    let result = quote! {
        #[::cot::test]
        async fn #memory_ident() {
            let mut cache = cot::test::TestCache::new_memory();
            #test_fn_name(&mut cache).await;

            #test_fn
        }


        #[ignore = "Tests that use Redis are ignored by default"]
        #[::cot::test]
        #[cfg(feature="redis")]
        async fn #redis_ident() {
            let mut cache = cot::test::TestCache::new_redis().await.unwrap();

            #test_fn_name(&mut cache).await;

            cache.cleanup().await.unwrap_or_else(|err| panic!("Failed to cleanup: {err:?}"));

            #test_fn
    }
    };
    result
}
