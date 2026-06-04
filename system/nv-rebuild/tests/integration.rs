/// Integration tests for nv-rebuild.
///
/// Each test sets up a temporary directory tree and invokes nv-rebuild
/// subcommands by calling the library functions directly (not via exec),
/// with environment overrides so no root privileges are required.
///
/// Stub binaries in tests/fixtures/bin/ handle `null eval` and `nv-pkg resolve`.
/// The PATH is prepended with that directory at the start of each test via
/// the helper `EnvGuard`.
///
/// IMPORTANT: Because the tests modify global process environment variables
/// (std::env is process-wide), and Cargo runs tests in parallel threads by
/// default, all tests acquire a shared mutex before touching env vars.  This
/// serialises env access without requiring `--test-threads 1`.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;

use nv_rebuild::cli::{self, Config};
use nv_rebuild::generation;

// ─── Global serialisation lock ────────────────────────────────────────────────
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Path to the fixture stub binaries.
fn fixture_bin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("bin")
}

/// RAII guard that holds the ENV_LOCK and restores env vars on drop.
struct EnvGuard<'a> {
    _lock: std::sync::MutexGuard<'a, ()>,
    saved_path: String,
    saved_manifest_json: Option<String>,
    saved_store_dir: Option<String>,
    saved_null_fail: Option<String>,
    saved_pkg_missing: Option<String>,
}

impl<'a> EnvGuard<'a> {
    fn acquire(
        manifest_json_path: Option<&Path>,
        store_dir: Option<&Path>,
        null_fail: bool,
        pkg_missing: Option<&str>,
    ) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let saved_path = std::env::var("PATH").unwrap_or_default();
        let saved_manifest_json = std::env::var("NV_FIXTURE_MANIFEST_JSON").ok();
        let saved_store_dir = std::env::var("NV_FIXTURE_STORE_DIR").ok();
        let saved_null_fail = std::env::var("NV_FIXTURE_NULL_FAIL").ok();
        let saved_pkg_missing = std::env::var("NV_FIXTURE_PKG_MISSING").ok();

        // Prepend fixture bin dir to PATH
        let new_path = format!("{}:{}", fixture_bin_dir().display(), saved_path);
        std::env::set_var("PATH", &new_path);

        // Set fixture vars
        if let Some(p) = manifest_json_path {
            std::env::set_var("NV_FIXTURE_MANIFEST_JSON", p);
        } else {
            std::env::remove_var("NV_FIXTURE_MANIFEST_JSON");
        }
        if let Some(d) = store_dir {
            std::env::set_var("NV_FIXTURE_STORE_DIR", d);
        } else {
            std::env::remove_var("NV_FIXTURE_STORE_DIR");
        }
        if null_fail {
            std::env::set_var("NV_FIXTURE_NULL_FAIL", "1");
        } else {
            std::env::remove_var("NV_FIXTURE_NULL_FAIL");
        }
        if let Some(m) = pkg_missing {
            std::env::set_var("NV_FIXTURE_PKG_MISSING", m);
        } else {
            std::env::remove_var("NV_FIXTURE_PKG_MISSING");
        }

        Self {
            _lock: lock,
            saved_path,
            saved_manifest_json,
            saved_store_dir,
            saved_null_fail,
            saved_pkg_missing,
        }
    }

    /// Update the manifest JSON file path while holding the lock.
    fn set_manifest(&self, p: &Path) {
        std::env::set_var("NV_FIXTURE_MANIFEST_JSON", p);
    }

    /// Update the pkg_missing list while holding the lock.
    #[allow(dead_code)]
    fn set_pkg_missing(&self, v: Option<&str>) {
        match v {
            Some(s) => std::env::set_var("NV_FIXTURE_PKG_MISSING", s),
            None => std::env::remove_var("NV_FIXTURE_PKG_MISSING"),
        }
    }
}

impl Drop for EnvGuard<'_> {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.saved_path);
        match &self.saved_manifest_json {
            Some(v) => std::env::set_var("NV_FIXTURE_MANIFEST_JSON", v),
            None => std::env::remove_var("NV_FIXTURE_MANIFEST_JSON"),
        }
        match &self.saved_store_dir {
            Some(v) => std::env::set_var("NV_FIXTURE_STORE_DIR", v),
            None => std::env::remove_var("NV_FIXTURE_STORE_DIR"),
        }
        match &self.saved_null_fail {
            Some(v) => std::env::set_var("NV_FIXTURE_NULL_FAIL", v),
            None => std::env::remove_var("NV_FIXTURE_NULL_FAIL"),
        }
        match &self.saved_pkg_missing {
            Some(v) => std::env::set_var("NV_FIXTURE_PKG_MISSING", v),
            None => std::env::remove_var("NV_FIXTURE_PKG_MISSING"),
        }
    }
}

/// Write a SystemManifest JSON to a file; return its path.
fn write_manifest(dir: &Path, manifest: &serde_json::Value) -> PathBuf {
    let p = dir.join("system_manifest.json");
    std::fs::write(&p, serde_json::to_string_pretty(manifest).unwrap()).unwrap();
    p
}

