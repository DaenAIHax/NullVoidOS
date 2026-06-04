/// Type-check + eval against the fixed `SystemManifest` schema (v2).
///
/// The schema is hardcoded at the compiler level — there is no inference
/// of record types from literal shape, no module system, no runtime merges.
/// See `system/null/SPEC.md` §4 for the authoritative shape.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ast::{Expr, Span};
use crate::diagnostics::{span_at, Diag, DiagCode, DiagLevel, Repair};
use crate::lexer::line_col;

// ---------------------------------------------------------------------------
// Output types (serialized to JSON by `null eval`)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManifest {
    pub hostname: String,
    pub caps: Vec<Capability>,
    pub packages: Vec<String>,
    pub services: HashMap<String, Service>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Service {
    pub exec: String,
    pub restart: RestartPolicy,
    pub requires: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Evaluated capability. Mirrors `Expr::Capability` but appears in the
/// output JSON, not the AST.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    pub path: Vec<String>,
    pub arg: Option<String>,
}

impl Capability {
    /// Canonical source-form rendering: `!net`, `!fs.read."/etc"`.
    pub fn render(&self) -> String {
        let mut s = String::from("!");
        s.push_str(&self.path.join("."));
        if let Some(arg) = &self.arg {
            s.push_str(".\"");
            for c in arg.chars() {
                match c {
                    '"' => s.push_str("\\\""),
                    '\\' => s.push_str("\\\\"),
                    other => s.push(other),
                }
            }
            s.push('"');
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Ambient environment
// ---------------------------------------------------------------------------

/// Built at evaluator start from `nv-pkg list --json`. SPEC §5.4.
pub struct Env {
    pub pkgs: HashMap<String, String>,
    pub pkgs_available: bool,
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

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

    fn diag(
        &self,
        span: &Span,
        code: DiagCode,
        message: String,
        expected: impl Into<String>,
        actual: impl Into<String>,
        repair: Option<Repair>,
    ) -> Diag {
        let (line, col) = line_col(self.src, span.offset);
        Diag {
            level: DiagLevel::Error,
            code,
            message,
            expected: expected.into(),
            actual: actual.into(),
            file: self.file.clone(),
            span: span_at(line, col),
            repair,
        }
    }

    pub fn eval_manifest(&self, expr: &Expr) -> Result<SystemManifest, Diag> {
        let attrs = self.expect_attrset(expr, "SystemManifest")?;

        let hostname = self.require_string_field(&attrs, expr.span(), "hostname")?;
        let caps = self.require_capability_list_field(&attrs, expr.span(), "caps")?;
        let packages = self.require_string_list_field(&attrs, expr.span(), "packages")?;
        let services = self.eval_services(&attrs, expr.span(), &caps)?;
        let environment = self.eval_environment(&attrs, expr.span())?;

        Ok(SystemManifest {
            hostname,
            caps,
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
                            DiagCode::Par001,
                            format!("duplicate attribute `{}`", attr.key),
                            "unique attribute names within an attrset",
                            format!("`{}` appears more than once", attr.key),
                            Some(Repair::new(
                                "remove-unknown-field",
                                json!({ "field": &attr.key }),
                            )),
                        ));
                    }
                    map.insert(attr.key.clone(), &attr.value);
                }
                Ok(map)
            }
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ001,
                format!(
                    "expected attrset ({}), found {}",
                    context,
                    expr_type_name(other)
                ),
                format!("AttrSet ({})", context),
                expr_type_name(other),
                None,
            )),
        }
    }

    fn eval_expr_as_string(&self, expr: &Expr) -> Result<String, Diag> {
        match expr {
            Expr::Str { value, .. } => Ok(value.clone()),
            Expr::FieldAccess { .. } | Expr::Ident { .. } => self.resolve_pkg_ref(expr),
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ001,
                format!("expected String, got {}", expr_type_name(other)),
                "String",
                expr_type_name(other),
                match other {
                    Expr::Int { value, .. } => Some(Repair::new(
                        "wrap-int-as-string",
                        json!({ "value": value }),
                    )),
                    _ => None,
                },
            )),
        }
    }

    fn resolve_pkg_ref(&self, expr: &Expr) -> Result<String, Diag> {
        match expr {
            Expr::Ident { name, span } => {
                if name == "pkgs" {
                    return Err(self.diag(
                        span,
                        DiagCode::Ref002,
                        "`pkgs` alone is not a valid value; use `pkgs.<name>`".to_string(),
                        "`pkgs.<name>` field access",
                        "bare `pkgs` identifier",
                        None,
                    ));
                }
                Err(self.diag(
                    span,
                    DiagCode::Ref002,
                    format!(
                        "unknown identifier `{}`; only `pkgs` is in scope",
                        name
                    ),
                    "in-scope identifier (only `pkgs` in v2)",
                    format!("`{}`", name),
                    Some(Repair::new(
                        "quote-bare-identifier",
                        json!({ "ident": name }),
                    )),
                ))
            }
            Expr::FieldAccess { lhs, field, span } => match lhs.as_ref() {
                Expr::Ident { name, span: lhs_span } if name == "pkgs" => {
                    if !self.env.pkgs_available {
                        return Err(self.diag(
                            lhs_span,
                            DiagCode::Ref002,
                            format!(
                                "`nv-pkg` is not on PATH; `pkgs.{}` cannot be resolved",
                                field
                            ),
                            "nv-pkg available on PATH OR literal \"<name>-<version>\"",
                            "nv-pkg not on PATH",
                            Some(Repair::new(
                                "quote-bare-identifier",
                                json!({ "ident": field }),
                            )),
                        ));
                    }
                    match self.env.pkgs.get(field.as_str()) {
                        Some(versioned) => Ok(versioned.clone()),
                        None => Err(self.diag(
                            span,
                            DiagCode::Ref002,
                            format!(
                                "`pkgs.{}` is not installed; \
                                 use `nv-pkg install {}.nvpkg` first",
                                field, field
                            ),
                            format!("`{}` installed in nv-store", field),
                            format!("`{}` not in nv-pkg list", field),
                            None,
                        )),
                    }
                }
                Expr::FieldAccess { .. } => Err(self.diag(
                    span,
                    DiagCode::Ref002,
                    "chained `pkgs.*.*` access is not supported".to_string(),
                    "`pkgs.<name>` (single level)",
                    "multi-level pkgs path",
                    None,
                )),
                other => Err(self.diag(
                    other.span(),
                    DiagCode::Ref002,
                    format!(
                        "field access on `{}` is not supported",
                        expr_type_name(other)
                    ),
                    "`pkgs.<name>`",
                    format!("field access on {}", expr_type_name(other)),
                    None,
                )),
            },
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ001,
                format!("expected String, got {}", expr_type_name(other)),
                "String",
                expr_type_name(other),
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
                DiagCode::Sch001,
                format!("missing required field `{}`", key),
                format!("`{}` present", key),
                format!("`{}` absent", key),
                Some(Repair::new(
                    "add-required-field",
                    json!({ "field": key, "type": "String" }),
                )),
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
                DiagCode::Sch001,
                format!("missing required field `{}`", key),
                format!("`{}` present", key),
                format!("`{}` absent", key),
                Some(Repair::new(
                    "add-required-field",
                    json!({ "field": key, "type": "[String]" }),
                )),
            )),
            Some(expr) => self.eval_string_list(expr),
        }
    }

    fn require_capability_list_field(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
        key: &str,
    ) -> Result<Vec<Capability>, Diag> {
        match attrs.get(key) {
            None => Err(self.diag(
                parent_span,
                DiagCode::Sch001,
                format!("missing required field `{}`", key),
                format!("`{}` present", key),
                format!("`{}` absent", key),
                Some(Repair::new(
                    "add-required-field",
                    json!({ "field": key, "type": "[Capability]" }),
                )),
            )),
            Some(expr) => self.eval_capability_list(expr),
        }
    }

    fn eval_string_list(&self, expr: &Expr) -> Result<Vec<String>, Diag> {
        match expr {
            Expr::List { items, .. } => {
                let mut result = Vec::new();
                for item in items {
                    result.push(self.eval_expr_as_string(item)?);
                }
                Ok(result)
            }
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ001,
                format!("expected [String], got {}", expr_type_name(other)),
                "[String]",
                expr_type_name(other),
                Some(Repair::new(
                    "homogenize-list",
                    json!({ "target-type": "String" }),
                )),
            )),
        }
    }

    fn eval_capability_list(&self, expr: &Expr) -> Result<Vec<Capability>, Diag> {
        match expr {
            Expr::List { items, .. } => {
                let mut result = Vec::new();
                for item in items {
                    result.push(self.eval_capability(item)?);
                }
                Ok(result)
            }
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ001,
                format!("expected [Capability], got {}", expr_type_name(other)),
                "[Capability]",
                expr_type_name(other),
                Some(Repair::new(
                    "homogenize-list",
                    json!({ "target-type": "Capability" }),
                )),
            )),
        }
    }

    /// Validate an `Expr::Capability` literal against the SPEC §5.5 set.
    fn eval_capability(&self, expr: &Expr) -> Result<Capability, Diag> {
        match expr {
            Expr::Capability { path, arg, span } => {
                let cap = Capability {
                    path: path.clone(),
                    arg: arg.clone(),
                };
                if !known_capability(&cap.path, cap.arg.is_some()) {
                    return Err(self.diag(
                        span,
                        DiagCode::Cap001,
                        format!(
                            "unknown capability `{}`; not in v2 vocabulary",
                            cap.render()
                        ),
                        "one of: !net, !net.localhost, !fs.read.\"<path>\", \
                         !fs.write.\"<path>\", !tty, !proc.spawn, !proc.exec, \
                         !time, !rand, !activate.system",
                        cap.render(),
                        None,
                    ));
                }
                Ok(cap)
            }
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ001,
                format!("expected Capability, got {}", expr_type_name(other)),
                "Capability",
                expr_type_name(other),
                None,
            )),
        }
    }

    fn eval_services(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
        system_caps: &[Capability],
    ) -> Result<HashMap<String, Service>, Diag> {
        match attrs.get("services") {
            None => Err(self.diag(
                parent_span,
                DiagCode::Sch001,
                "missing required field `services`".to_string(),
                "`services` present",
                "`services` absent",
                Some(Repair::new(
                    "add-required-field",
                    json!({ "field": "services", "type": "{ String: Service }" }),
                )),
            )),
            Some(expr) => {
                let service_attrs = self.expect_attrset(expr, "{ [String]: Service }")?;
                let mut services = HashMap::new();
                for (name, svc_expr) in &service_attrs {
                    let svc = self.eval_service(svc_expr, name, system_caps)?;
                    services.insert(name.clone(), svc);
                }
                Ok(services)
            }
        }
    }

    fn eval_service(
        &self,
        expr: &Expr,
        name: &str,
        system_caps: &[Capability],
    ) -> Result<Service, Diag> {
        let attrs = self.expect_attrset(expr, &format!("Service ({})", name))?;

        let exec = self.require_string_field(&attrs, expr.span(), "exec")?;
        let restart = self.eval_restart(&attrs, expr.span(), name)?;
        let requires = self.require_capability_list_field(&attrs, expr.span(), "requires")?;

        // SPEC §5.5: every service.requires ⊆ system.caps.
        for req in &requires {
            if !system_caps.iter().any(|granted| granted == req) {
                return Err(self.diag(
                    expr.span(),
                    DiagCode::Cap004,
                    format!(
                        "service `{}` requires capability `{}` not granted by system",
                        name,
                        req.render()
                    ),
                    format!("system.caps contains {}", req.render()),
                    format!(
                        "system.caps = [{}]",
                        system_caps
                            .iter()
                            .map(|c| c.render())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    Some(Repair::new(
                        "add-system-cap",
                        json!({
                            "cap": req.render(),
                            "path": req.path,
                            "arg": req.arg,
                        }),
                    )),
                ));
            }
        }

        Ok(Service {
            exec,
            restart,
            requires,
        })
    }

    fn eval_restart(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
        service_name: &str,
    ) -> Result<RestartPolicy, Diag> {
        let expr = match attrs.get("restart") {
            Some(e) => *e,
            None => {
                return Err(self.diag(
                    parent_span,
                    DiagCode::Sch001,
                    format!(
                        "service `{}` is missing required field `restart`",
                        service_name
                    ),
                    "`restart` present",
                    "`restart` absent",
                    Some(Repair::new(
                        "add-required-field",
                        json!({ "field": "restart", "type": "Restart" }),
                    )),
                ));
            }
        };
        match expr {
            Expr::Symbol { name, span } => match name.as_str() {
                "always" => Ok(RestartPolicy::Always),
                "on-failure" => Ok(RestartPolicy::OnFailure),
                "never" => Ok(RestartPolicy::Never),
                other => Err(self.diag(
                    span,
                    DiagCode::Typ004,
                    format!(
                        "invalid restart symbol `.{}`; expected one of \
                         .always, .on-failure, .never",
                        other
                    ),
                    "one of .always, .on-failure, .never",
                    format!(".{}", other),
                    Some(Repair::new(
                        "fix-enum-symbol",
                        json!({
                            "got": other,
                            "valid": [".always", ".on-failure", ".never"],
                        }),
                    )),
                )),
            },
            Expr::Str { value, span } => Err(self.diag(
                span,
                DiagCode::Typ004,
                format!("expected Restart symbol, got String \"{}\"", value),
                "one of .always, .on-failure, .never",
                format!("String \"{}\"", value),
                Some(Repair::new(
                    "fix-enum-symbol",
                    json!({
                        "got": value,
                        "valid": [".always", ".on-failure", ".never"],
                    }),
                )),
            )),
            other => Err(self.diag(
                other.span(),
                DiagCode::Typ004,
                format!(
                    "expected Restart symbol, got {}",
                    expr_type_name(other)
                ),
                "Restart symbol",
                expr_type_name(other),
                None,
            )),
        }
    }

    fn eval_environment(
        &self,
        attrs: &HashMap<String, &Expr>,
        parent_span: &Span,
    ) -> Result<HashMap<String, String>, Diag> {
        match attrs.get("environment") {
            None => Err(self.diag(
                parent_span,
                DiagCode::Sch001,
                "missing required field `environment`".to_string(),
                "`environment` present",
                "`environment` absent",
                Some(Repair::new(
                    "add-required-field",
                    json!({ "field": "environment", "type": "{ String: String }" }),
                )),
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

// ---------------------------------------------------------------------------
// Capability vocabulary (SPEC §5.5)
// ---------------------------------------------------------------------------

fn known_capability(path: &[String], has_arg: bool) -> bool {
    let p: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
    matches!(
        (p.as_slice(), has_arg),
        (["net"], false)
            | (["net", "localhost"], false)
            | (["fs", "read"], true)
            | (["fs", "write"], true)
            | (["tty"], false)
            | (["proc", "spawn"], false)
            | (["proc", "exec"], false)
            | (["time"], false)
            | (["rand"], false)
            | (["activate", "system"], false)
    )
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
        Expr::Symbol { .. } => "Symbol",
        Expr::Capability { .. } => "Capability",
    }
}
