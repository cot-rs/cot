use cot::db::{DbFieldValue, ToDbFieldValue};
use sea_query::Values;

use crate::db::query::expr::FieldRef;
use crate::db::{DbValues, Identifier, ToDbValue};

/// Ordering Options
#[derive(Debug, Clone, PartialEq)]
pub enum SortOrder {
    /// Sort in Ascending order.
    Asc,
    /// Sort in Descending Order.
    Desc,

    Custom(DbValues),
}

impl From<&SortOrder> for sea_query::Order {
    fn from(value: &SortOrder) -> Self {
        match value {
            SortOrder::Asc => sea_query::Order::Asc,
            SortOrder::Desc => sea_query::Order::Desc,
            SortOrder::Custom(v) => sea_query::Order::Field(v.clone()),
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
        self.set_nulls(NullsOrder::First);
        self
    }

    #[must_use]
    pub fn nulls_last(mut self) -> Self {
        self.set_nulls(NullsOrder::Last);
        self
    }

    #[track_caller]
    fn set_nulls(&mut self, nulls: NullsOrder) {
        match &mut self.order {
            SortOrder::Asc | SortOrder::Desc => self.nulls = Some(nulls),
            SortOrder::Custom(_) => panic!(
                "`nulls_first`/`nulls_last` can't be combined with `custom_order`: a custom-order term never produces \
                 a NULL sort key, so an explicit NULLS placement would have no effect"
            ),
        }
    }

    pub(crate) fn add_to_statement(&self, statement: &mut sea_query::SelectStatement) {
        let order: sea_query::Order = (&self.order).into();
        if let Some(nulls) = self.nulls {
            let nulls: sea_query::NullOrdering = nulls.into();
            statement.order_by_with_nulls(self.field, order, nulls);
        } else {
            statement.order_by(self.field, order);
        };
    }
}

pub trait ExprSort<T> {
    fn asc(&self) -> OrderByExpr;
    fn desc(&self) -> OrderByExpr;

    fn custom<I>(&self, values: I) -> OrderByExpr
    where
        I: IntoIterator,
        I::Item: ToDbValue;
}

impl<T: ToDbValue + 'static> ExprSort<T> for FieldRef<T> {
    fn asc(&self) -> OrderByExpr {
        OrderByExpr::new(self.identifier(), SortOrder::Asc)
    }

    fn desc(&self) -> OrderByExpr {
        OrderByExpr::new(self.identifier(), SortOrder::Desc)
    }

    fn custom<I>(&self, values: I) -> OrderByExpr
    where
        I: IntoIterator,
        I::Item: ToDbValue,
    {
        let values = values
            .into_iter()
            .map(|v| match v.to_db_field_value() {
                DbFieldValue::Value(value) => value,
                DbFieldValue::Auto => panic!("Cannot order by a non-value field"),
            })
            .collect::<Vec<_>>();
        OrderByExpr::new(self.identifier(), SortOrder::Custom(Values(values)))
    }
}
