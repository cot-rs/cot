use std::collections::HashMap;
use std::iter::FromIterator;

use quote::format_ident;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolResolver {
    /// List of imports in the format `"HashMap" -> VisibleSymbol`
    symbols: HashMap<String, VisibleSymbol>,
}

impl SymbolResolver {
    #[must_use]
    pub fn new(symbols: Vec<VisibleSymbol>) -> Self {
        let mut symbol_map = HashMap::new();
        for symbol in symbols {
            symbol_map.insert(symbol.alias.clone(), symbol);
        }

        Self {
            symbols: symbol_map,
        }
    }

    pub fn resolve_struct(&self, item: &mut syn::ItemStruct) {
        for field in &mut item.fields {
            self.resolve(&mut field.ty, Some(&item.ident.to_string()));
        }
    }

    pub fn resolve(&self, ty: &mut syn::Type, self_reference: Option<&String>) {
        if let syn::Type::Path(path) = ty {
            self.resolve_type_path(path, self_reference);
        }
    }

    /// Checks the provided `TypePath` and resolves the full type path, if
    /// available.
    fn resolve_type_path(&self, path: &mut syn::TypePath, self_reference: Option<&String>) {
        let first_segment = path.path.segments.first();

        if let Some(first_segment) = first_segment {
            let mut ident = first_segment.ident.to_string();
            if ident == "Self"
                && let Some(self_reference) = self_reference
            {
                ident.clone_from(self_reference);
            }
            if let Some(symbol) = self.symbols.get(&ident) {
                let mut new_segments: Vec<_> = symbol
                    .full_path_parts()
                    .map(|s| syn::PathSegment {
                        ident: format_ident!("{}", s, span = first_segment.ident.span()),
                        arguments: syn::PathArguments::None,
                    })
                    .collect();

                let first_arguments = first_segment.arguments.clone();
                new_segments
                    .last_mut()
                    .expect("new_segments must have at least one element")
                    .arguments = first_arguments;

                new_segments.extend(path.path.segments.iter().skip(1).cloned());
                path.path.segments = syn::punctuated::Punctuated::from_iter(new_segments);
            }

            for segment in &mut path.path.segments {
                self.resolve_path_arguments(&mut segment.arguments, self_reference);
            }
        }
    }

    fn resolve_path_arguments(
        &self,
        arguments: &mut syn::PathArguments,
        self_reference: Option<&String>,
    ) {
        if let syn::PathArguments::AngleBracketed(args) = arguments {
            for arg in &mut args.args {
                self.resolve_generic_argument(arg, self_reference);
            }
        }
    }

    fn resolve_generic_argument(
        &self,
        arg: &mut syn::GenericArgument,
        self_reference: Option<&String>,
    ) {
        if let syn::GenericArgument::Type(syn::Type::Path(path)) = arg {
            if let Some(new_arg) = self.try_resolve_generic_const(path) {
                *arg = new_arg;
            } else {
                self.resolve_type_path(path, self_reference);
            }
        }
    }

    fn try_resolve_generic_const(&self, path: &syn::TypePath) -> Option<syn::GenericArgument> {
        if path.qself.is_none() && path.path.segments.len() == 1 {
            let segment = path
                .path
                .segments
                .first()
                .expect("segments have exactly one element");
            if segment.arguments.is_none() {
                let ident = segment.ident.to_string();
                if let Some(symbol) = self.symbols.get(&ident)
                    && symbol.kind == VisibleSymbolKind::Const
                {
                    let path = &symbol.full_path;
                    return Some(syn::GenericArgument::Const(
                        syn::parse_str(path).expect("full_path should be a valid path"),
                    ));
                }
            }
        }

        None
    }
}

/// Represents a symbol visible in the current module. This might mean there is
/// a `use` statement for a given type, but also, for instance, the type is
/// defined in the current module.
///
/// For instance, for `use std::collections::HashMap;` the `VisibleSymbol`
/// would be:
/// ```
/// use cot_codegen::symbol_resolver::{VisibleSymbol, VisibleSymbolKind};
///
/// let _ = VisibleSymbol {
///     alias: String::from("HashMap"),
///     full_path: String::from("std::collections::HashMap"),
///     kind: VisibleSymbolKind::Use,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleSymbol {
    pub alias: String,
    pub full_path: String,
    pub kind: VisibleSymbolKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VisibleSymbolKind {
    Use,
    Struct,
    Enum,
    TypeAlias,
    Const,
}

