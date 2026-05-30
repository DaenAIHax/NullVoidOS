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
    // The existing flag-style enum must not gain a tagged-union struct or .tag
    // dispatch. (Assert on the enum's own `nlenum<id>` typedef, not the generic
    // `typedef struct` — the List runtime in the PRELUDE also defines structs.)
    let c = compile_to_c(STATUS, "status.null").expect("should compile");
    assert!(!c.contains("nlenum"));
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
fn mut_and_while_compile_to_a_c_loop() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let mut i = 0;
  let mut sum = 0;
  while i < 10 {
    sum = sum + i;
    i = i + 1;
  }
  print(world, str_of_int(sum));
  0
}
"#;
    let c = compile_to_c(src, "loop.null").expect("mut + while should compile");
    assert!(c.contains("for (;;)"));
    assert!(c.contains("if (!("));      // condition re-checked inside the loop
    assert!(c.contains("nlu_sum = (nlu_sum + nlu_i)"));
}

#[test]
fn assigning_a_non_mut_binding_is_rejected() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let x = 0;
  x = 1;
  0
}
"#;
    let err = compile_to_c(src, "immut.null").expect_err("assigning a let must fail");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn while_condition_must_be_bool() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let mut i = 0;
  while i { i = i + 1; }
  0
}
"#;
    let err = compile_to_c(src, "whilecond.null").expect_err("non-Bool while cond must fail");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn char_at_builtin_compiles() {
    // First builtin authored by the in-VM agent (within BUILTINS_CONTRACT.md):
    // a pure O(i) single-char read. Merged host-side.
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, char_at("banana", 2));
  0
}
"#;
    let c = compile_to_c(src, "charat.null").expect("char_at should compile");
    assert!(c.contains("nullang_char_at("));
    // pure: no World threaded into the C call
    assert!(c.contains("nullang_char_at(\"banana\", 2)"));
}

// ---- P0 stdlib: char_code + int_of_str (the String<->Int seam) -----------

#[test]
fn char_code_builtin_compiles_and_is_pure() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, str_of_int(char_code("A", 0)));
  0
}
"#;
    let c = compile_to_c(src, "cc.null").expect("char_code should compile");
    // Returns Int, pure (no World), lowered to the runtime helper.
    assert!(c.contains("nullang_char_code(\"A\", 0)"));
}

#[test]
fn int_of_str_builtin_compiles_and_is_pure() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, str_of_int(int_of_str("8080")));
  0
}
"#;
    let c = compile_to_c(src, "ios.null").expect("int_of_str should compile");
    assert!(c.contains("nullang_int_of_str(\"8080\")"));
}

#[test]
fn int_of_str_return_is_int_not_string() {
    // The result is an Int, so it can drive arithmetic directly.
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let n = int_of_str("40");
  print(world, str_of_int(n + 2));
  0
}
"#;
    let c = compile_to_c(src, "ios2.null").expect("int_of_str arithmetic compiles");
    assert!(c.contains("nullang_int_of_str(\"40\")"));
}

#[test]
fn char_code_returns_int_for_ranges() {
    // char_code feeds an arithmetic range test (the point of char->Int):
    // a digit class without a 10-arm == chain.
    let src = r#"
fn is_digit(c: Int) -> Bool { c >= 48 && c <= 57 }
fn main(world: World) -> Int uses !tty {
  let code = char_code("7", 0);
  print(world, str_of_int(code));
  if is_digit(code) { print(world, "digit"); } else { print(world, "no"); }
  0
}
"#;
    let c = compile_to_c(src, "cc2.null").expect("char_code range compiles");
    assert!(c.contains("nullang_char_code("));
}

// ---- P1 stdlib: index_of + split (authored by the in-VM agent) -----------

#[test]
fn index_of_builtin_compiles_and_is_pure() {
    let src = r#"
fn main(world: World) -> Int uses !tty {
  print(world, str_of_int(index_of("a=b", "=")));
  0
}
"#;
    let c = compile_to_c(src, "io.null").expect("index_of should compile");
    assert!(c.contains("nullang_index_of(\"a=b\", \"=\")"));
}

