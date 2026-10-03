//! Generates the configuration file reference (`docs/configuration.md`) from
//! `cot::config::ProjectConfig`'s JSON schema (via `schemars`).

use std::fmt::Write as _;

use serde_json::{Map, Value};

/// Generates the Markdown configuration reference.
///
/// # Panics
///
/// Panics if `cot::config::ProjectConfig`'s JSON schema doesn't have the shape
/// this generator expects.
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
        md.push_str(&doc_summary(desc));
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
        "This is a complete example with every key set explicitly to its default value. \
         Keys that are unset by default, or whose default isn't a single value (like `debug`), \
         are commented out. Keys without a default (like `secret_key`) are shown as \
         `\"...\"` and must be set explicitly:\n\n",
    );
    // `ignore`: this includes "..." placeholders, so it's not a valid,
    // parseable config on its own
    md.push_str("```toml,ignore\n");
    md.push_str(default_toml.trim());
    md.push('\n');
    md.push_str("```\n");

    md
}

enum PendingChild<'a> {
    Table(Vec<String>, &'a Map<String, Value>),
    Tagged(Vec<String>, &'a Vec<Value>, Option<&'a Value>),
}

/// Renders a field table for the given `properties` into `md`, followed by a
/// section (and, recursively, its own field table) for every property that
/// is a nested table.
fn render_fields(
    path: &[String],
    level: usize,
    properties: &Map<String, Value>,
    defs: &Map<String, Value>,
    md: &mut String,
    default_toml: &mut String,
) {
    md.push_str("| Key | TOML Type | Default | Description |\n|---|---|---|---|\n");
    let mut own_toml = String::new();
    let mut pending: Vec<PendingChild<'_>> = Vec::new();

    for (key, prop_value) in properties {
        let prop = prop_value
            .as_object()
            .expect("property schema must be an object");
        let description = escape_table_cell(&doc_summary(
            prop.get("description")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ));
        let default_val = prop.get("default");
        let resolved = deref(prop_value, defs);

        match classify(resolved) {
            Kind::Table(props) => {
                let child_path = extend(path, key);
                let anchor = heading_anchor(&child_path);
                let _ = writeln!(
                    md,
                    "| `{key}` | table | [*(see below)*](#{anchor}) | {description} |"
                );
                pending.push(PendingChild::Table(child_path, props));
            }
            Kind::TaggedTable(variants) => {
                let child_path = extend(path, key);
                let anchor = heading_anchor(&child_path);
                let default_cell = tagged_default_cell(default_val, &anchor);
                let _ = writeln!(md, "| `{key}` | table | {default_cell} | {description} |");
                pending.push(PendingChild::Tagged(child_path, variants, default_val));
            }
            Kind::LeafEnum(variants) => {
                let ty = leaf_enum_type_name(variants);
                let default = field_default(prop, ScalarKind::String);
                let _ = writeln!(
                    md,
                    "| `{key}` | {ty} | {} | {description} |",
                    default_cell(&default)
                );
                own_toml.push_str(&default_line(key, &default));
            }
            Kind::Scalar(kind) => {
                let ty = scalar_type_name(kind);
                let default = field_default(prop, kind);
                let _ = writeln!(
                    md,
                    "| `{key}` | {ty} | {} | {description} |",
                    default_cell(&default)
                );
                own_toml.push_str(&default_line(key, &default));
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

    for child in pending {
        match child {
            PendingChild::Table(child_path, props) => {
                render_object(&child_path, level + 1, props, defs, md, default_toml);
            }
            PendingChild::Tagged(child_path, variants, default_val) => {
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

    let default_type = default_val
        .and_then(Value::as_object)
        .and_then(|o| o.get("type"))
        .and_then(Value::as_str);

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
        let default_marker = if default_type == Some(type_const) {
            " (default)"
        } else {
            ""
        };
        let _ = writeln!(
            md,
            "{} `type = \"{type_const}\"`{default_marker}\n",
            heading_hashes(level + 1)
        );
        if let Some(desc) = variant.get("description").and_then(Value::as_str) {
            md.push_str(&doc_summary(desc));
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
            continue;
        }
        md.push_str("| Key | TOML Type | Default | Description |\n|---|---|---|---|\n");
        for (key, prop_value) in other_props {
            let prop = prop_value
                .as_object()
                .expect("property schema must be an object");
            let description = escape_table_cell(&doc_summary(
                prop.get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ));
            let resolved = deref(prop_value, defs);
            match classify(resolved) {
                Kind::Scalar(kind) => {
                    let ty = scalar_type_name(kind);
                    let _ = writeln!(
                        md,
                        "| `{key}` | {ty} | {} | {description} |",
                        default_cell(&field_default(prop, kind))
                    );
                }
                Kind::LeafEnum(variants) => {
                    let ty = leaf_enum_type_name(variants);
                    let _ = writeln!(
                        md,
                        "| `{key}` | {ty} | {} | {description} |",
                        default_cell(&field_default(prop, ScalarKind::String))
                    );
                }
                Kind::Table(_) | Kind::TaggedTable(_) => {
                    unimplemented!(
                        "tagged-enum variant field `{key}` is a nested table, which this generator doesn't handle yet"
                    );
                }
            }
        }
        md.push('\n');
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
    values.join(", ")
}

/// Schema extension key overriding a field's `default` with a Markdown
/// description, for defaults that can't be expressed as a single value (e.g.
/// ones that depend on the build profile).
const DEFAULT_DESCRIPTION_KEY: &str = "x-default-description";

/// A field's default, as far as the reference is concerned.
enum FieldDefault<'a> {
    /// A fixed default, rendered as a TOML literal.
    Value(String),
    /// The field is unset by default (e.g. `Option::None`).
    Unset,
    /// The default can't be expressed as a single value; this is a Markdown
    /// description of it instead.
    Described(&'a str),
    /// The field has no representable default and must be set explicitly.
    Required,
}

fn field_default(prop: &Map<String, Value>, kind: ScalarKind) -> FieldDefault<'_> {
    if let Some(description) = prop.get(DEFAULT_DESCRIPTION_KEY).and_then(Value::as_str) {
        return FieldDefault::Described(description);
    }
    match prop.get("default") {
        Some(Value::Null) => FieldDefault::Unset,
        default_val => {
            json_scalar_repr(default_val, kind).map_or(FieldDefault::Required, FieldDefault::Value)
        }
    }
}

fn default_cell(default: &FieldDefault<'_>) -> String {
    match default {
        FieldDefault::Value(s) => format!("`{s}`"),
        FieldDefault::Unset => "*(unset)*".to_string(),
        FieldDefault::Described(description) => escape_table_cell(description),
        FieldDefault::Required => "—".to_string(),
    }
}

/// Renders the Default cell for a tagged-enum table field: the concrete
/// default variant's `type` tag, linked to its subsection, e.g.
/// `` [`type = "none"`](#auth_backend) ``. Falls back to a generic linked
/// "see below" if the field has no representable default (i.e. it's
/// required).
fn tagged_default_cell(default_val: Option<&Value>, anchor: &str) -> String {
    let default_type = default_val
        .and_then(Value::as_object)
        .and_then(|o| o.get("type"))
        .and_then(Value::as_str);
    match default_type {
        Some(t) => format!("[`type = \"{t}\"`](#{anchor})"),
        None => format!("[*(see below)*](#{anchor})"),
    }
}

/// Computes the anchor fragment that cot-site's Markdown renderer generates for
/// a `` `[path]` `` heading.
fn heading_anchor(path: &[String]) -> String {
    // every path segment here is a lowercase Rust identifier (letters, digits,
    // underscores only, no spaces), so the only characters the algorithm
    // actually strips are the heading's own brackets and the dots joining
    // the segments - i.e. it reduces to concatenating the segments as-is.
    path.concat()
}

/// Renders a field's line for the "full default configuration" TOML example.
///
/// Fields with a fixed default get that default; fields that are unset by
/// default or whose default isn't a single value are commented out; fields
/// without a default (e.g. `secret_key`, which is required and has no sensible
/// default) get a `"..."` placeholder instead. This way, every key the config
/// accepts still shows up in the example rather than being silently dropped
/// from it.
fn default_line(key: &str, default: &FieldDefault<'_>) -> String {
    match default {
        FieldDefault::Value(value) => format!("{key} = {value}\n"),
        FieldDefault::Unset | FieldDefault::Described(_) => format!("# {key} = ...\n"),
        FieldDefault::Required => format!("{key} = \"...\"\n"),
    }
}

/// Renders a schema `default` value as a TOML-literal string, but only when it
/// actually matches the field's declared scalar type.
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

/// Extracts the summary of a rustdoc description: its first paragraph (the
/// short summary line) followed by the second one (the longer explanation),
/// joined into a single paragraph.
///
/// Extraction stops early at the first section heading or code block, so the
/// second paragraph is only included if nothing like that comes before it.
/// Intra-doc links are replaced with their plain text (see
/// [`strip_intra_doc_links`]).
fn doc_summary(desc: &str) -> String {
    const MAX_PARAGRAPHS: usize = 2;

    let mut lines = Vec::new();
    let mut paragraphs = 0;
    let mut in_paragraph = false;
    for line in desc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("```") {
            break;
        }
        if trimmed.is_empty() {
            if in_paragraph {
                in_paragraph = false;
                paragraphs += 1;
                if paragraphs == MAX_PARAGRAPHS {
                    break;
                }
            }
            continue;
        }
        in_paragraph = true;
        lines.push(trimmed);
    }
    strip_intra_doc_links(&lines.join(" "))
}

/// Replaces rustdoc intra-doc links (`` [`Foo`] `` and
/// `` [`Foo`](crate::Foo) ``), which don't resolve outside of rustdoc, with
/// their link text. Links to external URLs and text inside code spans are left
/// intact.
fn strip_intra_doc_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(['[', '`']) {
        out.push_str(&rest[..start]);
        rest = &rest[start..];

        if rest.starts_with('`') {
            let end = rest[1..].find('`').map_or(rest.len(), |i| i + 2);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }

        let Some(close) = rest.find(']') else {
            break;
        };
        let label = &rest[1..close];
        let after = &rest[close + 1..];
        if let Some(target_and_rest) = after.strip_prefix('(')
            && let Some(target_end) = target_and_rest.find(')')
        {
            let target = &target_and_rest[..target_end];
            if target.contains("://") {
                let link_len = close + 1 + 1 + target_end + 1;
                out.push_str(&rest[..link_len]);
            } else {
                out.push_str(label);
            }
            rest = &target_and_rest[target_end + 1..];
        } else {
            out.push_str(label);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_summary_takes_two_paragraphs() {
        assert_eq!(
            doc_summary("Short.\n\nLonger\nexplanation.\n\nMore details."),
            "Short. Longer explanation."
        );
    }

    #[test]
    fn doc_summary_stops_at_section() {
        assert_eq!(doc_summary("Short.\n\n# Examples\n\nExample."), "Short.");
        assert_eq!(doc_summary("Short.\n\n```\ncode\n```\n\nMore."), "Short.");
    }

    #[test]
    fn strip_intra_doc_links_keeps_external_links_and_code() {
        assert_eq!(
            strip_intra_doc_links("[`A`] and [`B`](crate::B), [`C`](https://example.com), `[d]`"),
            "`A` and `B`, [`C`](https://example.com), `[d]`"
        );
    }
}
