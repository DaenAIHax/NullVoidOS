//! Front-half pipeline tests (parse + check + C emission). The cc/ELF/run
//! back half (SPEC §13) is exercised by `nullang run examples/hello.null`.
use nullang::compile_to_c;

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
    assert!(c.contains("int main(void)"));
    assert!(c.contains("nullang_print(greeting())"));
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
    assert!(c.contains("(area - 42)"));
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
