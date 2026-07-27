//! Generates the configuration file reference (`docs/configuration.md`) from
//! `cot::config::ProjectConfig`'s JSON schema (via `schemars`), so the
//! reference can never drift from the actual set of tables/keys the type
//! accepts: every field is picked up automatically because the schema is
//! derived directly from the struct/enum definitions, not hand-copied.

use std::fmt::Write as _;

use serde_json::{Map, Value};

/// Generates the Markdown configuration reference.
///
/// # Panics
///
/// Panics if `cot::config::ProjectConfig`'s JSON schema doesn't have the shape
/// this generator expects (e.g. missing `properties`) - this would indicate a
/// schemars version change or a fundamentally different config type shape that
/// the generator needs to be updated for.
#[must_use]
pub fn generate_config_reference() -> String {
    let schema = schemars::schema_for!(cot::config::ProjectConfig);
    let root = schema
        .as_value()
        .as_object()
        .expect("root schema must be a JSON object");
    let defs: Map<String, Value> = root
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let properties = root
        .get("properties")
        .and_then(Value::as_object)
        .expect("ProjectConfig schema must declare properties");

    let mut md = String::new();
    md.push_str("---\ntitle: Configuration\n---\n\n");
    md.push_str(
        "<!--\nThis file is generated from `cot::config::ProjectConfig`'s type definition.\n\
         Do not edit it by hand -- run `just generate-config-docs` instead.\n-->\n\n",
    );
    if let Some(desc) = root.get("description").and_then(Value::as_str) {
        md.push_str(&first_paragraph(desc));
        md.push_str("\n\n");
    }
    md.push_str(
        "Cot projects are configured via a TOML file (typically `config/dev.toml` and \
         `config/prod.toml`, loaded with\n\
         [`ProjectConfig::from_toml`](https://docs.rs/cot/latest/cot/config/struct.ProjectConfig.html#method.from_toml)).\n\
         This page lists every table and key that `ProjectConfig` understands.\n\n",
    );
    md.push_str(
        "Any top-level table not listed below is preserved as-is and made available to your \
         application through `ProjectConfig::extra`, for app-specific configuration.\n\n",
    );

    md.push_str("## Top-level keys\n\n");
    let mut default_toml = String::new();
    render_fields(&[], 1, properties, &defs, &mut md, &mut default_toml);

    md.push_str("## Full default configuration\n\n");
    md.push_str(
        "This is a complete example with every key set explicitly to its default value \
         (fields without a well-defined default, like `secret_key`, are omitted):\n\n",
    );
    md.push_str("```toml\n");
    md.push_str(default_toml.trim());
    md.push('\n');
    md.push_str("```\n");

    md
}

enum PendingChild<'a> {
    Table(&'a Map<String, Value>),
    Tagged(&'a Vec<Value>, Option<&'a Value>),
}

