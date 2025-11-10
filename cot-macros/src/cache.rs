use darling::ast::NestedMeta;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Error, Expr, ExprLit, ItemFn, Lit, Meta};

pub(super) fn fn_to_cache_test(args: &[NestedMeta], test_fn: &ItemFn) -> TokenStream {
    let test_fn_name = &test_fn.sig.ident;
    let memory_ident = format_ident!("{}_memory", test_fn_name);
    let redis_ident = format_ident!("{}_redis", test_fn_name);

    let mut redis_db = syn::LitStr::new("0", Span::call_site());

    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(nv)) => {
                if nv.path.is_ident("redis_db") {
                    if let Expr::Lit(ExprLit { attrs: _, lit }) = &nv.value {
                        if let Lit::Str(s) = lit {
                            redis_db = s.clone();
                        }
                    }
                }
            }
            other => {
                let err = Error::new(
                    Span::call_site(),
                    format!(
                        "unexpected  argument {other:?}. supported: redis_db = \"<db_number>\""
                    ),
                )
                .to_compile_error();
                return TokenStream::from(err);
            }
        }
    }

    let result = quote! {
        #[::cot::test]
        async fn #memory_ident() {
            let mut cache = cot::test::TestCache::new_memory();
            #test_fn_name(&mut cache).await;

            #test_fn
        }


        #[cfg(feature = "redis")]
        #[::cot::test]
        async fn #redis_ident() {
            let mut cache = cot::test::TestCache::new_redis(#redis_db).unwrap();
            #test_fn_name(&mut cache).await;

            #test_fn
    }
    };
    result
}
