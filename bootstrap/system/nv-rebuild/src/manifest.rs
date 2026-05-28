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
