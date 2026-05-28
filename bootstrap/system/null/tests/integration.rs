/// Integration tests for the `null` language toolchain.
///
/// These tests exercise parse, type-check, eval, and fmt in isolation
/// (no subprocess spawn needed — we call the library functions directly).
use std::collections::HashMap;

use null::ast::Expr;
use null::diagnostics::DiagCode;
use null::fmt::format_expr;
use null::types::{Env, RestartPolicy};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn empty_env() -> Env {
    Env {
        pkgs: HashMap::new(),
        pkgs_available: false,
    }
}

fn pkgs_env(entries: &[(&str, &str)]) -> Env {
    let mut pkgs = HashMap::new();
    for (name, ver) in entries {
        pkgs.insert(name.to_string(), format!("{}-{}", name, ver));
    }
    Env {
        pkgs,
        pkgs_available: true,
    }
}

fn do_parse(src: &str) -> Result<Expr, null::diagnostics::Diag> {
    null::run_parse(src, "<test>")
}

fn do_eval(src: &str, env: &Env) -> Result<null::types::SystemManifest, null::diagnostics::Diag> {
    null::run_eval(src, "<test>", env)
}

fn do_fmt(src: &str) -> Result<String, null::diagnostics::Diag> {
    let ast = do_parse(src)?;
    Ok(format_expr(&ast))
}

// ---------------------------------------------------------------------------
// §2.2 Parse cases
// ---------------------------------------------------------------------------

#[test]
fn parse_string_literal() {
    let ast = do_parse(r#""hello world""#).unwrap();
    assert!(matches!(ast, Expr::Str { value, .. } if value == "hello world"));
}

#[test]
fn parse_int_literal() {
    let ast = do_parse("42").unwrap();
    assert!(matches!(ast, Expr::Int { value: 42, .. }));
}

#[test]
fn parse_negative_int() {
    let ast = do_parse("-7").unwrap();
    assert!(matches!(ast, Expr::Int { value: -7, .. }));
}

#[test]
fn parse_bool_true() {
    let ast = do_parse("true").unwrap();
    assert!(matches!(ast, Expr::Bool { value: true, .. }));
}

#[test]
fn parse_bool_false() {
    let ast = do_parse("false").unwrap();
    assert!(matches!(ast, Expr::Bool { value: false, .. }));
}

#[test]
fn parse_null_literal() {
    let ast = do_parse("null").unwrap();
    assert!(matches!(ast, Expr::Null { .. }));
}

#[test]
fn parse_empty_list() {
    let ast = do_parse("[ ]").unwrap();
    assert!(matches!(ast, Expr::List { ref items, .. } if items.is_empty()));
}

#[test]
fn parse_string_list() {
    let ast = do_parse(r#"[ "a" "b" "c" ]"#).unwrap();
    match ast {
        Expr::List { items, .. } => {
            assert_eq!(items.len(), 3);
            assert!(matches!(&items[0], Expr::Str { value, .. } if value == "a"));
            assert!(matches!(&items[1], Expr::Str { value, .. } if value == "b"));
            assert!(matches!(&items[2], Expr::Str { value, .. } if value == "c"));
        }
        _ => panic!("expected list"),
    }
}

#[test]
fn parse_empty_attrset() {
    let ast = do_parse("{ }").unwrap();
    assert!(matches!(ast, Expr::AttrSet { ref attrs, .. } if attrs.is_empty()));
}

#[test]
fn parse_attrset_single_field() {
    let ast = do_parse(r#"{ hostname = "nullvoid"; }"#).unwrap();
    match ast {
        Expr::AttrSet { attrs, .. } => {
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].key, "hostname");
            assert!(matches!(&attrs[0].value, Expr::Str { value, .. } if value == "nullvoid"));
        }
        _ => panic!("expected attrset"),
    }
}

#[test]
fn parse_nested_attrset() {
    let src = r#"{
  outer = {
    inner = "value";
  };
}"#;
    let ast = do_parse(src).unwrap();
    match ast {
        Expr::AttrSet { attrs, .. } => {
            assert_eq!(attrs[0].key, "outer");
            assert!(matches!(&attrs[0].value, Expr::AttrSet { .. }));
        }
        _ => panic!("expected attrset"),
    }
}

