use darling::Error;
use quote::quote;
use syn::{Data, Field, Fields};

use crate::cot_ident;

pub(super) fn impl_from_request_head_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    generic_from_request_head_for_struct(ast, ImplTarget::FromRequestHead)
}

pub(super) fn impl_from_error_request_head_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    generic_from_request_head_for_struct(ast, ImplTarget::FromErrorRequestHead)
}

fn generic_from_request_head_for_struct(
    ast: &syn::DeriveInput,
    target: ImplTarget,
) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;
    let cot = cot_ident();

    let trait_path = target.trait_path();

    let constructor = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => {
                let initializers = fields_named.named.iter().map(|field: &Field| {
                    let field_name = &field.ident;
                    let field_type = &field.ty;
                    quote! {
                        #field_name: <#field_type as #trait_path>::from_request_head(head).await?,
                    }
                });
                quote! { Self { #(#initializers)* } }
            }

            Fields::Unnamed(fields_unnamed) => {
                let initializers = fields_unnamed.unnamed.iter().map(|field: &Field| {
                    let field_type = &field.ty;
                    quote! {
                        <#field_type as #trait_path>::from_request_head(head).await?,
                    }
                });
                quote! { Self(#(#initializers)*) }
            }

            Fields::Unit => {
                quote! {
                    Self
                }
            }
        },
        _ => {
            return Error::custom(format!("Only structs can derive `{}`", target.name()))
                .write_errors();
        }
        _ => return Error::custom("Only structs can derive `FromRequestHead`").write_errors(),
    };

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

#[derive(Debug, Copy, Clone)]
enum ImplTarget {
    FromRequestHead,
    FromErrorRequestHead,
}

impl ImplTarget {
    fn name(self) -> &'static str {
        match self {
            ImplTarget::FromRequestHead => "FromRequestHead",
            ImplTarget::FromErrorRequestHead => "FromErrorRequestHead",
        }
    }

    fn trait_path(self) -> proc_macro2::TokenStream {
        let cot = cot_ident();
        match self {
            ImplTarget::FromRequestHead => quote! { #cot::request::extractors::FromRequestHead },
            ImplTarget::FromErrorRequestHead => {
                quote! { #cot::error::handler::FromErrorRequestHead }
            }
        }
    }
}