/// Renders a field table (`| Key | Type | Default | Description |`) for the
/// given `properties` into `md`, followed by a section (and, recursively, its
/// own field table) for every property that turns out to be a nested table.
/// Each table's own scalar defaults are written to `default_toml` under a
/// `[path]` header - but only if it actually has any, so a table that only
/// contains further nested tables (e.g. `middlewares`) doesn't get an empty,
/// redundant header.
fn render_fields(
    path: &[String],
    level: usize,
    properties: &Map<String, Value>,
    defs: &Map<String, Value>,
    md: &mut String,
    default_toml: &mut String,
) {
    md.push_str("| Key | Type | Default | Description |\n|---|---|---|---|\n");
    let mut own_toml = String::new();
    let mut pending: Vec<(String, PendingChild<'_>)> = Vec::new();

    for (key, prop_value) in properties {
        let prop = prop_value
            .as_object()
            .expect("property schema must be an object");
        let description = escape_table_cell(&first_paragraph(
            prop.get("description")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ));
        let default_val = prop.get("default");
        let resolved = deref(prop_value, defs);

        match classify(resolved) {
            Kind::Table(props) => {
                let _ = writeln!(md, "| `{key}` | table | *(see below)* | {description} |");
                pending.push((key.clone(), PendingChild::Table(props)));
            }
            Kind::TaggedTable(variants) => {
                let _ = writeln!(md, "| `{key}` | table | *(see below)* | {description} |");
                pending.push((key.clone(), PendingChild::Tagged(variants, default_val)));
            }
            Kind::LeafEnum(variants) => {
                let ty = leaf_enum_type_name(variants);
                let _ = writeln!(
                    md,
                    "| `{key}` | {ty} | {} | {description} |",
                    default_cell(default_val, ScalarKind::String)
                );
                if let Some(line) = default_line(key, default_val, ScalarKind::String) {
                    own_toml.push_str(&line);
                }
            }
            Kind::Scalar(kind) => {
                let ty = scalar_type_name(kind);
                let _ = writeln!(
                    md,
                    "| `{key}` | {ty} | {} | {description} |",
                    default_cell(default_val, kind)
                );
                if let Some(line) = default_line(key, default_val, kind) {
                    own_toml.push_str(&line);
                }
            }
        }
    }
    md.push('\n');

    if !own_toml.is_empty() {
        if !path.is_empty() {
            let _ = writeln!(default_toml, "\n[{}]", path.join("."));
        }
        default_toml.push_str(&own_toml);
    }

    for (key, child) in pending {
        let child_path = extend(path, &key);
        match child {
            PendingChild::Table(props) => {
                render_object(&child_path, level + 1, props, defs, md, default_toml);
            }
            PendingChild::Tagged(variants, default_val) => {
                render_tagged(
                    &child_path,
                    level + 1,
                    variants,
                    default_val,
                    defs,
                    md,
                    default_toml,
                );
            }
        }
    }
}

/// Renders a `## [path]` (or deeper) section for a plain nested table.
fn render_object(
    path: &[String],
    level: usize,
    properties: &Map<String, Value>,
    defs: &Map<String, Value>,
    md: &mut String,
    default_toml: &mut String,
) {
    let _ = writeln!(md, "{} `[{}]`\n", heading_hashes(level), path.join("."));
    render_fields(path, level, properties, defs, md, default_toml);
}

/// Renders a `## [path]` section for an internally-tagged enum (selected via a
/// `type` key), with one sub-section per variant.
fn render_tagged(
    path: &[String],
    level: usize,
    variants: &[Value],
    default_val: Option<&Value>,
    defs: &Map<String, Value>,
    md: &mut String,
    default_toml: &mut String,
) {
    let _ = writeln!(md, "{} `[{}]`\n", heading_hashes(level), path.join("."));
    md.push_str("Select the variant with the `type` key:\n\n");

    for variant in variants {
        let variant = variant
            .as_object()
            .expect("tagged enum variant schema must be an object");
        let type_const = variant
            .get("properties")
            .and_then(|p| p.get("type"))
            .and_then(|t| t.get("const"))
            .and_then(Value::as_str)
            .expect("tagged enum variant must declare a `type` const");
        let _ = writeln!(
            md,
            "{} `type = \"{type_const}\"`\n",
            heading_hashes(level + 1)
        );
        if let Some(desc) = variant.get("description").and_then(Value::as_str) {
            md.push_str(&first_paragraph(desc));
            md.push_str("\n\n");
        }

        let other_props: Vec<(&String, &Value)> = variant
            .get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(k, _)| k.as_str() != "type")
            .collect();
        if other_props.is_empty() {
            md.push_str("No additional keys.\n\n");
        } else {
            md.push_str("| Key | Type | Default | Description |\n|---|---|---|---|\n");
            for (key, prop_value) in other_props {
                let prop = prop_value
                    .as_object()
                    .expect("property schema must be an object");
                let description = escape_table_cell(&first_paragraph(
                    prop.get("description")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ));
                let default_val = prop.get("default");
                let resolved = deref(prop_value, defs);
                match classify(resolved) {
                    Kind::Scalar(kind) => {
                        let ty = scalar_type_name(kind);
                        let _ = writeln!(
                            md,
                            "| `{key}` | {ty} | {} | {description} |",
                            default_cell(default_val, kind)
                        );
                    }
                    Kind::LeafEnum(variants) => {
                        let ty = leaf_enum_type_name(variants);
                        let _ = writeln!(
                            md,
                            "| `{key}` | {ty} | {} | {description} |",
                            default_cell(default_val, ScalarKind::String)
                        );
                    }
                    // Not exercised by the current config tree (no tagged-enum variant has a
                    // nested-table field), but handled so the generator degrades gracefully
                    // rather than panicking if one is ever added.
                    Kind::Table(_) | Kind::TaggedTable(_) => {
                        let _ = writeln!(
                            md,
                            "| `{key}` | table | *(see type documentation)* | {description} |"
                        );
                    }
                }
            }
            md.push('\n');
        }
    }

    let mut own_toml = String::new();
    if let Some(Value::Object(default_obj)) = default_val {
        for (key, value) in default_obj {
            if let Some(line) = json_scalar_to_toml_line(key, value) {
                own_toml.push_str(&line);
            }
        }
    }
    if !own_toml.is_empty() {
        let _ = writeln!(default_toml, "\n[{}]", path.join("."));
        default_toml.push_str(&own_toml);
    }
}

