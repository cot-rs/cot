use darling::Error;
use quote::quote;
use syn::{Data, Field, Fields};

use crate::cot_ident;

pub(super) fn impl_from_request_parts_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    generic_from_request_parts_for_struct(ast, ImplTarget::FromRequestParts)
}

pub(super) fn impl_from_error_request_parts_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    generic_from_request_parts_for_struct(ast, ImplTarget::FromErrorRequestParts)
}

fn generic_from_request_parts_for_struct(
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
                        #field_name: <#field_type as #trait_path>::from_request_parts(parts).await?,
                    }
                });
                quote! { Self { #(#initializers)* } }
            }

            Fields::Unnamed(fields_unnamed) => {
                let initializers = fields_unnamed.unnamed.iter().map(|field: &Field| {
                    let field_type = &field.ty;
                    quote! {
                        <#field_type as #trait_path>::from_request_parts(parts).await?,
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
    };

    quote! {
        #[automatically_derived]
        impl #trait_path for #struct_name {
            async fn from_request_parts(
                parts: &mut #cot::http::request::Parts,
            ) -> #cot::Result<Self> {
                Ok(#constructor)
            }
        }
    }
}

enum ImplTarget {
    FromRequestParts,
    FromErrorRequestParts,
}

impl ImplTarget {
    fn name(&self) -> &'static str {
        match self {
            ImplTarget::FromRequestParts => "FromRequestParts",
            ImplTarget::FromErrorRequestParts => "FromErrorRequestParts",
        }
    }

    fn trait_path(&self) -> proc_macro2::TokenStream {
        let cot = cot_ident();
        match self {
            ImplTarget::FromRequestParts => quote! { #cot::request::extractors::FromRequestParts },
            ImplTarget::FromErrorRequestParts => {
                quote! { #cot::error::handler::FromErrorRequestParts }
            }
        }
    }
}
