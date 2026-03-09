use darling::Error;
use quote::quote;
use syn::{Data, DeriveInput};

use crate::cot_ident;

pub(super) fn impl_select_as_form_field_for_enum(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let enum_name = &ast.ident;
    let cot = cot_ident();

    match &ast.data {
        Data::Enum(_) => {}
        _ => {
            return Error::custom("`SelectAsFormField` can only be derived for enums")
                .write_errors();
        }
    }

    let impl_single = quote! {
        #[automatically_derived]
        impl #cot::form::AsFormField for #enum_name {
            type Type = #cot::form::fields::SelectField<Self>;

            fn clean_value(
                field: &Self::Type
            ) -> ::core::result::Result<Self, #cot::form::FormFieldValidationError> {
                match #cot::form::FormField::value(field) {
                    ::core::option::Option::Some(v) if !v.is_empty() => <Self as #cot::form::fields::SelectChoice>::from_str(v),
                    _ => ::core::result::Result::Err(#cot::form::FormFieldValidationError::Required),
                }
            }

            fn to_field_value(&self) -> ::std::string::String {
                <Self as #cot::form::fields::SelectChoice>::to_string(self)
            }
        }
    };

    quote! {
        #impl_single
    }
}