#[test]
fn parse_path_literal() {
    // Paths become Str nodes with the path string
    let ast = do_parse("./relative/file.txt").unwrap();
    assert!(matches!(ast, Expr::Str { value, .. } if value.starts_with("./")));
}

#[test]
fn parse_ident_pkgs() {
    let ast = do_parse("pkgs").unwrap();
    assert!(matches!(ast, Expr::Ident { ref name, .. } if name == "pkgs"));
}

#[test]
fn parse_field_access() {
    let ast = do_parse("pkgs.bash").unwrap();
    match ast {
        Expr::FieldAccess { lhs, field, .. } => {
            assert_eq!(field, "bash");
            assert!(matches!(*lhs, Expr::Ident { ref name, .. } if name == "pkgs"));
        }
        _ => panic!("expected FieldAccess"),
    }
}

#[test]
fn parse_chained_field_access() {
    let ast = do_parse("pkgs.bash.whatever").unwrap();
    // Outermost node is FieldAccess with field = "whatever"
    assert!(matches!(ast, Expr::FieldAccess { ref field, .. } if field == "whatever"));
}

#[test]
fn parse_comment_ignored() {
    let ast = do_parse(
        r#"{ # this is a comment
  key = "val"; # another comment
}"#,
    )
    .unwrap();
    match ast {
        Expr::AttrSet { attrs, .. } => {
            assert_eq!(attrs.len(), 1);
        }
        _ => panic!("expected attrset"),
    }
}

#[test]
fn parse_string_escape_sequences() {
    let ast = do_parse(r#""line1\nline2\ttabbed""#).unwrap();
    assert!(
        matches!(ast, Expr::Str { ref value, .. } if value.contains('\n') && value.contains('\t'))
    );
}

// ---------------------------------------------------------------------------
// PAR001 — deferred constructs rejected
// ---------------------------------------------------------------------------

#[test]
fn parse_rejects_let_in() {
    let err = do_parse("let x = 1; in x").unwrap_err();
    assert_eq!(err.code, DiagCode::Par001);
    assert!(err.message.contains("Phase 1 MVP"));
}

#[test]
fn parse_rejects_if_then_else() {
    let err = do_parse("if true then 1 else 2").unwrap_err();
    assert_eq!(err.code, DiagCode::Par001);
    assert!(err.message.contains("Phase 1 MVP"));
}

#[test]
fn parse_rejects_import() {
    let err = do_parse("import ./other.null").unwrap_err();
    assert_eq!(err.code, DiagCode::Par001);
    assert!(err.message.contains("Phase 1 MVP"));
}

// ---------------------------------------------------------------------------
// Type-check: valid system.null
// ---------------------------------------------------------------------------

const VALID_MANIFEST: &str = r#"{
  hostname = "nullvoid";
  packages = [
    "neovim-mini-0.1.0"
    "bash-5.3.9"
  ];
  services = {
    agent = {
      exec = "/run/current/bin/claude";
      restart = "always";
    };
  };
  environment = {
    EDITOR = "nvim-mini";
    LANG = "en_US.UTF-8";
  };
}"#;

#[test]
fn typecheck_valid_manifest_succeeds() {
    let result = do_eval(VALID_MANIFEST, &empty_env());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    let m = result.unwrap();
    assert_eq!(m.hostname, "nullvoid");
    assert_eq!(m.packages.len(), 2);
}

// ---------------------------------------------------------------------------
// Type errors — each kind per spec §7
// ---------------------------------------------------------------------------

#[test]
fn typecheck_missing_hostname() {
    let src = r#"{
  packages = [ "bash-5.3.9" ];
  services = {};
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("hostname"));
}

#[test]
fn typecheck_wrong_type_hostname_int() {
    let src = r#"{
  hostname = 42;
  packages = [];
  services = {};
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    // Should mention the type mismatch or provide a fix
    assert!(
        err.message.contains("String")
            || err
                .fix
                .as_deref()
                .map_or(false, |f| f.contains("42") || f.contains("\""))
    );
}

#[test]
fn typecheck_wrong_type_package_int_in_list() {
    // A list containing a non-string item
    let src = r#"{
  hostname = "h";
  packages = [ "bash-5.3.9" 42 ];
  services = {};
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("String") || err.message.contains("Int"));
}

