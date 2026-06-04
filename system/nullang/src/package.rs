//! Packaging: turn a compiled Nullang program into an `.nvpkg` so it enters
//! the OS content-addressed store via `nv-pkg` (SPEC §13; CONTRACTS.md §1).
//!
//! Nullang builds the artifact and its provenance manifest; `nv-pkg` owns the
//! CAS (the store hash is SHA-256 of the tarball). Provenance lives in the
//! manifest (`authoredBy`, `createdAt`, `sourceLanguage`, `buildSteps` with
//! source/C hashes) and in the `recipe.null` shipped inside the package.
//!
//! The package's `capabilities` are derived from `main`'s `uses` clause: the
//! language's static effect set becomes the package's declared capability set
//! — declaration grants, construction consumes, packaging records.
use serde::Serialize;

use crate::ast::{Capability, File, Item};

/// `manifest.json` per CONTRACTS.md §1.2 (schemaVersion 1).
#[derive(Debug, Clone, Serialize)]
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
    pub source_language: String,
    pub build_steps: Vec<String>,
}

impl Manifest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        version: &str,
        description: &str,
        authored_by: &str,
        created_at: &str,
        capabilities: Vec<String>,
        build_steps: Vec<String>,
    ) -> Self {
        Manifest {
            schema_version: 1,
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            authored_by: authored_by.to_string(),
            created_at: created_at.to_string(),
            deps: Vec::new(),
            exposed_bins: vec![name.to_string()],
            capabilities,
            source_language: "nullang".to_string(),
            build_steps,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("manifest serialization never fails")
    }
}

/// Map a Nullang capability value to the CONTRACTS.md §4 capability string:
/// `!fs.read."/etc"` → `fs:read:/etc`, `!net.localhost` → `net:localhost`,
/// `!tty` → `tty`.
pub fn cap_to_contract(cap: &Capability) -> String {
    let mut s = cap.path.join(":");
    if let Some(arg) = &cap.arg {
        s.push(':');
        s.push_str(arg);
    }
    s
}

/// The capability strings a program needs, taken from `main`'s `uses` clause.
pub fn capabilities_of_main(file: &File) -> Vec<String> {
    for item in &file.items {
        if let Item::Func(f) = item {
            if f.name == "main" {
                return f.uses.iter().map(cap_to_contract).collect();
            }
        }
    }
    Vec::new()
}
