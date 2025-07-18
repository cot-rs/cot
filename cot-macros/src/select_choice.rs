use darling::{Error, FromVariant};
use quote::quote;
use syn::{Data, DeriveInput};

use crate::cot_ident;

#[derive(FromVariant, Debug)]
#[darling(attributes(select_choice))]
struct SelectChoiceVariant {
    ident: syn::Ident,
    #[darling(default)]
    id: Option<String>,
    #[darling(default)]
    name: Option<String>,
}

pub(super) fn impl_select_choice_for_enum(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let enum_name = &ast.ident;
    let cot = cot_ident();

    let variants = match &ast.data {
        Data::Enum(data_enum) => &data_enum.variants,
        _ => return Error::custom("`SelectChoice` can only be derived for enums").write_errors(),
    };

    if variants.is_empty() {
        return Error::custom("`SelectChoice` cannot be derived for empty enums").write_errors();
    }

    for variant in variants {
        if !variant.fields.is_empty() {
            return Error::custom(
                "`SelectChoice` can only be derived for enums with unit variants",
            )
            .with_span(&variant)
            .write_errors();
        }
    }

    // Parse variants using darling
    let darling_variants: Vec<SelectChoiceVariant> = match variants
        .iter()
        .map(SelectChoiceVariant::from_variant)
        .collect::<Result<_, _>>()
    {
        Ok(vs) => vs,
        Err(e) => return e.write_errors(),
    };

    // default_choices
    let variant_idents: Vec<_> = darling_variants.iter().map(|v| &v.ident).collect();

    // from_str
    let from_str_match_arms = darling_variants.iter().map(|v| {
        let ident = &v.ident;
        let id = v.id.clone().unwrap_or_else(|| ident.to_string());
        let id_lit = syn::LitStr::new(&id, proc_macro2::Span::call_site());
        quote! { #id_lit => Ok(Self::#ident), }
    });

    // id
    let id_match_arms = darling_variants.iter().map(|v| {
        let ident = &v.ident;
        let id = v.id.clone().unwrap_or_else(|| ident.to_string());
        quote! { Self::#ident => #id, }
    });

    // to_string
    let to_string_match_arms = darling_variants.iter().map(|v| {
        let ident = &v.ident;
        let display = v.name.clone().unwrap_or_else(|| ident.to_string());
        quote! { Self::#ident => #display, }
    });

    quote! {
        #[automatically_derived]
        impl #cot::form::fields::SelectChoice for #enum_name {
            fn default_choices() -> ::std::vec::Vec<Self> {
                ::std::vec![ #(Self::#variant_idents),* ]
            }

            fn from_str(
                s: &::std::primitive::str
            ) -> ::core::result::Result<Self, #cot::form::FormFieldValidationError> {
                match s {
                    #( #from_str_match_arms )*
                    _ => ::core::result::Result::Err(
                        #cot::form::FormFieldValidationError::invalid_value(
                            ::std::string::String::from(s)
                        ),
                    ),
                }
            }

            fn id(&self) -> ::std::string::String {
                ::std::string::ToString::to_string(&match self {
                    #( #id_match_arms )*
                })
            }

            fn to_string(&self) -> ::std::string::String {
                ::std::string::ToString::to_string(&match self {
                    #( #to_string_match_arms )*
                })
            }
        }
    }
}
