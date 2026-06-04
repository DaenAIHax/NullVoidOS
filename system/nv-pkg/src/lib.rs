//! nv-pkg — NullVoidOS package manager (local store only, no network).
//!
//! § CONTRACTS §1.1–§1.4 implementation.

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

// ───────────────────────────────────────── store root ──

/// Return the nv-store root: $NV_STORE_ROOT or /var/lib/nv-store.
pub fn store_root() -> PathBuf {
    std::env::var("NV_STORE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/nv-store"))
}

// ───────────────────────────────────── manifest types ──

/// The manifest.json schema (§1.2). schemaVersion must be 1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authored_by: String,
    pub created_at: String,
    pub deps: Vec<String>,
    pub exposed_bins: Vec<String>,
    pub capabilities: Vec<String>,
    // optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_steps: Option<Vec<String>>,
}

/// Validate the manifest fields per §1.2.
pub fn validate_manifest(m: &Manifest) -> Result<()> {
    if m.schema_version != 1 {
        bail!("schemaVersion must be 1, got {}", m.schema_version);
    }
    // name: ^[a-z][a-z0-9-]*$ max 64
    if m.name.is_empty() || m.name.len() > 64 {
        bail!("name must be 1–64 chars, got {:?}", m.name);
    }
    let valid_name = m.name.starts_with(|c: char| c.is_ascii_lowercase())
        && m.name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid_name {
        bail!(
            "name {:?} must match ^[a-z][a-z0-9-]*$",
            m.name
        );
    }
    // version: semver MAJOR.MINOR.PATCH
    validate_semver(&m.version)
        .with_context(|| format!("invalid version {:?}", m.version))?;

    if m.description.is_empty() {
        bail!("description must not be empty");
    }
    if m.authored_by.is_empty() {
        bail!("authoredBy must not be empty");
    }
    if m.created_at.is_empty() {
        bail!("createdAt must not be empty");
    }
    Ok(())
}

fn validate_semver(v: &str) -> Result<()> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        bail!("must be MAJOR.MINOR.PATCH, got {:?}", v);
    }
    for part in &parts {
        part.parse::<u64>()
            .with_context(|| format!("component {:?} is not a non-negative integer", part))?;
    }
    Ok(())
}

// ─────────────────────────────────────── store hash ──

/// SHA-256 of the raw tarball bytes, first 32 hex chars (§1.3).
pub fn store_hash(tarball_bytes: &[u8]) -> String {
    let digest = Sha256::digest(tarball_bytes);
    format!("{:x}", digest)[..32].to_string()
}

// ──────────────────────────────── installed package ──

/// A record of one installed package (used by list / verify).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackage {
    pub store_path: PathBuf,
    pub store_hash: String,
    pub name: String,
    pub version: String,
}

// ──────────────────────────────────────── install ──

