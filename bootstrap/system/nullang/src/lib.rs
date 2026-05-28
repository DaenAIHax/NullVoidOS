//! Nullang toolchain — v0.1 construction core.
//!
//! Pipeline: `source → tokens → AST → effect check → C → cc → ELF`.
//! This crate exposes the front half (up to C emission); the binary
//! (`main.rs`) drives `cc` for the back half. See `../SPEC.md` §7, §13.

pub mod ast;
pub mod check;
pub mod codegen;
pub mod diagnostics;
pub mod lexer;
pub mod parser;

use diagnostics::Diag;

/// Parse a source string into a `File` AST (no semantic checks).
pub fn parse_only(src: &str, file: &str) -> Result<ast::File, Diag> {
    let tokens = lexer::Lexer::new(src).tokenize().map_err(|e| {
        let (line, col) = lexer::line_col(src, e.offset);
        diagnostics::Diag::error(
            diagnostics::DiagCode::Par001,
            e.message.clone(),
            "valid token",
            &e.message,
            file,
            line,
            col,
        )
    })?;
    parser::Parser::new(tokens, src, file).parse_file()
}

/// Front half of the pipeline: parse + check, then emit C source.
/// Returns the generated C as a string.
pub fn compile_to_c(src: &str, file: &str) -> Result<String, Diag> {
    let ast = parse_only(src, file)?;
    let checked = check::check_file(&ast, src, file)?;
    Ok(codegen::emit(&ast, &checked))
}
