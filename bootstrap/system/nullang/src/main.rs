//! `nullang` CLI — drives the v0.1 closed loop (SPEC §13):
//! `source → C → cc → ELF → run`. Diagnostics are NDJSON on stderr.
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use clap::{Parser as ClapParser, Subcommand};

use nullang::diagnostics::{self, Diag, DiagCode};
use nullang::{compile_to_c, parse_only};

#[derive(ClapParser, Debug)]
#[command(name = "nullang", version, about = "Nullang construction language (v0.1)")]
struct Cli {
    #[command(subcommand)]
    command: Command_,
}

#[derive(Subcommand, Debug)]
enum Command_ {
    /// Parse + effect-check a .null file. Exit 0 if OK.
    Check { file: PathBuf },
    /// Print the generated C to stdout (no compilation).
    EmitC { file: PathBuf },
    /// Compile a .null file to a native ELF via C. Prints the binary path.
    Build { file: PathBuf },
    /// Build (if needed) then execute, propagating the program's exit code.
    Run { file: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Command_::Check { file } => {
            let (src, name) = read(file);
            match parse_only(&src, &name).and_then(|_| compile_to_c(&src, &name)) {
                Ok(_) => {}
                Err(d) => fail(&d),
            }
        }
        Command_::EmitC { file } => {
            let (src, name) = read(file);
            match compile_to_c(&src, &name) {
                Ok(c) => print!("{}", c),
                Err(d) => fail(&d),
            }
        }
        Command_::Build { file } => {
            let bin = build(file);
            println!("{}", bin.display());
        }
        Command_::Run { file } => {
            let bin = build(file);
            let status = Command::new(&bin).status().unwrap_or_else(|e| {
                eprintln!("nullang: cannot execute {}: {}", bin.display(), e);
                process::exit(3);
            });
            process::exit(status.code().unwrap_or(1));
        }
    }
}

/// Full front-to-back compile: source → C → cc → ELF. Returns the binary
/// path or emits a diagnostic and exits.
fn build(file: &Path) -> PathBuf {
    let (src, name) = read(file);
    let c = match compile_to_c(&src, &name) {
        Ok(c) => c,
        Err(d) => fail(&d),
    };

    let parent = file.parent().filter(|p| !p.as_os_str().is_empty());
    let outdir = parent.unwrap_or_else(|| Path::new(".")).join(".nullang-build");
    if let Err(e) = std::fs::create_dir_all(&outdir) {
        fail(&cgn(&name, format!("cannot create build dir: {}", e)));
    }
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    let cpath = outdir.join(format!("{}.c", stem));
    let bin = outdir.join(&stem);

    if let Err(e) = std::fs::write(&cpath, &c) {
        fail(&cgn(&name, format!("cannot write C output: {}", e)));
    }

    let status = Command::new(cc())
        .arg("-O2")
        .arg("-o")
        .arg(&bin)
        .arg(&cpath)
        .status();

    match status {
        Ok(s) if s.success() => bin,
        Ok(s) => fail(&cgn(&name, format!("cc exited with status {}", s))),
        Err(e) => fail(&cgn(&name, format!("cannot run C compiler: {}", e))),
    }
}

/// The C compiler to use: honour $CC, else `cc`.
fn cc() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
}

fn cgn(file: &str, msg: String) -> Diag {
    Diag::error(DiagCode::Cgn001, msg, "successful C compilation", "cc failure", file, 0, 0)
}

fn read(file: &Path) -> (String, String) {
    let name = file.display().to_string();
    match std::fs::read_to_string(file) {
        Ok(s) => (s, name),
        Err(e) => fail(&Diag::error(
            DiagCode::Par001,
            format!("cannot read file: {}", e),
            "readable file",
            format!("{}", e),
            &name,
            0,
            0,
        )),
    }
}

/// Emit a diagnostic as NDJSON and exit(1). Never returns.
fn fail(d: &Diag) -> ! {
    diagnostics::emit(d);
    process::exit(1);
}