#[test]
fn typecheck_invalid_restart_policy() {
    let src = r#"{
  hostname = "h";
  packages = [];
  services = {
    s = {
      exec = "/bin/sh";
      restart = "whenever";
    };
  };
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("restart policy") || err.message.contains("whenever"));
}

#[test]
fn typecheck_unknown_identifier() {
    let src = r#"{
  hostname = "h";
  packages = [ unknownident ];
  services = {};
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("unknown identifier") || err.message.contains("scope"));
}

#[test]
fn typecheck_missing_services() {
    let src = r#"{
  hostname = "h";
  packages = [];
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("services"));
}

#[test]
fn typecheck_missing_environment() {
    let src = r#"{
  hostname = "h";
  packages = [];
  services = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("environment"));
}

#[test]
fn typecheck_service_missing_exec() {
    let src = r#"{
  hostname = "h";
  packages = [];
  services = {
    s = {
      restart = "always";
    };
  };
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("exec"));
}

#[test]
fn typecheck_service_missing_restart() {
    let src = r#"{
  hostname = "h";
  packages = [];
  services = {
    s = {
      exec = "/bin/sh";
    };
  };
  environment = {};
}"#;
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("restart"));
}

// ---------------------------------------------------------------------------
// Test 4: Eval a valid example → JSON matches expected
// ---------------------------------------------------------------------------

#[test]
fn eval_manifest_json_matches_expected() {
    let m = do_eval(VALID_MANIFEST, &empty_env()).unwrap();
    let json = serde_json::to_value(&m).unwrap();
    assert_eq!(json["hostname"], "nullvoid");
    assert_eq!(json["packages"][0], "neovim-mini-0.1.0");
    assert_eq!(json["packages"][1], "bash-5.3.9");
    assert_eq!(json["services"]["agent"]["exec"], "/run/current/bin/claude");
    assert_eq!(json["services"]["agent"]["restart"], "always");
    assert_eq!(json["environment"]["EDITOR"], "nvim-mini");
    assert_eq!(json["environment"]["LANG"], "en_US.UTF-8");
}

#[test]
fn eval_restart_on_failure() {
    let src = r#"{
  hostname = "h";
  packages = [];
  services = {
    s = {
      exec = "/bin/sh";
      restart = "on-failure";
    };
  };
  environment = {};
}"#;
    let m = do_eval(src, &empty_env()).unwrap();
    assert_eq!(m.services["s"].restart, RestartPolicy::OnFailure);
}

#[test]
fn eval_restart_never() {
    let src = r#"{
  hostname = "h";
  packages = [];
  services = {
    s = {
      exec = "/bin/sh";
      restart = "never";
    };
  };
  environment = {};
}"#;
    let m = do_eval(src, &empty_env()).unwrap();
    assert_eq!(m.services["s"].restart, RestartPolicy::Never);
}

// ---------------------------------------------------------------------------
// Test 5: pkgs ambient
// ---------------------------------------------------------------------------

#[test]
fn pkgs_resolves_when_available() {
    let env = pkgs_env(&[("bash", "5.3.9"), ("neovim-mini", "0.1.0")]);
    let src = r#"{
  hostname = "h";
  packages = [ pkgs.bash ];
  services = {};
  environment = {};
}"#;
    let m = do_eval(src, &env).unwrap();
    assert!(m.packages.contains(&"bash-5.3.9".to_string()));
}

#[test]
fn pkgs_fails_when_not_available_and_used() {
    // No pkgs available + pkgs.bash reference → hard error
    let env = empty_env(); // pkgs_available = false
    let src = r#"{
  hostname = "h";
  packages = [ pkgs.bash ];
  services = {};
  environment = {};
}"#;
    let err = do_eval(src, &env).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("nv-pkg") || err.message.contains("PATH"));
}

#[test]
fn pkgs_not_available_no_references_ok() {
    // No pkgs available but no pkgs.* references → should succeed
    let env = empty_env();
    let result = do_eval(VALID_MANIFEST, &env);
    assert!(result.is_ok());
}

#[test]
fn pkgs_unknown_package_fails() {
    let env = pkgs_env(&[("bash", "5.3.9")]);
    let src = r#"{
  hostname = "h";
  packages = [ pkgs.nonexistent ];
  services = {};
  environment = {};
}"#;
    let err = do_eval(src, &env).unwrap_err();
    assert_eq!(err.code, DiagCode::Typ001);
    assert!(err.message.contains("nonexistent") || err.message.contains("not installed"));
}

