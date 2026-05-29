//! Front-half pipeline tests (parse + check + C emission). The cc/ELF/run
//! back half (SPEC §13) is exercised by `nullang run examples/hello.null`.
use nullang::compile_to_c;
use nullang::package::{capabilities_of_main, Manifest};
use nullang::parse_only;

const HELLO: &str = r#"
fn greeting() -> String { "hi" }
fn main(world: World) -> Int uses !tty {
  print(world, greeting());
  0
}
"#;

#[test]
fn hello_compiles_to_c() {
    let c = compile_to_c(HELLO, "hello.null").expect("hello should compile");
    // main takes argv so the argc/argv builtins can reach the command line.
    assert!(c.contains("int main(int argc, char** argv)"));
    assert!(c.contains("nullang_print(nlu_greeting())"));
    assert!(c.contains("return 0;"));
    // World is erased: the call has no `world` argument.
    assert!(!c.contains("world"));
}

#[test]
fn effect_discipline_rejects_undeclared_capability() {
    // main calls print (requires !tty) without declaring `uses !tty`.
    let src = r#"
fn main(world: World) -> Int {
  print(world, "x");
  0
}
"#;
    let err = compile_to_c(src, "bad.null").expect_err("should fail effect check");
    assert_eq!(format!("{:?}", err.code), "Eff001");
    let repair = err.repair.expect("EFF001 carries a repair");
    assert_eq!(repair.id, "add-uses-clause");
}

#[test]
fn unknown_function_is_ref_error() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  frobnicate(world);
  0
}
"#;
    let err = compile_to_c(src, "ref.null").expect_err("should fail ref resolution");
    assert_eq!(format!("{:?}", err.code), "Ref001");
}

#[test]
fn main_must_return_int() {
    let src = r#"
fn main(world: World) -> String uses !tty {
  "nope"
}
"#;
    let err = compile_to_c(src, "main.null").expect_err("bad main shape");
    assert_eq!(format!("{:?}", err.code), "Sch001");
}

const COMPUTE: &str = r#"
fn sign(n: Int) -> Int {
  if n < 0 { -1 } else { if n == 0 { 0 } else { 1 } }
}
fn main(world: World) -> Int uses !tty {
  let area = 6 * 7;
  print(world, "x");
  sign(area - 42)
}
"#;

#[test]
fn arithmetic_and_if_compile() {
    let c = compile_to_c(COMPUTE, "compute.null").expect("should compile");
    assert!(c.contains("(6 * 7)"));
    assert!(c.contains("(nlu_area - 42)"));
    // `if` is lowered to a temporary + statement, not left in expression form.
    assert!(c.contains("if ("));
}

#[test]
fn if_branches_must_agree_in_type() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let x = if true { 1 } else { "no" };
  0
}
"#;
    let err = compile_to_c(src, "if.null").expect_err("branch type mismatch");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn arithmetic_rejects_non_int() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let x = 1 + "two";
  0
}
"#;
    let err = compile_to_c(src, "arith.null").expect_err("non-int arithmetic");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn if_condition_must_be_bool() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let x = if 3 { 1 } else { 2 };
  0
}
"#;
    let err = compile_to_c(src, "cond.null").expect_err("non-bool condition");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

const STATUS: &str = r#"
enum Restart = .always | .on_failure | .never;
fn code(r: Restart) -> Int {
  match r { .always => 0, .on_failure => 1, .never => 2 }
}
fn main(world: World) -> Int uses !tty {
  print(world, "x");
  code(.never)
}
"#;

#[test]
fn enum_match_compiles() {
    let c = compile_to_c(STATUS, "status.null").expect("should compile");
    assert!(c.contains("switch ("));
    assert!(c.contains("case 0:"));
    assert!(c.contains("case 2:"));
    // `.never` passed as an argument lowers to its index, 2.
    assert!(c.contains("code(2)"));
}

