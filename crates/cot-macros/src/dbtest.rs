use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ItemFn;

pub(super) fn fn_to_dbtest(test_function_decl: ItemFn) -> syn::Result<TokenStream> {
    let test_fn = &test_function_decl.sig.ident;
    let sqlite_ident = format_ident!("{}_sqlite", test_fn);
    let postgres_ident = format_ident!("{}_postgres", test_fn);
    let mysql_ident = format_ident!("{}_mysql", test_fn);

    if test_function_decl.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            test_function_decl.sig.inputs,
            "Database test function must have exactly one argument",
        ));
    }

    let result = quote! {
        #[::cot::test]
        #[cfg_attr(miri, ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`")]
        async fn #sqlite_ident() {
            let mut database = cot::test::TestDatabase::new_sqlite()
                .await
                .expect("failed to create SQLite test database");

            #test_fn(&mut database).await;

            database.cleanup().await.expect("failed to clean up SQLite test database");

            #test_function_decl
        }

        #[ignore = "Tests that use PostgreSQL are ignored by default"]
        #[::cot::test]
        async fn #postgres_ident() {
            let mut database = cot::test::TestDatabase::new_postgres(stringify!(#test_fn))
                .await
                .expect("failed to create PostgreSQL test database");

            #test_fn(&mut database).await;

            database.cleanup().await.expect("failed to clean up PostgreSQL test database");

            #test_function_decl
        }

        #[ignore = "Tests that use MySQL are ignored by default"]
        #[::cot::test]
        async fn #mysql_ident() {
            let mut database = cot::test::TestDatabase::new_mysql(stringify!(#test_fn))
                .await
                .expect("failed to create MySQL test database");

            #test_fn(&mut database).await;

            database.cleanup().await.expect("failed to clean up MySQL test database");

            #test_function_decl
        }
    };
    Ok(result)
}
