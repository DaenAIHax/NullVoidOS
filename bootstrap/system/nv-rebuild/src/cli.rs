use crate::generation;
use crate::manifest::{Capability, SystemManifest};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// NullVoidOS activation engine
#[derive(Parser, Debug)]
#[command(name = "nv-rebuild", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Eval system.null and validate all packages exist. No mutation.
    Check,
    /// Build the next generation directory but do not activate it.
    Build,
    /// Build and atomically activate the new generation.
    Switch,
    /// Atomically revert to the previous generation.
    Rollback,
    /// List all generations, marking the active one.
    Generations,
    /// Launch a declared service from the active generation, confined to
    /// exactly the capabilities its descriptor grants (Traccia A). The
    /// process inherits this command's exit code.
    Run {
        /// Service name, as declared in system.null `services`.
        service: String,
    },
}

/// Runtime configuration, resolved from environment variables.
pub struct Config {
    /// Root of the generation store (default: /var/lib/nv-system)
    pub system_root: PathBuf,
    /// Path to the system manifest (default: /etc/nullvoid/system.null)
    pub config_path: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            system_root: std::env::var("NV_SYSTEM_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/var/lib/nv-system")),
            config_path: std::env::var("NV_CONFIG")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/etc/nullvoid/system.null")),
        }
    }
}

/// Run `null eval <file>` and return the parsed SystemManifest.
pub fn eval_manifest(config_path: &std::path::Path) -> Result<SystemManifest> {
    let output = std::process::Command::new("null")
        .arg("eval")
        .arg(config_path)
        .output()
        .with_context(|| format!("failed to spawn `null eval {}`", config_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "`null eval` failed (exit {}):\n{}",
            output.status,
            stderr.trim()
        );
    }

    let manifest: SystemManifest = serde_json::from_slice(&output.stdout)
        .with_context(|| "could not parse JSON output of `null eval`")?;
    Ok(manifest)
}

/// Run `nv-pkg resolve <name>-<version>` and return the store path.
pub fn resolve_package(pkg: &str) -> Result<PathBuf> {
    let output = std::process::Command::new("nv-pkg")
        .arg("resolve")
        .arg(pkg)
        .output()
        .with_context(|| format!("failed to spawn `nv-pkg resolve {pkg}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "package `{pkg}` not found in store (nv-pkg resolve exited {}):\n{}",
            output.status,
            stderr.trim()
        );
    }

    let path = String::from_utf8(output.stdout)
        .with_context(|| format!("non-UTF-8 output from `nv-pkg resolve {pkg}`"))?;
    Ok(PathBuf::from(path.trim()))
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// `nv-rebuild check` — validate manifest, check all packages exist.
pub fn cmd_check(cfg: &Config) -> Result<()> {
    let manifest = eval_manifest(&cfg.config_path)?;
    eprintln!("manifest ok: hostname={}", manifest.hostname);

    let mut all_ok = true;
    for pkg in &manifest.packages {
        match resolve_package(pkg) {
            Ok(path) => eprintln!("  [ok] {pkg} -> {}", path.display()),
            Err(e) => {
                eprintln!("  [MISSING] {pkg}: {e}");
                all_ok = false;
            }
        }
    }

    if !all_ok {
        bail!("one or more packages are missing from the store");
    }
    eprintln!("check passed.");
    Ok(())
}

/// `nv-rebuild build` — prepare the next generation directory, do not activate.
pub fn cmd_build(cfg: &Config) -> Result<()> {
    let manifest = eval_manifest(&cfg.config_path)?;
    let store_paths = resolve_all_packages(&manifest)?;
    let n = generation::next_generation_number(&cfg.system_root)?;
    let gen_dir = cfg.system_root.join(format!("generation-{n}"));
    generation::build_generation(&gen_dir, &manifest, &store_paths)?;
    eprintln!("built: {} (not yet activated)", gen_dir.display());
    Ok(())
}

/// `nv-rebuild switch` — build and atomically activate new generation.
pub fn cmd_switch(cfg: &Config) -> Result<()> {
    std::fs::create_dir_all(&cfg.system_root)
        .with_context(|| format!("cannot create system root {}", cfg.system_root.display()))?;

    let manifest = eval_manifest(&cfg.config_path)?;
    let store_paths = resolve_all_packages(&manifest)?;
    let n = generation::next_generation_number(&cfg.system_root)?;
    let gen_dir = cfg.system_root.join(format!("generation-{n}"));

    eprintln!("building generation {n}...");
    generation::build_generation(&gen_dir, &manifest, &store_paths)?;

    let current_link = cfg.system_root.join("current");
    crate::swap::atomic_symlink(&gen_dir, &current_link, &cfg.system_root)?;

    eprintln!(
        "activated: {} -> generation-{n}",
        current_link.display()
    );
    Ok(())
}

/// `nv-rebuild rollback` — revert to generation N-1.
pub fn cmd_rollback(cfg: &Config) -> Result<()> {
    let current_link = cfg.system_root.join("current");
    let target = std::fs::read_link(&current_link)
        .with_context(|| format!("no current symlink at {}", current_link.display()))?;

    // target is e.g. "generation-3" or an absolute path ending in "generation-3"
    let gen_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("unexpected current symlink target: {}", target.display()))?;

    let n: u64 = gen_name
        .strip_prefix("generation-")
        .and_then(|s| s.parse().ok())
        .with_context(|| format!("cannot parse generation number from `{gen_name}`"))?;

    if n == 0 {
        bail!("already at generation 0, cannot roll back further");
    }

    let prev = n - 1;
    // Scan for the previous generation — it may not be n-1 if some were pruned,
    // but §3.4 spec says "generation-(N-1)"; we honour that exactly.
    let prev_dir = cfg.system_root.join(format!("generation-{prev}"));
    if !prev_dir.exists() {
        bail!(
            "previous generation {} does not exist — cannot roll back",
            prev_dir.display()
        );
    }

    crate::swap::atomic_symlink(&prev_dir, &current_link, &cfg.system_root)?;
    eprintln!(
        "rolled back: {} -> generation-{prev}",
        current_link.display()
    );
    Ok(())
}