#[test]
fn non_exhaustive_match_is_typ020_with_repair() {
    let src = r#"
enum Restart = .always | .on_failure | .never;
fn code(r: Restart) -> Int {
  match r { .always => 0, .never => 2 }
}
fn main(world: World) -> Int uses !tty { print(world, "x"); code(.always) }
"#;
    let err = compile_to_c(src, "ne.null").expect_err("non-exhaustive");
    assert_eq!(format!("{:?}", err.code), "Typ020");
    let r = err.repair.expect("carries a repair");
    assert_eq!(r.id, "add-missing-arm");
}

#[test]
fn foreign_symbol_in_match_is_typ020() {
    let src = r#"
enum Restart = .always | .never;
enum Color = .red | .green;
fn code(r: Restart) -> Int {
  match r { .always => 0, .red => 1, .never => 2 }
}
fn main(world: World) -> Int uses !tty { print(world, "x"); code(.always) }
"#;
    let err = compile_to_c(src, "fs.null").expect_err("foreign symbol");
    assert_eq!(format!("{:?}", err.code), "Typ020");
}

#[test]
fn duplicate_symbol_across_enums_is_sch010() {
    let src = r#"
enum A = .shared | .a2;
enum B = .shared | .b2;
fn main(world: World) -> Int uses !tty { print(world, "x"); 0 }
"#;
    let err = compile_to_c(src, "dup.null").expect_err("symbol collision");
    assert_eq!(format!("{:?}", err.code), "Sch010");
}

// --- Enum payloads (v0.2, SPEC §4.2/§4.5) -------------------------------

const PAYLOAD: &str = r#"
enum Status = .code(Int) | .message(String) | .none;
fn to_code(s: Status) -> Int {
  match s { .code(n) => n, .message(_) => -1, .none => 0 }
}
fn main(world: World) -> Int uses !tty {
  print(world, "x");
  to_code(.code(42))
}
"#;

#[test]
fn payload_enum_lowers_to_tagged_union() {
    let c = compile_to_c(PAYLOAD, "payload.null").expect("should compile");
    // A tagged-union typedef with one member per payload variant.
    assert!(c.contains("typedef struct"));
    assert!(c.contains("long _v0;"));
    assert!(c.contains("const char* _v1;"));
    // Construction is a compound literal carrying the tag and payload.
    assert!(c.contains(".tag = 0, .u._v0 = 42"));
    // The bare variant of a tagged enum still constructs the struct.
    assert!(c.contains("switch ("));
    assert!(c.contains(".tag)"));
    // The match arm binds the payload out of the union.
    assert!(c.contains("long n = "));
    assert!(c.contains(".u._v0;"));
}

#[test]
fn payload_free_enum_stays_bare_long() {
    // The existing flag-style enum must not gain a typedef or .tag dispatch.
    let c = compile_to_c(STATUS, "status.null").expect("should compile");
    assert!(!c.contains("typedef struct"));
    assert!(!c.contains(".tag"));
    assert!(c.contains("code(2)"));
}

