use cot_codegen::expr::Expr;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Token;
use syn::parse::{Parse, ParseStream};

use crate::cot_ident;

#[derive(Debug)]
pub(crate) struct Query {
    model_name: syn::Type,
    _comma: Token![,],
    expr: Expr,
}

impl Parse for Query {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            model_name: input.parse()?,
            _comma: input.parse()?,
            expr: input.parse()?,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub(crate) struct FieldRefMethod {
    name: &'static str,
    arity: u32,
}

const FIELD_REF_METHODS: &[FieldRefMethod] = &[
    FieldRefMethod {
        name: "contains",
        arity: 1,
    },
    FieldRefMethod {
        name: "icontains",
        arity: 1,
    },
    FieldRefMethod {
        name: "starts_with",
        arity: 1,
    },
    FieldRefMethod {
        name: "istarts_with",
        arity: 1,
    },
    FieldRefMethod {
        name: "ends_with",
        arity: 1,
    },
    FieldRefMethod {
        name: "iends_with",
        arity: 1,
    },
    FieldRefMethod {
        name: "raw_like",
        arity: 1,
    },
    FieldRefMethod {
        name: "iraw_like",
        arity: 1,
    },
];

impl FieldRefMethod {
    pub(crate) fn lookup(ident: &syn::Ident) -> Option<&'static Self> {
        let name = ident.to_string();
        FIELD_REF_METHODS.iter().find(|m| m.name == name)
    }

    pub(crate) fn as_ident(self) -> syn::Ident {
        format_ident!("{}", self.name)
    }

    pub(crate) fn all_names() -> impl Iterator<Item = &'static str> {
        FIELD_REF_METHODS.iter().map(|m| m.name)
    }
}

pub(super) fn query_to_tokens(query: Query) -> TokenStream {
    let crate_name = cot_ident();
    let model_name = query.model_name;
    let expr = expr_to_tokens(&model_name, query.expr);

    quote! {
        <#model_name as #crate_name::db::Model>::objects().filter(#expr)
    }
}

