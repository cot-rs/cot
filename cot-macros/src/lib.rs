mod dbtest;
mod form;
mod main_fn;
mod model;
mod query;

use darling::ast::NestedMeta;
use darling::Error;
use proc_macro::TokenStream;
use proc_macro_crate::crate_name;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

use crate::dbtest::fn_to_dbtest;
use crate::form::impl_form_for_struct;
use crate::main_fn::fn_to_cot_main;
use crate::model::impl_model_for_struct;
use crate::query::{query_to_tokens, Query};

/// Derive the [`Form`] trait for a struct.
///
/// This macro will generate an implementation of the [`Form`] trait for the
/// given named struct. Note that all the fields of the struct **must**
/// implement the [`AsFormField`] trait.
///
/// [`Form`]: trait.Form.html
/// [`AsFormField`]: trait.AsFormField.html
#[proc_macro_derive(Form, attributes(form))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let token_stream = impl_form_for_struct(&ast);
    token_stream.into()
}

/// Implement the [`Model`] trait for a struct.
///
/// This macro will generate an implementation of the [`Model`] trait for the
/// given named struct. Note that all the fields of the struct **must**
/// implement the [`DatabaseField`] trait.
///
/// # Model types
///
/// The model type can be specified using the `model_type` parameter. The model
/// type can be one of the following:
///
/// * `application` (default): The model represents an actual table in a
///   normally running instance of the application.
/// ```
/// use cot::db::model;
///
/// #[model(model_type = "application")]
/// // This is equivalent to:
/// // #[model]
/// struct User {
///     id: i32,
///     username: String,
/// }
/// ```
/// * `migration`: The model represents a table that is used for migrations. The
///   model name must be prefixed with an underscore. You shouldn't ever need to
///   use this type; the migration engine will generate the migration model
///   types for you.
///
///   Migration models have two major uses. The first is so that the migration
///   engine uses knows what was the state of model at the time the last
///   migration was generated. This allows the engine to automatically detect
///   the changes and generate the necessary migration code. The second use is
///   to allow custom code in the migrations: you might want the migration to
///   fill in some data, for instance. You can't use the actual model for this
///   because the model might have changed since the migration was generated.
///   You can, however, use the migration model, which will always represent
///   the state of the model at the time the migration runs.
/// ```
/// // In a migration file
/// use cot::db::model;
///
/// #[model(model_type = "migration")]
/// struct _User {
///     id: i32,
///     username: String,
/// }
/// ```
/// * `internal`: The model represents a table that is used internally by Cot
///   (e.g. the `cot__migrations` table, storing which migrations have been
///   applied). They are ignored by the migration generator and should never be
///   used outside Cot code.
/// ```
/// use cot::db::model;
///
/// #[model(model_type = "internal")]
/// struct CotMigrations {
///     id: i32,
///     app: String,
///     name: String,
/// }
/// ```
///
/// [`Model`]: trait.Model.html
/// [`DatabaseField`]: trait.DatabaseField.html
#[proc_macro_attribute]
pub fn model(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(Error::from(e).write_errors());
        }
    };
    let mut ast = parse_macro_input!(input as syn::DeriveInput);
    let token_stream = impl_model_for_struct(&attr_args, &mut ast);
    token_stream.into()
}

#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    let query_input = parse_macro_input!(input as Query);
    query_to_tokens(query_input).into()
}

#[proc_macro_attribute]
pub fn dbtest(_args: TokenStream, input: TokenStream) -> TokenStream {
    let fn_input = parse_macro_input!(input as ItemFn);
    fn_to_dbtest(fn_input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn main(_args: TokenStream, input: TokenStream) -> TokenStream {
    let fn_input = parse_macro_input!(input as ItemFn);
    fn_to_cot_main(fn_input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

pub(crate) fn cot_ident() -> proc_macro2::TokenStream {
    let cot_crate = crate_name("cot").expect("cot is not present in `Cargo.toml`");
    match cot_crate {
        proc_macro_crate::FoundCrate::Itself => {
            quote! { ::cot }
        }
        proc_macro_crate::FoundCrate::Name(name) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident }
        }
    }
}