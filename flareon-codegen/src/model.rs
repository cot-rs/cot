use convert_case::{Case, Casing};
use darling::{FromDeriveInput, FromField, FromMeta};
use syn::spanned::Spanned;

use crate::maybe_unknown::MaybeUnknown;
use crate::symbol_resolver::SymbolResolver;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default, FromMeta)]
pub struct ModelArgs {
    #[darling(default)]
    pub model_type: ModelType,
    pub table_name: Option<String>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default, FromMeta)]
pub enum ModelType {
    #[default]
    Application,
    Migration,
    Internal,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(forward_attrs(allow, doc, cfg), supports(struct_named))]
pub struct ModelOpts {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: darling::ast::Data<darling::util::Ignored, FieldOpts>,
}

impl ModelOpts {
    pub fn new_from_derive_input(input: &syn::DeriveInput) -> Result<Self, darling::error::Error> {
        let opts = Self::from_derive_input(input)?;
        if !opts.generics.params.is_empty() {
            return Err(
                darling::Error::custom("generics in models are not supported")
                    .with_span(&opts.generics),
            );
        }
        Ok(opts)
    }

    /// Get the fields of the struct.
    ///
    /// # Panics
    ///
    /// Panics if the [`ModelOpts`] was not parsed from a struct.
    #[must_use]
    pub fn fields(&self) -> Vec<&FieldOpts> {
        self.data
            .as_ref()
            .take_struct()
            .expect("Only structs are supported")
            .fields
    }

    /// Convert the model options into a model.
    ///
    /// # Errors
    ///
    /// Returns an error if the model name does not start with an underscore
    /// when the model type is [`ModelType::Migration`].
    pub fn as_model(
        &self,
        args: &ModelArgs,
        symbol_resolver: Option<&SymbolResolver>,
    ) -> Result<Model, syn::Error> {
        let fields = self
            .fields()
            .iter()
            .map(|field| field.as_field(symbol_resolver))
            .collect::<Result<Vec<_>, _>>()?;

        let mut original_name = self.ident.to_string();
        if args.model_type == ModelType::Migration {
            original_name = original_name
                .strip_prefix("_")
                .ok_or_else(|| {
                    syn::Error::new(
                        self.ident.span(),
                        "migration model names must start with an underscore",
                    )
                })?
                .to_string();
        }
        let table_name = if let Some(table_name) = &args.table_name {
            table_name.clone()
        } else {
            original_name.to_string().to_case(Case::Snake)
        };

        let primary_key_field = self.get_primary_key_field(&fields)?;

        let ty = match symbol_resolver {
            Some(symbol_resolver) => {
                let mut ty = syn::Type::Path(syn::TypePath {
                    qself: None,
                    path: syn::Path::from(self.ident.clone()),
                });
                symbol_resolver.resolve(&mut ty);
                Some(ty)
            }
            None => None,
        };

        Ok(Model {
            name: self.ident.clone(),
            original_name,
            resolved_ty: ty,
            model_type: args.model_type,
            table_name,
            pk_field: primary_key_field.clone(),
            fields,
        })
    }

    fn get_primary_key_field<'a>(&self, fields: &'a [Field]) -> Result<&'a Field, syn::Error> {
        let pks: Vec<_> = fields.iter().filter(|field| field.primary_key).collect();
        if pks.is_empty() {
            return Err(syn::Error::new(
                self.ident.span(),
                "models must have a primary key field, either named `id` \
                or annotated with the `#[model(primary_key)]` attribute",
            ));
        }
        if pks.len() > 1 {
            return Err(syn::Error::new(
                pks[1].field_name.span(),
                "composite primary keys are not supported; only one primary key field is allowed",
            ));
        }

        Ok(pks[0])
    }
}

#[derive(Debug, Clone, FromField)]
#[darling(attributes(model))]
pub struct FieldOpts {
    pub ident: Option<syn::Ident>,
    pub ty: syn::Type,
    pub primary_key: darling::util::Flag,
    pub unique: darling::util::Flag,
}

impl FieldOpts {
    fn find_type(&self, type_to_find: &str, symbol_resolver: &SymbolResolver) -> Option<syn::Type> {
        let mut ty = self.ty.clone();
        symbol_resolver.resolve(&mut ty);
        Self::find_type_resolved(&ty, type_to_find)
    }

    fn find_type_resolved(ty: &syn::Type, type_to_find: &str) -> Option<syn::Type> {
        if let syn::Type::Path(type_path) = ty {
            let name = type_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");

            if name == type_to_find {
                return Some(ty.clone());
            }

            for arg in &type_path.path.segments {
                if let syn::PathArguments::AngleBracketed(arg) = &arg.arguments {
                    if let Some(ty) = Self::find_type_in_generics(arg, type_to_find) {
                        return Some(ty);
                    }
                }
            }
        }

        None
    }