#[test]
fn payload_type_mismatch_is_typ001() {
    let src = r#"
enum Status = .code(Int) | .none;
fn main(world: World) -> Int uses !tty {
  print(world, "x");
  match .code(true) { .code(n) => n, .none => 0 }
}
"#;
    let err = compile_to_c(src, "pt.null").expect_err("payload type mismatch");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn missing_payload_is_typ021_with_repair() {
    let src = r#"
enum Status = .code(Int) | .none;
fn main(world: World) -> Int uses !tty {
  print(world, "x");
  match .code { .code(n) => n, .none => 0 }
}
"#;
    let err = compile_to_c(src, "mp.null").expect_err("missing payload");
    assert_eq!(format!("{:?}", err.code), "Typ021");
    assert_eq!(err.repair.expect("carries a repair").id, "supply-payload");
}

#[test]
fn payload_on_bare_variant_is_typ021() {
    let src = r#"
enum Status = .code(Int) | .none;
fn main(world: World) -> Int uses !tty {
  print(world, "x");
  match .none(5) { .code(n) => n, .none => 0 }
}
"#;
    let err = compile_to_c(src, "bp.null").expect_err("payload on bare variant");
    assert_eq!(format!("{:?}", err.code), "Typ021");
}

#[test]
fn match_arm_missing_binder_is_typ021() {
    let src = r#"
enum Status = .code(Int) | .none;
fn to_code(s: Status) -> Int { match s { .code => 1, .none => 0 } }
fn main(world: World) -> Int uses !tty { print(world, "x"); to_code(.none) }
"#;
    let err = compile_to_c(src, "nb.null").expect_err("payload arm needs a binder");
    assert_eq!(format!("{:?}", err.code), "Typ021");
    assert_eq!(err.repair.expect("carries a repair").id, "bind-payload");
}

#[test]
fn match_arm_binder_on_bare_variant_is_typ021() {
    let src = r#"
enum Status = .code(Int) | .none;
fn to_code(s: Status) -> Int { match s { .code(n) => n, .none(x) => x } }
fn main(world: World) -> Int uses !tty { print(world, "x"); to_code(.none) }
"#;
    let err = compile_to_c(src, "bb.null").expect_err("binder on a bare arm");
    assert_eq!(format!("{:?}", err.code), "Typ021");
}

#[test]
fn disallowed_payload_type_is_sch010() {
    let src = r#"
enum Inner = .a | .b;
enum Outer = .wrap(Inner) | .none;
fn main(world: World) -> Int uses !tty { print(world, "x"); 0 }
"#;
    let err = compile_to_c(src, "dp.null").expect_err("enum payloads not allowed yet");
    assert_eq!(format!("{:?}", err.code), "Sch010");
}

#[test]
fn discarded_payload_binder_compiles() {
    // `_` discards the payload: no binding is emitted, but it still compiles.
    let src = r#"
enum Status = .code(Int) | .none;
fn to_code(s: Status) -> Int { match s { .code(_) => 1, .none => 0 } }
fn main(world: World) -> Int uses !tty { print(world, "x"); to_code(.code(9)) }
"#;
    let c = compile_to_c(src, "disc.null").expect("should compile");
    assert!(c.contains("switch ("));
    // No binding variable for the discarded payload.
    assert!(!c.contains("_ = "));
}

// --- Explicit string composition (v0.2, SPEC §10) ----------------------

#[test]
fn string_composition_builtins_lower_to_c() {
    // `concat`/`str_of_int` are pure: a String-composing fn needs no `uses`.
    let src = r#"
fn label(n: Int) -> String { concat("n=", str_of_int(n)) }
fn main(world: World) -> Int uses !tty { print(world, label(7)); 0 }
"#;
    let c = compile_to_c(src, "compose.null").expect("should compile");
    assert!(c.contains("nullang_concat("));
    assert!(c.contains("nullang_str_of_int("));
}

#[test]
fn concat_is_binary_only() {
    // §10 forbids variadics; `concat` takes exactly two arguments.
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, concat("a", "b", "c"));
  0
}
"#;
    let err = compile_to_c(src, "var.null").expect_err("concat is binary");
    assert_eq!(format!("{:?}", err.code), "Typ002");
}

#[test]
fn str_of_int_rejects_non_int() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, str_of_int("x"));
  0
}
"#;
    let err = compile_to_c(src, "soi.null").expect_err("str_of_int wants Int");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn capabilities_derived_from_main_uses() {
    let src = r#"
fn main(world: World) -> Int uses !net, !fs.read."/etc", !tty {
  print(world, "x");
  0
}
"#;
    let f = parse_only(src, "caps.null").expect("parses");
    let caps = capabilities_of_main(&f);
    // Mapped to CONTRACTS.md §4 strings, in declaration order.
    assert_eq!(caps, vec!["net", "fs:read:/etc", "tty"]);
}

#[test]
fn manifest_is_schema_v1_nullang() {
    let m = Manifest::new(
        "notes",
        "0.1.0",
        "desc",
        "agent-x",
        "2026-01-01T00:00:00Z",
        vec!["tty".to_string()],
        vec!["nullang 0.1.0".to_string()],
    );
    let j = m.to_json();
    assert!(j.contains("\"schemaVersion\": 1"));
    assert!(j.contains("\"sourceLanguage\": \"nullang\""));
    assert!(j.contains("\"exposedBins\""));
    assert!(j.contains("notes"));
}

// ─── Tier 0 — string decomposition, String ==, file I/O ───────────────────────

