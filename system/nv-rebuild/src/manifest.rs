use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The evaluated system manifest produced by `null eval`.
/// Matches the SystemManifest schema in CONTRACTS.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManifest {
    pub hostname: String,

    /// List of "<name>-<version>" strings, in declaration order.
    pub packages: Vec<String>,

    /// Services keyed by name.
    #[serde(default)]
    pub services: HashMap<String, Service>,

    /// Environment variables to expose in etc/environment.
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub exec: String,
    pub restart: RestartPolicy,
    /// Capabilities this service is granted. Emitted by `null eval` as the
    /// service's `requires` set (already validated ⊆ system `caps`). The
    /// supervisor confines the launched process to exactly these (Traccia A).
    #[serde(default)]
    pub requires: Vec<Capability>,
}

/// A capability as a structured value, matching `null eval`'s serialization:
/// `{"path":["net"],"arg":null}`, `{"path":["fs","read"],"arg":"/etc"}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capability {
    pub path: Vec<String>,
    #[serde(default)]
    pub arg: Option<String>,
}

impl Capability {
    /// Compact single-token form for the service descriptor: the path joined
    /// by `.`, plus `:<arg>` when an arg is present. `!net` → `net`,
    /// `!net.localhost` → `net.localhost`, `!fs.read."/etc"` → `fs.read:/etc`.
    pub fn token(&self) -> String {
        let base = self.path.join(".");
        match &self.arg {
            Some(a) => format!("{base}:{a}"),
            None => base,
        }
    }

    /// Inverse of [`token`]: parse one descriptor token back into a capability.
    pub fn parse_token(tok: &str) -> Capability {
        match tok.split_once(':') {
            Some((p, a)) => Capability {
                path: p.split('.').map(str::to_string).collect(),
                arg: Some(a.to_string()),
            },
            None => Capability {
                path: tok.split('.').map(str::to_string).collect(),
                arg: None,
            },
        }
    }

    /// Whether this capability grants outbound network access. Covers both
    /// `!net` and `!net.localhost`. NOTE (Traccia A slice): the supervisor
    /// treats both as "keep host netns"; the loopback-only refinement for
    /// `.localhost` (isolated netns with `lo` up) is a documented follow-up.
    pub fn grants_net(&self) -> bool {
        self.path.first().map(String::as_str) == Some("net")
    }

    /// The subtree path of a `!fs.read."P"` capability, if this is one.
    pub fn fs_read_path(&self) -> Option<String> {
        match (self.path.first().map(String::as_str), self.path.get(1).map(String::as_str)) {
            (Some("fs"), Some("read")) => self.arg.clone(),
            _ => None,
        }
    }

    /// The subtree path of a `!fs.write."P"` capability, if this is one.
    pub fn fs_write_path(&self) -> Option<String> {
        match (self.path.first().map(String::as_str), self.path.get(1).map(String::as_str)) {
            (Some("fs"), Some("write")) => self.arg.clone(),
            _ => None,
        }
    }

    fn is(&self, a: &str, b: Option<&str>) -> bool {
        self.path.first().map(String::as_str) == Some(a)
            && self.path.get(1).map(String::as_str) == b
    }

    /// `!proc.spawn` — may create new processes (fork/clone family).
    pub fn grants_proc_spawn(&self) -> bool {
        self.is("proc", Some("spawn"))
    }

    /// `!proc.exec` — may exec other binaries.
    pub fn grants_proc_exec(&self) -> bool {
        self.is("proc", Some("exec"))
    }

    /// `!rand` — may read kernel randomness (`getrandom`).
    pub fn grants_rand(&self) -> bool {
        self.is("rand", None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

/// Per-package manifest stored in the nv-store.
/// Matches the manifest.json schema in CONTRACTS.md §1.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authored_by: String,
    pub created_at: String,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub exposed_bins: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_steps: Vec<String>,
}