/// Validate and install a `.nvpkg` tarball into the store.
///
/// Returns the store path on success.
/// If the exact tarball is already installed (same hash), returns the
/// existing path (idempotent).
/// If a *different* tarball with the same name-version is already in
/// the store, returns an error unless `force` is true.
pub fn install(tarball_path: &Path, force: bool) -> Result<PathBuf> {
    let tarball_bytes =
        fs::read(tarball_path).with_context(|| format!("reading {:?}", tarball_path))?;

    let hash = store_hash(&tarball_bytes);
    let root = store_root();

    // Parse and validate manifest first (cheap — reuse bytes).
    let manifest = read_manifest_from_bytes(&tarball_bytes)
        .context("extracting manifest.json from tarball")?;
    validate_manifest(&manifest).context("manifest validation failed")?;

    let target_dir_name = format!("{}-{}-{}", hash, manifest.name, manifest.version);
    let target_dir = root.join(&target_dir_name);

    // Idempotency: same hash → same store path → already done.
    if target_dir.exists() {
        return Ok(target_dir);
    }

    // Collision: different tarball with same name-version already installed.
    if !force {
        detect_collision(&root, &manifest.name, &manifest.version, &hash)
            .context("store collision check")?;
    }

    // Full tarball validation (symlink safety, allowed top-level entries).
    validate_tarball(&tarball_bytes).context("tarball structure validation")?;

    // Unpack into a temp dir inside the store root, then rename atomically.
    fs::create_dir_all(&root)
        .with_context(|| format!("creating store root {:?}", root))?;

    let tmp_dir = {
        // Use a temp dir inside the same filesystem for atomic rename.
        let t = root.join(format!(".tmp-{}", hash));
        if t.exists() {
            fs::remove_dir_all(&t)?;
        }
        t
    };
    fs::create_dir_all(&tmp_dir)?;

    unpack_tarball(&tarball_bytes, &tmp_dir)
        .context("unpacking tarball")?;

    // Compute and save the content hash of the unpacked directory before
    // renaming, so `verify` can later confirm integrity without the tarball.
    let content_hash = hash_directory(&tmp_dir)?;
    fs::write(
        tmp_dir.join(".nv-content-hash"),
        content_hash.as_bytes(),
    )
    .context("writing .nv-content-hash")?;

    fs::rename(&tmp_dir, &target_dir)
        .with_context(|| format!("renaming tmp dir to {:?}", target_dir))?;

    Ok(target_dir)
}

/// Detect if a different tarball with the same name-version is already
/// installed (collision per §1.3).
fn detect_collision(root: &Path, name: &str, version: &str, hash: &str) -> Result<()> {
    let suffix = format!("-{}-{}", name, version);
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        if fname.ends_with(&suffix) && !fname.starts_with(hash) {
            bail!(
                "a different tarball of {}-{} is already installed at {:?}. \
                 Use --force to overwrite.",
                name,
                version,
                root.join(fname.as_ref())
            );
        }
    }
    Ok(())
}

// ──────────────────────────────── tarball validation ──

/// Allowed top-level entries inside a .nvpkg tarball.
const ALLOWED_TOPLEVEL: &[&str] = &["manifest.json", "payload", "recipe.null"];

/// Validate tarball structure per §1.1:
/// - only manifest.json, payload/, recipe.null at root
/// - no symlinks pointing outside the package
pub fn validate_tarball(bytes: &[u8]) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);

    for entry in archive.entries()? {
        let entry = entry.context("reading tar entry")?;
        let header = entry.header();
        let path = entry.path().context("reading entry path")?;
        let path = path.to_path_buf();

        // Strip leading ./ if present.
        let path = strip_dot_slash(path);

        // Validate top-level component.
        let top = path
            .components()
            .next()
            .ok_or_else(|| anyhow!("empty path in tarball"))?;
        let top_str = top
            .as_os_str()
            .to_str()
            .ok_or_else(|| anyhow!("non-UTF-8 path in tarball"))?;

        if !ALLOWED_TOPLEVEL.contains(&top_str) {
            bail!(
                "forbidden top-level entry {:?} in tarball (allowed: {:?})",
                top_str,
                ALLOWED_TOPLEVEL
            );
        }

        // Validate symlinks: absolute targets are forbidden; relative
        // targets that escape the package root are forbidden.
        if header.entry_type() == tar::EntryType::Symlink {
            let target = header
                .link_name()
                .context("reading symlink target")?
                .ok_or_else(|| anyhow!("symlink with no target: {:?}", path))?;
            let target = target.to_path_buf();

            if target.is_absolute() {
                bail!(
                    "absolute symlink target {:?} in entry {:?} is forbidden",
                    target,
                    path
                );
            }

            // Check relative escapes: resolve target relative to the
            // symlink's parent directory and ensure it stays within the
            // package root (i.e., the resolved path doesn't start with "..")
            // when normalised from the package root.
            let link_dir = path.parent().unwrap_or(Path::new(""));
            let resolved = normalise_path(&link_dir.join(&target));
            // If the normalised path starts with ".." it escapes the root.
            if resolved
                .components()
                .next()
                .map(|c| c == std::path::Component::ParentDir)
                .unwrap_or(false)
            {
                bail!(
                    "symlink {:?} -> {:?} escapes the package root",
                    path,
                    target
                );
            }
        }
    }

    Ok(())
}

