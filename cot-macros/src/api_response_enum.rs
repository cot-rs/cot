use darling::Error;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::cot_ident;

pub(super) fn impl_api_operation_response_for_enum(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &ast.ident;
    let cot = cot_ident();
    let variants = match &ast.data {
        Data::Enum(e) => &e.variants,
        _ => return Error::custom("only enums can derive `ApiOperationResponse`").write_errors(),
    };

    let mut errors = proc_macro2::TokenStream::new();
    let mut arms_into: ::std::vec::Vec<proc_macro2::TokenStream> = ::std::vec::Vec::new();
    let mut arms_api: ::std::vec::Vec<proc_macro2::TokenStream> = ::std::vec::Vec::new();

    for v in variants.iter() {
        let ident = &v.ident;
        match &v.fields {
            Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                let ty = &f
                    .unnamed
                    .first()
                    .expect("exactly one element is guaranteed by match condition")
                    .ty;
                arms_into.push(quote! {
                    Self::#ident(inner) => inner.into_response(),
                });
                arms_api.push(quote! {
                    responses.extend(<#ty as #cot::openapi::ApiOperationResponse>::api_operation_responses(operation, route_context, schema_generator));
                });
            }
            _ => {
                errors.extend(
                    Error::custom("only tuple variants with a single field are supported")
                        .write_errors(),
                );
            }
        }
    }

    if !errors.is_empty() {
        return errors;
    }

    quote! {
        #[automatically_derived]
        impl #cot::response::IntoResponse for #name {
            fn into_response(self) -> #cot::Result<#cot::response::Response> {
                use #cot::response::IntoResponse;
                match self {
                    #(#arms_into)*
                }
            }
        }

        #[automatically_derived]
        impl #cot::openapi::ApiOperationResponse for #name {
            fn api_operation_responses(
                operation: &mut #cot::__private::Operation,
                route_context: &#cot::openapi::RouteContext<'_>,
                schema_generator: &mut #cot::schemars::SchemaGenerator,
            ) -> ::std::vec::Vec<(::core::option::Option<#cot::__private::StatusCode>, #cot::__private::OpenApiResponse)> {
                let mut responses = ::std::vec::Vec::new();
                #(#arms_api)*
                responses
            }
        }
    }
}
