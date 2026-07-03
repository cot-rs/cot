//! Database query builder.

pub mod expr;

use std::marker::PhantomData;

use derive_more::with_trait::Debug;
use sea_query::ExprTrait;
use thiserror::Error;

use crate::db;
use crate::db::query::expr::SqlDialect;
pub use crate::db::query::expr::{Expr, ExprAdd, ExprDiv, ExprMul, ExprOrd, ExprSub};
use crate::db::{
    Auto, Database, DatabaseBackend, ForeignKey, Model, StatementResult, ToDbFieldValue,
};
const ERROR_PREFIX: &str = "expression error:";

/// An error that can occur when building a query.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum QueryBuildingError {
    /// Error when building an expression.
    #[error("{ERROR_PREFIX} unsupported expression: {0}")]
    UnsupportedExpr(String),
    /// Error when building a query.
    #[error(transparent)]
    SeaQuery(#[from] sea_query::error::Error),
}

/// A query that can be executed on a database. Can be used to filter, update,
/// or delete rows.
///
/// # Example
///
/// ```
/// use cot::db::model;
/// use cot::db::query::Query;
///
/// #[model]
/// struct User {
///     #[model(primary_key)]
///     id: i32,
///     name: String,
///     age: i32,
/// }
///
/// let query = Query::<User>::new();
/// ```
pub struct Query<T> {
    filter: Option<Expr>,
    limit: Option<u64>,
    offset: Option<u64>,
    phantom_data: PhantomData<fn() -> T>,
}

// manual implementation to avoid `T: Debug` in the trait bounds
impl<T> Debug for Query<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query")
            .field("filter", &self.filter)
            .field("limit", &self.limit)
            .field("offset", &self.offset)
            .field("phantom_data", &self.phantom_data)
            .finish()
    }
}

// manual implementation to avoid `T: Clone` in the trait bounds
impl<T> Clone for Query<T> {
    fn clone(&self) -> Self {
        Self {
            filter: self.filter.clone(),
            limit: self.limit,
            offset: self.offset,
            phantom_data: PhantomData,
        }
    }
}

// manual implementation to avoid `T: PartialEq` in the trait bounds
impl<T> PartialEq for Query<T> {
    fn eq(&self, other: &Self) -> bool {
        self.filter == other.filter
    }
}

impl<T: Model> Default for Query<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Model> Query<T> {
    /// Create a new query.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::model;
    /// use cot::db::query::Query;
    ///
    /// #[model]
    /// struct User {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    ///     age: i32,
    /// }
    ///
    /// let query = Query::<User>::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            filter: None,
            limit: None,
            offset: None,
            phantom_data: PhantomData,
        }
    }

    /// Set the filter expression for the query.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::model;
    /// use cot::db::query::{Expr, Query};
    ///
    /// #[model]
    /// struct User {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    ///     age: i32,
    /// }
    ///
    /// let query = Query::<User>::new().filter(Expr::eq(Expr::field("name"), Expr::value("John")));
    /// ```
    pub fn filter(&mut self, filter: Expr) -> &mut Self {
        self.filter = Some(filter);
        self
    }

    /// Set the limit for the query.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::model;
    /// use cot::db::query::{Expr, Query};
    ///
    /// #[model]
    /// struct User {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    ///     age: i32,
    /// }
    ///
    /// let query = Query::<User>::new().limit(10);
    /// ```
    pub fn limit(&mut self, limit: u64) -> &mut Self {
        self.limit = Some(limit);
        self
    }

    /// Set the offset for the query.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::db::model;
    /// use cot::db::query::{Expr, Query};
    ///
    /// #[model]
    /// struct User {
    ///     #[model(primary_key)]
    ///     id: i32,
    ///     name: String,
    ///     age: i32,
    /// }
    ///
    /// let query = Query::<User>::new().offset(10);
    /// ```
    pub fn offset(&mut self, offset: u64) -> &mut Self {
        self.offset = Some(offset);
        self
    }

    /// Execute the query and return all results.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn all<DB: DatabaseBackend>(&self, db: &DB) -> db::Result<Vec<T>> {
        db.query(self).await
    }

    /// Execute the query and return the first result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn get<DB: DatabaseBackend>(&self, db: &DB) -> db::Result<Option<T>> {
        // TODO panic/error if more than one result
        db.get(self).await
    }

    /// Execute the query and return the number of results.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn count(&self, db: &Database) -> db::Result<u64> {
        let mut select = sea_query::Query::select();
        select
            .from(T::TABLE_NAME)
            .expr(sea_query::Expr::col(sea_query::Asterisk).count());
        self.add_filter_to_statement(&mut select, db)?;
        let row = db.fetch_option(&select).await?;
        let count = match row {
            #[expect(clippy::cast_sign_loss)]
            Some(row) => row.get::<i64>(0)? as u64,
            None => 0,
        };
        Ok(count)
    }

    /// Execute the query and check if any results exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn exists<DB: DatabaseBackend>(&self, db: &DB) -> db::Result<bool> {
        db.exists(self).await
    }

    /// Delete all rows that match the query.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn delete<DB: DatabaseBackend>(&self, db: &DB) -> db::Result<StatementResult> {
        db.delete(self).await
    }

    pub(super) fn add_filter_to_statement<S: sea_query::ConditionalStatement>(
        &self,
        statement: &mut S,
        db: &dyn SqlDialect,
    ) -> Result<(), QueryBuildingError> {
        if let Some(filter) = &self.filter {
            statement.and_where(filter.as_sea_query_expr(db)?);
        }
        Ok(())
    }

    pub(super) fn add_limit_to_statement(&self, statement: &mut sea_query::SelectStatement) {
        if let Some(limit) = self.limit {
            statement.limit(limit);
        }
    }

    pub(super) fn add_offset_to_statement(&self, statement: &mut sea_query::SelectStatement) {
        if let Some(offset) = self.offset {
            statement.offset(offset);
        }
    }
}