/// Lexically normalise a path (resolve `..` and `.` without hitting
/// the filesystem).
fn normalise_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                // only pop if the last component is a normal segment
                if matches!(out.components().last(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn strip_dot_slash(p: PathBuf) -> PathBuf {
    use std::path::Component;
    let mut comps = p.components();
    if let Some(Component::CurDir) = comps.next() {
        comps.collect()
    } else {
        p
    }
}

// ────────────────────────────────────── unpacking ──

fn unpack_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);
    archive.set_preserve_permissions(true);
    archive.set_unpack_xattrs(false);
    archive
        .unpack(dest)
        .with_context(|| format!("unpacking to {:?}", dest))?;
    Ok(())
}

// ─────────────────────────────── read manifest only ──

fn read_manifest_from_bytes(bytes: &[u8]) -> Result<Manifest> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let path = strip_dot_slash(path);
        if path == Path::new("manifest.json") {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            let m: Manifest = serde_json::from_str(&content)
                .context("parsing manifest.json")?;
            return Ok(m);
        }
    }
    bail!("manifest.json not found in tarball");
}

// ─────────────────────────────────────── resolve ──

/// Find the store path for an installed `name-version`.
///
/// Returns `None` if not found, `Err` on I/O errors.
pub fn resolve(name: &str, version: &str) -> Result<Option<PathBuf>> {
    let root = store_root();
    if !root.exists() {
        return Ok(None);
    }
    let suffix = format!("-{}-{}", name, version);
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if fname_str.ends_with(&suffix) && entry.path().is_dir() {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

// ─────────────────────────────────────────── list ──

/// Return all installed packages.
pub fn list_installed() -> Result<Vec<InstalledPackage>> {
    let root = store_root();
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut pkgs = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        // Directory name format: <32-hex-chars>-<name>-<version>
        // hash is exactly 32 hex chars, followed by a '-'
        if fname_str.len() < 34 {
            continue;
        }
        let (hash_part, rest) = fname_str.split_at(32);
        if !hash_part.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        // rest starts with '-'
        let rest = rest.trim_start_matches('-');
        // rest is now "<name>-<version>" — split on last '-'
        // version is MAJOR.MINOR.PATCH so it contains dots, not hyphens.
        // Split at the last '-' before the version.
        // Actually version may have pre-release (semver) but §1.2 only
        // requires MAJOR.MINOR.PATCH, so we split at the last '-'.
        if let Some(idx) = rest.rfind('-') {
            let (name, version) = rest.split_at(idx);
            let version = &version[1..]; // skip leading '-'
            if validate_semver(version).is_ok() {
                pkgs.push(InstalledPackage {
                    store_path: path,
                    store_hash: hash_part.to_string(),
                    name: name.to_string(),
                    version: version.to_string(),
                });
            }
        }
    }
    // Stable sort by name then version.
    pkgs.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    Ok(pkgs)
}

// ─────────────────────────────────────── remove ──

/// Remove an installed package from the store.
///
/// Phase 1 does not track current-system references; we remove
/// unconditionally but emit a loud warning.
pub fn remove(name: &str, version: &str) -> Result<PathBuf> {
    let path = resolve(name, version)?
        .ok_or_else(|| anyhow!("package {}-{} is not installed", name, version))?;
    fs::remove_dir_all(&path)
        .with_context(|| format!("removing {:?}", path))?;
    Ok(path)
}

// ─────────────────────────────────────── verify ──

/// Re-hash the installed store directory and confirm it matches the
/// content hash recorded at install time (`.nv-content-hash` file).
///
/// Returns `Ok(true)` on match, `Ok(false)` on mismatch.
/// The hash in the *directory name* is the original tarball hash (§1.3);
/// this function verifies post-install content integrity using a separate
/// content-addressable hash written during `install`.
pub fn verify(name: &str, version: &str) -> Result<bool> {
    let store_path = resolve(name, version)?
        .ok_or_else(|| anyhow!("package {}-{} is not installed", name, version))?;

    let hash_file = store_path.join(".nv-content-hash");
    let stored_hash = fs::read_to_string(&hash_file)
        .with_context(|| format!("reading {:?}", hash_file))?;
    let stored_hash = stored_hash.trim();

    let computed = hash_directory(&store_path)?;
    Ok(computed == stored_hash)
}

/// Walk a directory, collect (relative_path, sha256_of_file_bytes) for
/// all regular files in sorted order, then SHA-256 the whole manifest
/// → truncate to 32 hex chars.
pub fn hash_directory(dir: &Path) -> Result<String> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    for entry in WalkDir::new(dir).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(dir)
            .context("stripping prefix")?
            .to_string_lossy()
            .to_string();
        // Skip the content-hash file itself to avoid a circular dependency.
        if rel == ".nv-content-hash" {
            continue;
        }
        let bytes = fs::read(entry.path())
            .with_context(|| format!("reading {:?}", entry.path()))?;
        let file_hash = Sha256::digest(&bytes).to_vec();
        entries.push((rel, file_hash));
    }

    // Build a deterministic byte string: for each entry emit
    // "<path>\0<hex_hash>\n" then SHA-256 the whole thing.
    let mut hasher = Sha256::new();
    for (path, file_hash) in &entries {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        // Encode file_hash bytes as lowercase hex.
        let hex_str: String = file_hash
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        hasher.update(hex_str.as_bytes());
        hasher.update(b"\n");
    }
    let final_digest = hasher.finalize();
    let hex: String = final_digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    Ok(hex[..32].to_string())
}

