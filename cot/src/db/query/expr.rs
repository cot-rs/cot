//! Database expressions.
pub mod like;

use std::marker::PhantomData;

use cot::db::query::{IntoField, QueryBuildingError};
use cot::db::{DbFieldValue, DbValue, FromDbValue, Identifier, ToDbFieldValue};
pub use like::ExprLike;
use like::{CaseSensitivity, LikeExprBuilder, LikeMode};
use sea_query::{ExprTrait, IntoColumnRef, SimpleExpr};

/// An expression that can be used to filter, update, or delete rows.
///
/// This is used to create complex queries with multiple conditions. Typically,
/// it is only internally used by the [`cot::db::query!`] macro to create a
/// [`Query`].
///
/// # Example
///
/// ```
/// use cot::db::{model, query};
/// use cot::db::query::Query;
/// use cot::db::query::expr::Expr;
///
/// #[model]
/// struct MyModel {
///     #[model(primary_key)]
///     id: i32,
/// };
///
/// let expr = Expr::eq(Expr::field("id"), Expr::value(5));
///
/// assert_eq!(
///     <Query<MyModel>>::new().filter(expr),
///     query!(MyModel, $id == 5)
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Expr {
    /// An expression containing a reference to a column.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == 5)
    /// );
    /// ```
    Field(Identifier),
    /// An expression containing a literal value.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::ne(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id != 5)
    /// );
    /// ```
    Value(DbValue),
    /// An `AND` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::and(
    ///     Expr::gt(Expr::field("id"), Expr::value(10)),
    ///     Expr::lt(Expr::field("id"), Expr::value(20))
    /// );
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 10 && $id < 20)
    /// );
    /// ```
    And(Box<Expr>, Box<Expr>),
    /// An `OR` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::or(
    ///     Expr::gt(Expr::field("id"), Expr::value(10)),
    ///     Expr::lt(Expr::field("id"), Expr::value(20))
    /// );
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 10 || $id < 20)
    /// );
    /// ```
    Or(Box<Expr>, Box<Expr>),
    /// An `=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == 5)
    /// );
    /// ```
    Eq(Box<Expr>, Box<Expr>),
    /// A `!=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::ne(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id != 5)
    /// );
    /// ```
    Ne(Box<Expr>, Box<Expr>),
    /// A `<` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::lt(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id < 5)
    /// );
    /// ```
    Lt(Box<Expr>, Box<Expr>),
    /// A `<=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::lte(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id <= 5)
    /// );
    /// ```
    Lte(Box<Expr>, Box<Expr>),
    /// A `>` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::gt(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 5)
    /// );
    /// ```
    Gt(Box<Expr>, Box<Expr>),
    /// A `>=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::gte(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id >= 5)
    /// );
    /// ```
    Gte(Box<Expr>, Box<Expr>),
    /// A `+` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::add(Expr::field("id_2"), Expr::value(5)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 + 5)
    /// );
    /// ```
    Add(Box<Expr>, Box<Expr>),
    /// A `-` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::sub(Expr::field("id_2"), Expr::value(5)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 - 5)
    /// );
    /// ```
    Sub(Box<Expr>, Box<Expr>),
    /// A `*` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::mul(Expr::field("id_2"), Expr::value(2)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 * 2)
    /// );
    /// ```
    Mul(Box<Expr>, Box<Expr>),
    /// A `/` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::div(Expr::field("id_2"), Expr::value(2)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 / 2)
    /// );
    /// ```
    Div(Box<Expr>, Box<Expr>),
    /// A case-sensitive substring match, checking whether the left-hand
    /// expression contains the right-hand expression as a literal
    /// substring.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// };
    ///
    /// let expr = Expr::contains(Expr::field("name"), Expr::value("test"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.contains("test"))
    /// );
    /// ```
    Contains(Box<Expr>, Box<Expr>, CaseSensitivity),
    /// A prefix match, checking whether the left-hand expression starts
    /// with the right-hand expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// };
    ///
    /// let expr = Expr::starts_with(Expr::field("name"), Expr::value("Mr."));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.starts_with("Mr."))
    /// );
    /// ```
    StartsWith(Box<Expr>, Box<Expr>, CaseSensitivity),
    /// A suffix match, checking whether the left-hand expression ends
    /// with the right-hand expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     email: String,
    /// };
    ///
    /// let expr = Expr::ends_with(Expr::field("email"), Expr::value("@example.com"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $email.ends_with("@example.com"))
    /// );
    /// ```
    EndsWith(Box<Expr>, Box<Expr>, CaseSensitivity),
    /// A match against a raw pattern expressed in Cot's glob syntax.
    /// See [`Expr::raw_like`] for the full syntax reference.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     code: String,
    /// };
    ///
    /// let expr = Expr::raw_like(Expr::field("code"), Expr::value("f??o"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $code.raw_like("f??o"))
    /// );
    /// ```
    RawLike(Box<Expr>, Box<Expr>, CaseSensitivity),
}

