// TODO: This should be removed when https://github.com/TedDriggs/darling/pull/346
// is merged and released

use darling::{FromMeta, Result};
use syn::Expr;

/// A wrapper around [`Expr`] that preserves the original expression
/// without evaluating it.
///
/// For compatibility reasons, `darling` evaluates the expression inside string
/// literals, which might be undesirable. In many cases,
/// [`darling::util::parse_expr::preserve_str_literal`] can be used. However,
/// when using [`Expr`] inside a container (such as a
/// [`HashMap`](std::collections::HashMap)), it is not possible to use it.
///
/// This wrapper preserves the original expression without evaluating it.
///
/// # Example
///
/// ```compile_fail
/// #[derive(FromMeta)]
/// #[darling(attributes(demo))]
/// struct Demo {
///     option: Option<HashMap<syn::Ident, PreservedStrExpr>>,
/// }
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct PreservedStrExpr(pub Expr);

impl FromMeta for PreservedStrExpr {
    fn from_expr(expr: &Expr) -> Result<Self> {
        Ok(Self(expr.clone()))
    }
}

impl From<Expr> for PreservedStrExpr {
    fn from(value: Expr) -> Self {
        Self(value)
    }
}

impl From<PreservedStrExpr> for Expr {
    fn from(value: PreservedStrExpr) -> Self {
        value.0
    }
}

impl quote::ToTokens for PreservedStrExpr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use syn::{Meta, MetaNameValue, parse_quote};

    use super::*;

    #[test]
    fn preserved_str_expr_from_meta() {
        let name_value: MetaNameValue = parse_quote!(test = "Hello, world!");
        let preserved = PreservedStrExpr::from_meta(&Meta::NameValue(name_value)).unwrap();

        assert_eq!(preserved.0, parse_quote!("Hello, world!"));
    }
}
