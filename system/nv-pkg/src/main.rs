//! nv-pkg — NullVoidOS package manager CLI.
//!
//! Usage: nv-pkg <COMMAND> [ARGS…]
//!
//! Commands:
//!   install <file.nvpkg>        install into the store, print store path
//!   resolve <name>-<version>    print store path, exit 1 if missing
//!   list [--json]               list installed packages
//!   remove <name>-<version>     remove from store (warns loudly, Phase 1)
//!   verify <name>-<version>     re-hash store contents and confirm match
//!
//! Override store root via NV_STORE_ROOT env var (default /var/lib/nv-store).

use std::process;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "nv-pkg",
    version,
    about = "NullVoidOS package manager — local store only, no network"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install a .nvpkg tarball into the store.
    Install {
        /// Path to the .nvpkg file.
        file: String,
        /// Overwrite if a different tarball with the same name-version exists.
        #[arg(long)]
        force: bool,
    },

    /// Print the store path for an installed package (exit 1 if not found).
    Resolve {
        /// Package identifier, e.g. "neovim-mini-0.1.0".
        name_version: String,
    },

    /// List installed packages.
    List {
        /// Emit JSON array.
        #[arg(long)]
        json: bool,
    },

    /// Remove a package from the store.
    ///
    /// Phase 1 warning: no reference tracking. Removing a package that is
    /// in the active system manifest will break your system.
    Remove {
        /// Package identifier, e.g. "neovim-mini-0.1.0".
        name_version: String,
    },

    /// Re-hash the store entry and confirm it matches the recorded hash.
    Verify {
        /// Package identifier, e.g. "neovim-mini-0.1.0".
        name_version: String,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {:#}", e);
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::Install { file, force } => {
            let path = std::path::Path::new(&file);
            let store_path = nv_pkg::install(path, force)?;
            println!("{}", store_path.display());
        }

        Cmd::Resolve { name_version } => {
            let (name, version) = split_name_version(&name_version)?;
            match nv_pkg::resolve(name, version)? {
                Some(p) => println!("{}", p.display()),
                None => {
                    eprintln!("not found: {}", name_version);
                    process::exit(1);
                }
            }
        }

        Cmd::List { json } => {
            let pkgs = nv_pkg::list_installed()?;
            if json {
                let arr: Vec<_> = pkgs
                    .iter()
                    .map(|p| {
                        json!({
                            "name": p.name,
                            "version": p.version,
                            "storeHash": p.store_hash,
                            "storePath": p.store_path,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&arr)?);
            } else if pkgs.is_empty() {
                println!("(no packages installed)");
            } else {
                for p in &pkgs {
                    println!("{}-{}  {}", p.name, p.version, p.store_path.display());
                }
            }
        }

        Cmd::Remove { name_version } => {
            let (name, version) = split_name_version(&name_version)?;
            // LOUD WARNING: Phase 1 has no reference tracking.
            eprintln!(
                "WARNING: nv-pkg remove does NOT check whether {}-{} is \
                 referenced by the active system generation. \
                 Removing it while it is active WILL break your system. \
                 Use `nv-rebuild switch` to apply a manifest that \
                 excludes this package before removing it.",
                name, version
            );
            let removed = nv_pkg::remove(name, version)?;
            eprintln!("removed: {}", removed.display());
        }

        Cmd::Verify { name_version } => {
            let (name, version) = split_name_version(&name_version)?;
            match nv_pkg::verify(name, version)? {
                true => {
                    let path = nv_pkg::resolve(name, version)?.unwrap();
                    println!("ok: {}", path.display());
                }
                false => {
                    eprintln!("TAMPERED: hash mismatch for {}-{}", name, version);
                    process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Split a `name-version` string at the last `-` that is followed by a
/// semver-like token.
///
/// Examples:
///   "neovim-mini-0.1.0"  → ("neovim-mini", "0.1.0")
///   "bash-5.3.9"         → ("bash", "5.3.9")
fn split_name_version(s: &str) -> Result<(&str, &str)> {
    // Walk backwards over '-' separated tokens until we find one that
    // looks like MAJOR.MINOR.PATCH.
    let bytes = s.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        // Find the previous '-'
        let dash = match bytes[..i].iter().rposition(|&b| b == b'-') {
            Some(pos) => pos,
            None => break,
        };
        let candidate = &s[dash + 1..i];
        if looks_like_semver(candidate) {
            if dash == 0 {
                bail!("cannot parse {:?}: name part is empty", s);
            }
            return Ok((&s[..dash], &s[dash + 1..]));
        }
        i = dash;
    }
    bail!("cannot parse {:?} as <name>-<version>", s)
}

fn looks_like_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.parse::<u64>().is_ok())
}