#[test]
fn split_returns_a_list_of_string() {
    // split is the first builtin that PRODUCES a List<T>: its result indexes
    // and feeds list_len like any other List<String>.
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let parts = split("a,b,c", ",");
  print(world, str_of_int(list_len(parts)));
  print(world, parts[1]);
  0
}
"#;
    let c = compile_to_c(src, "sp.null").expect("split should compile");
    assert!(c.contains("nullang_split(\"a,b,c\", \",\")"));
    // Result is a list handle: indexing unboxes a String back out.
    assert!(c.contains("nl_list_get"));
    assert!(c.contains("(const char*)(intptr_t)"));
}

#[test]
fn split_result_type_is_list_string_not_string() {
    // The checker must see split's return as List<String>, so a String-typed
    // use of the bare result is a type error (proves the return type is wired).
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let bad: String = split("x", ",");
  0
}
"#;
    let err = compile_to_c(src, "sp2.null").expect_err("split returns a List, not a String");
    assert_eq!(format!("{:?}", err.code), "Typ001");
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

// ---- struct (SPEC §11, v0.4) ---------------------------------------------

const POINT: &str = "type Point = { x: Int, y: Int };\n";

#[test]
fn struct_construct_and_read_compile() {
    let src = format!(
        "{POINT}fn main(world: World) -> Int uses !tty {{\n  let p = Point {{ x: 1, y: 2 }};\n  print(world, str_of_int(p.x));\n  0\n}}\n"
    );
    let c = compile_to_c(&src, "s.null").expect("struct construct + read compiles");
    // Reference semantics: the value is a heap handle, built with malloc.
    assert!(c.contains("typedef struct nlstruct0_s* nlstruct0;"));
    assert!(c.contains("malloc(sizeof("));
    // Fields are mangled (`nlf_`) to dodge C keywords; read is `->`.
    assert!(c.contains("->nlf_x"));
}