impl Expr {
    /// Create a new field expression. This represents a reference to a column
    /// in the database.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == 5)
    /// );
    /// ```
    #[must_use]
    pub fn field<T: Into<Identifier>>(identifier: T) -> Self {
        Self::Field(identifier.into())
    }

    /// Create a new value expression. This represents a literal value that gets
    /// passed into the SQL query.
    ///
    /// # Panics
    ///
    /// If the value provided is a [`DbFieldValue::Auto`].
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::ne(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id != 5)
    /// );
    /// ```
    #[must_use]
    #[expect(clippy::needless_pass_by_value)]
    pub fn value<T: ToDbFieldValue>(value: T) -> Self {
        match value.to_db_field_value() {
            DbFieldValue::Value(value) => Self::Value(value),
            DbFieldValue::Auto => panic!("Cannot create query with a non-value field"),
        }
    }

    /// Create a new `AND` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::and(
    ///     Expr::gt(Expr::field("id"), Expr::value(10)),
    ///     Expr::lt(Expr::field("id"), Expr::value(20))
    /// );
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 10 && $id < 20)
    /// );
    /// ```
    #[must_use]
    pub fn and(lhs: Self, rhs: Self) -> Self {
        Self::And(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `OR` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::or(
    ///     Expr::gt(Expr::field("id"), Expr::value(10)),
    ///     Expr::lt(Expr::field("id"), Expr::value(20))
    /// );
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 10 || $id < 20)
    /// );
    /// ```
    #[must_use]
    pub fn or(lhs: Self, rhs: Self) -> Self {
        Self::Or(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == 5)
    /// );
    /// ```
    #[must_use]
    pub fn eq(lhs: Self, rhs: Self) -> Self {
        Self::Eq(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `!=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::ne(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id != 5)
    /// );
    /// ```
    #[must_use]
    pub fn ne(lhs: Self, rhs: Self) -> Self {
        Self::Ne(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `<` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::lt(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id < 5)
    /// );
    /// ```
    #[must_use]
    pub fn lt(lhs: Self, rhs: Self) -> Self {
        Self::Lt(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `<=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::lte(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id <= 5)
    /// );
    /// ```
    #[must_use]
    pub fn lte(lhs: Self, rhs: Self) -> Self {
        Self::Lte(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `>` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::gt(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 5)
    /// );
    /// ```
    #[must_use]
    pub fn gt(lhs: Self, rhs: Self) -> Self {
        Self::Gt(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `>=` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = Expr::gte(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id >= 5)
    /// );
    /// ```
    #[must_use]
    pub fn gte(lhs: Self, rhs: Self) -> Self {
        Self::Gte(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `+` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::add(Expr::field("id_2"), Expr::value(5)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 + 5)
    /// );
    /// ```
    #[expect(clippy::should_implement_trait)]
    #[must_use]
    pub fn add(lhs: Self, rhs: Self) -> Self {
        Self::Add(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `-` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::sub(Expr::field("id_2"), Expr::value(5)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 - 5)
    /// );
    /// ```
    #[expect(clippy::should_implement_trait)]
    #[must_use]
    pub fn sub(lhs: Self, rhs: Self) -> Self {
        Self::Sub(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `*` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::mul(Expr::field("id_2"), Expr::value(2)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 * 2)
    /// );
    /// ```
    #[expect(clippy::should_implement_trait)]
    #[must_use]
    pub fn mul(lhs: Self, rhs: Self) -> Self {
        Self::Mul(Box::new(lhs), Box::new(rhs))
    }

    /// Create a new `/` expression.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     id_2: i32,
    /// };
    ///
    /// let expr = Expr::eq(Expr::field("id"), Expr::div(Expr::field("id_2"), Expr::value(2)));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == $id_2 / 2)
    /// );
    /// ```
    #[expect(clippy::should_implement_trait)]
    #[must_use]
    pub fn div(lhs: Self, rhs: Self) -> Self {
        Self::Div(Box::new(lhs), Box::new(rhs))
    }

    /// Creates an expression that checks whether `lhs` contains `rhs` as a
    /// substring, comparing case-sensitively.
    ///
    /// The search string in `rhs` is treated as a literal: any characters in
    /// it that would otherwise be treated as wildcards by [`raw_like`] are
    /// escaped automatically, so `"50% off"` matches the literal text
    /// `50% off`, not a wildcard pattern.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let expr = Expr::contains(Expr::field("name"), Expr::value("test"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.contains("test"))
    /// );
    /// ```
    #[must_use]
    pub fn contains(lhs: Self, rhs: Self) -> Self {
        Self::Contains(Box::new(lhs), Box::new(rhs), CaseSensitivity::Sensitive)
    }

    /// Creates an expression that checks whether `lhs` contains `rhs` as a
    /// substring, comparing case-insensitively.
    ///
    /// The case-insensitive counterpart to [`contains`](Self::contains). As
    /// with `contains`, the search string in `rhs` is treated as a literal
    /// and any wildcard-like characters in it are escaped automatically.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let expr = Expr::icontains(Expr::field("name"), Expr::value("TEST"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.icontains("TEST"))
    /// );
    /// ```
    #[must_use]
    pub fn icontains(lhs: Self, rhs: Self) -> Self {
        Self::Contains(Box::new(lhs), Box::new(rhs), CaseSensitivity::Insensitive)
    }

    /// Creates an expression that checks whether `lhs` starts with `rhs`,
    /// comparing case-sensitively.
    ///
    /// Behaves like [`contains`](Self::contains) but anchored to the start of
    /// the value: `rhs` is treated as a literal prefix, with any
    /// wildcard-like characters in it escaped automatically.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let expr = Expr::starts_with(Expr::field("name"), Expr::value("Mr."));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.starts_with("Mr."))
    /// );
    /// ```
    #[must_use]
    pub fn starts_with(lhs: Self, rhs: Self) -> Self {
        Self::StartsWith(Box::new(lhs), Box::new(rhs), CaseSensitivity::Sensitive)
    }

    /// Creates an expression that checks whether `lhs` starts with `rhs`,
    /// comparing case-insensitively.
    ///
    /// The case-insensitive counterpart to [`starts_with`](Self::starts_with).
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let expr = Expr::istarts_with(Expr::field("name"), Expr::value("mr."));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.istarts_with("mr."))
    /// );
    /// ```
    #[must_use]
    pub fn istarts_with(lhs: Self, rhs: Self) -> Self {
        Self::StartsWith(Box::new(lhs), Box::new(rhs), CaseSensitivity::Insensitive)
    }

    /// Creates an expression that checks whether `lhs` ends with `rhs`,
    /// comparing case-sensitively.
    ///
    /// Behaves like [`contains`](Self::contains) but anchored to the end of
    /// the value: `rhs` is treated as a literal suffix, with any
    /// wildcard-like characters in it escaped automatically.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let expr = Expr::ends_with(Expr::field("name"), Expr::value("foo"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.ends_with("foo"))
    /// );
    /// ```
    #[must_use]
    pub fn ends_with(lhs: Self, rhs: Self) -> Self {
        Self::EndsWith(Box::new(lhs), Box::new(rhs), CaseSensitivity::Sensitive)
    }

    /// Creates an expression that checks whether `lhs` ends with `rhs`,
    /// comparing case-insensitively.
    ///
    /// The case-insensitive counterpart to [`ends_with`](Self::ends_with).
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let expr = Expr::iends_with(Expr::field("name"), Expr::value("FOO"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $name.iends_with("FOO"))
    /// );
    /// ```
    #[must_use]
    pub fn iends_with(lhs: Self, rhs: Self) -> Self {
        Self::EndsWith(Box::new(lhs), Box::new(rhs), CaseSensitivity::Insensitive)
    }

    /// Creates an expression that matches `lhs` against a raw pattern in
    /// `rhs`, comparing case-sensitively.
    ///
    /// Unlike [`contains`](Self::contains)/[`starts_with`](Self::starts_with)/
    /// [`ends_with`](Self::ends_with), which escape the search string and
    /// wrap it for you, `raw_like` passes `rhs` through as a **pattern you
    /// write yourself** — use it when you need a shape those three methods
    /// can't express, such as a match with wildcards in the middle, or a
    /// fixed-width positional match.
    ///
    /// # Pattern syntax
    ///
    /// The pattern in `rhs` uses glob syntax:
    ///
    /// | Character | Meaning |
    /// |-----------|---------|
    /// | `*` | matches zero or more of any character |
    /// | `?` | matches exactly one character |
    /// | `\` | escapes the following character, making it literal |
    ///
    /// Any other character matches itself literally.
    ///
    /// # Escaping literal `*`, `?`, or `\` in the pattern
    ///
    /// If the text you're matching against can itself contain a literal `*`
    /// or `?`, escape it with a backslash so it isn't treated as a wildcard.
    /// For example, to match values that literally contain the substring
    /// `"a * here"`, write the pattern as `"*a \\* here*"`. `\\*` in the Rust
    /// string literal produces the two glob characters `\*`, i.e. an escaped,
    /// literal asterisk, while the surrounding bare `*` characters remain
    /// wildcards.
    ///
    /// If you don't need this level of control, prefer
    /// [`contains`](Self::contains)/[`starts_with`](Self::starts_with)/
    /// [`ends_with`](Self::ends_with), which handle escaping for you
    /// automatically based on a plain search string.
    ///
    /// # Examples
    ///
    /// Substring match (equivalent to `contains("foo")`, spelled out
    /// explicitly):
    ///
    /// ```
    /// use cot::db::query::expr::Expr;
    ///
    /// let expr = Expr::raw_like(Expr::field("name"), Expr::value("*foo*"));
    /// ```
    ///
    /// Prefix match (equivalent to `starts_with("foo")`):
    ///
    /// ```
    /// use cot::db::query::expr::Expr;
    ///
    /// let expr = Expr::raw_like(Expr::field("name"), Expr::value("foo*"));
    /// ```
    ///
    /// **Positional match**: something `contains`/`starts_with`/`ends_with`
    /// cannot express at all: match values that are exactly `f`, then any two
    /// characters, then `o` (e.g. matches `faXo`, `fooo`, but not `fo` or
    /// `faXYo`):
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     code: String,
    /// }
    ///
    /// let expr = Expr::raw_like(Expr::field("code"), Expr::value("f??o"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $code.raw_like("f??o"))
    /// );
    /// ```
    ///
    /// Wildcards in the middle of a pattern; matching values that contain
    /// `foo`, later followed by `bar`, later followed by `baz`, in that
    /// order, with anything in between:
    ///
    /// ```
    /// use cot::db::query::expr::Expr;
    ///
    /// let expr = Expr::raw_like(Expr::field("path"), Expr::value("*foo*bar*baz*"));
    /// ```
    #[must_use]
    pub fn raw_like(lhs: Self, rhs: Self) -> Self {
        Self::RawLike(Box::new(lhs), Box::new(rhs), CaseSensitivity::Sensitive)
    }

    /// Creates an expression that matches `lhs` against a raw pattern in
    /// `rhs`, comparing case-insensitively.
    ///
    /// The case-insensitive counterpart to [`raw_like`](Self::raw_like). See
    /// its documentation for the pattern syntax (`*`/`?`/`\`).
    ///
    /// # Example
    ///
    /// Case-insensitive positional match — matches `README`, `ReadMe`,
    /// `readme`, etc., but not `READMEE` or `REDME`:
    ///
    /// ```
    /// use cot::db::{model, query};
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::Expr;
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     filename: String,
    /// }
    ///
    /// let expr = Expr::iraw_like(Expr::field("filename"), Expr::value("re?dme"));
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $filename.iraw_like("re?dme"))
    /// );
    /// ```
    #[must_use]
    pub fn iraw_like(lhs: Self, rhs: Self) -> Self {
        Self::RawLike(Box::new(lhs), Box::new(rhs), CaseSensitivity::Insensitive)
    }

    /// Returns the expression as a [`sea_query::SimpleExpr`].
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::query::expr::Expr;
    /// use cot::db::{Database, Identifier};
    /// use sea_query::{ExprTrait, IntoColumnRef};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let db = Database::new("sqlite::memory:").await.unwrap();
    /// let expr = Expr::eq(Expr::field("id"), Expr::value(5));
    ///
    /// assert_eq!(
    ///     expr.as_sea_query_expr(&db).unwrap(),
    ///     ExprTrait::eq(
    ///         sea_query::SimpleExpr::Column(Identifier::new("id").into_column_ref()),
    ///         sea_query::SimpleExpr::Value(sea_query::Value::Int(Some(5)))
    ///     )
    /// );
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`QueryBuildingError`] if the backend cannot express a given
    /// expression.
    pub fn as_sea_query_expr(
        &self,
        sql_builder: &dyn SqlQueryBuilder,
    ) -> Result<SimpleExpr, QueryBuildingError> {
        match self {
            Self::Field(identifier) => Ok((*identifier).into_column_ref().into()),
            Self::Value(value) => Ok((*value).clone().into()),
            Self::And(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .and(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Or(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .or(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Eq(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .eq(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Ne(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .ne(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Lt(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .lt(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Lte(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .lte(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Gt(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .gt(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Gte(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .gte(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Add(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .add(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Sub(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .sub(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Mul(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .mul(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Div(lhs, rhs) => Ok(lhs
                .as_sea_query_expr(sql_builder)?
                .div(rhs.as_sea_query_expr(sql_builder)?)),
            Self::Contains(lhs, rhs, case_sensitivity) => {
                like::like_expr(sql_builder, lhs, rhs, LikeMode::Contains, *case_sensitivity)
            }
            Self::StartsWith(lhs, rhs, case_sensitivity) => like::like_expr(
                sql_builder,
                lhs,
                rhs,
                LikeMode::StartsWith,
                *case_sensitivity,
            ),
            Self::EndsWith(lhs, rhs, case_sensitivity) => {
                like::like_expr(sql_builder, lhs, rhs, LikeMode::EndsWith, *case_sensitivity)
            }
            Self::RawLike(lhs, rhs, case_sensitivity) => {
                like::like_expr(sql_builder, lhs, rhs, LikeMode::Raw, *case_sensitivity)
            }
        }
    }
}

/// A reference to a field in a database table.
///
/// This is used to create expressions that reference a specific column in a
/// table with a specific type. This allows for type-safe creation of queries
/// with some common operators like `=`, `!=`, `+`, `-`, `*`, and `/`.
#[derive(Debug)]
pub struct FieldRef<T> {
    identifier: Identifier,
    phantom_data: PhantomData<T>,
}

impl<T: FromDbValue + ToDbFieldValue> FieldRef<T> {
    /// Create a new field reference.
    #[must_use]
    pub const fn new(identifier: Identifier) -> Self {
        Self {
            identifier,
            phantom_data: PhantomData,
        }
    }
}

impl<T> FieldRef<T> {
    /// Returns the field reference as an [`Expr`].
    #[must_use]
    pub fn as_expr(&self) -> Expr {
        Expr::Field(self.identifier)
    }
}

/// A trait for types that can be compared in database expressions.
pub trait ExprEq<T> {
    /// Creates an expression that checks if the field is equal to the given
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprEq};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.eq(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id == 5)
    /// );
    /// ```
    fn eq<V: IntoField<T>>(self, other: V) -> Expr;

    /// Creates an expression that checks if the field is not equal to the given
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprEq};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.ne(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id != 5)
    /// );
    /// ```
    fn ne<V: IntoField<T>>(self, other: V) -> Expr;
}

impl<T: ToDbFieldValue + 'static> ExprEq<T> for FieldRef<T> {
    fn eq<V: IntoField<T>>(self, other: V) -> Expr {
        Expr::eq(self.as_expr(), Expr::value(other.into_field()))
    }

    fn ne<V: IntoField<T>>(self, other: V) -> Expr {
        Expr::ne(self.as_expr(), Expr::value(other.into_field()))
    }
}

/// A trait for database types that can be added to each other.
pub trait ExprAdd<T> {
    /// Creates an expression that adds the field to the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::{Expr, ExprAdd};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.add(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(Expr::eq(Expr::field("id"), expr)),
    ///     query!(MyModel, $id == $id + 5)
    /// );
    /// ```
    fn add<V: Into<T>>(self, other: V) -> Expr;
}

/// A trait for database types that can be subtracted from each other.
pub trait ExprSub<T> {
    /// Creates an expression that subtracts the field from the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprSub};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.sub(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(Expr::eq(Expr::field("id"), expr)),
    ///     query!(MyModel, $id == $id - 5)
    /// );
    /// ```
    fn sub<V: Into<T>>(self, other: V) -> Expr;
}

/// A trait for database types that can be multiplied by each other.
pub trait ExprMul<T> {
    /// Creates an expression that multiplies the field by the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprMul};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.mul(2);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(Expr::eq(Expr::field("id"), expr)),
    ///     query!(MyModel, $id == $id * 2)
    /// );
    /// ```
    fn mul<V: Into<T>>(self, other: V) -> Expr;
}

/// A trait for database types that can be divided by each other.
pub trait ExprDiv<T> {
    /// Creates an expression that divides the field by the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprDiv};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.div(2);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(Expr::eq(Expr::field("id"), expr)),
    ///     query!(MyModel, $id == $id / 2)
    /// );
    /// ```
    fn div<V: Into<T>>(self, other: V) -> Expr;
}

/// A trait for database types that can be ordered.
pub trait ExprOrd<T> {
    /// Creates an expression that checks if the field is less than the given
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprOrd};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.lt(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id < 5)
    /// );
    /// ```
    fn lt<V: IntoField<T>>(self, other: V) -> Expr;
    /// Creates an expression that checks if the field is less than or equal to
    /// the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::expr::{Expr, ExprOrd};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.lte(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id <= 5)
    /// );
    /// ```
    fn lte<V: IntoField<T>>(self, other: V) -> Expr;

    /// Creates an expression that checks if the field is greater than the given
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::{Expr, ExprOrd};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.gt(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id > 5)
    /// );
    /// ```
    fn gt<V: IntoField<T>>(self, other: V) -> Expr;

    /// Creates an expression that checks if the field is greater than or equal
    /// to the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::query::Query;
    /// use cot::db::query::{Expr, ExprOrd};
    /// use cot::db::{model, query};
    ///
    /// #[model]
    /// struct MyModel {
    ///     #[model(primary_key)]
    ///     id: i32,
    /// };
    ///
    /// let expr = <MyModel as cot::db::Model>::Fields::id.gte(5);
    ///
    /// assert_eq!(
    ///     <Query<MyModel>>::new().filter(expr),
    ///     query!(MyModel, $id >= 5)
    /// );
    /// ```
    fn gte<V: IntoField<T>>(self, other: V) -> Expr;
}

impl<T: ToDbFieldValue + Ord + 'static> ExprOrd<T> for FieldRef<T> {
    fn lt<V: IntoField<T>>(self, other: V) -> Expr {
        Expr::lt(self.as_expr(), Expr::value(other.into_field()))
    }

    fn lte<V: IntoField<T>>(self, other: V) -> Expr {
        Expr::lte(self.as_expr(), Expr::value(other.into_field()))
    }

    fn gt<V: IntoField<T>>(self, other: V) -> Expr {
        Expr::gt(self.as_expr(), Expr::value(other.into_field()))
    }

    fn gte<V: IntoField<T>>(self, other: V) -> Expr {
        Expr::gte(self.as_expr(), Expr::value(other.into_field()))
    }
}

/// A marker trait that represents the full set of query-translation
/// capabilities a database backend may support.
pub trait SqlQueryBuilder: LikeExprBuilder {}

impl<T> SqlQueryBuilder for T where T: LikeExprBuilder {}

macro_rules! impl_expr {
    ($ty:ty, $trait:ident, $method:ident) => {
        impl $trait<$ty> for FieldRef<$ty> {
            fn $method<V: Into<$ty>>(self, other: V) -> Expr {
                Expr::$method(self.as_expr(), Expr::value(other.into()))
            }
        }
    };
}

macro_rules! impl_num_expr {
    ($ty:ty) => {
        impl_expr!($ty, ExprAdd, add);
        impl_expr!($ty, ExprSub, sub);
        impl_expr!($ty, ExprMul, mul);
        impl_expr!($ty, ExprDiv, div);
    };
}

impl_num_expr!(i8);
impl_num_expr!(i16);
impl_num_expr!(i32);
impl_num_expr!(i64);
impl_num_expr!(u8);
impl_num_expr!(u16);
impl_num_expr!(u32);
impl_num_expr!(u64);
impl_num_expr!(f32);
impl_num_expr!(f64);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn expr_field() {
        let expr = Expr::field("name");
        if let Expr::Field(identifier) = expr {
            assert_eq!(identifier.to_string(), "name");
        } else {
            panic!("Expected Expr::Field");
        }
    }

    #[test]
    fn expr_value() {
        let expr = Expr::value(30);
        if let Expr::Value(value) = expr {
            assert_eq!(value.to_string(), "30");
        } else {
            panic!("Expected Expr::Value");
        }
    }

    macro_rules! test_expr_constructor {
        ($test_name:ident, $match:ident, $constructor:ident) => {
            #[test]
            fn $test_name() {
                let expr = Expr::$constructor(Expr::field("name"), Expr::value("John"));
                if let Expr::$match(lhs, rhs) = expr {
                    assert!(matches!(*lhs, Expr::Field(_)));
                    assert!(matches!(*rhs, Expr::Value(_)));
                } else {
                    panic!(concat!("Expected Expr::", stringify!($match)));
                }
            }
        };
    }

    test_expr_constructor!(expr_and, And, and);
    test_expr_constructor!(expr_or, Or, or);
    test_expr_constructor!(expr_eq, Eq, eq);
    test_expr_constructor!(expr_ne, Ne, ne);
    test_expr_constructor!(expr_lt, Lt, lt);
    test_expr_constructor!(expr_lte, Lte, lte);
    test_expr_constructor!(expr_gt, Gt, gt);
    test_expr_constructor!(expr_gte, Gte, gte);
    test_expr_constructor!(expr_add, Add, add);
    test_expr_constructor!(expr_sub, Sub, sub);
    test_expr_constructor!(expr_mul, Mul, mul);
    test_expr_constructor!(expr_div, Div, div);
}
