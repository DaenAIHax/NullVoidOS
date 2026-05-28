//! Integration tests for nv-pkg.
//!
//! All tests use a temporary NV_STORE_ROOT so they never touch
//! /var/lib/nv-store and never require root.
//!
//! Because NV_STORE_ROOT is a process-global env var, tests acquire a
//! mutex before mutating it and hold it until the test finishes.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use nv_pkg::{build_nvpkg, install, list_installed, remove, resolve, store_hash, test_manifest, verify};
use tempfile::TempDir;

// ──────────────────────── global serialisation lock ──

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Set NV_STORE_ROOT to a fresh temp dir while holding the env lock.
/// Returns (lock_guard, TempDir, store_path). Keep all three alive.
fn with_store() -> (MutexGuard<'static, ()>, TempDir, PathBuf) {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("nv-store");
    fs::create_dir_all(&store).unwrap();
    std::env::set_var("NV_STORE_ROOT", &store);
    (guard, dir, store)
}

/// Write bytes to a temp file and return the path.
fn write_tmp_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

// ──────────────────────────────────────── test 1 ──

/// Install a valid .nvpkg → succeeds, store path correct, files present.
#[test]
fn test_install_valid_package() {
    let (_lock, _guard, store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let manifest = test_manifest("hello-world", "1.0.0");
    let payload = &[("bin/hello", b"#!/bin/sh\necho hello\n".as_slice())];
    let tarball = build_nvpkg(&manifest, payload, false).unwrap();
    let hash = store_hash(&tarball);
    let pkg_file = write_tmp_file(tmp.path(), "hello-world-1.0.0.nvpkg", &tarball);

    let result = install(&pkg_file, false);
    assert!(result.is_ok(), "install failed: {:?}", result);

    let store_path = result.unwrap();
    let expected_dir_name = format!("{}-hello-world-1.0.0", &hash[..32]);
    let expected_path = store.join(&expected_dir_name);

    assert_eq!(store_path, expected_path, "wrong store path");
    assert!(store_path.exists(), "store path does not exist");
    assert!(
        store_path.join("manifest.json").exists(),
        "manifest.json missing"
    );
    assert!(
        store_path.join("payload").join("bin").join("hello").exists(),
        "payload file missing"
    );
}

// ──────────────────────────────────────── test 2 ──

/// Install twice → idempotent: same store path, no error.
#[test]
fn test_install_idempotent() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let manifest = test_manifest("idempotent-pkg", "0.2.0");
    let payload = &[("bin/idempotent", b"#!/bin/sh\n".as_slice())];
    let tarball = build_nvpkg(&manifest, payload, false).unwrap();
    let pkg_file = write_tmp_file(tmp.path(), "idempotent-pkg-0.2.0.nvpkg", &tarball);

    let first = install(&pkg_file, false).unwrap();
    let second = install(&pkg_file, false).unwrap();

    assert_eq!(first, second, "second install returned a different path");
    assert!(first.exists(), "store path missing after idempotent install");
}

// ──────────────────────────────────────── test 3 ──

/// Install a tarball containing a malicious symlink → rejected.
#[test]
fn test_install_malicious_symlink_rejected() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let tarball = build_nvpkg_with_escaping_symlink();
    let pkg_file = write_tmp_file(tmp.path(), "malicious.nvpkg", &tarball);

    let result = install(&pkg_file, false);
    assert!(result.is_err(), "expected error for malicious symlink, got Ok");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("symlink") || msg.contains("escapes") || msg.contains("absolute"),
        "error message doesn't mention symlink issue: {}",
        msg
    );
}

/// Build a tarball with a symlink that escapes the package root:
/// payload/evil-link -> ../../../../etc/passwd
fn build_nvpkg_with_escaping_symlink() -> Vec<u8> {
    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, EntryType, Header};

    let buf = Vec::new();
    let gz = GzEncoder::new(buf, Compression::default());
    let mut archive = Builder::new(gz);

    let m = test_manifest("malicious-pkg", "0.0.1");
    let manifest_json = serde_json::to_vec(&m).unwrap();
    let mut hdr = Header::new_gnu();
    hdr.set_size(manifest_json.len() as u64);
    hdr.set_mode(0o644);
    hdr.set_cksum();
    archive
        .append_data(&mut hdr, "manifest.json", manifest_json.as_slice())
        .unwrap();

    {
        let mut hdr = Header::new_gnu();
        hdr.set_entry_type(EntryType::Directory);
        hdr.set_size(0);
        hdr.set_mode(0o755);
        hdr.set_cksum();
        archive
            .append_data(&mut hdr, "payload/", std::io::empty())
            .unwrap();
    }

    {
        let mut hdr = Header::new_gnu();
        hdr.set_entry_type(EntryType::Symlink);
        hdr.set_size(0);
        hdr.set_mode(0o777);
        hdr.set_link_name("../../../../etc/passwd").unwrap();
        hdr.set_cksum();
        archive
            .append_data(&mut hdr, "payload/evil-link", std::io::empty())
            .unwrap();
    }

    archive.into_inner().unwrap().finish().unwrap()
}