#[test]
fn tier0_string_decomposition_compiles() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, substr("hello", 0, 2));
  str_len("hello")
}
"#;
    let c = compile_to_c(src, "dec.null").expect("str_len/substr should compile");
    assert!(c.contains("nullang_str_len("));
    assert!(c.contains("nullang_substr("));
}

#[test]
fn tier0_string_equality_lowers_to_strcmp() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  if "a" == "a" { 0 } else { 1 }
}
"#;
    let c = compile_to_c(src, "eq.null").expect("String == should compile");
    // Content comparison, not pointer comparison.
    assert!(c.contains("strcmp("));
    assert!(c.contains(") == 0)"));
}

#[test]
fn tier0_string_equality_rejects_mixed_operands() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  if "a" == 1 { 0 } else { 1 }
}
"#;
    let err = compile_to_c(src, "mix.null").expect_err("String == Int must fail");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn tier0_file_io_requires_fs_effects() {
    // read_file exercises !fs.read; main does not declare it.
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, read_file(world, "/etc/hostname"));
  0
}
"#;
    let err = compile_to_c(src, "noeff.null").expect_err("read_file needs uses !fs.read");
    assert_eq!(format!("{:?}", err.code), "Eff001");
    assert_eq!(err.repair.expect("repair").id, "add-uses-clause");
}

#[test]
fn tier0_file_io_compiles_with_effects_and_erases_world() {
    let src = r#"
fn main(world: World) -> Int uses !fs.read, !fs.write {
  write_file(world, "/tmp/x", read_file(world, "/tmp/y"));
  0
}
"#;
    let c = compile_to_c(src, "io.null").expect("file I/O should compile with effects");
    // World erased: the C calls take only the data arguments.
    assert!(c.contains("nullang_read_file(\"/tmp/y\")"));
    assert!(c.contains("nullang_write_file(\"/tmp/x\", nullang_read_file(\"/tmp/y\"))"));
}

#[test]
fn tier0_file_effects_flow_into_package_capabilities() {
    // The seam to Traccia A: a file-reading/writing program's capabilities are
    // derived from its `uses` clause, so nv-rebuild's Landlock confinement
    // gates exactly what the language declared.
    let src = r#"
fn main(world: World) -> Int uses !fs.read, !fs.write, !tty {
  write_file(world, "/tmp/x", "data");
  0
}
"#;
    let f = parse_only(src, "seam.null").expect("parses");
    assert_eq!(capabilities_of_main(&f), vec!["fs:read", "fs:write", "tty"]);
}

#[test]
fn argv_builtins_compile_and_need_no_effect() {
    // argc/argv are pure (no World, no `uses`): a CLI tool reads its args
    // without declaring a capability. Gate for `cat <file>`/`sed`-likes.
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, str_of_int(argc()));
  print(world, argv(1));
  0
}
"#;
    let c = compile_to_c(src, "argv.null").expect("argv/argc should compile");
    assert!(c.contains("nullang_argc()"));
    assert!(c.contains("nullang_argv(1)"));
    // main now receives the command line.
    assert!(c.contains("int main(int argc, char** argv)"));
    assert!(c.contains("nl_argc = argc; nl_argv = argv;"));
}

#[test]
fn user_identifiers_clashing_with_c_keywords_are_mangled() {
    // `double` is a C keyword; a Nullang fn/param/let named so must not reach
    // the emitted C verbatim (it did, and broke the cc step — found by the
    // in-VM smoke probe). Everything user-named is prefixed `nlu_`.
    let src = r#"
fn double(int: Int) -> Int {
  let static = int + int;
  static
}
fn main(world: World) -> Int uses !tty {
  print(world, str_of_int(double(21)));
  0
}
"#;
    let c = compile_to_c(src, "kw.null").expect("C-keyword identifiers must compile");
    // Definition, call, param and let are all prefixed; no bare keyword leaks.
    assert!(c.contains("nlu_double("));
    assert!(c.contains("nlu_int"));   // param `int`
    assert!(c.contains("nlu_static")); // let `static`
    // main stays the C entry point, unmangled.
    assert!(c.contains("int main(int argc, char** argv)"));
    assert!(!c.contains("nlu_main"));
}