// ─────────────────────────────── fixture builder ──
// Exposed for use in integration tests.

/// Build a `.nvpkg` tarball in memory from a manifest and a map of
/// payload file paths → contents.
///
/// `payload_files` keys are relative paths like `"bin/hello"`.
pub fn build_nvpkg(
    manifest: &Manifest,
    payload_files: &[(&str, &[u8])],
    include_recipe: bool,
) -> Result<Vec<u8>> {
    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};

    let buf = Vec::new();
    let gz = GzEncoder::new(buf, Compression::default());
    let mut archive = Builder::new(gz);

    // manifest.json
    let manifest_json = serde_json::to_vec_pretty(manifest)?;
    let mut header = Header::new_gnu();
    header.set_size(manifest_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append_data(&mut header, "manifest.json", manifest_json.as_slice())?;

    // payload/ directory entry
    {
        let mut header = Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_cksum();
        archive.append_data(&mut header, "payload/", io::empty())?;
    }

    for (rel_path, content) in payload_files {
        let tar_path = format!("payload/{}", rel_path);
        let mut header = Header::new_gnu();
        header.set_size(content.len() as u64);
        // If it's under bin/, set execute bit.
        if rel_path.starts_with("bin/") {
            header.set_mode(0o755);
        } else {
            header.set_mode(0o644);
        }
        header.set_cksum();
        archive.append_data(&mut header, &tar_path, *content)?;
    }

    if include_recipe {
        let recipe = b"# auto-generated recipe stub\n";
        let mut header = Header::new_gnu();
        header.set_size(recipe.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, "recipe.null", recipe.as_slice())?;
    }

    let gz = archive.into_inner()?;
    let bytes = gz.finish()?;
    Ok(bytes)
}

/// Build a minimal valid manifest for tests.
pub fn test_manifest(name: &str, version: &str) -> Manifest {
    Manifest {
        schema_version: 1,
        name: name.to_string(),
        version: version.to_string(),
        description: "test package".to_string(),
        authored_by: "test-agent".to_string(),
        created_at: "2026-05-28T00:00:00Z".to_string(),
        deps: vec![],
        exposed_bins: vec![],
        capabilities: vec![],
        source_language: None,
        build_steps: None,
    }
}