// ---------------------------------------------------------------------------
// Test 5: null fmt idempotency
// ---------------------------------------------------------------------------

#[test]
fn fmt_idempotent_valid_manifest() {
    let first = do_fmt(VALID_MANIFEST).unwrap();
    let second = do_fmt(&first).unwrap();
    assert_eq!(first, second, "fmt is not idempotent");
}

#[test]
fn fmt_idempotent_empty_structures() {
    let src = "{ }";
    let first = do_fmt(src).unwrap();
    let second = do_fmt(&first).unwrap();
    assert_eq!(first, second);
}

#[test]
fn fmt_idempotent_nested() {
    let src = r#"{
  services = {
    s = {
      exec = "/bin/sh";
      restart = "always";
    };
  };
  hostname = "h";
  packages = [
    "a"
    "b"
  ];
  environment = {
    X = "1";
  };
}"#;
    let first = do_fmt(src).unwrap();
    let second = do_fmt(&first).unwrap();
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// Test 6: null parse --json round-trip
// ---------------------------------------------------------------------------

#[test]
fn parse_json_round_trip() {
    let ast1 = do_parse(VALID_MANIFEST).unwrap();
    let json1 = serde_json::to_string(&ast1).unwrap();
    let ast2: null::ast::Expr = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string(&ast2).unwrap();
    assert_eq!(json1, json2, "AST JSON round-trip failed");
}

// ---------------------------------------------------------------------------
// Diagnostics: file:line:col + error codes
// ---------------------------------------------------------------------------

#[test]
fn diag_has_file_line_col() {
    let src = "{ hostname = 42; packages = []; services = {}; environment = {}; }";
    let err = do_eval(src, &empty_env()).unwrap_err();
    assert_eq!(err.file, "<test>");
    assert!(err.line >= 1);
    assert!(err.col >= 1);
    assert_eq!(err.code, DiagCode::Typ001);
}

#[test]
fn diag_parse_error_has_par001() {
    let err = do_parse("let x = 1; in x").unwrap_err();
    assert_eq!(err.code, DiagCode::Par001);
    assert!(err.line >= 1);
}

#[test]
fn diag_fix_field_for_int_hostname() {
    let src = "{ hostname = 42; packages = []; services = {}; environment = {}; }";
    let err = do_eval(src, &empty_env()).unwrap_err();
    // Should have a fix suggestion for wrapping int in quotes
    assert!(
        err.fix.is_some(),
        "expected a fix hint for wrapping int in quotes"
    );
}

// ---------------------------------------------------------------------------
// Examples smoke-test: all three example files parse without error
// ---------------------------------------------------------------------------

#[test]
fn example_minimal_parses() {
    let src = include_str!("../examples/minimal.null");
    let result = do_parse(src);
    assert!(result.is_ok(), "minimal.null failed to parse: {:?}", result.err());
}

#[test]
fn example_minimal_typechecks() {
    let src = include_str!("../examples/minimal.null");
    let result = do_eval(src, &empty_env());
    assert!(result.is_ok(), "minimal.null failed type-check: {:?}", result.err());
}

#[test]
fn example_standard_parses() {
    let src = include_str!("../examples/standard.null");
    let result = do_parse(src);
    assert!(result.is_ok(), "standard.null failed to parse: {:?}", result.err());
}

#[test]
fn example_standard_typechecks() {
    let src = include_str!("../examples/standard.null");
    // standard.null uses only literal strings, no pkgs.* — should succeed
    let result = do_eval(src, &empty_env());
    assert!(result.is_ok(), "standard.null failed type-check: {:?}", result.err());
}

#[test]
fn example_multi_service_parses() {
    let src = include_str!("../examples/multi-service.null");
    let result = do_parse(src);
    assert!(result.is_ok(), "multi-service.null failed to parse: {:?}", result.err());
}

#[test]
fn example_multi_service_typechecks() {
    let src = include_str!("../examples/multi-service.null");
    // multi-service.null uses only literal strings too
    let result = do_eval(src, &empty_env());
    assert!(result.is_ok(), "multi-service.null failed type-check: {:?}", result.err());
}
