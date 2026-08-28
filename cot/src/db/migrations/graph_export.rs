//! Rendering of the migration dependency graph for external visualization
//! tools (Graphviz `dot`, Mermaid).
mod dot;
mod mermaid;

use std::collections::HashMap;

use cot::db::migrations::MigrationEngineError;

use crate::db::migrations::DynMigration;
use crate::db::migrations::sorter::MigrationSorter;

/// The output format for a rendered migration dependency graph.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub(crate) enum GraphFormat {
    /// [Graphviz DOT](https://graphviz.org/doc/info/lang.html) format.
    Dot,
    /// [Mermaid](https://mermaid.js.org/syntax/flowchart.html) flowchart syntax.
    Mermaid,
}

mod style {
    /// Node fill color.
    pub(super) const NODE_FILL: &str = "#eef2ff";
    /// Node border color.
    pub(super) const NODE_STROKE: &str = "#4c51bf";
    /// Node label text color.
    pub(super) const NODE_TEXT: &str = "#1e1b4b";

    /// Cluster (app group) fill color.
    pub(super) const CLUSTER_FILL: &str = "#f9fafb";
    /// Cluster border color.
    pub(super) const CLUSTER_STROKE: &str = "#d1d5db";
    /// Cluster title text color.
    pub(super) const CLUSTER_TEXT: &str = "#374151";

    /// Edge/arrow color.
    pub(super) const EDGE_COLOR: &str = "#9aa5b1";

    /// Font family used for node and cluster labels (DOT only; Mermaid picks
    /// up the surrounding theme's font).
    pub(super) const FONT_FAMILY: &str = "Helvetica,Arial,sans-serif";

    /// Maximum label line length before we wrap to the next line.
    pub(super) const LABEL_WRAP_WIDTH: usize = 16;
}

struct Node<'a> {
    id: String,
    app: &'a str,
    label: &'a str,
}

pub(crate) struct GraphExporter<'a, T> {
    migrations: &'a [T],
}

impl<'a, T: DynMigration> GraphExporter<'a, T> {
    pub(crate) fn new(migrations: &'a [T]) -> Self {
        Self { migrations }
    }
    pub(crate) fn export(&self, format: GraphFormat) -> super::Result<String> {
        let graph = MigrationSorter::generate_graph(self.migrations).map_err(|e| {
            MigrationEngineError::Custom(format!("Failed to generate migration graph: {e}"))
        })?;

        let nodes = self
            .migrations
            .iter()
            .enumerate()
            .map(|(i, m)| Node {
                id: format!("n{i}"),
                app: m.app_name(),
                label: m.name(),
            })
            .collect::<Vec<_>>();

        Ok(match format {
            GraphFormat::Dot => dot::render(&nodes, &graph),
            GraphFormat::Mermaid => mermaid::render(&nodes, &graph),
        })
    }
}

