/// Type-check + eval in a single pass, producing a `SystemManifest` JSON value.
///
/// The evaluator receives the parsed AST and the ambient `pkgs` mapping,
/// then walks the tree against the expected schema.  All type errors are
/// accumulated (the first error terminates evaluation immediately because
/// we can't safely continue when we don't know the type of a sub-expression).
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ast::{Expr, Span};
use crate::diagnostics::{Diag, DiagCode, DiagLevel};
use crate::lexer::line_col;

/// The evaluated `SystemManifest` (ready for JSON serialization).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManifest {
    pub hostname: String,
    pub packages: Vec<String>,
    pub services: HashMap<String, Service>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Service {
    pub exec: String,
    pub restart: RestartPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

impl std::fmt::Display for RestartPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestartPolicy::Always => write!(f, "always"),
            RestartPolicy::OnFailure => write!(f, "on-failure"),
            RestartPolicy::Never => write!(f, "never"),
        }
    }
}

/// The ambient environment passed into evaluation.
pub struct Env {
    /// `pkgs` mapping: `name -> "name-version"` string.
    pub pkgs: HashMap<String, String>,
    /// `pkgs` was built from real `nv-pkg list` output (vs empty fallback).
    pub pkgs_available: bool,
}

pub struct Evaluator<'a> {
    src: &'a str,
    file: String,
    env: &'a Env,
}

impl<'a> Evaluator<'a> {
    pub fn new(src: &'a str, file: impl Into<String>, env: &'a Env) -> Self {
        Evaluator {
            src,
            file: file.into(),
            env,
        }
    }

    fn diag(&self, span: &Span, message: String, fix: Option<String>) -> Diag {
        let (line, col) = line_col(self.src, span.offset);
        Diag {
            level: DiagLevel::Error,
            code: DiagCode::Typ001,
            file: self.file.clone(),
            line,
            col,
            message,
            fix,
        }
    }

    /// Evaluate the top-level expression as a `SystemManifest`.
    pub fn eval_manifest(&self, expr: &Expr) -> Result<SystemManifest, Diag> {
        let attrs = self.expect_attrset(expr, "SystemManifest")?;

        let hostname = self.require_string_field(&attrs, expr.span(), "hostname")?;
        let packages = self.require_string_list_field(&attrs, expr.span(), "packages")?;
        let services = self.eval_services(&attrs, expr.span())?;
        let environment = self.eval_environment(&attrs, expr.span())?;

        Ok(SystemManifest {
            hostname,
            packages,
            services,
            environment,
        })
    }

