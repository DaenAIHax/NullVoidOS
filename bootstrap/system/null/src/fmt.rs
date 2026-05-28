/// Canonical formatter for `.null` source files.
///
/// Rules (Phase 1 MVP):
///   - Indent with 2 spaces per level.
///   - One attribute per line in attrsets.
///   - List items one per line when the list is non-empty.
///   - Strings: double-quoted, minimal escaping.
///   - Bool / null: lowercase keywords.
///   - Field access: `lhs.field` with no spaces.
///   - No trailing whitespace on any line.
///   - Single trailing newline at EOF.
use crate::ast::{Attr, Expr};

pub fn format_expr(expr: &Expr) -> String {
    let mut out = String::new();
    write_expr(expr, 0, &mut out);
    out.push('\n');
    out
}

fn write_expr(expr: &Expr, indent: usize, out: &mut String) {
    match expr {
        Expr::Str { value, .. } => {
            out.push('"');
            for c in value.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    other => out.push(other),
                }
            }
            out.push('"');
        }
        Expr::Int { value, .. } => {
            out.push_str(&value.to_string());
        }
        Expr::Bool { value, .. } => {
            out.push_str(if *value { "true" } else { "false" });
        }
        Expr::Null { .. } => {
            out.push_str("null");
        }
        Expr::List { items, .. } => {
            if items.is_empty() {
                out.push_str("[ ]");
            } else {
                out.push_str("[\n");
                for item in items {
                    push_indent(out, indent + 1);
                    write_expr(item, indent + 1, out);
                    out.push('\n');
                }
                push_indent(out, indent);
                out.push(']');
            }
        }
        Expr::AttrSet { attrs, .. } => {
            if attrs.is_empty() {
                out.push_str("{ }");
            } else {
                out.push_str("{\n");
                for attr in attrs {
                    write_attr(attr, indent + 1, out);
                }
                push_indent(out, indent);
                out.push('}');
            }
        }
        Expr::Ident { name, .. } => {
            out.push_str(name);
        }
        Expr::FieldAccess { lhs, field, .. } => {
            write_expr(lhs, indent, out);
            out.push('.');
            out.push_str(field);
        }
    }
}

fn write_attr(attr: &Attr, indent: usize, out: &mut String) {
    push_indent(out, indent);
    out.push_str(&attr.key);
    out.push_str(" = ");
    write_expr(&attr.value, indent, out);
    out.push_str(";\n");
}

fn push_indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}
