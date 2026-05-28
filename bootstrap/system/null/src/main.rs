use std::path::PathBuf;
use std::process;
use std::collections::HashMap;

use clap::{Parser as ClapParser, Subcommand};

// Re-use everything from the library crate.
use null::diagnostics::{self, emit, Diag};
use null::fmt;
use null::types::Env;
use null::{run_check, run_eval, run_parse};

/// `null` — NullVoidOS system description language CLI (Phase 1 MVP)
#[derive(ClapParser, Debug)]
#[command(name = "null", version, about, long_about = None)]
struct Cli {
    /// Emit machine-readable JSON diagnostics on stderr.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse and type-check a .null file. Exit 0 if OK.
    Check {
        file: PathBuf,
    },
    /// Type-check and evaluate a .null file, printing SystemManifest JSON.
    Eval {
        file: PathBuf,
    },
    /// Format a .null file in-place (idempotent canonical style).
    Fmt {
        file: PathBuf,
    },
    /// Emit the AST as JSON (for tooling).
    Parse {
        #[arg(long = "json")]
        json_ast: bool,
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let json_diag = cli.json;

    match &cli.command {
        Command::Check { file } => {
            let (src, fname) = read_file(file, json_diag);
            let env = build_env(json_diag);
            match run_check(&src, &fname, &env) {
                Ok(()) => {
                    if !json_diag {
                        eprintln!("{}: OK", fname);
                    }
                }
                Err(diag) => {
                    emit(&diag, json_diag);
                    process::exit(1);
                }
            }
        }
        Command::Eval { file } => {
            let (src, fname) = read_file(file, json_diag);
            let env = build_env(json_diag);
            match run_eval(&src, &fname, &env) {
                Ok(manifest) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&manifest)
                            .expect("manifest serialization never fails")
                    );
                }
                Err(diag) => {
                    emit(&diag, json_diag);
                    process::exit(1);
                }
            }
        }
        Command::Fmt { file } => {
            let (src, fname) = read_file(file, json_diag);
            match run_parse(&src, &fname) {
                Ok(expr) => {
                    let formatted = fmt::format_expr(&expr);
                    std::fs::write(file, &formatted).unwrap_or_else(|e| {
                        eprintln!("error writing {}: {}", fname, e);
                        process::exit(1);
                    });
                }
                Err(diag) => {
                    emit(&diag, json_diag);
                    process::exit(1);
                }
            }
        }
        Command::Parse { json_ast, file } => {
            if !json_ast {
                eprintln!("usage: null parse --json <file.null>");
                process::exit(2);
            }
            let (src, fname) = read_file(file, json_diag);
            match run_parse(&src, &fname) {
                Ok(ast) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&ast)
                            .expect("AST serialization never fails")
                    );
                }
                Err(diag) => {
                    emit(&diag, json_diag);
                    process::exit(1);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers private to the binary
// ---------------------------------------------------------------------------

fn read_file(path: &PathBuf, json_diag: bool) -> (String, String) {
    let fname = path.display().to_string();
    match std::fs::read_to_string(path) {
        Ok(s) => (s, fname),
        Err(e) => {
            let diag = Diag {
                level: diagnostics::DiagLevel::Error,
                code: diagnostics::DiagCode::Par001,
                file: fname.clone(),
                line: 0,
                col: 0,
                message: format!("cannot read file: {}", e),
                fix: None,
            };
            emit(&diag, json_diag);
            process::exit(1);
        }
    }
}

/// Build the ambient `pkgs` env by calling `nv-pkg list --json`.
/// If `nv-pkg` is not on PATH, emits a warning and returns empty pkgs.
fn build_env(json_diag: bool) -> Env {
    match try_nv_pkg_list() {
        Ok(pkgs) => Env {
            pkgs,
            pkgs_available: true,
        },
        Err(reason) => {
            let warn = Diag {
                level: diagnostics::DiagLevel::Warning,
                code: diagnostics::DiagCode::Typ001,
                file: String::new(),
                line: 0,
                col: 0,
                message: format!(
                    "nv-pkg not available ({}); `pkgs.*` references will fail if used",
                    reason
                ),
                fix: Some("install nv-pkg and ensure it is on PATH".to_string()),
            };
            emit(&warn, json_diag);
            Env {
                pkgs: HashMap::new(),
                pkgs_available: false,
            }
        }
    }
}

/// Attempt to call `nv-pkg list --json` and parse the output.
fn try_nv_pkg_list() -> Result<HashMap<String, String>, String> {
    let output = std::process::Command::new("nv-pkg")
        .args(["list", "--json"])
        .output()
        .map_err(|e| format!("could not run nv-pkg: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "nv-pkg list --json exited with status {}",
            output.status
        ));
    }

    let text = String::from_utf8(output.stdout)
        .map_err(|e| format!("nv-pkg output is not UTF-8: {}", e))?;

    parse_nv_pkg_list_json(&text)
}

/// Parse `[{"name":"bash","version":"5.3.9"}, ...]` → name → "name-version".
fn parse_nv_pkg_list_json(text: &str) -> Result<HashMap<String, String>, String> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(text)
        .map_err(|e| format!("failed to parse nv-pkg JSON output: {}", e))?;

    let mut map = HashMap::new();
    for obj in arr {
        let name = obj["name"]
            .as_str()
            .ok_or_else(|| "nv-pkg entry missing 'name'".to_string())?
            .to_string();
        let version = obj["version"]
            .as_str()
            .ok_or_else(|| "nv-pkg entry missing 'version'".to_string())?
            .to_string();
        let versioned = format!("{}-{}", name, version);
        map.insert(name, versioned);
    }
    Ok(map)
}