pub(super) fn expr_to_tokens(model_name: &syn::Type, expr: Expr) -> TokenStream {
    let crate_name = cot_ident();
    match expr {
        Expr::FieldRef { field_name, .. } => {
            quote!(<#model_name as #crate_name::db::Model>::Fields::#field_name.as_expr())
        }
        Expr::Value(value) => {
            quote!(#crate_name::db::query::expr::Expr::value(#value))
        }
        Expr::MemberAccess {
            parent,
            member_name,
            ..
        } => match parent.as_tokens() {
            Some(tokens) => {
                quote!(#crate_name::db::query::expr::Expr::value(#tokens.#member_name))
            }
            None => syn::Error::new_spanned(
                parent.as_tokens_full(),
                "accessing members of values that reference database fields is unsupported",
            )
            .to_compile_error(),
        },
        Expr::PathAccess {
            parent,
            path_segment,
            ..
        } => match parent.as_tokens() {
            Some(tokens) => {
                quote!(#crate_name::db::query::expr::Expr::value(#tokens::#path_segment))
            }
            None => syn::Error::new_spanned(
                parent.as_tokens_full(),
                "accessing paths of values that reference database fields is unsupported",
            )
            .to_compile_error(),
        },
        Expr::FunctionCall { function, args } => {
            let non_field_tokens = function.as_tokens();

            if non_field_tokens.is_none()
                && let Expr::MemberAccess { member_name, .. } = &*function
                && let Some(method) = FieldRefMethod::lookup(member_name)
            {
                let Expr::MemberAccess { parent, .. } = *function else {
                    unreachable!("function call must have a parent");
                };
                return handle_field_ref_method(model_name, *parent, &args, *method);
            }

            if let Some(tokens) = non_field_tokens {
                quote!(#crate_name::db::query::expr::Expr::value(#tokens(#(#args),*)))
            } else {
                let all_function_names = FieldRefMethod::all_names().collect::<Vec<_>>().join(", ");
                let msg = format!(
                    "calling functions that reference database fields is unsupported \
                        (only {all_function_names} are supported directly on database fields)"
                );
                syn::Error::new_spanned(function.as_tokens_full(), msg).to_compile_error()
            }
        }
        Expr::And(lhs, rhs) => {
            let lhs = expr_to_tokens(model_name, *lhs);
            let rhs = expr_to_tokens(model_name, *rhs);
            quote!(#crate_name::db::query::expr::Expr::and(#lhs, #rhs))
        }
        Expr::Or(lhs, rhs) => {
            let lhs = expr_to_tokens(model_name, *lhs);
            let rhs = expr_to_tokens(model_name, *rhs);
            quote!(#crate_name::db::query::expr::Expr::or(#lhs, #rhs))
        }
        Expr::Eq(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "eq", "ExprEq"),
        Expr::Ne(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "ne", "ExprEq"),
        Expr::Lt(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "lt", "ExprOrd"),
        Expr::Lte(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "lte", "ExprOrd"),
        Expr::Gt(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "gt", "ExprOrd"),
        Expr::Gte(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "gte", "ExprOrd"),
        Expr::Add(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "add", "ExprAdd"),
        Expr::Sub(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "sub", "ExprSub"),
        Expr::Mul(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "mul", "ExprMul"),
        Expr::Div(lhs, rhs) => handle_binary_comparison(model_name, *lhs, *rhs, "div", "ExprDiv"),
    }
}

fn handle_binary_comparison(
    model_name: &syn::Type,
    lhs: Expr,
    rhs: Expr,
    bin_fn: &str,
    bin_trait: &str,
) -> TokenStream {
    let crate_name = cot_ident();
    let bin_fn = format_ident!("{}", bin_fn);
    let bin_trait = format_ident!("{}", bin_trait);

    if let Expr::FieldRef { ref field_name, .. } = lhs
        && let Some(rhs_tokens) = rhs.as_tokens()
    {
        return quote!(#crate_name::db::query::expr::#bin_trait::#bin_fn(<#model_name as #crate_name::db::Model>::Fields::#field_name, #rhs_tokens));
    }

    let lhs = expr_to_tokens(model_name, lhs);
    let rhs = expr_to_tokens(model_name, rhs);
    quote!(#crate_name::db::query::expr::Expr::#bin_fn(#lhs, #rhs))
}

fn handle_field_ref_method(
    model_name: &syn::Type,
    receiver: Expr,
    args: &[syn::Expr],
    method: FieldRefMethod,
) -> TokenStream {
    let crate_name = cot_ident();
    let method_ident = method.as_ident();

    if method.arity as usize != args.len() {
        let arity = method.arity;
        let span = args
            .first()
            .map_or_else(proc_macro2::Span::call_site, syn::spanned::Spanned::span);
        return syn::Error::new(
            span,
            format!(
                "`{method_ident}` expects {arity} argument(s), found {}",
                args.len()
            ),
        )
        .to_compile_error();
    }

    if let Expr::FieldRef { ref field_name, .. } = receiver {
        return quote! {
            #crate_name::db::query::expr::ExprLike::#method_ident(
                <#model_name as #crate_name::db::Model>::Fields::#field_name,
                #(#args),*
            )
        };
    }

    let receiver_tokens = expr_to_tokens(model_name, receiver);
    let wrapped_args = args
        .iter()
        .map(|arg| quote!(#crate_name::db::query::expr::Expr::value(#arg)));

    quote! {
        #crate_name::db::query::expr::Expr::#method_ident(
            #receiver_tokens,
            #(#wrapped_args),*
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_ref_method_all_names() {
        let all_names = FieldRefMethod::all_names().collect::<Vec<_>>();
        assert_eq!(all_names.len(), 8);
        assert_eq!(
            all_names,
            [
                "contains",
                "icontains",
                "starts_with",
                "istarts_with",
                "ends_with",
                "iends_with",
                "raw_like",
                "iraw_like",
            ]
        );
    }

    #[test]
    fn test_field_ref_method_lookup() {
        let idents = [
            (
                "contains",
                Some(FieldRefMethod {
                    name: "contains",
                    arity: 1,
                }),
            ),
            (
                "icontains",
                Some(FieldRefMethod {
                    name: "icontains",
                    arity: 1,
                }),
            ),
            (
                "starts_with",
                Some(FieldRefMethod {
                    name: "starts_with",
                    arity: 1,
                }),
            ),
            (
                "istarts_with",
                Some(FieldRefMethod {
                    name: "istarts_with",
                    arity: 1,
                }),
            ),
            (
                "ends_with",
                Some(FieldRefMethod {
                    name: "ends_with",
                    arity: 1,
                }),
            ),
            (
                "iends_with",
                Some(FieldRefMethod {
                    name: "iends_with",
                    arity: 1,
                }),
            ),
            (
                "raw_like",
                Some(FieldRefMethod {
                    name: "raw_like",
                    arity: 1,
                }),
            ),
            (
                "iraw_like",
                Some(FieldRefMethod {
                    name: "iraw_like",
                    arity: 1,
                }),
            ),
            ("__non_existent__", None),
        ];

        for (ident, expected) in idents {
            assert_eq!(
                FieldRefMethod::lookup(&format_ident!("{}", ident)).copied(),
                expected
            );
        }
    }
}
