use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

pub(crate) fn fn_to_migration_op(mut item: ItemFn) -> syn::Result<TokenStream> {
    if item.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &item.sig,
            "migration operation must be an `async` function",
        ));
    }

    let block = item.block;
    let ret_type = &item.sig.output;

    let ret_type = match ret_type {
        syn::ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &item.sig,
                "migration operation must return `cot::Result<()>`",
            ));
        }
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    item.sig.asyncness = None;
    item.sig.output = syn::parse_quote! {
        -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = #ret_type> + Send + '_>>
    };

    item.block = syn::parse_quote! {
        {
            Box::pin(async move #block)
        }
    };

    Ok(quote! {
        #item
    })
}
