//! `nullang` CLI — drives the v0.1 closed loop (SPEC §13):
//! `source → C → cc → ELF → run`, and `package` to emit an `.nvpkg` that
//! enters the OS content-addressed store via `nv-pkg`. Diagnostics are
//! NDJSON on stderr.
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use clap::{Parser as ClapParser, Subcommand};

use nullang::diagnostics::{self, Diag, DiagCode};
use nullang::package::Manifest;
use nullang::{compile_to_c, package, parse_only};

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
    /// Build, then emit an `.nvpkg` package (manifest + ELF + recipe). With
    /// `--install`, also install it into the store via `nv-pkg`.
    Package {
        file: PathBuf,
        /// Package name ([a-z][a-z0-9-]*).
        #[arg(long)]
        name: String,
        /// Semver version.
        #[arg(long, default_value = "0.1.0")]
        version: String,
        /// `authoredBy` — the agent/tool that produced this.
        #[arg(long, default_value = "nullang-0.1.0")]
        author: String,
        /// One-line human description.
        #[arg(long, default_value = "built by nullang")]
        description: String,
        /// Also install the package into the store via `nv-pkg install`.
        #[arg(long)]
        install: bool,
    },
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
            let (_, bin) = compile(file);
            println!("{}", bin.display());
        }
        Command_::Run { file } => {
            let (_, bin) = compile(file);
            let status = Command::new(&bin).status().unwrap_or_else(|e| {
                eprintln!("nullang: cannot execute {}: {}", bin.display(), e);
                process::exit(3);
            });
            process::exit(status.code().unwrap_or(1));
        }
        Command_::Package {
            file,
            name,
            version,
            author,
            description,
            install,
        } => package_cmd(file, name, version, author, description, *install),
    }
}

/// Compile source → C → ELF. Returns (C path, binary path). On any
/// diagnostic, emits NDJSON and exits.
fn compile(file: &Path) -> (PathBuf, PathBuf) {
    let (src, name) = read(file);
    let c = match compile_to_c(&src, &name) {
        Ok(c) => c,
        Err(d) => fail(&d),
    };

    let outdir = build_dir(file);
    if let Err(e) = std::fs::create_dir_all(&outdir) {
        fail(&cgn(&name, format!("cannot create build dir: {}", e)));
    }
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into());
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
        Ok(s) if s.success() => (cpath, bin),
        Ok(s) => fail(&cgn(&name, format!("cc exited with status {}", s))),
        Err(e) => fail(&cgn(&name, format!("cannot run C compiler: {}", e))),
    }
}

fn package_cmd(
    file: &Path,
    name: &str,
    version: &str,
    author: &str,
    description: &str,
    install: bool,
) {
    let (src, fname) = read(file);
    let (cpath, bin) = compile(file);

    // Capabilities are derived from `main`'s `uses` clause (SPEC §5).
    let ast = match parse_only(&src, &fname) {
        Ok(a) => a,
        Err(d) => fail(&d),
    };
    let caps = package::capabilities_of_main(&ast);

    let created = now_rfc3339();
    let build_steps = vec![
        format!("nullang {}", env!("CARGO_PKG_VERSION")),
        format!("source sha256: {}", sha256_file(file)),
        format!("emitted-c sha256: {}", sha256_file(&cpath)),
        format!("cc: {}", cc()),
    ];
    let manifest = Manifest::new(name, version, description, author, &created, caps, build_steps);

    // Stage the package tree under the build dir, then tar it.
    let stage = build_dir(file).join(format!("{}-{}.pkg", name, version));
    let _ = std::fs::remove_dir_all(&stage);
    let bindir = stage.join("payload").join("bin");
    if let Err(e) = std::fs::create_dir_all(&bindir) {
        fail(&cgn(&fname, format!("cannot stage package: {}", e)));
    }
    write_or_die(&fname, &stage.join("manifest.json"), manifest.to_json().as_bytes());
    if let Err(e) = std::fs::copy(&bin, bindir.join(name)) {
        fail(&cgn(&fname, format!("cannot copy binary into payload: {}", e)));
    }
    write_or_die(&fname, &stage.join("recipe.null"), src.as_bytes());

    let nvpkg = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}-{}.nvpkg", name, version));

    let tar = Command::new("tar")
        .arg("czf")
        .arg(&nvpkg)
        .arg("-C")
        .arg(&stage)
        .arg("manifest.json")
        .arg("payload")
        .arg("recipe.null")
        .status();
    match tar {
        Ok(s) if s.success() => {}
        Ok(s) => fail(&cgn(&fname, format!("tar exited with status {}", s))),
        Err(e) => fail(&cgn(&fname, format!("cannot run tar: {}", e))),
    }

    if install {
        let out = Command::new("nv-pkg")
            .arg("install")
            .arg(&nvpkg)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let store = String::from_utf8_lossy(&o.stdout);
                eprintln!("installed: {}", store.trim());
            }
            Ok(o) => fail(&cgn(
                &fname,
                format!(
                    "nv-pkg install failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ),
            )),
            Err(e) => fail(&cgn(
                &fname,
                format!("cannot run nv-pkg (is it on PATH?): {}", e),
            )),
        }
    }

    println!("{}", nvpkg.display());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_dir(file: &Path) -> PathBuf {
    file.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(".nullang-build")
}

/// The C compiler to use: honour $CC, else `cc`.
fn cc() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
}

/// SHA-256 of a file via `sha256sum`, or "unknown" if unavailable.
fn sha256_file(path: &Path) -> String {
    match Command::new("sha256sum").arg(path).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string(),
        _ => "unknown".to_string(),
    }
}

/// Current UTC time as RFC3339 via `date`, or the epoch if unavailable.
fn now_rfc3339() -> String {
    match Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "1970-01-01T00:00:00Z".to_string(),
    }
}

fn write_or_die(fname: &str, path: &Path, bytes: &[u8]) {
    if let Err(e) = std::fs::write(path, bytes) {
        fail(&cgn(fname, format!("cannot write {}: {}", path.display(), e)));
    }
}

fn cgn(file: &str, msg: String) -> Diag {
    Diag::error(
        DiagCode::Cgn001,
        msg,
        "successful packaging",
        "packaging failure",
        file,
        0,
        0,
    )
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
