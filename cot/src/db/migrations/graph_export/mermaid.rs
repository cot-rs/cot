use std::fmt::Write;

use super::{Node, group_by_app, style, wrap_label};
use crate::utils::graph::Graph;

pub(super) fn render(nodes: &[Node<'_>], graph: &Graph) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "%%{{init: {{'theme': 'base', 'themeVariables': {{'background': 'transparent'}}}}}}%%"
    );
    let _ = writeln!(out, "flowchart LR");
    let _ = writeln!(
        out,
        "  classDef migration fill:{},stroke:{},stroke-width:1px,color:{},font-size:12px,rx:6,ry:6;\n",
        style::NODE_FILL,
        style::NODE_STROKE,
        style::NODE_TEXT
    );

    let mut all_node_ids = Vec::new();
    let clusters = group_by_app(nodes);

    for (cluster_index, (app, indices)) in clusters.iter().enumerate() {
        let _ = writeln!(
            out,
            "  subgraph cluster{cluster_index}[\"{}\"]",
            escape_mermaid(app)
        );
        for &i in indices {
            let mermaid_label = wrap_label(nodes[i].label)
                .iter()
                .map(|line| escape_mermaid(line))
                .collect::<Vec<_>>()
                .join("<br/>");
            let _ = writeln!(out, "    {}[\"{}\"]", nodes[i].id, mermaid_label);
            all_node_ids.push(nodes[i].id.clone());
        }
        let _ = writeln!(out, "  end");
    }
    out.push('\n');

    for (index, node) in nodes.iter().enumerate() {
        for &dependent in graph.get_edges(index) {
            let _ = writeln!(out, "  {} --> {}", node.id, nodes[dependent].id);
        }
    }
    out.push('\n');

    if !all_node_ids.is_empty() {
        let _ = writeln!(out, "  class {} migration;", all_node_ids.join(","));
    }
    for cluster_index in 0..clusters.len() {
        let _ = writeln!(
            out,
            "  style cluster{cluster_index} fill:{},stroke:{},stroke-width:1px",
            style::CLUSTER_FILL,
            style::CLUSTER_STROKE
        );
    }
    let _ = writeln!(
        out,
        "  linkStyle default stroke:{},stroke-width:1.5px",
        style::EDGE_COLOR
    );

    out
}

fn escape_mermaid(s: &str) -> String {
    s.replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use cot::db::migrations::graph_export::GraphExporter;

    use super::*;
    use crate::db::migrations::{GraphFormat, MigrationDependency, MigrationWrapper};
    use crate::test::TestMigration;

    fn wrap(migrations: Vec<TestMigration>) -> Vec<MigrationWrapper> {
        migrations.into_iter().map(MigrationWrapper::new).collect()
    }

    fn render_mermaid(migrations: &[MigrationWrapper]) -> String {
        let exporter = GraphExporter::new(migrations);
        exporter.export(GraphFormat::Mermaid).unwrap()
    }

    #[test]
    fn mermaid_contains_edge_and_subgraph() {
        let migrations = wrap(vec![
            TestMigration::new("app1", "m1", [], []),
            TestMigration::new(
                "app1",
                "m2",
                [MigrationDependency::migration("app1", "m1")],
                [],
            ),
        ]);

        let mermaid = render_mermaid(&migrations);

        assert!(mermaid.contains("flowchart LR"));
        assert!(mermaid.contains("subgraph cluster0"));
        assert!(mermaid.contains("n0 --> n1"));
        assert!(mermaid.contains("background': 'transparent'"));
        assert!(mermaid.contains("classDef migration"));
    }

    #[test]
    fn escapes_quotes_in_labels() {
        assert_eq!(escape_mermaid(r#"a"b"#), "a&quot;b");
    }

    #[test]
    fn escape_mermaid_empty_string() {
        assert_eq!(escape_mermaid(""), "");
    }

    #[test]
    fn escape_mermaid_multiple_quotes() {
        assert_eq!(escape_mermaid("\"a\""), "&quot;a&quot;");
    }

    #[test]
    fn escape_mermaid_does_not_touch_backslashes() {
        assert_eq!(escape_mermaid(r"a\b"), r"a\b");
    }

    #[test]
    fn mermaid_wraps_long_label_with_br() {
        let migrations = wrap(vec![TestMigration::new(
            "app1",
            "m_0002_auto_20260527_004236",
            [],
            [],
        )]);
        assert!(render_mermaid(&migrations).contains("<br/>"));
    }

    #[test]
    fn mermaid_single_migration_no_edges() {
        let migrations = wrap(vec![TestMigration::new("solo", "m1", [], [])]);
        let mermaid = render_mermaid(&migrations);

        assert!(mermaid.contains("n0[\"m1\"]"));
        assert!(mermaid.contains("class n0 migration;"));
        assert!(!mermaid.contains("-->"));
    }

    #[test]
    fn mermaid_clusters_sorted_alphabetically_by_app() {
        let migrations = wrap(vec![
            TestMigration::new("zeta", "m1", [], []),
            TestMigration::new("alpha", "m1", [], []),
        ]);
        let mermaid = render_mermaid(&migrations);

        let alpha_pos = mermaid
            .find("[\"alpha\"]")
            .expect("alpha subgraph should be present");
        let zeta_pos = mermaid
            .find("[\"zeta\"]")
            .expect("zeta subgraph should be present");
        assert!(alpha_pos < zeta_pos);
    }

    #[test]
    fn mermaid_diamond_dependency_all_edges_rendered() {
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
        let mermaid = render_mermaid(&migrations);

        assert!(mermaid.contains("n0 --> n1"));
        assert!(mermaid.contains("n0 --> n2"));
        assert!(mermaid.contains("n1 --> n3"));
        assert!(mermaid.contains("n2 --> n3"));
        assert_eq!(mermaid.matches("-->").count(), 4);
    }

    #[test]
    fn mermaid_escapes_quotes_in_app_name_subgraph_label() {
        let migrations = wrap(vec![TestMigration::new("weird\"app", "m1", [], [])]);
        assert!(render_mermaid(&migrations).contains("weird&quot;app"));
    }
}
