use std::fmt::Write;

use super::{Node, group_by_app, wrap_label};
use crate::utils::graph::Graph;

pub(super) fn render(nodes: &[Node<'_>], graph: &Graph) -> String {
    #[allow(clippy::allow_attributes, clippy::wildcard_imports)]
    use super::style::*;

    let mut out = String::new();
    let _ = writeln!(out, "digraph migrations {{");
    let _ = writeln!(out, "  rankdir=LR;");
    let _ = writeln!(out, "  splines=spline;");
    let _ = writeln!(out, "  nodesep=0.4;");
    let _ = writeln!(out, "  ranksep=0.6;");
    let _ = writeln!(out, "  bgcolor=\"transparent\";\n");

    let _ = writeln!(out, "  graph [fontname=\"{FONT_FAMILY}\"];");
    let _ = writeln!(out, "  node  [fontname=\"{FONT_FAMILY}\", fontsize=11];");
    let _ = writeln!(out, "  edge  [fontname=\"{FONT_FAMILY}\", fontsize=9];\n");

    let _ = writeln!(out, "  node [");
    let _ = writeln!(out, "    shape=box,");
    let _ = writeln!(out, "    style=\"rounded,filled\",");
    let _ = writeln!(out, "    fillcolor=\"{NODE_FILL}\",");
    let _ = writeln!(out, "    color=\"{NODE_STROKE}\",");
    let _ = writeln!(out, "    fontcolor=\"{NODE_TEXT}\",");
    let _ = writeln!(out, "    penwidth=1,");
    let _ = writeln!(out, "    margin=\"0.18,0.12\"");
    let _ = writeln!(out, "  ];\n");

    let _ = writeln!(out, "  edge [");
    let _ = writeln!(out, "    color=\"{EDGE_COLOR}\",");
    let _ = writeln!(out, "    penwidth=1.2,");
    let _ = writeln!(out, "    arrowsize=0.8");
    let _ = writeln!(out, "  ];\n");

    for (cluster_index, (app, indices)) in group_by_app(nodes).into_iter().enumerate() {
        let _ = writeln!(out, "  subgraph cluster_{cluster_index} {{");
        let _ = writeln!(out, "    label=\"{}\";", escape_dot(app));
        let _ = writeln!(out, "    style=\"rounded,filled\";");
        let _ = writeln!(out, "    color=\"{CLUSTER_STROKE}\";");
        let _ = writeln!(out, "    fillcolor=\"{CLUSTER_FILL}\";");
        let _ = writeln!(out, "    fontcolor=\"{CLUSTER_TEXT}\";");
        let _ = writeln!(out, "    fontsize=12;");
        let _ = writeln!(out, "    margin=12;");
        for i in indices {
            let dot_label = wrap_label(nodes[i].label)
                .iter()
                .map(|line| escape_dot(line))
                .collect::<Vec<_>>()
                .join("\\n");
            let _ = writeln!(out, "    {} [label=\"{}\"];", nodes[i].id, dot_label);
        }
        let _ = writeln!(out, "  }}");
    }
    out.push('\n');

    for (index, node) in nodes.iter().enumerate() {
        for &dependent in graph.get_edges(index) {
            let _ = writeln!(out, "  {} -> {};", node.id, nodes[dependent].id);
        }
    }

    out.push_str("}\n");
    out
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use cot::db::migrations::GraphFormat;
    use cot::db::migrations::graph_export::GraphExporter;

    use crate::db::migrations::graph_export::dot::escape_dot;
    use crate::db::migrations::graph_export::style::*;
    use crate::db::migrations::{MigrationDependency, MigrationWrapper};
    use crate::test::TestMigration;

    fn wrap(migrations: Vec<TestMigration>) -> Vec<MigrationWrapper> {
        migrations.into_iter().map(MigrationWrapper::new).collect()
    }

    fn render_dot(migrations: &[MigrationWrapper]) -> String {
        let exporter = GraphExporter::new(migrations);
        exporter.export(GraphFormat::Dot).unwrap()
    }

    #[test]
    fn dot_contains_edge_and_cluster() {
        let migrations = wrap(vec![
            TestMigration::new("app1", "m1", [], []),
            TestMigration::new(
                "app1",
                "m2",
                [MigrationDependency::migration("app1", "m1")],
                [],
            ),
        ]);

        let dot = render_dot(&migrations);

        assert!(dot.contains("digraph migrations"));
        assert!(dot.contains("subgraph cluster_0"));
        assert!(dot.contains("n0 -> n1;"));
        assert!(dot.contains(NODE_FILL));
    }

    #[test]
    fn escapes_quotes_in_labels() {
        assert_eq!(escape_dot(r#"a"b"#), r#"a\"b"#);
    }

    #[test]
    fn escape_dot_empty_string() {
        assert_eq!(escape_dot(""), "");
    }

    #[test]
    fn escape_dot_backslash_only() {
        assert_eq!(escape_dot(r"a\b"), r"a\\b");
    }

    #[test]
    fn escape_dot_backslash_and_quote_combined() {
        let escaped = escape_dot("a\\\"b");
        assert_eq!(escaped.matches('\\').count(), 3);
        assert_eq!(escaped.matches('"').count(), 1);
        assert!(escaped.starts_with('a'));
        assert!(escaped.ends_with('b'));
    }

    #[test]
    fn dot_wraps_long_label_with_literal_newline() {
        let migrations = wrap(vec![TestMigration::new(
            "app1",
            "m_0002_auto_20260527_004236",
            [],
            [],
        )]);
        assert!(render_dot(&migrations).contains("\\n"));
    }

    #[test]
    fn dot_single_migration_no_edges() {
        let migrations = wrap(vec![TestMigration::new("solo", "m1", [], [])]);
        let dot = render_dot(&migrations);

        assert!(dot.contains("subgraph cluster_0"));
        assert!(dot.contains("n0 [label=\"m1\"];"));
        assert!(!dot.contains("->"));
    }

    #[test]
    fn dot_clusters_sorted_alphabetically_by_app() {
        let migrations = wrap(vec![
            TestMigration::new("zeta", "m1", [], []),
            TestMigration::new("alpha", "m1", [], []),
        ]);
        let dot = render_dot(&migrations);

        let alpha_pos = dot.find("label=\"alpha\";").expect("alpha cluster present");
        let zeta_pos = dot.find("label=\"zeta\";").expect("zeta cluster present");
        assert!(alpha_pos < zeta_pos);
    }

    #[test]
    fn dot_multiple_migrations_same_app_share_one_cluster() {
        let migrations = wrap(vec![
            TestMigration::new("app1", "m1", [], []),
            TestMigration::new(
                "app1",
                "m2",
                [MigrationDependency::migration("app1", "m1")],
                [],
            ),
        ]);
        assert_eq!(
            render_dot(&migrations).matches("subgraph cluster_").count(),
            1
        );
    }

    #[test]
    fn node_ids_assigned_in_input_order_not_sorted_order() {
        let migrations = wrap(vec![
            TestMigration::new("zeta", "first", [], []),
            TestMigration::new("alpha", "second", [], []),
        ]);
        let dot = render_dot(&migrations);

        assert!(dot.contains("n0 [label=\"first\"];"));
        assert!(dot.contains("n1 [label=\"second\"];"));
    }

    #[test]
    fn dot_diamond_dependency_all_edges_rendered() {
        let migrations = wrap(vec![
            TestMigration::new("diamond", "a", [], []),
            TestMigration::new(
                "diamond",
                "b",
                [MigrationDependency::migration("diamond", "a")],
                [],
            ),
            TestMigration::new(
                "diamond",
                "c",
                [MigrationDependency::migration("diamond", "a")],
                [],
            ),
            TestMigration::new(
                "diamond",
                "d",
                [
                    MigrationDependency::migration("diamond", "b"),
                    MigrationDependency::migration("diamond", "c"),
                ],
                [],
            ),
        ]);
        let dot = render_dot(&migrations);

        assert!(dot.contains("n0 -> n1;"));
        assert!(dot.contains("n0 -> n2;"));
        assert!(dot.contains("n1 -> n3;"));
        assert!(dot.contains("n2 -> n3;"));
        assert_eq!(dot.matches("->").count(), 4);
    }

    #[test]
    fn dot_cross_app_dependency_edge_render() {
        let migrations = wrap(vec![
            TestMigration::new("upstream", "m1", [], []),
            TestMigration::new(
                "downstream",
                "m1",
                [MigrationDependency::migration("upstream", "m1")],
                [],
            ),
        ]);
        let dot = render_dot(&migrations);

        assert!(dot.contains("n0 -> n1;"));
        assert_eq!(dot.matches("subgraph cluster_").count(), 2);
    }

    #[test]
    fn dot_render_does_not_fail_on_cyclic_dependencies() {
        let migrations = wrap(vec![
            TestMigration::new(
                "cyclic",
                "a",
                [MigrationDependency::migration("cyclic", "b")],
                [],
            ),
            TestMigration::new(
                "cyclic",
                "b",
                [MigrationDependency::migration("cyclic", "a")],
                [],
            ),
        ]);
        let dot = render_dot(&migrations);

        assert!(dot.contains("n0 -> n1;"));
        assert!(dot.contains("n1 -> n0;"));
    }

    #[test]
    fn dot_escapes_quotes_in_app_name_cluster_label() {
        let migrations = wrap(vec![TestMigration::new("weird\"app", "m1", [], [])]);
        assert!(render_dot(&migrations).contains("weird\\\"app"));
    }
}
