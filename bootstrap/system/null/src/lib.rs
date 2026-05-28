/// Public API surface for the `null` language toolchain.
/// The `[[bin]]` target in Cargo.toml re-uses this crate; integration
/// tests reference `null` (the lib) directly.
pub mod ast;
pub mod diagnostics;
pub mod fmt;
pub mod lexer;
pub mod parser;
pub mod types;

use diagnostics::Diag;
use lexer::Lexer;
use parser::Parser;
use types::{Env, Evaluator};

// ---------------------------------------------------------------------------
// Core pipeline helpers (used by both main.rs and integration tests)
// ---------------------------------------------------------------------------

pub fn run_parse(src: &str, file: &str) -> Result<ast::Expr, Diag> {
    let mut lex = Lexer::new(src);
    let tokens = lex.tokenize().map_err(|e| {
        let (line, col) = lexer::line_col(src, e.offset);
        Diag {
            level: diagnostics::DiagLevel::Error,
            code: diagnostics::DiagCode::Par001,
            file: file.to_string(),
            line,
            col,
            message: e.message,
            fix: None,
        }
    })?;
    let mut p = Parser::new(tokens, src, file);
    p.parse_file()
}

pub fn run_check(src: &str, file: &str, env: &Env) -> Result<(), Diag> {
    let ast = run_parse(src, file)?;
    let evaluator = Evaluator::new(src, file, env);
    evaluator.eval_manifest(&ast).map(|_| ())
}

pub fn run_eval(src: &str, file: &str, env: &Env) -> Result<types::SystemManifest, Diag> {
    let ast = run_parse(src, file)?;
    let evaluator = Evaluator::new(src, file, env);
    evaluator.eval_manifest(&ast)
}
