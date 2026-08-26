use crate::db::Identifier;
use crate::db::query::expr::FieldRef;

/// Ordering Options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortOrder {
    /// Sort in Ascending order.
    Asc,
    /// Sort in Descending Order.
    Desc,
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

impl From<NullsOrder> for sea_query::NullOrdering {
    fn from(value: NullsOrder) -> Self {
        match value {
            NullsOrder::First => sea_query::NullOrdering::First,
            NullsOrder::Last => sea_query::NullOrdering::Last,
        }
    }
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
    field: Identifier,
    order: SortOrder,
    nulls: Option<NullsOrder>,
}

impl OrderByExpr {
    pub(crate) fn new(field: Identifier, order: SortOrder) -> Self {
        Self {
            field,
            order,
            nulls: None,
        }
    }

    #[must_use]
    pub fn nulls_first(mut self) -> Self {
        self.nulls = Some(NullsOrder::First);
        self
    }

    #[must_use]
    pub fn nulls_last(mut self) -> Self {
        self.nulls = Some(NullsOrder::Last);
        self
    }

    pub(crate) fn add_to_statement(&self, statement: &mut sea_query::SelectStatement) {
        match self.nulls {
            None => {
                statement.order_by(self.field, self.order.into());
            }
            Some(nulls) => {
                statement.order_by_with_nulls(self.field, self.order.into(), nulls.into());
            }
        };
    }
}

pub trait ExprSort {
    fn asc(&self) -> OrderByExpr;
    fn desc(&self) -> OrderByExpr;
}

impl<T> ExprSort for FieldRef<T> {
    fn asc(&self) -> OrderByExpr {
        OrderByExpr::new(self.identifier(), SortOrder::Asc)
    }

    fn desc(&self) -> OrderByExpr {
        OrderByExpr::new(self.identifier(), SortOrder::Desc)
    }
}
