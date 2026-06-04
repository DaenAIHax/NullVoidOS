//! Atomic symlink swap: point `link` at `target`.
//!
//! # Strategy
//!
//! We prefer `renameat2(RENAME_EXCHANGE)` (Linux ≥ 3.15) which atomically
//! swaps two filesystem entries with a single syscall.  However, the target
//! *link* may not exist yet on a fresh system, so a pure exchange is only
//! possible if both paths already exist.
//!
//! The algorithm therefore is:
//!
//! 1. Write a new temporary symlink `<system_root>/.current.new -> target`.
//! 2a. If `link` already exists and `renameat2` is available:
//!     Call `renameat2(RENAME_EXCHANGE, link, .current.new)`.
//!     Then remove `.current.new` (which now points to the old target).
//! 2b. Otherwise (fresh state, or renameat2 not available):
//!     `rename(.current.new, link)` — atomic on the same filesystem.
//!
//! `rename(2)` is atomic when src and dst live on the same filesystem;
//! since both `.current.new` and `current` are under `system_root` they
//! are always on the same filesystem, so 2b is safe.
//!
//! The `nix` crate is used for `renameat2`; we fall back gracefully if the
//! kernel returns `ENOSYS` or the crate feature is absent.

use anyhow::{Context, Result};
use std::path::Path;

pub fn atomic_symlink(target: &Path, link: &Path, system_root: &Path) -> Result<()> {
    let tmp = system_root.join(".current.new");

    // Remove stale tmp link if present from a crashed previous run.
    if tmp.symlink_metadata().is_ok() {
        std::fs::remove_file(&tmp)
            .with_context(|| format!("cannot remove stale {}", tmp.display()))?;
    }

    // Step 1: create tmp -> target
    // target may be absolute or relative; we store the relative name so the
    // symlink stays valid if the root is bind-mounted elsewhere.
    let target_name = target
        .file_name()
        .with_context(|| format!("target path has no filename: {}", target.display()))?;
    std::os::unix::fs::symlink(target_name, &tmp)
        .with_context(|| format!("cannot create tmp symlink {}", tmp.display()))?;

    // Step 2: move tmp -> link (atomic rename on same filesystem)
    // If link already exists, std::fs::rename replaces it atomically.
    std::fs::rename(&tmp, link).with_context(|| {
        format!(
            "cannot rename {} -> {}",
            tmp.display(),
            link.display()
        )
    })?;

    Ok(())
}

/// Attempt `renameat2(RENAME_EXCHANGE)` between `a` and `b`.
/// Returns `Ok(true)` if the exchange succeeded, `Ok(false)` if the
/// kernel returned ENOSYS / EINVAL (kernel too old or filesystem does
/// not support it), and `Err` for any other failure.
///
/// NOTE: This is currently unused in the main activation path because
/// `rename(2)` is sufficient for a symlink that points into a directory
/// tree that is not being modified concurrently.  The function is kept
/// here so Phase 2 can wire it in when two live service trees need to
/// be swapped in-place without any window where `current` is absent.
#[allow(dead_code)]
fn try_renameat2_exchange(a: &Path, b: &Path) -> Result<bool> {
    use nix::errno::Errno;
    use nix::libc;

    let a_cstr = std::ffi::CString::new(a.as_os_str().as_encoded_bytes())
        .context("non-NUL path for renameat2 arg a")?;
    let b_cstr = std::ffi::CString::new(b.as_os_str().as_encoded_bytes())
        .context("non-NUL path for renameat2 arg b")?;

    // RENAME_EXCHANGE = 2
    const RENAME_EXCHANGE: libc::c_uint = 2;

    let ret = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            a_cstr.as_ptr(),
            libc::AT_FDCWD,
            b_cstr.as_ptr(),
            RENAME_EXCHANGE,
        )
    };

    if ret == 0 {
        return Ok(true);
    }

    let err = Errno::last();
    match err {
        Errno::ENOSYS | Errno::EINVAL => Ok(false),
        _ => Err(anyhow::anyhow!(
            "renameat2({}, {}) failed: {}",
            a.display(),
            b.display(),
            err
        )),
    }
}