/// Build a minimal SystemManifest JSON value.
fn manifest_json(hostname: &str, packages: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "hostname": hostname,
        "packages": packages,
        "services": {},
        "environment": {}
    })
}

/// Create a fake package in `store_dir` named `<hash>-<name>-<version>` with
/// the given `exposed_bins`. Each binary is a tiny executable shell script.
fn make_pkg(store_dir: &Path, name: &str, version: &str, exposed_bins: &[&str]) -> PathBuf {
    let pkg_id = format!("{}-{}", name, version);
    // Deterministic short "hash" — use wrapping mul to avoid debug-mode overflow
    let hash = format!("{:016x}", (pkg_id.len() as u64).wrapping_mul(0x9e3779b97f4a7c15));
    let pkg_dir = store_dir.join(format!("{hash}-{pkg_id}"));
    let bin_dir = pkg_dir.join("payload").join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();

    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "name": name,
        "version": version,
        "description": format!("test package {}", name),
        "authoredBy": "test",
        "createdAt": "2026-05-28T00:00:00Z",
        "deps": [],
        "exposedBins": exposed_bins,
        "capabilities": []
    });
    std::fs::write(
        pkg_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    for bin in exposed_bins {
        let p = bin_dir.join(bin);
        std::fs::write(&p, b"#!/bin/sh\necho hello\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    pkg_dir
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// 1. `check` on a valid manifest with all packages resolvable → exit 0.
#[test]
fn test_check_valid() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    std::fs::create_dir_all(&store).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);

    let m_path = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let _env = EnvGuard::acquire(Some(&m_path), Some(&store), false, None);

    let cfg = Config {
        system_root: tmp.path().join("system"),
        config_path: tmp.path().join("system.null"),
    };

    let result = cli::cmd_check(&cfg);
    assert!(result.is_ok(), "check failed: {:?}", result.err());
}

/// 2. `check` on a manifest referencing a missing package → error.
#[test]
fn test_check_missing_package() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    std::fs::create_dir_all(&store).unwrap();
    // do NOT create the package

    let m_path = write_manifest(tmp.path(), &manifest_json("nullvoid", &["missing-pkg-1.0.0"]));
    let _env = EnvGuard::acquire(Some(&m_path), Some(&store), false, Some("missing-pkg-1.0.0"));

    let cfg = Config {
        system_root: tmp.path().join("system"),
        config_path: tmp.path().join("system.null"),
    };

    let result = cli::cmd_check(&cfg);
    assert!(result.is_err(), "expected check to fail for missing package");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("missing") || msg.contains("not found") || msg.contains("one or more"),
        "error should mention missing package, got: {msg}"
    );
}

/// 3. `build` produces generation-N/ with manifest.json + bin symlinks; does NOT touch `current`.
#[test]
fn test_build_no_current() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::create_dir_all(&system_root).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);

    let m_path = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let _env = EnvGuard::acquire(Some(&m_path), Some(&store), false, None);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    cli::cmd_build(&cfg).expect("build failed");

    let gen1 = system_root.join("generation-1");
    assert!(gen1.is_dir(), "generation-1 should exist");
    assert!(gen1.join("manifest.json").exists(), "manifest.json missing");
    assert!(
        gen1.join("bin").join("bash").symlink_metadata().is_ok(),
        "bin/bash symlink missing"
    );
    // `current` must NOT exist
    assert!(
        !system_root.join("current").exists() && system_root.join("current").symlink_metadata().is_err(),
        "build must not create `current`"
    );
}

/// 4. `switch` from fresh state: creates generation-1, points current at it.
#[test]
fn test_switch_fresh() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);

    let m_path = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let _env = EnvGuard::acquire(Some(&m_path), Some(&store), false, None);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    cli::cmd_switch(&cfg).expect("switch failed");

    let gen1 = system_root.join("generation-1");
    assert!(gen1.is_dir(), "generation-1 should exist");

    let current = system_root.join("current");
    assert!(current.symlink_metadata().is_ok(), "current symlink missing");
    let target = std::fs::read_link(&current).unwrap();
    assert_eq!(
        target.file_name().unwrap().to_str().unwrap(),
        "generation-1",
        "current should point to generation-1"
    );
}

