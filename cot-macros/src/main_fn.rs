use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_quote};

use crate::cot_ident;

pub(super) fn fn_to_cot_main(main_function_decl: ItemFn) -> syn::Result<TokenStream> {
    let mut new_main_decl = main_function_decl.clone();
    new_main_decl.sig.ident = syn::Ident::new("__cot_main", main_function_decl.sig.ident.span());

    if !main_function_decl.sig.inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            main_function_decl.sig.inputs,
            "cot::main function must have zero arguments",
        ));
    }

    let crate_name = cot_ident();
    let result = quote! {
        fn main() {
            let body = async {
                let project = __cot_main();
                #crate_name::run_cli(project).await.expect(
                    "failed to run the Cot project"
                );

                #new_main_decl
            };
            #[expect(clippy::expect_used)]
            {
                return #crate_name::__private::tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed building the Runtime")
                    .block_on(body);
            }
        }
    };

    Ok(result)
}

pub(super) fn fn_to_cot_test(test_function_decl: &ItemFn) -> TokenStream {
    let crate_name = cot_ident();
    let tokio_path = quote! { #crate_name::__private::tokio }.to_string();

    quote! {
        #[#crate_name::__private::tokio::test(crate = #tokio_path)]
        #test_function_decl
    }
}

pub(super) fn fn_to_cot_e2e_test(test_function_decl: &ItemFn) -> TokenStream {
    let crate_name = cot_ident();

    let block = test_function_decl.block.clone();
    let mut new_test_fn = test_function_decl.clone();

    new_test_fn.block = parse_quote! {{
        #crate_name::__private::tokio::task::LocalSet::new()
            .run_until(async move {
                #block
            }).await
    }};

    fn_to_cot_test(&new_test_fn)
}
