use cot::db::{DbFieldValue, ToDbFieldValue};

use crate::db::Identifier;
use crate::db::query::expr::{FieldRef, SqlQueryBuilder};
use crate::db::query::{Expr, IntoField, QueryBuildingError};

/// Ordering Options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    /// Sort in Ascending order.
    Asc,
    /// Sort in Descending Order.
    Desc,
}

impl From<&SortOrder> for sea_query::Order {
    fn from(value: &SortOrder) -> Self {
        match value {
            SortOrder::Asc => sea_query::Order::Asc,
            SortOrder::Desc => sea_query::Order::Desc,
        }
    }
}

impl From<SortOrder> for sea_query::Order {
    fn from(value: SortOrder) -> Self {
        match value {
            SortOrder::Asc => sea_query::Order::Asc,
            SortOrder::Desc => sea_query::Order::Desc,
        }
    }
}

/// The order to sort null values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NullsOrder {
    /// Null values will appear first
    First,
    /// Null values will appear last
    Last,
}

impl From<&NullsOrder> for sea_query::NullOrdering {
    fn from(value: &NullsOrder) -> Self {
        match value {
            NullsOrder::First => sea_query::NullOrdering::First,
            NullsOrder::Last => sea_query::NullOrdering::Last,
        }
    }
}

impl From<NullsOrder> for sea_query::NullOrdering {
    fn from(value: NullsOrder) -> Self {
        match value {
            NullsOrder::First => sea_query::NullOrdering::First,
            NullsOrder::Last => sea_query::NullOrdering::Last,
        }
    }
}

/// The type of the order field
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) enum OrderTarget {
    /// Whether the order field is a column
    Column(Identifier),
    /// Whether the order field is an expression
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
enum OrderMode {
    Directional {
        order: SortOrder,
        nulls: Option<NullsOrder>,
    },
    Custom(sea_query::Values),
}

/// An `ORDER BY` term.
///
/// # Example
///
/// ```
/// use cot::db::model;
/// use cot::db::query::{ExprSort, Query};
///
/// #[model]
/// struct User {
///     #[model(primary_key)]
///     id: i32,
///     name: String,
/// }
///
/// let mut query = Query::<User>::new();
/// query.order_by([
///     <User as cot::db::Model>::Fields::id.asc(),
///     <User as cot::db::Model>::Fields::name.desc().nulls_first(),
/// ]);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderByExpr {
    target: OrderTarget,
    mode: OrderMode,
}

impl OrderByExpr {
    pub(crate) fn directional(target: OrderTarget, order: SortOrder) -> Self {
        Self {
            target,
            mode: OrderMode::Directional { order, nulls: None },
        }
    }

    pub(crate) fn custom(target: OrderTarget, values: sea_query::Values) -> Self {
        assert!(
            !values.0.is_empty(),
            "`custom` requires at least one value to rank by"
        );
        Self {
            target,
            mode: OrderMode::Custom(values),
        }
    }

    /// Places `NULL` values before all non-`NULL` values for this term,
    /// regardless of database backend or sort direction.
    ///
    /// # Panics
    ///
    /// Panics if this term was built with [`ExprSort::custom_order`]. A
    /// custom-order term never produces a `NULL` sort key,
    /// so an explicit `NULLS` placement on top of it can never have any
    /// effect.
    #[must_use]
    pub fn nulls_first(mut self) -> Self {
        self.set_nulls(NullsOrder::First);
        self
    }

    /// Places `NULL` values after all non-`NULL` values for this term.
    ///
    /// # Panics
    ///
    /// See [`Self::nulls_first`].
    #[must_use]
    pub fn nulls_last(mut self) -> Self {
        self.set_nulls(NullsOrder::Last);
        self
    }

    #[track_caller]
    fn set_nulls(&mut self, nulls: NullsOrder) {
        match &mut self.mode {
            OrderMode::Directional { nulls: n, .. } => *n = Some(nulls),
            OrderMode::Custom(_) => panic!(
                "`nulls_first`/`nulls_last` can't be combined with `custom`: a custom-order \
                 term never produces a NULL sort key, so an explicit NULLS placement would \
                 have no effect"
            ),
        }
    }

    pub(crate) fn add_to_statement(
        &self,
        statement: &mut sea_query::SelectStatement,
        sql_builder: &dyn SqlQueryBuilder,
    ) -> Result<(), QueryBuildingError> {
        let (sea_order, nulls): (sea_query::Order, Option<NullsOrder>) = match &self.mode {
            OrderMode::Directional { order, nulls } => (order.into(), *nulls),
            OrderMode::Custom(values) => (sea_query::Order::Field(values.clone()), None),
        };

        match &self.target {
            OrderTarget::Column(field) => match nulls {
                Some(nulls) => {
                    statement.order_by_with_nulls(*field, sea_order, nulls.into());
                }
                None => {
                    statement.order_by(*field, sea_order);
                }
            },
            OrderTarget::Expression(expr) => {
                let expr = expr.as_sea_query_expr(sql_builder)?;
                match nulls {
                    Some(nulls) => {
                        statement.order_by_expr_with_nulls(expr, sea_order, nulls.into());
                    }
                    None => {
                        statement.order_by_expr(expr, sea_order);
                    }
                }
            }
        }
        Ok(())
    }
}

impl<T: ToDbFieldValue + 'static> From<FieldRef<T>> for OrderByExpr {
    fn from(field: FieldRef<T>) -> Self {
        OrderByExpr::directional(OrderTarget::Column(field.identifier()), SortOrder::Asc)
    }
}

impl From<Expr> for OrderByExpr {
    fn from(expr: Expr) -> Self {
        expr.asc()
    }
}

/// A trait for database types that support sorting.
pub trait ExprSort<T> {
    /// Sort by this field in ascending order.
    fn asc(&self) -> OrderByExpr;
    /// Sort by this field in descending order.
    fn desc(&self) -> OrderByExpr;

    /// Sorts rows by the position of this field's value
    fn custom<I>(&self, values: I) -> OrderByExpr
    where
        I: IntoIterator,
        I::Item: IntoField<T>;
}

impl<T: ToDbFieldValue + 'static> ExprSort<T> for FieldRef<T> {
    fn asc(&self) -> OrderByExpr {
        OrderByExpr::directional(OrderTarget::Column(self.identifier()), SortOrder::Asc)
    }

    fn desc(&self) -> OrderByExpr {
        OrderByExpr::directional(OrderTarget::Column(self.identifier()), SortOrder::Desc)
    }

    fn custom<I>(&self, values: I) -> OrderByExpr
    where
        I: IntoIterator,
        I::Item: IntoField<T>,
    {
        let values = values
            .into_iter()
            .map(|v| match v.into_field().to_db_field_value() {
                DbFieldValue::Value(value) => value,
                DbFieldValue::Auto => {
                    panic!("cannot use an auto-generated value as a custom ordering key")
                }
            })
            .collect();
        OrderByExpr::custom(
            OrderTarget::Column(self.identifier()),
            sea_query::Values(values),
        )
    }
}