fn wrap_label(label: &str) -> Vec<String> {
    if label.len() <= style::LABEL_WRAP_WIDTH {
        return vec![label.to_owned()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for segment in label.split('_') {
        let candidate_len = if current.is_empty() {
            segment.len()
        } else {
            current.len() + 1 + segment.len()
        };

        if candidate_len > style::LABEL_WRAP_WIDTH && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }

        if !current.is_empty() {
            current.push('_');
        }
        current.push_str(segment);

        if current.len() > style::LABEL_WRAP_WIDTH {
            lines.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn group_by_app<'a>(nodes: &[Node<'a>]) -> Vec<(&'a str, Vec<usize>)> {
    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();

    for (i, node) in nodes.iter().enumerate() {
        groups.entry(node.app).or_default().push(i);
    }
    let mut ord = groups.into_iter().collect::<Vec<_>>();
    ord.sort();
    ord
}

#[cfg(test)]
mod tests {
    use cot::auth::db::DatabaseUserApp;
    use cot::db::migrations::{
        Field, Migration, MigrationDependency, MigrationEngine, Operation, SyncDynMigration,
        wrap_migrations,
    };
    use cot::db::{DatabaseField, Identifier};
    use cot::session::db::SessionApp;

    use super::*;
    use crate::App;
    use crate::db::migrations::MigrationWrapper;
    use crate::test::TestMigration;

    const SNAPSHOT_RELATIVE_PATH: &str = "../../../tests/db_testing/snapshots/migrations";

    struct App1Initial;

    impl Migration for App1Initial {
        const APP_NAME: &'static str = "app1";
        const MIGRATION_NAME: &'static str = "m_0001_initial";
        const DEPENDENCIES: &'static [MigrationDependency] = &[];
        const OPERATIONS: &'static [Operation] = &[Operation::create_model()
            .table_name(Identifier::new("single__first"))
            .fields(&[
                Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                    .primary_key()
                    .auto(),
            ])
            .build()];
    }

    struct App10002;

    impl Migration for App10002 {
        const APP_NAME: &'static str = "app1";
        const MIGRATION_NAME: &'static str = "m_0002_second";
        const DEPENDENCIES: &'static [MigrationDependency] =
            &[MigrationDependency::migration("app1", "m_0001_initial")];
        const OPERATIONS: &'static [Operation] = &[Operation::create_model()
            .table_name(Identifier::new("app1__second"))
            .fields(&[
                Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                    .primary_key()
                    .auto(),
            ])
            .build()];
    }

    struct App1003;

    impl Migration for App1003 {
        const APP_NAME: &'static str = "app1";
        const MIGRATION_NAME: &'static str = "m_0003_third";
        const DEPENDENCIES: &'static [MigrationDependency] =
            &[MigrationDependency::migration("app1", "m_0002_second")];
        const OPERATIONS: &'static [Operation] = &[Operation::create_model()
            .table_name(Identifier::new("single__third"))
            .fields(&[
                Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                    .primary_key()
                    .auto(),
            ])
            .build()];
    }

    struct App2Initial;

    impl Migration for App2Initial {
        const APP_NAME: &'static str = "app2";
        const MIGRATION_NAME: &'static str = "m_0001_initial";
        const DEPENDENCIES: &'static [MigrationDependency] = &[];
        const OPERATIONS: &'static [Operation] = &[Operation::create_model()
            .table_name(Identifier::new("app2__foo"))
            .fields(&[
                Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                    .primary_key()
                    .auto(),
            ])
            .build()];
    }

    struct DependentInitial;

    impl Migration for DependentInitial {
        const APP_NAME: &'static str = "dependent";
        const MIGRATION_NAME: &'static str = "m_0001_initial";
        const DEPENDENCIES: &'static [MigrationDependency] =
            &[MigrationDependency::migration("app1", "m_0002_second")];
        const OPERATIONS: &'static [Operation] = &[Operation::create_model()
            .table_name(Identifier::new("dependent__bar"))
            .fields(&[
                Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                    .primary_key()
                    .auto(),
            ])
            .build()];
    }

    fn render_mermaid(migrations: &[MigrationWrapper]) -> String {
        let exporter = GraphExporter::new(migrations);
        exporter.export(GraphFormat::Mermaid).unwrap()
    }

    fn render_dot(migrations: &[MigrationWrapper]) -> String {
        let exporter = GraphExporter::new(migrations);
        exporter.export(GraphFormat::Dot).unwrap()
    }

    fn wrap(migrations: Vec<TestMigration>) -> Vec<MigrationWrapper> {
        migrations.into_iter().map(MigrationWrapper::new).collect()
    }

    #[test]
    fn wrap_label_short_label_unchanged() {
        assert_eq!(wrap_label("m_0001_initial"), vec!["m_0001_initial"]);
    }

    #[test]
    fn wrap_label_long_label_splits_on_underscore() {
        let lines = wrap_label("m_0002_auto_20260527_004236");
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.len() <= style::LABEL_WRAP_WIDTH + 8));
        assert_eq!(lines.join("_"), "m_0002_auto_20260527_004236");
    }

    #[test]
    fn dot_render_empty_migrations() {
        let migrations: Vec<MigrationWrapper> = Vec::new();
        let exporter = GraphExporter::new(&migrations);
        let dot = exporter.export(GraphFormat::Dot).unwrap();

        assert!(dot.starts_with("digraph migrations {"));
        assert!(dot.trim_end().ends_with('}'));
        assert!(!dot.contains("subgraph"));
        assert!(!dot.contains("->"));
    }

    #[test]
    fn mermaid_render_empty_migrations() {
        let migrations: Vec<MigrationWrapper> = Vec::new();
        let exporter = GraphExporter::new(&migrations);
        let mermaid = exporter.export(GraphFormat::Mermaid).unwrap();

        assert!(mermaid.contains("flowchart LR"));
        assert!(!mermaid.contains("subgraph"));
        assert!(!mermaid.contains("-->"));
        assert!(!mermaid.contains("n0"));
    }

    #[test]
    fn render_dispatches_dot_vs_mermaid() {
        let migrations = wrap(vec![TestMigration::new("app", "m1", [], [])]);
        let exporter = GraphExporter::new(&migrations);
        let dot = exporter.export(GraphFormat::Dot).unwrap();
        let mermaid = exporter.export(GraphFormat::Mermaid).unwrap();

        assert!(dot.contains("digraph migrations"));
        assert!(!dot.contains("flowchart"));
        assert!(mermaid.contains("flowchart LR"));
        assert!(!mermaid.contains("digraph"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: socketpair: type 0x5 is unsupported, only SOCK_STREAM, SOCK_CLOEXEC and SOCK_NONBLOCK are allowed"
    )]
    fn test_migration_graph_single_app() {
        #[expect(trivial_casts)]
        let engine = MigrationEngine::new([
            &App1Initial as &SyncDynMigration,
            &App10002 as &SyncDynMigration,
            &App1003 as &SyncDynMigration,
        ])
        .unwrap();
        let dot = render_dot(engine.migrations());
        insta::with_settings!({snapshot_path => SNAPSHOT_RELATIVE_PATH}, {
            insta::assert_snapshot!("migration_graph_dot_single_app", dot);
        });

        let mermaid = render_mermaid(engine.migrations());
        insta::with_settings!({snapshot_path => SNAPSHOT_RELATIVE_PATH}, {
            insta::assert_snapshot!("migration_graph_mermaid_single_app", mermaid);
        });
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: socketpair: type 0x5 is unsupported, only SOCK_STREAM, SOCK_CLOEXEC and SOCK_NONBLOCK are allowed"
    )]
    fn test_migration_graph_unrelated_apps() {
        let mut migrations = DatabaseUserApp::new().migrations();

        #[expect(trivial_casts)]
        migrations.extend(wrap_migrations(&[
            &App1Initial as &SyncDynMigration,
            &App10002 as &SyncDynMigration,
            &App2Initial as &SyncDynMigration,
        ]));
        migrations.extend(SessionApp::new().migrations());

        let engine = MigrationEngine::new(migrations).unwrap();

        let dot = render_dot(engine.migrations());
        insta::with_settings!({snapshot_path => SNAPSHOT_RELATIVE_PATH}, {
            insta::assert_snapshot!("migration_graph_dot_unrelated_apps", dot);
        });

        let mermaid = render_mermaid(engine.migrations());
        insta::with_settings!({snapshot_path => SNAPSHOT_RELATIVE_PATH}, {
            insta::assert_snapshot!("migration_graph_mermaid_unrelated_apps", mermaid);
        });
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: socketpair: type 0x5 is unsupported, only SOCK_STREAM, SOCK_CLOEXEC and SOCK_NONBLOCK are allowed"
    )]
    fn test_migration_graph_dependent_apps() {
        #[expect(trivial_casts)]
        let engine = MigrationEngine::new([
            &App1Initial as &SyncDynMigration,
            &App10002 as &SyncDynMigration,
            &DependentInitial as &SyncDynMigration,
            &App2Initial as &SyncDynMigration,
        ])
        .unwrap();
        let dot = render_dot(engine.migrations());
        insta::with_settings!({snapshot_path => SNAPSHOT_RELATIVE_PATH}, {
            insta::assert_snapshot!("migration_graph_dot_dependent_apps", dot);
        });

        let mermaid = render_mermaid(engine.migrations());
        insta::with_settings!({snapshot_path => SNAPSHOT_RELATIVE_PATH}, {
            insta::assert_snapshot!("migration_graph_mermaid_dependent_apps", mermaid);
        });
    }
}
