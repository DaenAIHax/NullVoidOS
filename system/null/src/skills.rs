/// Version-matched skill bundle. The markdown files under
/// `system/null/skills/` are embedded at compile time, so an
/// agent inside the VM can recover the language model from the binary
/// alone with no network access (SPEC §1, §8).
///
/// Each skill is one Markdown document with YAML-ish frontmatter.
/// Adding a new skill: drop a file under `skills/`, add a const +
/// case below.

pub struct Skill {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

const NULL_MD: &str = include_str!("../skills/null.md");
const LANGUAGE_MD: &str = include_str!("../skills/language.md");
const SCHEMA_MD: &str = include_str!("../skills/schema.md");
const CAPS_MD: &str = include_str!("../skills/caps.md");
const CLI_MD: &str = include_str!("../skills/cli.md");
const DIAGNOSTICS_MD: &str = include_str!("../skills/diagnostics.md");

pub fn all() -> Vec<Skill> {
    vec![
        Skill {
            name: "null",
            description: "Top-level \"how to use me\" for the .null configuration language.",
            body: NULL_MD,
        },
        Skill {
            name: "language",
            description: "Compact .null syntax and semantics guide for agents.",
            body: LANGUAGE_MD,
        },
        Skill {
            name: "schema",
            description: "The SystemManifest schema that every system.null must satisfy.",
            body: SCHEMA_MD,
        },
        Skill {
            name: "caps",
            description: "Capability vocabulary and the system-grants/service-requires model.",
            body: CAPS_MD,
        },
        Skill {
            name: "cli",
            description: "Command surface and JSON output contracts for the null binary.",
            body: CLI_MD,
        },
        Skill {
            name: "diagnostics",
            description: "Error code table, when each fires, and the repair IDs they emit.",
            body: DIAGNOSTICS_MD,
        },
    ]
}

pub fn get(name: &str) -> Option<&'static str> {
    all().into_iter().find(|s| s.name == name).map(|s| s.body)
}