enum Kind<'a> {
    Table(&'a Map<String, Value>),
    TaggedTable(&'a Vec<Value>),
    LeafEnum(&'a Vec<Value>),
    Scalar(ScalarKind),
}

#[derive(Clone, Copy)]
enum ScalarKind {
    Bool,
    Integer,
    String,
    Array,
}

/// Classifies an already-dereferenced schema node.
fn classify(resolved: &Value) -> Kind<'_> {
    let Some(obj) = resolved.as_object() else {
        return Kind::Scalar(ScalarKind::String);
    };
    if let Some(Value::Array(one_of)) = obj.get("oneOf") {
        let tagged = !one_of.is_empty() && one_of.iter().all(|v| v.get("properties").is_some());
        return if tagged {
            Kind::TaggedTable(one_of)
        } else {
            Kind::LeafEnum(one_of)
        };
    }
    if let Some(Value::Object(props)) = obj.get("properties") {
        return Kind::Table(props);
    }
    let ty_str = match obj.get("type") {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .find(|s| *s != "null")
            .unwrap_or("string"),
        _ => "string",
    };
    Kind::Scalar(match ty_str {
        "boolean" => ScalarKind::Bool,
        "integer" | "number" => ScalarKind::Integer,
        "array" => ScalarKind::Array,
        _ => ScalarKind::String,
    })
}

/// Follows a `$ref` (one level - this schema never nests them further) or picks
/// the non-null branch of an `anyOf` (produced by `Option<T>` fields),
/// returning the schema node that actually describes the field's shape.
fn deref<'a>(prop: &'a Value, defs: &'a Map<String, Value>) -> &'a Value {
    let Some(obj) = prop.as_object() else {
        return prop;
    };
    if let Some(Value::String(r)) = obj.get("$ref") {
        let name = r.rsplit('/').next().unwrap_or(r);
        if let Some(target) = defs.get(name) {
            return target;
        }
    }
    if let Some(Value::Array(any_of)) = obj.get("anyOf") {
        for branch in any_of {
            if branch.get("type").and_then(Value::as_str) != Some("null") {
                return deref(branch, defs);
            }
        }
    }
    prop
}

fn scalar_type_name(kind: ScalarKind) -> &'static str {
    match kind {
        ScalarKind::Bool => "boolean",
        ScalarKind::Integer => "integer",
        ScalarKind::String => "string",
        ScalarKind::Array => "array of strings",
    }
}

fn leaf_enum_type_name(variants: &[Value]) -> String {
    let values: Vec<String> = variants
        .iter()
        .filter_map(|v| v.get("const").and_then(Value::as_str))
        .map(|s| format!("`\"{s}\"`"))
        .collect();
    format!("string (one of: {})", values.join(", "))
}

fn default_cell(default_val: Option<&Value>, kind: ScalarKind) -> String {
    match json_scalar_repr(default_val, kind) {
        Some(s) => format!("`{s}`"),
        None => "—".to_string(),
    }
}

fn default_line(key: &str, default_val: Option<&Value>, kind: ScalarKind) -> Option<String> {
    json_scalar_repr(default_val, kind).map(|v| format!("{key} = {v}\n"))
}

/// Renders a schema `default` value as a TOML-literal string, but only when it
/// actually matches the field's declared scalar type. A couple of fields
/// (`secret_key`, `fallback_secret_keys`) wrap [`cot::config::SecretKey`],
/// whose real `Serialize` impl emits a byte array rather than the string this
/// schema declares (see the `schemars(with = "String")` override on that type),
/// so schemars' computed default for them doesn't match the declared type.
/// Rather than print a misleading example, such mismatches are treated as "no
/// representable default" and omitted.
fn json_scalar_repr(default_val: Option<&Value>, kind: ScalarKind) -> Option<String> {
    match (kind, default_val?) {
        (ScalarKind::Bool, Value::Bool(b)) => Some(b.to_string()),
        (ScalarKind::Integer, Value::Number(n)) => Some(n.to_string()),
        (ScalarKind::String, Value::String(s)) => Some(toml_quote(s)),
        (ScalarKind::Array, Value::Array(items)) => {
            let mut rendered = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(s) => rendered.push(toml_quote(s)),
                    _ => return None,
                }
            }
            Some(format!("[{}]", rendered.join(", ")))
        }
        _ => None,
    }
}

fn json_scalar_to_toml_line(key: &str, value: &Value) -> Option<String> {
    match value {
        Value::Bool(b) => Some(format!("{key} = {b}\n")),
        Value::Number(n) => Some(format!("{key} = {n}\n")),
        Value::String(s) => Some(format!("{key} = {}\n", toml_quote(s))),
        _ => None,
    }
}

fn toml_quote(s: &str) -> String {
    format!("{s:?}")
}

fn heading_hashes(level: usize) -> String {
    "#".repeat(level.clamp(2, 6))
}

fn extend(path: &[String], key: &str) -> Vec<String> {
    let mut v = path.to_vec();
    v.push(key.to_string());
    v
}

fn escape_table_cell(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Extracts the first paragraph of a rustdoc description, stopping at the first
/// blank line, heading (`#`), or code fence (some doc comments in `config.rs`
/// are missing the blank line before `# Examples`, so a heading/fence also ends
/// the paragraph). Multiple lines within the paragraph are joined with spaces
/// so they read as flowing prose in a table cell.
fn first_paragraph(desc: &str) -> String {
    let mut lines = Vec::new();
    for line in desc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("```") {
            break;
        }
        if trimmed.is_empty() {
            if lines.is_empty() {
                continue;
            }
            break;
        }
        lines.push(trimmed);
    }
    lines.join(" ")
}