impl VisibleSymbol {
    #[must_use]
    pub fn new(alias: &str, full_path: &str, kind: VisibleSymbolKind) -> Self {
        assert_ne!(alias, "", "alias must not be empty");
        assert!(!alias.contains("::"), "alias must not contain '::'");
        Self {
            alias: alias.to_string(),
            full_path: full_path.to_string(),
            kind,
        }
    }

    fn full_path_parts(&self) -> impl Iterator<Item = &str> {
        self.full_path.split("::")
    }
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens, quote};
    use syn::parse_quote;

    use super::*;
    use crate::symbol_resolver::VisibleSymbolKind;

    #[test]
    fn import_resolver() {
        let resolver = SymbolResolver::new(vec![
            VisibleSymbol::new("MyType", "crate::models::MyType", VisibleSymbolKind::Use),
            VisibleSymbol::new(
                "HashMap",
                "std::collections::HashMap",
                VisibleSymbolKind::Use,
            ),
        ]);

        let path = &mut parse_quote!(MyType);
        resolver.resolve_type_path(path, None);
        assert_eq!(
            quote!(crate::models::MyType).to_string(),
            path.into_token_stream().to_string()
        );

        let path = &mut parse_quote!(HashMap<String, u8>);
        resolver.resolve_type_path(path, None);
        assert_eq!(
            quote!(std::collections::HashMap<String, u8>).to_string(),
            path.into_token_stream().to_string()
        );

        let path = &mut parse_quote!(Option<MyType>);
        resolver.resolve_type_path(path, None);
        assert_eq!(
            quote!(Option<crate::models::MyType>).to_string(),
            path.into_token_stream().to_string()
        );
    }

    #[test]
    fn import_resolver_resolve_struct() {
        let resolver = SymbolResolver::new(vec![
            VisibleSymbol::new("MyType", "crate::models::MyType", VisibleSymbolKind::Use),
            VisibleSymbol::new(
                "HashMap",
                "std::collections::HashMap",
                VisibleSymbolKind::Use,
            ),
            VisibleSymbol::new(
                "LimitedString",
                "cot::db::LimitedString",
                VisibleSymbolKind::Use,
            ),
            VisibleSymbol::new(
                "MY_CONSTANT",
                "crate::constants::MY_CONSTANT",
                VisibleSymbolKind::Const,
            ),
        ]);

        let mut actual = parse_quote! {
            struct Example {
                field_1: MyType,
                field_2: HashMap<String, MyType>,
                field_3: Option<String>,
                field_4: LimitedString<MY_CONSTANT>,
            }
        };
        resolver.resolve_struct(&mut actual);
        let expected = quote! {
            struct Example {
                field_1: crate::models::MyType,
                field_2: std::collections::HashMap<String, crate::models::MyType>,
                field_3: Option<String>,
                field_4: cot::db::LimitedString<{ crate::constants::MY_CONSTANT }>,
            }
        };
        assert_eq!(actual.into_token_stream().to_string(), expected.to_string());
    }

    #[test]
    fn import_resolver_resolve_struct_with_self() {
        let resolver = SymbolResolver::new(vec![
            VisibleSymbol::new("ForeignKey", "cot::db::ForeignKey", VisibleSymbolKind::Use),
            VisibleSymbol::new("MyModel", "crate::MyModel", VisibleSymbolKind::Struct),
        ]);

        let mut actual = parse_quote! {
            struct MyModel {
                foreign_key: ForeignKey<Self>,
            }
        };
        resolver.resolve_struct(&mut actual);
        let expected = quote! {
            struct MyModel {
                foreign_key: cot::db::ForeignKey<crate::MyModel>,
            }
        };
        assert_eq!(actual.into_token_stream().to_string(), expected.to_string());
    }
}
