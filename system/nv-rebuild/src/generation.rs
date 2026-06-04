use crate::manifest::{PackageManifest, SystemManifest};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Scan system_root for directories named `generation-N` and return their numbers.
pub fn list_generations(system_root: &Path) -> Result<Vec<u64>> {
    let mut gens = Vec::new();
    if !system_root.exists() {
        return Ok(gens);
    }
    for entry in std::fs::read_dir(system_root)
        .with_context(|| format!("cannot read {}", system_root.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if let Some(n) = s.strip_prefix("generation-") {
            if let Ok(num) = n.parse::<u64>() {
                gens.push(num);
            }
        }
    }
    Ok(gens)
}

/// Return the next generation number (max existing + 1, or 1 if none).
pub fn next_generation_number(system_root: &Path) -> Result<u64> {
    let existing = list_generations(system_root)?;
    Ok(existing.into_iter().max().unwrap_or(0) + 1)
}

/// Build the generation directory at `gen_dir`.
///
/// Layout:
/// ```text
/// generation-N/
///   manifest.json          -- evaluated SystemManifest
///   bin/<name> -> <store>/payload/bin/<name>   (symlinks)
///   etc/
///     environment          -- KEY=value lines
///     services/<name>      -- one file per service
/// ```
///
/// Conflict resolution: if two packages expose the same binary name, the last
/// package in `packages` wins and a warning is printed to stderr.
pub fn build_generation(
    gen_dir: &Path,
    manifest: &SystemManifest,
    store_paths: &[(String, PathBuf)],
) -> Result<()> {
    // Create directories
    let bin_dir = gen_dir.join("bin");
    let etc_dir = gen_dir.join("etc");
    let services_dir = etc_dir.join("services");
    std::fs::create_dir_all(&bin_dir)
        .with_context(|| format!("cannot create {}", bin_dir.display()))?;
    std::fs::create_dir_all(&services_dir)
        .with_context(|| format!("cannot create {}", services_dir.display()))?;

    // Write manifest.json (the evaluated SystemManifest)
    let manifest_json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(gen_dir.join("manifest.json"), &manifest_json)
        .context("cannot write generation manifest.json")?;

    // Build bin/ symlinks.
    // We track which package "won" each bin name for conflict reporting.
    let mut bin_owner: HashMap<String, String> = HashMap::new();
    for (pkg, store_path) in store_paths {
        let pkg_manifest = read_package_manifest(store_path)?;
        for bin_name in &pkg_manifest.exposed_bins {
            let target = store_path.join("payload").join("bin").join(bin_name);
            let link = bin_dir.join(bin_name);

            if let Some(prev_owner) = bin_owner.get(bin_name) {
                eprintln!(
                    "warning: bin `{bin_name}` exposed by both `{prev_owner}` and `{pkg}` — `{pkg}` wins (last declaration)"
                );
            }
            // Remove previous symlink if it exists (conflict case or re-build)
            if link.exists() || link.symlink_metadata().is_ok() {
                std::fs::remove_file(&link)
                    .with_context(|| format!("cannot remove old symlink {}", link.display()))?;
            }
            std::os::unix::fs::symlink(&target, &link).with_context(|| {
                format!(
                    "cannot create symlink {} -> {}",
                    link.display(),
                    target.display()
                )
            })?;
            bin_owner.insert(bin_name.clone(), pkg.clone());
        }
    }

    // Write etc/environment
    let env_content: String = manifest
        .environment
        .iter()
        .map(|(k, v)| format!("{k}={v}\n"))
        .collect();
    std::fs::write(etc_dir.join("environment"), &env_content)
        .context("cannot write etc/environment")?;

    // Write etc/services/<name>
    for (name, svc) in &manifest.services {
        let restart = match svc.restart {
            crate::manifest::RestartPolicy::Always => "always",
            crate::manifest::RestartPolicy::OnFailure => "on-failure",
            crate::manifest::RestartPolicy::Never => "never",
        };
        // Persist the granted capability set into the descriptor so the
        // supervisor can confine the process at launch (Traccia A). Space-
        // separated compact tokens; an empty `requires=` means "no caps".
        let caps: Vec<String> = svc.requires.iter().map(|c| c.token()).collect();
        let content = format!(
            "exec={}\nrestart={}\nrequires={}\n",
            svc.exec,
            restart,
            caps.join(" ")
        );
        std::fs::write(services_dir.join(name), &content)
            .with_context(|| format!("cannot write services/{name}"))?;
    }

    Ok(())
}

/// Read and parse the package manifest.json from a store path.
fn read_package_manifest(store_path: &Path) -> Result<PackageManifest> {
    let p = store_path.join("manifest.json");
    let data = std::fs::read_to_string(&p)
        .with_context(|| format!("cannot read package manifest at {}", p.display()))?;
    serde_json::from_str(&data)
        .with_context(|| format!("cannot parse package manifest at {}", p.display()))
}
