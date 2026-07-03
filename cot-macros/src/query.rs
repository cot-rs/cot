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

#[derive(Debug, Clone, Copy)]
pub(crate) enum StringMethod {
    Contains { case_sensitive: bool },
    StartsWith { case_sensitive: bool },
    EndsWith { case_sensitive: bool },
    Raw { case_sensitive: bool },
}

impl StringMethod {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Option<Self> {
        match ident.to_string().as_str() {
            "contains" => Some(Self::Contains {
                case_sensitive: true,
            }),
            "icontains" => Some(Self::Contains {
                case_sensitive: false,
            }),
            "starts_with" => Some(Self::StartsWith {
                case_sensitive: true,
            }),
            "istarts_with" => Some(Self::StartsWith {
                case_sensitive: false,
            }),
            "ends_with" => Some(Self::EndsWith {
                case_sensitive: true,
            }),
            "iends_with" => Some(Self::EndsWith {
                case_sensitive: false,
            }),
            "raw" => Some(Self::Raw {
                case_sensitive: true,
            }),
            "iraw" => Some(Self::Raw {
                case_sensitive: false,
            }),
            _ => None,
        }
    }

    pub(crate) fn as_ident(self) -> syn::Ident {
        let name = match self {
            Self::Contains {
                case_sensitive: true,
            } => "contains",
            Self::Contains {
                case_sensitive: false,
            } => "icontains",
            Self::StartsWith {
                case_sensitive: true,
            } => "starts_with",
            Self::StartsWith {
                case_sensitive: false,
            } => "istarts_with",
            Self::EndsWith {
                case_sensitive: true,
            } => "ends_with",
            Self::EndsWith {
                case_sensitive: false,
            } => "iends_with",
            Self::Raw {
                case_sensitive: true,
            } => "raw",
            Self::Raw {
                case_sensitive: false,
            } => "iraw",
        };
        format_ident!("{name}")
    }

    pub(crate) fn all_names() -> &'static [&'static str] {
        &[
            "contains",
            "icontains",
            "starts_with",
            "istarts_with",
            "ends_with",
            "iends_with",
            "raw",
            "iraw",
        ]
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
                && let Some(method) = StringMethod::from_ident(member_name)
            {
                let Expr::MemberAccess { parent, .. } = *function else {
                    unreachable!("function call must have a parent");
                };
                return handle_string_method(model_name, *parent, args, method);
            }

            if let Some(tokens) = non_field_tokens {
                quote!(#crate_name::db::query::expr::Expr::value(#tokens(#(#args),*)))
            } else {
                let all_function_names = StringMethod::all_names().join(", ");
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

fn handle_string_method(
    model_name: &syn::Type,
    receiver: Expr,
    args: Vec<syn::Expr>,
    method: StringMethod,
) -> TokenStream {
    let crate_name = cot_ident();
    let method_ident = method.as_ident();

    let arg = match <[syn::Expr; 1]>::try_from(args) {
        Ok([arg]) => arg,
        Err(args) => {
            let span = args
                .first()
                .map_or_else(proc_macro2::Span::call_site, syn::spanned::Spanned::span);
            return syn::Error::new(
                span,
                format!("`{method_ident}` expects exactly one string argument"),
            )
            .to_compile_error();
        }
    };

    if let Expr::FieldRef { ref field_name, .. } = receiver {
        return quote! {
            #crate_name::db::query::expr::ExprLike::#method_ident(
                <#model_name as #crate_name::db::Model>::Fields::#field_name,
                #arg
            )
        };
    }

    let receiver_tokens = expr_to_tokens(model_name, receiver);
    quote! {
        #crate_name::db::query::expr::Expr::#method_ident(
            #receiver_tokens,
            #crate_name::db::query::expr::Expr::value(#arg)
        )
    }
}
