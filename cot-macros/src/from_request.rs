use darling::Error;
use quote::quote;
use syn::{Data, Field, Fields};

use crate::cot_ident;

pub(super) fn impl_from_request_head_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;
    let cot = cot_ident();

    let (constructor, is_empty) = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => {
                let initializers = fields_named.named.iter().map(|field: &Field| {
                    let field_name = &field.ident;
                    let field_type = &field.ty;
                    quote! {
                        #field_name: <#field_type as #cot::request::extractors::FromRequestHead>::from_request_head(head).await?,
                    }
                });
                (
                    quote! { Self { #(#initializers)* } },
                    fields_named.named.is_empty(),
                )
            }

            Fields::Unnamed(fields_unnamed) => {
                let initializers = fields_unnamed.unnamed.iter().map(|field: &Field| {
                    let field_type = &field.ty;
                    quote! {
                        <#field_type as #cot::request::extractors::FromRequestHead>::from_request_head(head).await?,
                    }
                });
                (
                    quote! { Self(#(#initializers)*) },
                    fields_unnamed.unnamed.is_empty(),
                )
            }

            Fields::Unit => (quote! { Self }, true),
        },
        _ => return Error::custom("Only structs can derive `FromRequestHead`").write_errors(),
    };

    if is_empty {
        quote! {
            #[automatically_derived]
            impl #cot::request::extractors::FromRequestHead for #struct_name {
                fn from_request_head(
                    _head: &#cot::request::RequestHead,
                ) -> impl ::core::future::Future<Output = #cot::Result<Self>> + Send {
                    ::core::future::ready(Ok(#constructor))
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            impl #cot::request::extractors::FromRequestHead for #struct_name {
                async fn from_request_head(
                    head: &#cot::request::RequestHead,
                ) -> #cot::Result<Self> {
                    Ok(#constructor)
                }
            }
        }
    }
}
