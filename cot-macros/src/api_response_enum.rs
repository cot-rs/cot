use darling::Error;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::cot_ident;

pub(super) fn impl_api_operation_response_for_enum(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &ast.ident;
    let cot = cot_ident();
    let variants = match &ast.data {
        Data::Enum(e) => &e.variants,
        _ => return Error::custom("Only enums can derive `ApiOperationResponse`").write_errors(),
    };

    let arms_into = variants.iter().map(|v| {
        let ident = &v.ident;
        match &v.fields {
            Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                quote! { #name::#ident(inner) => inner.into_response(), }
            }
            _ => Error::custom("Only tuple variants with a single field are supported")
                .write_errors(),
        }
    });

    let arms_api = variants.iter().map(|v| {
        let ty = match &v.fields {
            Fields::Unnamed(f) if f.unnamed.len() == 1 => &f.unnamed.first().unwrap().ty,
            _ => {
                return Error::custom("Only tuple variants with a single field are supported")
                    .write_errors();
            }
        };
        quote! {
            responses.extend(<#ty as #cot::openapi::ApiOperationResponse>::api_operation_responses(
                operation, route_context, schema_generator
            ));
        }
    });

    quote! {
        #[automatically_derived]
        impl #cot::response::IntoResponse for #name {
            fn into_response(self) -> #cot::Result<#cot::response::Response> {
                match self {
                    #(#arms_into)*
                }
            }
        }

        #[automatically_derived]
        impl #cot::openapi::ApiOperationResponse for #name {
            fn api_operation_responses(
                operation: &mut #cot::openapi::Operation,
                route_context: &#cot::openapi::RouteContext<'_>,
                schema_generator: &mut #cot::schemars::SchemaGenerator,
            ) -> Vec<(Option<#cot::openapi::StatusCode>, #cot::openapi::OpenApiResponse)> {
                let mut responses = Vec::new();
                #(#arms_api)*
                responses
            }
        }
    }
}