#[test]
fn struct_field_write_requires_mut() {
    // p.x = v mutates through the handle; the root binding must be `let mut`.
    let src = format!(
        "{POINT}fn main(world: World) -> Int uses !tty {{\n  let p = Point {{ x: 1, y: 2 }};\n  p.x = 9;\n  0\n}}\n"
    );
    let err = compile_to_c(&src, "s.null").expect_err("field write needs let mut");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn struct_field_write_compiles_with_mut() {
    let src = format!(
        "{POINT}fn main(world: World) -> Int uses !tty {{\n  let mut p = Point {{ x: 1, y: 2 }};\n  p.x = 9;\n  0\n}}\n"
    );
    let c = compile_to_c(&src, "s.null").expect("mut field write compiles");
    assert!(c.contains("->nlf_x = 9"));
}

#[test]
fn struct_missing_field_is_rejected() {
    let src = format!(
        "{POINT}fn main(world: World) -> Int uses !tty {{\n  let p = Point {{ x: 1 }};\n  0\n}}\n"
    );
    let err = compile_to_c(&src, "s.null").expect_err("missing field");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn struct_unknown_field_is_ref_error() {
    let src = format!(
        "{POINT}fn main(world: World) -> Int uses !tty {{\n  let p = Point {{ x: 1, y: 2, z: 3 }};\n  0\n}}\n"
    );
    let err = compile_to_c(&src, "s.null").expect_err("unknown field");
    assert_eq!(format!("{:?}", err.code), "Ref001");
}

#[test]
fn struct_field_type_mismatch_is_typ001() {
    let src = format!(
        "{POINT}fn main(world: World) -> Int uses !tty {{\n  let p = Point {{ x: 1, y: \"two\" }};\n  0\n}}\n"
    );
    let err = compile_to_c(&src, "s.null").expect_err("field type mismatch");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn reading_field_of_non_struct_is_rejected() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let n = 5;\n  let bad = n.x;\n  0\n}\n";
    let err = compile_to_c(src, "s.null").expect_err("only structs have fields");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn struct_field_can_hold_another_struct() {
    // A field of struct type lowers to a nested handle; chained read `p.a.b`
    // and write `p.a.b = v` go through the pointers.
    let src = "type Inner = { v: Int };\ntype Outer = { inner: Inner };\nfn main(world: World) -> Int uses !tty {\n  let mut o = Outer { inner: Inner { v: 1 } };\n  o.inner.v = 42;\n  print(world, str_of_int(o.inner.v));\n  0\n}\n";
    let c = compile_to_c(src, "s.null").expect("nested struct compiles");
    assert!(c.contains("nlstruct0")); // Inner
    assert!(c.contains("nlstruct1")); // Outer
    assert!(c.contains("->nlf_inner")); // chain hop
}

#[test]
fn struct_field_cannot_be_list_yet() {
    // List-typed fields are deferred in v0.4 (resolve_field_ty rejects them).
    let src = "type Bad = { items: List<Int> };\nfn main(world: World) -> Int uses !tty {\n  print(world, \"x\");\n  0\n}\n";
    let err = compile_to_c(src, "s.null").expect_err("list field deferred");
    assert_eq!(format!("{:?}", err.code), "Sch010");
}

#[test]
fn duplicate_type_name_is_sch010() {
    let src = "type Point = { x: Int };\ntype Point = { y: Int };\nfn main(world: World) -> Int uses !tty {\n  print(world, \"x\");\n  0\n}\n";
    let err = compile_to_c(src, "s.null").expect_err("duplicate type name");
    assert_eq!(format!("{:?}", err.code), "Sch010");
}

#[test]
fn struct_value_returns_from_function_by_handle() {
    // A struct returned from a fn is the same handle the callee built.
    let src = "type Box = { v: Int };\nfn make(n: Int) -> Box { Box { v: n } }\nfn main(world: World) -> Int uses !tty {\n  let b = make(5);\n  print(world, str_of_int(b.v));\n  0\n}\n";
    let c = compile_to_c(src, "s.null").expect("struct return compiles");
    assert!(c.contains("nlstruct0 nlu_make("));
}

// ---- List<struct> (SPEC §11, v0.4) ---------------------------------------

#[test]
fn list_of_structs_pushes_and_reads_by_handle() {
    // A struct fits the uniform list slot (it is a pointer): push boxes the
    // handle via intptr_t, index unboxes it back to the struct type, and a
    // field read goes through the handle.
    let src = "type T = { v: Int };\nfn main(world: World) -> Int uses !tty {\n  let mut xs: List<T> = [];\n  push(xs, T { v: 7 });\n  let e = xs[0];\n  print(world, str_of_int(e.v));\n  0\n}\n";
    let c = compile_to_c(src, "ls.null").expect("List<struct> compiles");
    // Boxing path: handle stored via intptr_t, read back cast to the struct.
    assert!(c.contains("(long)(intptr_t)"));
    assert!(c.contains("(nlstruct0)(intptr_t)"));
}

#[test]
fn empty_list_of_structs_takes_annotation() {
    // `List<struct>` is unresolved by the parser; the checker resolves the
    // empty-literal annotation against the struct table.
    let src = "type P = { x: Int, y: Int };\nfn main(world: World) -> Int uses !tty {\n  let mut ps: List<P> = [];\n  push(ps, P { x: 1, y: 2 });\n  print(world, str_of_int(list_len(ps)));\n  0\n}\n";
    let c = compile_to_c(src, "ls.null").expect("empty List<struct> annotation resolves");
    assert!(c.contains("nl_list_new"));
}

#[test]
fn list_of_structs_literal_compiles() {
    let src = "type P = { x: Int };\nfn main(world: World) -> Int uses !tty {\n  let ps = [P { x: 1 }, P { x: 2 }];\n  print(world, str_of_int(ps[1].x));\n  0\n}\n";
    let c = compile_to_c(src, "ls.null").expect("List<struct> literal compiles");
    assert!(c.contains("nl_list_push"));
    assert!(c.contains("(nlstruct0)(intptr_t)"));
}

#[test]
fn list_push_struct_type_must_match_element() {
    // Pushing the wrong struct (or a scalar) into a List<struct> is rejected.
    let src = "type A = { v: Int };\ntype B = { w: Int };\nfn main(world: World) -> Int uses !tty {\n  let mut xs: List<A> = [];\n  push(xs, B { w: 1 });\n  0\n}\n";
    let err = compile_to_c(src, "ls.null").expect_err("wrong struct element");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn list_of_lists_is_still_rejected() {
    // Nested lists remain deferred: List is not a legal list element. Use a
    // parameter type so resolution runs in pass 1 (resolve_ty → Typ003),
    // rather than the empty-literal path (which reports a Typ001 annotation
    // error instead).
    let src = "fn f(xs: List<List<Int>>) -> Int { 0 }\nfn main(world: World) -> Int uses !tty {\n  print(world, \"x\");\n  0\n}\n";
    let err = compile_to_c(src, "ls.null").expect_err("nested lists deferred");
    assert_eq!(format!("{:?}", err.code), "Typ003");
}

// ---- List<T> (SPEC §11, v0.3) --------------------------------------------

#[test]
fn list_literal_and_index_compile() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let xs = [10, 20, 30];\n  print(world, str_of_int(xs[1]));\n  0\n}\n";
    let c = compile_to_c(src, "lists.null").expect("list literal + index compiles");
    assert!(c.contains("nl_list_new"));
    assert!(c.contains("nl_list_push"));
    assert!(c.contains("nl_list_get"));
}

#[test]
fn list_len_push_set_lower_to_runtime() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let mut xs = [1, 2];\n  push(xs, 3);\n  set(xs, 0, 9);\n  print(world, str_of_int(list_len(xs)));\n  0\n}\n";
    let c = compile_to_c(src, "lists.null").expect("push/set/list_len compile");
    assert!(c.contains("nl_list_push"));
    assert!(c.contains("nl_list_set"));
    assert!(c.contains("nl_list_len"));
}