    fn find_type_in_generics(
        arg: &syn::AngleBracketedGenericArguments,
        type_to_find: &str,
    ) -> Option<syn::Type> {
        arg.args
            .iter()
            .filter_map(|arg| {
                if let syn::GenericArgument::Type(ty) = arg {
                    Self::find_type_resolved(ty, type_to_find)
                } else {
                    None
                }
            })
            .next()
    }

    /// Convert the field options into a field.
    ///
    /// # Panics
    ///
    /// Panics if the field does not have an identifier (i.e. it is a tuple
    /// struct).
    pub fn as_field(&self, symbol_resolver: Option<&SymbolResolver>) -> Result<Field, syn::Error> {
        let name = self.ident.as_ref().unwrap();
        let column_name = name.to_string();

        let (auto_value, foreign_key) = match symbol_resolver {
            Some(resolver) => (
                MaybeUnknown::Known(self.find_type("flareon::db::Auto", resolver).is_some()),
                MaybeUnknown::Known(
                    self.find_type("flareon::db::ForeignKey", resolver)
                        .map(ForeignKeySpec::try_from)
                        .transpose()?,
                ),
            ),
            None => (MaybeUnknown::Unknown, MaybeUnknown::Unknown),
        };
        let is_primary_key = column_name == "id" || self.primary_key.is_present();

        Ok(Field {
            field_name: name.clone(),
            column_name,
            ty: self.ty.clone(),
            auto_value,
            primary_key: is_primary_key,
            foreign_key,
            unique: self.unique.is_present(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Model {
    pub name: syn::Ident,
    pub original_name: String,
    /// The type of the model, or [`None`] if the symbol resolver was not
    /// enabled.
    pub resolved_ty: Option<syn::Type>,
    pub model_type: ModelType,
    pub table_name: String,
    pub pk_field: Field,
    pub fields: Vec<Field>,
}

impl Model {
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    pub field_name: syn::Ident,
    pub column_name: String,
    pub ty: syn::Type,
    /// Whether the field is an auto field (e.g. `id`);
    /// [`MaybeUnknown::Unknown`] if this `Field` instance was not resolved with
    /// a [`SymbolResolver`].
    pub auto_value: MaybeUnknown<bool>,
    pub primary_key: bool,
    /// [`Some`] wrapped in [`MaybeUnknown::Known`] if this field is a
    /// foreign key; [`None`] wrapped in [`MaybeUnknown::Known`] if this
    /// field is determined not to be a foreign key; [`MaybeUnknown::Unknown`]
    /// if this `Field` instance was not resolved with a [`SymbolResolver`].
    pub foreign_key: MaybeUnknown<Option<ForeignKeySpec>>,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForeignKeySpec {
    pub to_model: syn::Type,
}

impl TryFrom<syn::Type> for ForeignKeySpec {
    type Error = syn::Error;

    fn try_from(ty: syn::Type) -> Result<Self, Self::Error> {
        let type_path = if let syn::Type::Path(type_path) = &ty {
            type_path
        } else {
            panic!("Expected a path type for a foreign key");
        };

        let args = if let syn::PathArguments::AngleBracketed(args) = &type_path
            .path
            .segments
            .last()
            .expect("type path must have at least one segment")
            .arguments
        {
            args
        } else {
            return Err(syn::Error::new(
                ty.span(),
                "expected ForeignKey to have angle-bracketed generic arguments",
            ));
        };

        if args.args.len() != 1 {
            return Err(syn::Error::new(
                ty.span(),
                "expected ForeignKey to have only one generic parameter",
            ));
        }

        let inner = &args.args[0];
        if let syn::GenericArgument::Type(ty) = inner {
            Ok(Self {
                to_model: ty.clone(),
            })
        } else {
            Err(syn::Error::new(
                ty.span(),
                "expected ForeignKey to have a type generic argument",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;
    #[cfg(feature = "symbol-resolver")]
    use crate::symbol_resolver::{VisibleSymbol, VisibleSymbolKind};

    #[test]
    fn model_args_default() {
        let args: ModelArgs = Default::default();
        assert_eq!(args.model_type, ModelType::Application);
        assert!(args.table_name.is_none());
    }

    #[test]
    fn model_type_default() {
        let model_type: ModelType = Default::default();
        assert_eq!(model_type, ModelType::Application);
    }

    #[test]
    fn model_opts_fields() {
        let input: syn::DeriveInput = parse_quote! {
            struct TestModel {
                id: i32,
                name: String,
            }
        };
        let opts = ModelOpts::new_from_derive_input(&input).unwrap();
        let fields = opts.fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].ident.as_ref().unwrap().to_string(), "id");
        assert_eq!(fields[1].ident.as_ref().unwrap().to_string(), "name");
    }

    #[test]
    fn model_opts_as_model() {
        let input: syn::DeriveInput = parse_quote! {
            struct TestModel {
                id: i32,
                name: String,
            }
        };
        let opts = ModelOpts::new_from_derive_input(&input).unwrap();
        let args = ModelArgs::default();
        let model = opts.as_model(&args, None).unwrap();
        assert_eq!(model.name.to_string(), "TestModel");
        assert_eq!(model.table_name, "test_model");
        assert_eq!(model.fields.len(), 2);
        assert_eq!(model.field_count(), 2);
    }

    #[test]
    fn model_opts_as_model_migration() {
        let input: syn::DeriveInput = parse_quote! {
            #[model(model_type = "migration")]
            struct TestModel {
                id: i32,
                name: String,
            }
        };
        let opts = ModelOpts::new_from_derive_input(&input).unwrap();
        let args = ModelArgs::from_meta(&input.attrs.first().unwrap().meta).unwrap();
        let err = opts.as_model(&args, None).unwrap_err();
        assert_eq!(
            err.to_string(),
            "migration model names must start with an underscore"
        );
    }

    #[test]
    fn model_opts_as_model_pk_attr() {
        let input: syn::DeriveInput = parse_quote! {
            #[model]
            struct TestModel {
                #[model(primary_key)]
                name: i32,
            }
        };
        let opts = ModelOpts::new_from_derive_input(&input).unwrap();
        let args = ModelArgs::default();
        let model = opts.as_model(&args, None).unwrap();
        assert_eq!(model.fields.len(), 1);
        assert!(model.fields[0].primary_key);
    }

    #[test]
    fn model_opts_as_model_no_pk() {
        let input: syn::DeriveInput = parse_quote! {
            #[model]
            struct TestModel {
                name: String,
            }
        };
        let opts = ModelOpts::new_from_derive_input(&input).unwrap();
        let args = ModelArgs::default();
        let err = opts.as_model(&args, None).unwrap_err();
        assert_eq!(
            err.to_string(),
            "models must have a primary key field, either named `id` \
            or annotated with the `#[model(primary_key)]` attribute"
        );
    }

    #[test]
    fn model_opts_as_model_multiple_pks() {
        let input: syn::DeriveInput = parse_quote! {
            #[model]
            struct TestModel {
                id: i64,
                #[model(primary_key)]
                id_2: i64,
                name: String,
            }
        };
        let opts = ModelOpts::new_from_derive_input(&input).unwrap();
        let args = ModelArgs::default();
        let err = opts.as_model(&args, None).unwrap_err();
        assert_eq!(
            err.to_string(),
            "composite primary keys are not supported; only one primary key field is allowed"
        );
    }

    #[test]
    fn field_opts_as_field() {
        let input: syn::Field = parse_quote! {
            #[model(unique)]
            name: String
        };
        let field_opts = FieldOpts::from_field(&input).unwrap();
        let field = field_opts.as_field(None).unwrap();
        assert_eq!(field.field_name.to_string(), "name");
        assert_eq!(field.column_name, "name");
        assert_eq!(field.ty, parse_quote!(String));
        assert!(field.unique);
        assert_eq!(field.auto_value, MaybeUnknown::Unknown);
        assert_eq!(field.foreign_key, MaybeUnknown::Unknown);
    }

    #[test]
    fn find_type_resolved() {
        let input: syn::Type =
            parse_quote! { ::my_crate::MyContainer<'a, Vec<std::string::String>> };
        assert!(FieldOpts::find_type_resolved(&input, "my_crate::MyContainer").is_some());
        assert!(FieldOpts::find_type_resolved(&input, "Vec").is_some());
        assert!(FieldOpts::find_type_resolved(&input, "std::string::String").is_some());
        assert!(FieldOpts::find_type_resolved(&input, "OtherType").is_none());
    }

    #[cfg(feature = "symbol-resolver")]
    #[test]
    fn find_type() {
        let symbols = vec![VisibleSymbol::new(
            "MyContainer",
            "my_crate::MyContainer",
            VisibleSymbolKind::Use,
        )];
        let resolver = SymbolResolver::new(symbols);

        let opts = FieldOpts {
            ident: None,
            ty: parse_quote! { MyContainer<std::string::String> },
            primary_key: Default::default(),
            unique: Default::default(),
        };

        assert!(opts.find_type("my_crate::MyContainer", &resolver).is_some());
        assert!(opts.find_type("std::string::String", &resolver).is_some());
        assert!(opts.find_type("MyContainer", &resolver).is_none());
        assert!(opts.find_type("String", &resolver).is_none());
    }
}
