use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

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
    let result = quote! {
        #[cot::__private::tokio::test(crate = "cot::__private::tokio")]
        #test_function_decl
    };

    result
}
