mod admin;
mod api_response_enum;
mod cache;
mod dbtest;
mod form;
mod from_request;
mod main_fn;
mod model;
mod query;
mod select_as_form_field;
mod select_choice;

use darling::Error;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use proc_macro_crate::crate_name;
use quote::quote;
use syn::{DeriveInput, ItemFn, parse_macro_input};

use crate::admin::impl_admin_model_for_struct;
use crate::api_response_enum::{impl_api_operation_response_for_enum, impl_into_response_for_enum};
use crate::dbtest::fn_to_dbtest;
use crate::form::impl_form_for_struct;
use crate::from_request::impl_from_request_head_for_struct;
use crate::main_fn::{fn_to_cot_e2e_test, fn_to_cot_main, fn_to_cot_test};
use crate::model::impl_model_for_struct;
use crate::query::{Query, query_to_tokens};
use crate::select_as_form_field::impl_select_as_form_field_for_enum;
use crate::select_choice::impl_select_choice_for_enum;

#[proc_macro_derive(Form, attributes(form))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let token_stream = impl_form_for_struct(&ast);
    token_stream.into()
}

#[proc_macro_derive(AdminModel)]
pub fn derive_admin_model(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let token_stream = impl_admin_model_for_struct(&ast);
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
///     #[model(primary_key)]
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
///     #[model(primary_key)]
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
///     #[model(primary_key)]
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
    let mut ast = parse_macro_input!(input as DeriveInput);
    let token_stream = impl_model_for_struct(&attr_args, &mut ast);
    token_stream.into()
}

// Simple derive macro that doesn't do anything useful but defines the `model`
// helper attribute.
//
// It's used internally by the `model` macro to avoid having to remove the
// `#[model]` attributes from the struct fields. Typically, the Rust compiler
// complains when there are unused attributes, so we define this macro to avoid
// that. This way, we don't have to remove the `#[model]` attributes from the
// struct fields, while other derive macros (such as `AdminModel`) can still use
// them.
#[proc_macro_derive(ModelHelper, attributes(model))]
pub fn derive_model_helper(_item: TokenStream) -> TokenStream {
    TokenStream::new()
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

#[proc_macro_attribute]
pub fn cachetest(_args: TokenStream, input: TokenStream) -> TokenStream {
    let fn_input = parse_macro_input!(input as ItemFn);
    cache::fn_to_cache_test(&fn_input).into()
}

/// An attribute macro that defines an `async` test function for a Cot-powered
/// app.
///
/// This is pretty much an equivalent to `#[tokio::test]` provided so that you
/// don't have to declare `tokio` as a dependency in your tests.
///
/// # Examples
///
/// ```no_run
/// use cot::test::TestDatabase;
///
/// #[cot::test]
/// async fn test_db() {
///     let db = TestDatabase::new_sqlite().await.unwrap();
///     // do something with the database
///     db.cleanup().await.unwrap();
/// }
/// ```
#[proc_macro_attribute]
pub fn test(_args: TokenStream, input: TokenStream) -> TokenStream {
    let fn_input = parse_macro_input!(input as ItemFn);
    fn_to_cot_test(&fn_input).into()
}

#[proc_macro_attribute]
pub fn e2e_test(_args: TokenStream, input: TokenStream) -> TokenStream {
    let fn_input = parse_macro_input!(input as ItemFn);
    fn_to_cot_e2e_test(&fn_input).into()
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

#[proc_macro_derive(FromRequestHead)]
pub fn derive_from_request_head(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let token_stream = impl_from_request_head_for_struct(&ast);
    token_stream.into()
}

#[proc_macro_derive(SelectChoice, attributes(select_choice))]
pub fn derive_select_choice(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);
    let token_stream = impl_select_choice_for_enum(&ast);
    token_stream.into()
}

#[proc_macro_derive(SelectAsFormField)]
pub fn derive_select_as_form_field(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);
    let token_stream = impl_select_as_form_field_for_enum(&ast);
    token_stream.into()
}

#[proc_macro_derive(IntoResponse)]
pub fn derive_into_response(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_into_response_for_enum(&ast).into()
}

#[proc_macro_derive(ApiOperationResponse)]
pub fn derive_api_operation_response(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_api_operation_response_for_enum(&ast).into()
}

/// The `Template` derive macro and its `template()` attribute.
///
/// Please see our [template guide](https://cot.rs/guide/latest/templates/) and [askama's book](
/// https://askama.readthedocs.io/en/stable/creating_templates.html) for more information.
#[proc_macro_derive(Template, attributes(template))]
pub fn derive_template(input: TokenStream) -> TokenStream {
    askama_derive::derive_template(input.into(), import_askama).into()
}

/// A macro attribute to write custom filters for askama templates.
///
/// Please see [askama's book](https://askama.readthedocs.io/en/stable/filters.html#custom-filters)
/// for more information.
#[proc_macro_attribute]
pub fn filter_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    askama_derive::derive_filter_fn(attr.into(), item.into(), import_askama).into()
}

fn import_askama() -> proc_macro2::TokenStream {
    let cot = cot_ident();
    quote!(use #cot::__private::askama;)
}