/// 5. `switch` again: creates generation-2, current points there, generation-1 untouched.
#[test]
fn test_switch_second() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);
    make_pkg(&store, "task-tui", "0.3.2", &["task"]);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    // First switch
    let m1 = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let env = EnvGuard::acquire(Some(&m1), Some(&store), false, None);
    cli::cmd_switch(&cfg).expect("first switch failed");

    // Second switch (simulate edited manifest)
    let m2 = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9", "task-tui-0.3.2"]));
    env.set_manifest(&m2);
    cli::cmd_switch(&cfg).expect("second switch failed");
    drop(env);

    let gen2 = system_root.join("generation-2");
    assert!(gen2.is_dir(), "generation-2 should exist");

    let current = system_root.join("current");
    let target = std::fs::read_link(&current).unwrap();
    assert_eq!(target.file_name().unwrap().to_str().unwrap(), "generation-2");

    // generation-1 still present and unmodified
    let gen1 = system_root.join("generation-1");
    assert!(gen1.is_dir(), "generation-1 must still exist");
    let g1_manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(gen1.join("manifest.json")).unwrap()).unwrap();
    let pkgs = g1_manifest["packages"].as_array().unwrap();
    assert_eq!(pkgs.len(), 1, "generation-1 should still have only 1 package");
}

/// 6. `rollback` after second switch: current back to generation-1.
#[test]
fn test_rollback() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);
    make_pkg(&store, "task-tui", "0.3.2", &["task"]);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    let m1 = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let env = EnvGuard::acquire(Some(&m1), Some(&store), false, None);
    cli::cmd_switch(&cfg).expect("first switch failed");

    let m2 = write_manifest(
        tmp.path(),
        &manifest_json("nullvoid", &["bash-5.3.9", "task-tui-0.3.2"]),
    );
    env.set_manifest(&m2);
    cli::cmd_switch(&cfg).expect("second switch failed");

    // current should be generation-2
    let current = system_root.join("current");
    let t = std::fs::read_link(&current).unwrap();
    assert_eq!(t.file_name().unwrap().to_str().unwrap(), "generation-2");

    // rollback
    cli::cmd_rollback(&cfg).expect("rollback failed");
    drop(env);

    let t2 = std::fs::read_link(&current).unwrap();
    assert_eq!(
        t2.file_name().unwrap().to_str().unwrap(),
        "generation-1",
        "after rollback current should be generation-1"
    );
}

/// 7. `rollback` when there is no previous generation: fails clearly.
#[test]
fn test_rollback_no_previous() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);

    let m_path = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let _env = EnvGuard::acquire(Some(&m_path), Some(&store), false, None);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    // Only one switch → no generation-0
    cli::cmd_switch(&cfg).expect("switch failed");

    let result = cli::cmd_rollback(&cfg);
    assert!(result.is_err(), "rollback on only generation should fail");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("does not exist") || msg.contains("cannot roll back") || msg.contains("generation-0"),
        "error should explain no previous generation, got: {msg}"
    );
}

/// 8. Conflict resolution: two packages expose the same bin; last one wins, warning emitted.
#[test]
fn test_bin_conflict_last_wins() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();

    // Both packages expose "editor"
    make_pkg(&store, "vi", "1.0.0", &["editor"]);
    make_pkg(&store, "nvim", "0.9.0", &["editor"]);

    // nvim is listed last → it should win
    let m_path = write_manifest(
        tmp.path(),
        &manifest_json("nullvoid", &["vi-1.0.0", "nvim-0.9.0"]),
    );
    let _env = EnvGuard::acquire(Some(&m_path), Some(&store), false, None);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    // switch succeeds despite the conflict
    cli::cmd_switch(&cfg).expect("switch with conflict should succeed");

    // The symlink target should point into the nvim store path
    let link = system_root.join("generation-1").join("bin").join("editor");
    assert!(
        link.symlink_metadata().is_ok(),
        "bin/editor symlink should exist"
    );
    let target = std::fs::read_link(&link).unwrap();
    let target_str = target.to_string_lossy();
    assert!(
        target_str.contains("nvim"),
        "bin/editor should point into nvim's store path, got: {target_str}"
    );
}

/// 9. `generations` lists all in order, marks current.
#[test]
fn test_generations_list() {
    let tmp = TempDir::new().unwrap();
    let store = tmp.path().join("store");
    let system_root = tmp.path().join("system");
    std::fs::create_dir_all(&store).unwrap();
    make_pkg(&store, "bash", "5.3.9", &["bash"]);
    make_pkg(&store, "task-tui", "0.3.2", &["task"]);

    let cfg = Config {
        system_root: system_root.clone(),
        config_path: tmp.path().join("system.null"),
    };

    let m1 = write_manifest(tmp.path(), &manifest_json("nullvoid", &["bash-5.3.9"]));
    let env = EnvGuard::acquire(Some(&m1), Some(&store), false, None);
    cli::cmd_switch(&cfg).unwrap();

    let m2 = write_manifest(
        tmp.path(),
        &manifest_json("nullvoid", &["bash-5.3.9", "task-tui-0.3.2"]),
    );
    env.set_manifest(&m2);
    cli::cmd_switch(&cfg).unwrap();
    drop(env);

    // list_generations returns [1, 2]
    let gens = generation::list_generations(&system_root).unwrap();
    let mut sorted = gens.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![1, 2], "expected generations 1 and 2");

    // current points to generation-2
    let current = system_root.join("current");
    let target = std::fs::read_link(&current).unwrap();
    assert_eq!(target.file_name().unwrap().to_str().unwrap(), "generation-2");
}