// ──────────────────────────────────────── test 4 ──

/// Install with a malformed manifest (missing required fields) → rejected.
#[test]
fn test_install_malformed_manifest_rejected() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let tarball = build_nvpkg_missing_field();
    let pkg_file = write_tmp_file(tmp.path(), "broken.nvpkg", &tarball);

    let result = install(&pkg_file, false);
    assert!(result.is_err(), "expected error for bad manifest, got Ok");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("manifest") || msg.contains("missing") || msg.contains("field")
            || msg.contains("description") || msg.contains("authoredBy"),
        "error doesn't mention manifest problem: {}",
        msg
    );
}

/// Build a tarball whose manifest.json is missing required fields.
fn build_nvpkg_missing_field() -> Vec<u8> {
    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};

    let buf = Vec::new();
    let gz = GzEncoder::new(buf, Compression::default());
    let mut archive = Builder::new(gz);

    // Missing description, authoredBy, createdAt, deps, exposedBins, capabilities.
    let bad_manifest = r#"{"schemaVersion":1,"name":"broken-pkg","version":"0.1.0"}"#;
    let mut hdr = Header::new_gnu();
    hdr.set_size(bad_manifest.len() as u64);
    hdr.set_mode(0o644);
    hdr.set_cksum();
    archive
        .append_data(&mut hdr, "manifest.json", bad_manifest.as_bytes())
        .unwrap();

    {
        let mut hdr = Header::new_gnu();
        hdr.set_entry_type(tar::EntryType::Directory);
        hdr.set_size(0);
        hdr.set_mode(0o755);
        hdr.set_cksum();
        archive
            .append_data(&mut hdr, "payload/", std::io::empty())
            .unwrap();
    }

    archive.into_inner().unwrap().finish().unwrap()
}

// ──────────────────────────────────────── test 5 ──

/// resolve finds installed packages and exits 1 (returns None) on missing.
#[test]
fn test_resolve_installed_and_missing() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let manifest = test_manifest("bash", "5.3.9");
    let tarball = build_nvpkg(&manifest, &[], false).unwrap();
    let pkg_file = write_tmp_file(tmp.path(), "bash-5.3.9.nvpkg", &tarball);
    install(&pkg_file, false).unwrap();

    let found = resolve("bash", "5.3.9").unwrap();
    assert!(found.is_some(), "expected Some, got None");
    assert!(found.unwrap().exists(), "resolved path does not exist");

    let not_found = resolve("bash", "9.9.9").unwrap();
    assert!(not_found.is_none(), "expected None, got {:?}", not_found);
}

// ──────────────────────────────────────── test 6 ──

/// list --json returns parseable JSON array with correct fields.
#[test]
fn test_list_json_parseable() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    for (name, ver) in [("alpha-tool", "1.0.0"), ("beta-tool", "2.3.4")] {
        let manifest = test_manifest(name, ver);
        let tarball = build_nvpkg(&manifest, &[], false).unwrap();
        let fname = format!("{}-{}.nvpkg", name, ver);
        let pkg_file = write_tmp_file(tmp.path(), &fname, &tarball);
        install(&pkg_file, false).unwrap();
    }

    let pkgs = list_installed().unwrap();
    assert_eq!(pkgs.len(), 2, "expected 2 packages, got {}", pkgs.len());

    // Serialize to JSON and parse back.
    let json_str = serde_json::to_string(&pkgs).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.is_array(), "expected JSON array");
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    for item in arr {
        assert!(item["name"].is_string(), "name is not string: {}", item);
        assert!(item["version"].is_string(), "version is not string: {}", item);
        assert!(item["storeHash"].is_string(), "storeHash is not string: {}", item);
    }
}

// ──────────────────────────────────────── test 7 ──