#[test]
fn string_list_boxes_via_intptr() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let mut xs: List<String> = [];\n  push(xs, \"a\");\n  print(world, xs[0]);\n  0\n}\n";
    let c = compile_to_c(src, "lists.null").expect("String list compiles");
    // String elements round-trip through the uniform slot via intptr_t.
    assert!(c.contains("(long)(intptr_t)"));
    assert!(c.contains("(const char*)(intptr_t)"));
}

#[test]
fn push_requires_mut() {
    // push mutates in place, so its target must be a `let mut` list.
    let src = "fn main(world: World) -> Int uses !tty {\n  let xs = [1, 2];\n  push(xs, 3);\n  0\n}\n";
    let err = compile_to_c(src, "lists.null").expect_err("push needs let mut");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn push_value_type_must_match_element() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let mut xs = [1, 2];\n  push(xs, \"x\");\n  0\n}\n";
    let err = compile_to_c(src, "lists.null").expect_err("element type mismatch");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn empty_list_needs_annotation() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let mut xs = [];\n  0\n}\n";
    let err = compile_to_c(src, "lists.null").expect_err("empty list needs annotation");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn heterogeneous_list_is_rejected() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let xs = [1, \"two\"];\n  0\n}\n";
    let err = compile_to_c(src, "lists.null").expect_err("mixed element types");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn indexing_a_non_list_is_rejected() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let n = 5;\n  let bad = n[0];\n  0\n}\n";
    let err = compile_to_c(src, "lists.null").expect_err("cannot index a non-list");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn list_index_must_be_int() {
    let src = "fn main(world: World) -> Int uses !tty {\n  let xs = [1, 2];\n  let bad = xs[\"k\"];\n  0\n}\n";
    let err = compile_to_c(src, "lists.null").expect_err("index must be Int");
    assert_eq!(format!("{:?}", err.code), "Typ001");
}

#[test]
fn if_in_statement_position_needs_no_semicolon() {
    // A bare `if` not in tail position is a statement; another statement may
    // follow directly. This used to be PAR010 ("expected `}` ... found `if`").
    let src = r#"
fn main(world: World) -> Int uses !tty {
  let mut n = 0;
  if n == 0 { n = n + 1; } else { }
  if n == 1 { print(world, "one"); } else { }
  0
}
"#;
    compile_to_c(src, "ifstmt.null").expect("bare if-statements should compile");
}

#[test]
fn if_in_tail_position_is_still_the_block_value() {
    // The same `if`, when it *is* the last thing in the block, remains the
    // trailing value — so it must type-match the function's return type.
    let src = r#"
fn pick(n: Int) -> Int {
  if n == 0 { 10 } else { 20 }
}
fn main(world: World) -> Int uses !tty {
  print(world, "x");
  pick(0)
}
"#;
    let c = compile_to_c(src, "iftail.null").expect("tail if should compile");
    assert!(c.contains("return"));
}
