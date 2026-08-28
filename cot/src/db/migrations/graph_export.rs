use std::collections::HashMap;
use std::fmt::Write;

use cot::db::migrations::MigrationEngineError;

use crate::db::migrations::sorter::MigrationSorter;
use crate::db::migrations::{DynMigration, MigrationWrapper};
use crate::utils::graph::Graph;

/// The output format for a rendered migration dependency graph.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GraphFormat {
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

#[derive(Debug)]
struct Node<'a> {
    id: String,
    app: &'a str,
    label: &'a str,
}

pub(super) fn render(
    migrations: &[MigrationWrapper],
    format: GraphFormat,
) -> super::Result<String> {
    let graph = MigrationSorter::generate_graph(migrations).map_err(|e| {
        MigrationEngineError::Custom(format!("Failed to generate migration graph: {e}"))
    })?;

    let nodes = migrations
        .iter()
        .enumerate()
        .map(|(i, m)| Node {
            id: format!("n{i}"),
            app: m.app_name(),
            label: m.name(),
        })
        .collect::<Vec<_>>();

    Ok(match format {
        GraphFormat::Dot => render_dot(&nodes, &graph),
        GraphFormat::Mermaid => render_mermaid(&nodes, &graph),
    })
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

        // A single segment longer than the wrap width on its own: emit it as
        // its own line rather than trying to split mid-word.
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

fn render_dot(nodes: &[Node<'_>], graph: &Graph) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "digraph migrations {{");
    let _ = writeln!(out, "  rankdir=LR;");
    let _ = writeln!(out, "  splines=spline;");
    let _ = writeln!(out, "  nodesep=0.4;");
    let _ = writeln!(out, "  ranksep=0.6;");
    let _ = writeln!(out, "  bgcolor=\"transparent\";\n");

    let _ = writeln!(out, "  graph [fontname=\"{}\"];", style::FONT_FAMILY);
    let _ = writeln!(
        out,
        "  node  [fontname=\"{}\", fontsize=11];",
        style::FONT_FAMILY
    );
    let _ = writeln!(
        out,
        "  edge  [fontname=\"{}\", fontsize=9];\n",
        style::FONT_FAMILY
    );

    let _ = writeln!(out, "  node [");
    let _ = writeln!(out, "    shape=box,");
    let _ = writeln!(out, "    style=\"rounded,filled\",");
    let _ = writeln!(out, "    fillcolor=\"{}\",", style::NODE_FILL);
    let _ = writeln!(out, "    color=\"{}\",", style::NODE_STROKE);
    let _ = writeln!(out, "    fontcolor=\"{}\",", style::NODE_TEXT);
    let _ = writeln!(out, "    penwidth=1,");
    let _ = writeln!(out, "    margin=\"0.18,0.12\"");
    let _ = writeln!(out, "  ];\n");

    let _ = writeln!(out, "  edge [");
    let _ = writeln!(out, "    color=\"{}\",", style::EDGE_COLOR);
    let _ = writeln!(out, "    penwidth=1.2,");
    let _ = writeln!(out, "    arrowsize=0.8");
    let _ = writeln!(out, "  ];\n");

    for (cluster_index, (app, indices)) in group_by_app(nodes).into_iter().enumerate() {
        let _ = writeln!(out, "  subgraph cluster_{cluster_index} {{");
        let _ = writeln!(out, "    label=\"{}\";", escape_dot(app));
        let _ = writeln!(out, "    style=\"rounded,filled\";");
        let _ = writeln!(out, "    color=\"{}\";", style::CLUSTER_STROKE);
        let _ = writeln!(out, "    fillcolor=\"{}\";", style::CLUSTER_FILL);
        let _ = writeln!(out, "    fontcolor=\"{}\";", style::CLUSTER_TEXT);
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

fn render_mermaid(nodes: &[Node<'_>], graph: &Graph) -> String {
    let mut out = String::new();

    // Transparent background so the diagram doesn't carry a hardcoded white
    // canvas regardless of where it's rendered.
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

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_mermaid(s: &str) -> String {
    s.replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::MigrationDependency;
    use crate::test::TestMigration;

    fn wrap(migrations: Vec<TestMigration>) -> Vec<MigrationWrapper> {
        migrations.into_iter().map(MigrationWrapper::new).collect()
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

        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        assert!(dot.contains("digraph migrations"));
        assert!(dot.contains("subgraph cluster_0"));
        assert!(dot.contains("n0 -> n1;"));
        assert!(dot.contains(style::NODE_FILL));
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

        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        assert!(mermaid.contains("flowchart LR"));
        assert!(mermaid.contains("subgraph cluster0"));
        assert!(mermaid.contains("n0 --> n1"));
        assert!(mermaid.contains("background': 'transparent'"));
        assert!(mermaid.contains("classDef migration"));
    }

    #[test]
    fn escapes_quotes_in_labels() {
        assert_eq!(escape_dot(r#"a"b"#), r#"a\"b"#);
        assert_eq!(escape_mermaid(r#"a"b"#), "a&quot;b");
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
        let input = "a\\\"b";
        let escaped = escape_dot(input);

        assert_eq!(escaped.matches('\\').count(), 3);
        assert_eq!(escaped.matches('"').count(), 1);
        assert!(escaped.starts_with('a'));
        assert!(escaped.ends_with('b'));
    }

    #[test]
    fn escape_mermaid_empty_string() {
        assert_eq!(escape_mermaid(""), "");
    }

    #[test]
    fn escape_mermaid_multiple_quotes() {
        let input = "\"a\""; // "a"
        assert_eq!(escape_mermaid(input), "&quot;a&quot;");
    }

    #[test]
    fn escape_mermaid_does_not_touch_backslashes() {
        assert_eq!(escape_mermaid(r"a\b"), r"a\b");
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
    fn dot_wraps_long_label_with_literal_newline() {
        let migrations = wrap(vec![TestMigration::new(
            "app1",
            "m_0002_auto_20260527_004236",
            [],
            [],
        )]);

        let dot = render(&migrations, GraphFormat::Dot).unwrap();
        assert!(dot.contains("\\n"));
    }

    #[test]
    fn mermaid_wraps_long_label_with_br() {
        let migrations = wrap(vec![TestMigration::new(
            "app1",
            "m_0002_auto_20260527_004236",
            [],
            [],
        )]);

        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();
        assert!(mermaid.contains("<br/>"));
    }

    #[test]
    fn dot_render_empty_migrations() {
        let migrations: Vec<MigrationWrapper> = Vec::new();
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        assert!(dot.starts_with("digraph migrations {"));
        assert!(dot.trim_end().ends_with('}'));
        assert!(!dot.contains("subgraph"));
        assert!(!dot.contains("->"));
    }

    #[test]
    fn mermaid_render_empty_migrations() {
        let migrations: Vec<MigrationWrapper> = Vec::new();
        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        assert!(mermaid.contains("flowchart LR"));
        assert!(!mermaid.contains("subgraph"));
        assert!(!mermaid.contains("-->"));
        assert!(!mermaid.contains("n0"));
    }

    #[test]
    fn dot_single_migration_no_edges() {
        let migrations = wrap(vec![TestMigration::new("solo", "m1", [], [])]);
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        assert!(dot.contains("subgraph cluster_0"));
        assert!(dot.contains("n0 [label=\"m1\"];"));
        assert!(!dot.contains("->"));
    }

    #[test]
    fn mermaid_single_migration_no_edges() {
        let migrations = wrap(vec![TestMigration::new("solo", "m1", [], [])]);
        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        assert!(mermaid.contains("n0[\"m1\"]"));
        assert!(mermaid.contains("class n0 migration;"));
        assert!(!mermaid.contains("-->"));
    }

    #[test]
    fn dot_clusters_sorted_alphabetically_by_app() {
        let migrations = wrap(vec![
            TestMigration::new("zeta", "m1", [], []),
            TestMigration::new("alpha", "m1", [], []),
        ]);
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        let alpha_pos = dot.find("label=\"alpha\";").expect("alpha cluster present");
        let zeta_pos = dot.find("label=\"zeta\";").expect("zeta cluster present");
        assert!(alpha_pos < zeta_pos);
    }

    #[test]
    fn mermaid_clusters_sorted_alphabetically_by_app() {
        let migrations = wrap(vec![
            TestMigration::new("zeta", "m1", [], []),
            TestMigration::new("alpha", "m1", [], []),
        ]);
        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        let alpha_pos = mermaid
            .find("[\"alpha\"]")
            .expect("alpha subgraph should be present");
        let zeta_pos = mermaid
            .find("[\"zeta\"]")
            .expect("zeta subgraph should be present");
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
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        assert_eq!(dot.matches("subgraph cluster_").count(), 1);
    }

    #[test]
    fn node_ids_assigned_in_input_order_not_sorted_order() {
        let migrations = wrap(vec![
            TestMigration::new("zeta", "first", [], []),
            TestMigration::new("alpha", "second", [], []),
        ]);
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

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
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        assert!(dot.contains("n0 -> n1;"));
        assert!(dot.contains("n0 -> n2;"));
        assert!(dot.contains("n1 -> n3;"));
        assert!(dot.contains("n2 -> n3;"));
        assert_eq!(dot.matches("->").count(), 4);
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
        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        assert!(mermaid.contains("n0 --> n1"));
        assert!(mermaid.contains("n0 --> n2"));
        assert!(mermaid.contains("n1 --> n3"));
        assert!(mermaid.contains("n2 --> n3"));
        assert_eq!(mermaid.matches("-->").count(), 4);
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
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

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

        let result = render(&migrations, GraphFormat::Dot);
        assert!(result.is_ok());
        let dot = result.unwrap();
        assert!(dot.contains("n0 -> n1;"));
        assert!(dot.contains("n1 -> n0;"));
    }

    #[test]
    fn dot_escapes_quotes_in_app_name_cluster_label() {
        let migrations = wrap(vec![TestMigration::new("weird\"app", "m1", [], [])]);
        let dot = render(&migrations, GraphFormat::Dot).unwrap();

        assert!(dot.contains("weird\\\"app"));
    }

    #[test]
    fn mermaid_escapes_quotes_in_app_name_subgraph_label() {
        let migrations = wrap(vec![TestMigration::new("weird\"app", "m1", [], [])]);
        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        assert!(mermaid.contains("weird&quot;app"));
    }

    #[test]
    fn render_dispatches_dot_vs_mermaid() {
        let migrations = wrap(vec![TestMigration::new("app", "m1", [], [])]);

        let dot = render(&migrations, GraphFormat::Dot).unwrap();
        let mermaid = render(&migrations, GraphFormat::Mermaid).unwrap();

        assert!(dot.contains("digraph migrations"));
        assert!(!dot.contains("flowchart"));
        assert!(mermaid.contains("flowchart LR"));
        assert!(!mermaid.contains("digraph"));
    }
}