/// `nv-rebuild generations` — list all generations, mark active.
pub fn cmd_generations(cfg: &Config) -> Result<()> {
    let mut gens = generation::list_generations(&cfg.system_root)?;
    gens.sort_unstable();

    let current_target = cfg
        .system_root
        .join("current")
        .read_link()
        .ok()
        .and_then(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(str::to_owned)
        });

    if gens.is_empty() {
        println!("no generations found in {}", cfg.system_root.display());
        return Ok(());
    }

    for n in &gens {
        let label = format!("generation-{n}");
        let active = current_target.as_deref() == Some(&label);
        if active {
            println!("* {label}  (current)");
        } else {
            println!("  {label}");
        }
    }
    Ok(())
}

/// `nv-rebuild run <service>` — launch a declared service under its declared
/// capabilities. This is the runtime enforcement seam (Traccia A): the
/// capability set the service was *granted* (its `requires`, persisted into
/// the active generation's descriptor) becomes the set it can *exercise*.
///
/// Slice scope: `!net` is enforced via network-namespace isolation. A service
/// without a `net` capability is launched in a fresh, empty network namespace
/// (`unshare -n`) — loopback only, down — so it cannot reach the network. A
/// service with `!net` stays in the host netns. Other capabilities are
/// recorded in the audit line but not yet enforced (next increments).
pub fn cmd_run(cfg: &Config, service: &str) -> Result<()> {
    let desc_path = cfg
        .system_root
        .join("current")
        .join("etc/services")
        .join(service);
    let content = std::fs::read_to_string(&desc_path).with_context(|| {
        format!(
            "no descriptor for service `{service}` in the active generation ({})",
            desc_path.display()
        )
    })?;

    let mut exec: Option<String> = None;
    let mut requires: Vec<Capability> = Vec::new();
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("exec=") {
            exec = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("requires=") {
            requires = v.split_whitespace().map(Capability::parse_token).collect();
        }
    }
    let exec = exec.with_context(|| format!("service `{service}` descriptor has no exec="))?;
    let net_granted = requires.iter().any(Capability::grants_net);

    // Audit line: what we are about to enforce, before we enforce it.
    let cap_tokens: Vec<String> = requires.iter().map(Capability::token).collect();
    eprintln!("nv-rebuild run: service `{service}`");
    eprintln!("  exec:     {exec}");
    eprintln!("  requires: [{}]", cap_tokens.join(" "));

    let mut cmd = if net_granted {
        eprintln!("  net:      GRANTED — host network namespace");
        std::process::Command::new(&exec)
    } else {
        eprintln!("  net:      DENIED — isolated network namespace (unshare -n)");
        let mut c = std::process::Command::new("unshare");
        c.arg("-n").arg(&exec);
        c
    };

    let status = cmd
        .status()
        .with_context(|| format!("failed to launch service `{service}` ({exec})"))?;
    let code = status.code().unwrap_or(255);
    eprintln!("  exit:     {code}");
    std::process::exit(code);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Resolve every package in the manifest; return ordered vec of (pkg, store_path).
pub fn resolve_all_packages(
    manifest: &SystemManifest,
) -> Result<Vec<(String, PathBuf)>> {
    let mut paths = Vec::new();
    for pkg in &manifest.packages {
        let path = resolve_package(pkg)
            .with_context(|| format!("switch aborted — package `{pkg}` missing"))?;
        paths.push((pkg.clone(), path));
    }
    Ok(paths)
}
