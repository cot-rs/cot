//! The canonical user type of a Cot project.
//!
//! The canonical user is the main user type of a project: authentication
//! backends resolve credentials to it.
//!
//! The canonical user is a regular Cot ORM [`Model`] defined by the project,
//! so it can store any data the project needs.
//!
//! If the project doesn't have users at all, use [`NoUser`]. This is also the
//! only canonical user type available without the `db` feature.

use std::borrow::Cow;

use crate::auth::UserId;
#[cfg(feature = "db")]
use crate::db::{Auto, model};

/// The main user object of the Cot authentication system.
///
/// A canonical user must be a [`Model`](crate::db::Model) with an `Auto<i64>`
/// primary key. Without the `db` feature, the only canonical user type is
/// [`NoUser`].
///
/// # Examples
///
/// ```
/// use std::borrow::Cow;
///
/// use cot::auth::canonical::CanonicalUser;
/// use cot::db::{Auto, LimitedString, model};
///
/// #[model]
/// struct MyUser {
///     #[model(primary_key)]
///     id: Auto<i64>,
///     display_name: LimitedString<255>,
///     is_staff: bool,
/// }
///
/// impl CanonicalUser for MyUser {
///     fn display_name(&self) -> Option<Cow<'_, str>> {
///         Some(Cow::Borrowed(&self.display_name))
///     }
///
///     fn is_admin(&self) -> bool {
///         self.is_staff
///     }
/// }
/// ```
pub trait CanonicalUser: private::UserModel + Send + Sync + 'static {
    /// Returns the ID of the user.
    ///
    /// This returns [`None`] if the user hasn't been saved to the database
    /// yet.
    fn id(&self) -> Option<UserId> {
        #[cfg(feature = "db")]
        match self.primary_key() {
            Auto::Fixed(id) => Some(UserId::Int(*id)),
            Auto::Auto => None,
        }
        #[cfg(not(feature = "db"))]
        None
    }

    /// Returns the name of the user that can be displayed to humans, if there
    /// is one.
    fn display_name(&self) -> Option<Cow<'_, str>>;

    /// Returns whether the user is active.
    ///
    /// Inactive users can't be logged in. If an already logged-in user
    /// becomes inactive, they are logged out on their next request.
    ///
    /// The default implementation returns `true`.
    fn is_active(&self) -> bool {
        true
    }

    /// Returns whether the user has access to the admin panel.
    ///
    /// The default implementation returns `false`.
    fn is_admin(&self) -> bool {
        false
    }
}

/// The canonical user type for projects that don't have users.
///
/// Nobody can be logged in as this user: it can't be constructed, its table
/// doesn't exist, and it is never [active](CanonicalUser::is_active).
#[cfg(feature = "db")]
#[derive(Debug, Clone)]
#[model(table_name = "cot__no_user", model_type = "internal")]
pub struct NoUser {
    #[model(primary_key)]
    id: Auto<i64>,
}

/// The canonical user type for projects that don't have users.
///
/// This is just a mock object, because the `db` feature is disabled and
/// therefore the authentication system cannot be used anyways.
#[cfg(not(feature = "db"))]
#[derive(Debug, Clone)]
#[expect(
    missing_copy_implementations,
    reason = "NoUser cannot be Copy, as it depends on `Model` if the `db` feature is enabled"
)]
pub struct NoUser {
    _private: (),
}

impl CanonicalUser for NoUser {
    fn display_name(&self) -> Option<Cow<'_, str>> {
        None
    }

    fn is_active(&self) -> bool {
        false
    }
}

/// The requirements for a [`CanonicalUser`] that depend on the enabled
/// features.
///
/// This is necessary because otherwise we would have to duplicate the
/// definition of `CanonicalUser` due to the supertrait dependence on `Model`
mod private {
    #[cfg(feature = "db")]
    pub trait UserModel: crate::db::Model<PrimaryKey = crate::db::Auto<i64>> {}

    #[cfg(feature = "db")]
    impl<T: crate::db::Model<PrimaryKey = crate::db::Auto<i64>>> UserModel for T {}

    #[cfg(not(feature = "db"))]
    pub trait UserModel {}

    #[cfg(not(feature = "db"))]
    impl UserModel for super::NoUser {}
}