/// verify succeeds on a clean install and fails after a tamper.
#[test]
fn test_verify_clean_and_tampered() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let manifest = test_manifest("verify-me", "0.5.0");
    let payload = &[("bin/verify-me", b"#!/bin/sh\necho hi\n".as_slice())];
    let tarball = build_nvpkg(&manifest, payload, false).unwrap();
    let pkg_file = write_tmp_file(tmp.path(), "verify-me-0.5.0.nvpkg", &tarball);
    let store_path = install(&pkg_file, false).unwrap();

    let ok = verify("verify-me", "0.5.0").unwrap();
    assert!(ok, "expected verify to pass on clean install");

    let bin_path = store_path.join("payload").join("bin").join("verify-me");
    fs::write(&bin_path, b"tampered!\n").unwrap();

    let ok_after = verify("verify-me", "0.5.0").unwrap();
    assert!(!ok_after, "expected verify to fail after tamper");
}

// ──────────────────── bonus: forbidden top-level entry ──

/// Install a tarball with a forbidden top-level entry → rejected.
#[test]
fn test_install_forbidden_toplevel_rejected() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let tarball = build_nvpkg_forbidden_toplevel();
    let pkg_file = write_tmp_file(tmp.path(), "forbidden.nvpkg", &tarball);
    let result = install(&pkg_file, false);
    assert!(result.is_err(), "expected error for forbidden top-level entry");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("forbidden") || msg.contains("top-level"),
        "error doesn't mention forbidden entry: {}",
        msg
    );
}

fn build_nvpkg_forbidden_toplevel() -> Vec<u8> {
    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};

    let buf = Vec::new();
    let gz = GzEncoder::new(buf, Compression::default());
    let mut archive = Builder::new(gz);

    let m = test_manifest("forbidden-pkg", "0.0.1");
    let manifest_json = serde_json::to_vec(&m).unwrap();
    let mut hdr = Header::new_gnu();
    hdr.set_size(manifest_json.len() as u64);
    hdr.set_mode(0o644);
    hdr.set_cksum();
    archive
        .append_data(&mut hdr, "manifest.json", manifest_json.as_slice())
        .unwrap();

    let evil = b"rm -rf /\n";
    let mut hdr = Header::new_gnu();
    hdr.set_size(evil.len() as u64);
    hdr.set_mode(0o644);
    hdr.set_cksum();
    archive
        .append_data(&mut hdr, "secret_backdoor/script.sh", evil.as_slice())
        .unwrap();

    archive.into_inner().unwrap().finish().unwrap()
}

// ────────────────────────────────── remove test ──

/// remove deletes the store path.
#[test]
fn test_remove_package() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let manifest = test_manifest("removable-pkg", "1.0.0");
    let tarball = build_nvpkg(&manifest, &[], false).unwrap();
    let pkg_file = write_tmp_file(tmp.path(), "removable-pkg-1.0.0.nvpkg", &tarball);
    let store_path = install(&pkg_file, false).unwrap();
    assert!(store_path.exists());

    let removed = remove("removable-pkg", "1.0.0").unwrap();
    assert_eq!(removed, store_path);
    assert!(!removed.exists(), "store path still exists after remove");
}

// ─────────────────── hash determinism / collision detection ──

/// Two different tarballs with the same name-version → collision error.
#[test]
fn test_collision_detected() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let m1 = test_manifest("collide-pkg", "1.0.0");
    let t1 = build_nvpkg(&m1, &[("bin/v1", b"v1".as_slice())], false).unwrap();
    install(&write_tmp_file(tmp.path(), "collide-v1.nvpkg", &t1), false).unwrap();

    let m2 = test_manifest("collide-pkg", "1.0.0");
    let t2 = build_nvpkg(&m2, &[("bin/v2", b"v2".as_slice())], false).unwrap();
    assert_ne!(store_hash(&t1), store_hash(&t2), "tarballs must differ");
    let result = install(&write_tmp_file(tmp.path(), "collide-v2.nvpkg", &t2), false);
    assert!(result.is_err(), "expected collision error");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("collision") || msg.contains("different tarball") || msg.contains("already installed"),
        "unexpected error message: {}",
        msg
    );
}

/// --force overrides the collision guard.
#[test]
fn test_collision_force_override() {
    let (_lock, _guard, _store) = with_store();
    let tmp = tempfile::tempdir().unwrap();

    let m1 = test_manifest("force-pkg", "1.0.0");
    let t1 = build_nvpkg(&m1, &[("bin/v1", b"v1".as_slice())], false).unwrap();
    install(&write_tmp_file(tmp.path(), "force-v1.nvpkg", &t1), false).unwrap();

    let m2 = test_manifest("force-pkg", "1.0.0");
    let t2 = build_nvpkg(&m2, &[("bin/v2", b"v2".as_slice())], false).unwrap();
    let result = install(&write_tmp_file(tmp.path(), "force-v2.nvpkg", &t2), true);
    assert!(result.is_ok(), "force install failed: {:?}", result);
}
