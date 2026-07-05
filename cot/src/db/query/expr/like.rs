//! Database expressions for pattern-matching.

use cot::db::ToDbFieldValue;
use cot::db::query::expr::{FieldRef, SqlQueryBuilder};
use cot::db::query::{Expr, QueryBuildingError};
use sea_query::{ExprTrait, SimpleExpr};

pub(crate) const LIKE_ESCAPE_CHAR: char = '\\';

/// A trait for database types that support literal-substring pattern
/// matching.
pub trait ExprLike<T> {
    /// Checks if the field contains `other` as a literal substring,
    /// comparing case-sensitively.
    ///
    /// See [`Expr::contains`] for the underlying semantics.
    fn contains<V: Into<String>>(self, other: V) -> Expr;

    /// Checks if the field contains `other` as a literal substring.
    /// This is the case-insensitive counterpart of [`Self::contains`].
    ///
    /// See [`Expr::contains`] for the underlying semantics.
    fn icontains<V: Into<String>>(self, other: V) -> Expr;

    /// Checks if the field starts with `other`, comparing
    /// case-sensitively.
    ///
    /// See [`Expr::starts_with`] for the underlying semantics.
    fn starts_with<V: Into<String>>(self, other: V) -> Expr;

    /// Checks if the field starts with `other`.
    /// This is the case-insensitive counterpart of [`Self::starts_with`].
    ///
    /// See [`Expr::starts_with`] for the underlying semantics.
    fn istarts_with<V: Into<String>>(self, other: V) -> Expr;

    /// Checks if the field ends with `other`, comparing case-sensitively.
    ///
    /// See [`Expr::ends_with`] for the underlying semantics.
    fn ends_with<V: Into<String>>(self, other: V) -> Expr;

    /// Checks if the field ends with `other`.
    /// This is the case-insensitive counterpart of [`Self::ends_with`].
    ///
    /// See [`Expr::ends_with`] for the underlying semantics.
    fn iends_with<V: Into<String>>(self, other: V) -> Expr;

    /// Matches an expression against the raw pattern provided in `other`,
    /// matching case-sensitively.
    ///
    /// See [`Expr::raw_like`] for the underlying semantics.
    fn raw_like<V: Into<String>>(self, other: V) -> Expr;

    /// Matches an expression against the raw pattern provided in `other`.
    /// This is the case-insensitive counterpart of [`Self::raw_like`].
    ///
    /// See [`Expr::iraw_like`] for the underlying semantics.
    fn iraw_like<V: Into<String>>(self, other: V) -> Expr;
}

impl<T: ToDbFieldValue + 'static> ExprLike<T> for FieldRef<T> {
    fn contains<V: Into<String>>(self, other: V) -> Expr {
        Expr::contains(self.as_expr(), Expr::value(other.into()))
    }

    fn icontains<V: Into<String>>(self, other: V) -> Expr {
        Expr::icontains(self.as_expr(), Expr::value(other.into()))
    }

    fn starts_with<V: Into<String>>(self, other: V) -> Expr {
        Expr::starts_with(self.as_expr(), Expr::value(other.into()))
    }

    fn istarts_with<V: Into<String>>(self, other: V) -> Expr {
        Expr::istarts_with(self.as_expr(), Expr::value(other.into()))
    }

    fn ends_with<V: Into<String>>(self, other: V) -> Expr {
        Expr::ends_with(self.as_expr(), Expr::value(other.into()))
    }

    fn iends_with<V: Into<String>>(self, other: V) -> Expr {
        Expr::iends_with(self.as_expr(), Expr::value(other.into()))
    }

    fn raw_like<V: Into<String>>(self, other: V) -> Expr {
        Expr::raw_like(self.as_expr(), Expr::value(other.into()))
    }

    fn iraw_like<V: Into<String>>(self, other: V) -> Expr {
        Expr::iraw_like(self.as_expr(), Expr::value(other.into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LikeMode {
    Contains,
    StartsWith,
    EndsWith,
    Raw,
}

fn escape_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '*' | '?' | LIKE_ESCAPE_CHAR) {
            escaped.push(LIKE_ESCAPE_CHAR);
        }
        escaped.push(c);
    }
    escaped
}

pub(crate) fn to_sql_like(glob: &str) -> String {
    let mut escaped = String::with_capacity(glob.len());
    let mut chars = glob.chars();
    while let Some(ch) = chars.next() {
        match ch {
            LIKE_ESCAPE_CHAR => {
                if let Some(ch) = chars.next() {
                    push_like_literal(&mut escaped, ch);
                }
            }
            '*' => escaped.push('%'),
            '?' => escaped.push('_'),
            other => push_like_literal(&mut escaped, other),
        }
    }
    escaped
}

fn push_like_literal(out: &mut String, ch: char) {
    if matches!(ch, '%' | '_' | LIKE_ESCAPE_CHAR) {
        out.push(LIKE_ESCAPE_CHAR);
    }
    out.push(ch);
}

pub(crate) fn like_expr(
    sql_builder: &dyn SqlQueryBuilder,
    lhs: &Expr,
    rhs: &Expr,
    mode: LikeMode,
    case_sensitivity: CaseSensitivity,
) -> Result<SimpleExpr, QueryBuildingError> {
    let lhs_expr = lhs.as_sea_query_expr(sql_builder)?;

    let Expr::Value(value) = rhs else {
        return Ok(sea_query::Expr::val(1).eq(0));
    };

    let sea_value: sea_query::Value = value.clone();
    let sea_query::Value::String(Some(text)) = sea_value else {
        return Ok(sea_query::Expr::val(1).eq(0));
    };
    let glob_pattern = match mode {
        LikeMode::Contains => format!("*{}*", escape_literal(&text)),
        LikeMode::EndsWith => format!("*{}", escape_literal(&text)),
        LikeMode::StartsWith => format!("{}*", escape_literal(&text)),
        LikeMode::Raw => text.clone(),
    };

    sql_builder.like_expr(lhs_expr, &glob_pattern, case_sensitivity)
}

/// Whether a pattern-matching expression should consider letter case when
/// comparing text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseSensitivity {
    /// Matching considers letter case (`"Foo"` does not match `"foo"`).
    Sensitive,
    /// Matching ignores letter case (`"Foo"` matches `"foo"`).
    Insensitive,
}

/// Translates Cot's pattern-matching query expressions (`contains`,
/// `starts_with`, `ends_with`, `raw_like`, and their case-insensitive and
/// `i`-prefixed counterparts) into a backend-specific `sea_query`
/// expression.
///
/// Each of Cot's built-in database backends implements this trait; a
/// custom `sea_query`-based backend can implement it too to get correct,
/// dialect-appropriate translation of pattern-matching expressions without
/// needing to know anything about the individual `Expr` variants that
/// produce them. An implementor only ever sees a single, already-built
/// left-hand-side expression, a pattern already expressed in Cot's
/// canonical glob syntax (`*`/`?`/`\` — see [`Expr::raw_like`]), and the
/// requested [`CaseSensitivity`].
pub trait LikeExprBuilder {
    /// Builds the `sea_query` expression that checks whether `lhs`
    /// matches `glob_pattern`, honoring `case_sensitivity`.
    ///
    /// # Errors
    ///
    /// Returns [`QueryBuildingError`] if the backend cannot
    /// express pattern matching, or cannot express the requested
    /// [`CaseSensitivity`] variant.
    fn like_expr(
        &self,
        lhs: SimpleExpr,
        glob_pattern: &str,
        case_sensitivity: CaseSensitivity,
    ) -> Result<SimpleExpr, QueryBuildingError>;
}
