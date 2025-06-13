use darling::{Error, FromVariant};
use quote::quote;
use syn::{Data, DeriveInput};

use crate::cot_ident;

#[derive(FromVariant, Debug)]
#[darling(attributes(select, select_choice))]
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
        _ => return Error::custom("SelectChoice can only be derived for enums").write_errors(),
    };

    if variants.is_empty() {
        return Error::custom("SelectChoice cannot be derived for empty enums").write_errors();
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
    let default_choices = quote! { vec![ #(Self::#variant_idents),* ] };

    // from_str
    let from_str_match_arms = darling_variants.iter().map(|v| {
        let ident = &v.ident;
        let id =
            v.id.clone()
                .unwrap_or_else(|| ident.to_string().to_lowercase());
        quote! { #id => Ok(Self::#ident), }
    });

    // id
    let id_match_arms = darling_variants.iter().map(|v| {
        let ident = &v.ident;
        let id =
            v.id.clone()
                .unwrap_or_else(|| ident.to_string().to_lowercase());
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
            fn default_choices() -> Vec<Self> {
                #default_choices
            }
            fn from_str(s: &str) -> Result<Self, #cot::form::FormFieldValidationError> {
                match s {
                    #( #from_str_match_arms )*
                    _ => Err(#cot::form::FormFieldValidationError::invalid_value(s.to_owned())),
                }
            }
            fn id(&self) -> String {
                match self {
                    #( #id_match_arms )*
                }.to_string()
            }
            fn to_string(&self) -> String {
                match self {
                    #( #to_string_match_arms )*
                }.to_string()
            }
        }
    }
}