/// A trait for database types that can be converted to the field type.
///
/// This trait is mostly a helper trait to make comparisons like `$id == 5`
/// where `id` is of type [`Auto`] or [`ForeignKey`] easier to write and more
/// readable.
///
/// # Example
///
/// ```
/// use cot::db::query::Query;
/// use cot::db::query::expr::{Expr, ExprEq};
/// use cot::db::{Auto, model, query};
///
/// #[model]
/// struct MyModel {
///     #[model(primary_key)]
///     id: Auto<i32>,
/// };
///
/// // uses the `IntoField` trait to convert the `5` to `Auto<i32>`
/// let expr = <MyModel as cot::db::Model>::Fields::id.eq(5);
/// ```
pub trait IntoField<T> {
    /// Converts the type to the field type.
    fn into_field(self) -> T;
}

impl<T: ToDbFieldValue> IntoField<T> for T {
    fn into_field(self) -> T {
        self
    }
}

impl<T> IntoField<Auto<T>> for T {
    fn into_field(self) -> Auto<T> {
        Auto::fixed(self)
    }
}

impl IntoField<String> for &str {
    fn into_field(self) -> String {
        self.to_string()
    }
}

impl<T: Model + Send + Sync> IntoField<ForeignKey<T>> for T {
    fn into_field(self) -> ForeignKey<T> {
        ForeignKey::from(self)
    }
}

impl<T: Model + Send + Sync> IntoField<ForeignKey<T>> for &T {
    fn into_field(self) -> ForeignKey<T> {
        ForeignKey::from(self)
    }
}

#[cfg(test)]
mod tests {
    use cot_macros::model;

    use super::*;
    use crate::db::{MockDatabaseBackend, RowsNum};

    #[model]
    #[derive(std::fmt::Debug, PartialEq, Eq)]
    struct MockModel {
        #[model(primary_key)]
        id: i32,
    }

    #[test]
    fn query_new() {
        let query: Query<MockModel> = Query::new();

        assert!(query.filter.is_none());
        assert!(query.limit.is_none());
        assert!(query.offset.is_none());
    }

    #[test]
    fn query_default() {
        let query: Query<MockModel> = Query::default();

        assert!(query.filter.is_none());
        assert!(query.limit.is_none());
        assert!(query.offset.is_none());
    }

    #[test]
    fn query_filter() {
        let mut query: Query<MockModel> = Query::new();

        query.filter(Expr::eq(Expr::field("name"), Expr::value("John")));

        assert!(query.filter.is_some());
    }

    #[test]
    fn query_limit() {
        let mut query: Query<MockModel> = Query::new();
        query.limit(10);
        assert!(query.limit.is_some());
        assert_eq!(query.limit.unwrap(), 10);
    }

    #[test]
    fn query_offset() {
        let mut query: Query<MockModel> = Query::new();
        query.offset(10);
        assert!(query.offset.is_some());
        assert_eq!(query.offset.unwrap(), 10);
    }

    #[cot::test]
    async fn query_all() {
        let mut db = MockDatabaseBackend::new();
        db.expect_query().returning(|_| Ok(Vec::<MockModel>::new()));
        let query: Query<MockModel> = Query::new();

        let result = query.all(&db).await;

        assert_eq!(result.unwrap(), Vec::<MockModel>::new());
    }

    #[cot::test]
    async fn query_get() {
        let mut db = MockDatabaseBackend::new();
        db.expect_get().returning(|_| Ok(Option::<MockModel>::None));
        let query: Query<MockModel> = Query::new();

        let result = query.get(&db).await;

        assert_eq!(result.unwrap(), Option::<MockModel>::None);
    }

    #[cot::test]
    async fn query_exists() {
        let mut db = MockDatabaseBackend::new();
        db.expect_exists()
            .returning(|_: &Query<MockModel>| Ok(false));

        let query: Query<MockModel> = Query::new();

        let result = query.exists(&db).await;
        assert!(result.is_ok());
    }

    #[cot::test]
    async fn query_delete() {
        let mut db = MockDatabaseBackend::new();
        db.expect_delete()
            .returning(|_: &Query<MockModel>| Ok(StatementResult::new(RowsNum(0))));
        let query: Query<MockModel> = Query::new();

        let result = query.delete(&db).await;

        assert!(result.is_ok());
    }
}