    fn expect_attrset<'b>(
        &self,
        expr: &'b Expr,
        context: &str,
    ) -> Result<HashMap<String, &'b Expr>, Diag> {
        match expr {
            Expr::AttrSet { attrs, .. } => {
                let mut map = HashMap::new();
                for attr in attrs {
                    if map.contains_key(&attr.key) {
                        return Err(self.diag(
                            &attr.key_span,
                            format!("duplicate attribute `{}`", attr.key),
                            Some(format!("remove one of the `{}` entries", attr.key)),
                        ));
                    }
                    map.insert(attr.key.clone(), &attr.value);
                }
                Ok(map)
            }
            other => Err(self.diag(
                other.span(),
                format!("expected attrset ({}), found {}", context, expr_type_name(other)),
                None,
            )),
        }
    }

    fn eval_expr_as_string(&self, expr: &Expr) -> Result<String, Diag> {
        match expr {
            Expr::Str { value, .. } => Ok(value.clone()),
            Expr::FieldAccess { .. } | Expr::Ident { .. } => {
                // Resolve pkg reference
                self.resolve_pkg_ref(expr)
            }
            other => Err(self.diag(
                other.span(),
                format!(
                    "expected String, got {}",
                    expr_type_name(other)
                ),
                match other {
                    Expr::Int { value, .. } => Some(format!("wrap {} in quotes", value)),
                    Expr::Bool { value, .. } => Some(format!("wrap `{}` in quotes", value)),
                    _ => None,
                },
            )),
        }
    }

    fn resolve_pkg_ref(&self, expr: &Expr) -> Result<String, Diag> {
        // Walk a chain of field accesses rooted at `pkgs`.
        // Only `pkgs.<name>` is valid in Phase 1.
        match expr {
            Expr::Ident { name, span } => {
                if name == "pkgs" {
                    return Err(self.diag(
                        span,
                        "`pkgs` alone is not a valid value; use `pkgs.<name>`".to_string(),
                        None,
                    ));
                }
                Err(self.diag(
                    span,
                    format!("unknown identifier `{}`; only `pkgs` is in scope in Phase 1", name),
                    None,
                ))
            }
            Expr::FieldAccess { lhs, field, span } => {
                // Only `pkgs.<name>` is supported — no deeper nesting in Phase 1.
                match lhs.as_ref() {
                    Expr::Ident { name, span: lhs_span } if name == "pkgs" => {
                        if !self.env.pkgs_available {
                            // pkgs is empty — this is a hard error
                            return Err(self.diag(
                                lhs_span,
                                format!(
                                    "`nv-pkg` is not on PATH; `pkgs.{}` cannot be resolved. \
                                     Use a literal string like \"{}-<version>\" instead, \
                                     or ensure nv-pkg is installed.",
                                    field, field
                                ),
                                Some(format!("\"{}\"", field)),
                            ));
                        }
                        match self.env.pkgs.get(field.as_str()) {
                            Some(versioned) => Ok(versioned.clone()),
                            None => Err(self.diag(
                                span,
                                format!(
                                    "`pkgs.{}` is not installed; \
                                     use `nv-pkg install {}.nvpkg` first",
                                    field, field
                                ),
                                None,
                            )),
                        }
                    }
                    Expr::FieldAccess { .. } => {
                        // Deeper nesting like pkgs.foo.bar — not valid
                        Err(self.diag(
                            span,
                            "chained `pkgs.*.*` access is not supported in Phase 1".to_string(),
                            None,
                        ))
                    }
                    other => Err(self.diag(
                        other.span(),
                        format!(
                            "field access on `{}` is not supported; \
                             only `pkgs.<name>` field access is in scope",
                            expr_type_name(other)
                        ),
                        None,
                    )),
                }
            }
            other => Err(self.diag(
                other.span(),
                format!("expected String, got {}", expr_type_name(other)),
                None,
            )),
        }
    }

    fn require_string_field(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
        key: &str,
    ) -> Result<String, Diag> {
        match attrs.get(key) {
            None => Err(self.diag(
                parent_span,
                format!("missing required field `{}`", key),
                Some(format!("{} = \"<value>\";", key)),
            )),
            Some(expr) => self.eval_expr_as_string(expr),
        }
    }

    fn require_string_list_field(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
        key: &str,
    ) -> Result<Vec<String>, Diag> {
        match attrs.get(key) {
            None => Err(self.diag(
                parent_span,
                format!("missing required field `{}`", key),
                Some(format!("{} = [];", key)),
            )),
            Some(expr) => self.eval_string_list(expr),
        }
    }

    fn eval_string_list(&self, expr: &Expr) -> Result<Vec<String>, Diag> {
        match expr {
            Expr::List { items, .. } => {
                let mut result = Vec::new();
                for item in items {
                    let s = self.eval_expr_as_string(item)?;
                    result.push(s);
                }
                // Homogeneity is enforced by requiring all items to be strings above.
                Ok(result)
            }
            other => Err(self.diag(
                other.span(),
                format!("expected [String], got {}", expr_type_name(other)),
                None,
            )),
        }
    }

    fn eval_services(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
    ) -> Result<HashMap<String, Service>, Diag> {
        match attrs.get("services") {
            None => Err(self.diag(
                parent_span,
                "missing required field `services`".to_string(),
                Some("services = {};".to_string()),
            )),
            Some(expr) => {
                let service_attrs = self.expect_attrset(expr, "{ [String]: Service }")?;
                let mut services = HashMap::new();
                for (name, svc_expr) in &service_attrs {
                    let svc = self.eval_service(svc_expr, name)?;
                    services.insert(name.clone(), svc);
                }
                Ok(services)
            }
        }
    }

    fn eval_service(&self, expr: &Expr, name: &str) -> Result<Service, Diag> {
        let attrs = self.expect_attrset(expr, &format!("Service ({})", name))?;

        let exec = self.require_string_field(&attrs, expr.span(), "exec")?;

        let restart = match attrs.get("restart") {
            None => Err(self.diag(
                expr.span(),
                format!("service `{}` is missing required field `restart`", name),
                Some("restart = \"always\";".to_string()),
            )),
            Some(e) => {
                let s = self.eval_expr_as_string(e)?;
                match s.as_str() {
                    "always" => Ok(RestartPolicy::Always),
                    "on-failure" => Ok(RestartPolicy::OnFailure),
                    "never" => Ok(RestartPolicy::Never),
                    other => Err(self.diag(
                        e.span(),
                        format!(
                            "invalid restart policy `{}`; expected one of: \
                             \"always\", \"on-failure\", \"never\"",
                            other
                        ),
                        Some("restart = \"always\";".to_string()),
                    )),
                }
            }
        }?;

        Ok(Service { exec, restart })
    }

    fn eval_environment(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
    ) -> Result<HashMap<String, String>, Diag> {
        match attrs.get("environment") {
            None => Err(self.diag(
                parent_span,
                "missing required field `environment`".to_string(),
                Some("environment = {};".to_string()),
            )),
            Some(expr) => {
                let env_attrs = self.expect_attrset(expr, "{ [String]: String }")?;
                let mut env = HashMap::new();
                for (key, val_expr) in &env_attrs {
                    let val = self.eval_expr_as_string(val_expr)?;
                    env.insert(key.clone(), val);
                }
                Ok(env)
            }
        }
    }
}

fn expr_type_name(expr: &Expr) -> &'static str {
    match expr {
        Expr::Str { .. } => "String",
        Expr::Int { .. } => "Int",
        Expr::Bool { .. } => "Bool",
        Expr::Null { .. } => "Null",
        Expr::List { .. } => "List",
        Expr::AttrSet { .. } => "AttrSet",
        Expr::Ident { .. } | Expr::FieldAccess { .. } => "Identifier",
    }
}
