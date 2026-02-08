//! Database-backed session management backend.
//!
//! This module provides a session type and an app for storing session data
//! in a database using the Cot ORM.
pub mod migrations;

use cot::db::migrations::SyncDynMigration;

use crate::App;
use crate::db::{Auto, model};

/// Session data stored in the database.
#[derive(Debug, Clone)]
#[model]
pub struct Session {
    #[model(primary_key)]
    pub(crate) id: Auto<i32>,
    #[model(unique)]
    pub(crate) key: String,
    pub(crate) data: String,
    pub(crate) expiry: chrono::DateTime<chrono::FixedOffset>,
}

/// An app that provides session management via a session model stored in the
/// database.
///
/// This app registers the session model and its migrations, enabling persistent
/// session storage in the database.
///
/// # Examples
///
/// ```no_run
/// use cot::config::{DatabaseConfig, ProjectConfig};
/// use cot::project::RegisterAppsContext;
/// use cot::session::db::SessionApp;
/// use cot::{App, AppBuilder, Project};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
///         Ok(ProjectConfig::builder()
///             .database(DatabaseConfig::builder().url("sqlite::memory:").build())
///             .build())
///     }
///
///     fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
///         apps.register_with_views(SessionApp::new(), "");
///     }
/// }
///
/// #[cot::main]
/// fn main() -> impl Project {
///     MyProject
/// }
/// ```

#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub struct SessionApp;

impl SessionApp {
    /// Create a new instance of the session management app.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::session::db::SessionApp;
    /// let app = SessionApp::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SessionApp {
    fn default() -> Self {
        Self::new()
    }
}

impl App for SessionApp {
    fn name(&self) -> &'static str {
        "cot_session"
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;

    #[test]
    fn test_session_app_basic_behavior() {
        let app1 = SessionApp::new();

        let app2 = SessionApp::default();

        assert_eq!(app1.name(), "cot_session");
        assert_eq!(app2.name(), "cot_session");

        let migrations = app1.migrations();
        assert!(!migrations.is_empty());
    }
}
