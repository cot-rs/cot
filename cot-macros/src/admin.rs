use cot_codegen::model::FieldOpts;
use darling::FromDeriveInput;
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use crate::cot_ident;

pub(super) fn impl_admin_model_for_struct(ast: &syn::DeriveInput) -> TokenStream {
    let opts = match AdminModelOpts::from_derive_input(ast) {
        Ok(val) => val,
        Err(err) => {
            return err.write_errors();
        }
    };

    let mut builder = opts.as_form_derive_builder();
    for field in opts.fields() {
        builder.push_field(field);
    }

    quote!(#builder)
}

#[derive(Debug, FromDeriveInput)]
#[darling(forward_attrs(allow, doc, cfg), supports(struct_named))]
struct AdminModelOpts {
    ident: syn::Ident,
    data: darling::ast::Data<darling::util::Ignored, FieldOpts>,
}

impl AdminModelOpts {
    fn fields(&self) -> Vec<&FieldOpts> {
        self.data
            .as_ref()
            .take_struct()
            .expect("Only structs are supported")
            .fields
    }

    fn as_form_derive_builder(&self) -> AdminModelDeriveBuilder {
        AdminModelDeriveBuilder {
            name: self.ident.clone(),
            primary_key: None,
        }
    }
}

#[derive(Debug)]
struct AdminModelDeriveBuilder {
    name: syn::Ident,
    primary_key: Option<FieldOpts>,
}

impl ToTokens for AdminModelDeriveBuilder {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let admin_context_impl = self.build_admin_model_impl();

        let new_tokens = quote! {
            const _: () = {
                #admin_context_impl
            };
        };

        new_tokens.to_tokens(tokens);
    }
}

impl AdminModelDeriveBuilder {
    fn push_field(&mut self, field: &FieldOpts) {
        if field.primary_key.is_present() {
            self.primary_key = Some(field.clone());
        }
    }

    #[allow(clippy::too_many_lines)] // it's mostly the AdminModel impl
    fn build_admin_model_impl(&self) -> TokenStream {
        let crate_ident = cot_ident();

        let name = &self.name;
        let name_slug = name.to_string().to_snake_case();

        let pk_name = if let Some(primary_key) = &self.primary_key {
            primary_key
                .ident
                .clone()
                .expect("Only structs are supported")
        } else {
            return syn::Error::new(
                self.name.span(),
                "models must have a primary key field annotated with \
            the `#[model(primary_key)]` attribute",
            )
            .into_compile_error();
        };

        quote! {
            #[#crate_ident::__private::async_trait]
            impl #crate_ident::admin::AdminModel for #name {
                fn as_any(&self) -> &dyn ::core::any::Any {
                    self
                }

                async fn get_total_object_counts(
                    request: &#crate_ident::request::Request,
                ) -> #crate_ident::Result<u64> {
                    use #crate_ident::db::Model;
                    use #crate_ident::request::RequestExt;

                    Ok(Self::objects().count(request.db()).await?)
                }

                async fn get_objects(
                    request: &#crate_ident::request::Request,
                    pagination: #crate_ident::admin::Pagination,
                ) -> #crate_ident::Result<::std::vec::Vec<Self>> {
                    use #crate_ident::db::Model;
                    use #crate_ident::request::RequestExt;

                    Ok(Self::objects().limit(pagination.limit()).offset(pagination.offset()).all(request.db()).await?)
                }

                async fn get_object_by_id(
                    request: &#crate_ident::request::Request,
                    id: &str,
                ) -> #crate_ident::Result<::core::option::Option<Self>>
                where
                    Self: Sized,
                {
                    use #crate_ident::request::RequestExt;

                    let id = parse_id::<Self>(id)?;

                    Ok(#crate_ident::db::query!(Self, $#pk_name == id).get(request.db()).await?)
                }

                fn name() -> &'static str {
                    stringify!(#name)
                }

                fn url_name() -> &'static str {
                    #name_slug
                }

                fn id(&self) -> ::std::string::String {
                    use ::std::string::ToString;

                    <Self as #crate_ident::db::Model>::primary_key(self).to_string()
                }

                fn display(&self) -> ::std::string::String {
                    ::std::format!("{self}")
                }

                fn form_context() -> ::std::boxed::Box<dyn #crate_ident::form::FormContext>
                where
                    Self: Sized,
                {
                    ::std::boxed::Box::new(<Self as #crate_ident::form::Form>::Context::new())
                }

                fn form_context_from_self(&self) -> ::std::boxed::Box<dyn #crate_ident::form::FormContext> {
                    ::std::boxed::Box::new(<Self as Form>::to_context(self))
                }

                async fn save_from_request(
                    request: &mut #crate_ident::request::Request,
                    object_id: ::core::option::Option<&str>,
                ) -> #crate_ident::Result<::core::option::Option<::std::boxed::Box<dyn #crate_ident::form::FormContext>>>
                where
                    Self: Sized,
                {
                    use #crate_ident::form::Form;
                    use #crate_ident::request::RequestExt;

                    let form_result = <Self as #crate_ident::form::Form>::from_request(request).await?;
                    match form_result {
                        #crate_ident::form::FormResult::Ok(mut object_from_form) => {
                            if let Some(object_id) = object_id {
                                let id = parse_id::<Self>(object_id)?;

                                object_from_form.set_primary_key(id);
                                object_from_form.update(request.db()).await?;
                            } else {
                                object_from_form.insert(request.db()).await?;
                            }
                            ::std::result::Result::Ok(None)
                        }
                        #crate_ident::form::FormResult::ValidationError(context) => ::std::result::Result::Ok(
                            ::core::option::Option::Some(::std::boxed::Box::new(context)),
                        ),
                    }
                }

                async fn remove_by_id(
                    request: &mut #crate_ident::request::Request,
                    object_id: &str,
                ) -> #crate_ident::Result<()>
                where
                    Self: Sized,
                {
                    use #crate_ident::request::RequestExt;

                    let id = parse_id::<Self>(object_id)?;

                    #crate_ident::db::query!(Self, $#pk_name == id).delete(request.db()).await?;

                    Ok(())
                }
            }

            fn parse_id<T>(id: &str) -> #crate_ident::Result<<T as #crate_ident::db::Model>::PrimaryKey>
            where
                T: #crate_ident::db::Model,
                <T as #crate_ident::db::Model>::PrimaryKey: ::std::str::FromStr,
            {
                use ::std::str::FromStr;

                <T as #crate_ident::db::Model>::PrimaryKey::from_str(id).map_err(|_| {
                    #crate_ident::Error::admin(::std::format!(
                        "Invalid ID for {model_name}: `{id}`",
                        model_name = stringify!(#name)
                    ))
                })
            }
        }
    }
}
