/// Documentation strings for each `DiagCode`, served by `null explain CODE`.
///
/// These exist so that an agent that has never seen `.null` can recover the
/// "why this code fires and what to do about it" knowledge directly from
/// the binary, with no network access. SPEC §1 (compiler ships its docs)
/// and §8 (skills bundle).
///
/// Keep these in lockstep with `DiagCode` variants and with the actual
/// behavior in `types.rs` / `parser.rs`. If you add a code, add a doc here.

use crate::diagnostics::DiagCode;

pub fn lookup(code_str: &str) -> Option<&'static str> {
    let code = match code_str.to_ascii_uppercase().as_str() {
        "PAR001" => DiagCode::Par001,
        "TYP001" => DiagCode::Typ001,
        "TYP004" => DiagCode::Typ004,
        "SCH001" => DiagCode::Sch001,
        "REF002" => DiagCode::Ref002,
        "CAP001" => DiagCode::Cap001,
        "CAP004" => DiagCode::Cap004,
        _ => return None,
    };
    Some(doc_for(code))
}

pub fn list_codes() -> &'static [&'static str] {
    &[
        "PAR001", "TYP001", "TYP004", "SCH001", "REF002", "CAP001", "CAP004",
    ]
}

fn doc_for(code: DiagCode) -> &'static str {
    match code {
        DiagCode::Par001 => PAR001,
        DiagCode::Typ001 => TYP001,
        DiagCode::Typ004 => TYP004,
        DiagCode::Sch001 => SCH001,
        DiagCode::Ref002 => REF002,
        DiagCode::Cap001 => CAP001,
        DiagCode::Cap004 => CAP004,
    }
}

const PAR001: &str = "\
PAR001 — generic parse error

Fires when the source does not match `.null` v2 grammar at some position.
Common causes:

  * a token where the grammar expected something else
    (`=` instead of `;`, `,` between list items, etc.)
  * use of an anti-feature: `let in`, `if then else`, `import`,
    functions, string interpolation (SPEC §2)
  * malformed string or capability literal

Repair IDs that may appear:

  * add-required-field { field, type }   — when `=` is missing after a key
  * remove-unknown-field { field }       — on duplicate attribute keys

The `actual` field carries the offending token. The `expected` field names
the token shape that would have been valid at that position.
";

const TYP001: &str = "\
TYP001 — type mismatch

Fires when a value's type does not match the position it occupies in the
fixed SystemManifest schema (SPEC §4).

Examples:
  * `hostname = 42;`              (expected String, got Int)
  * `packages = \"bash-5\";`        (expected [String], got String)
  * `services = [];`              (expected AttrSet, got List)

Repair IDs that may appear:

  * wrap-int-as-string { value }     — for the Int → String case
  * homogenize-list { target-type }  — when a list's element type mismatches

The `expected` field carries the schema-declared type; `actual` carries the
type of the value that was found.
";

const TYP004: &str = "\
TYP004 — invalid Restart value

Fires when `services.<name>.restart` is anything other than one of the
three valid symbols: `.always`, `.on-failure`, `.never`.

The most common case is writing a String (`\"always\"`) instead of a Symbol
(`.always`) — a v1 → v2 migration leftover.

Repair IDs that may appear:

  * fix-enum-symbol { got, valid }    — applied by rewriting the value as
                                        the corresponding symbol from
                                        `valid` whose name matches `got`
";

const SCH001: &str = "\
SCH001 — missing required field

Fires when an attrset is missing a field the schema declares as required
at that position. SystemManifest requires: hostname, caps, packages,
services, environment. Service requires: exec, restart, requires.

Repair IDs that may appear:

  * add-required-field { field, type }   — inserts a stub for the missing
                                           field with the expected type
";

const REF002: &str = "\
REF002 — unknown reference

Fires when an identifier or field access cannot be resolved. In v2.0 the
only in-scope identifier is `pkgs`, and only single-level `pkgs.<name>`
access is supported.

Sub-cases:
  * unknown identifier — anything other than `pkgs`
  * `pkgs.<name>` where `<name>` is not in `nv-pkg list --json`
  * chained `pkgs.<name>.<more>` (not supported in v2.0)
  * `nv-pkg` not on PATH at evaluator startup (emitted as a warning)

Repair IDs that may appear:

  * quote-bare-identifier { ident }    — turns a bare ident into a literal
                                         string \"<ident>\"
";

const CAP001: &str = "\
CAP001 — unknown capability

Fires when a `!capability` literal does not match one of the SPEC §5.5
shapes:

  !net                       !net.localhost
  !fs.read.\"<path>\"          !fs.write.\"<path>\"
  !tty                       !proc.spawn         !proc.exec
  !time                      !rand               !activate.system

The vocabulary is closed and versioned with the language. Adding a new
capability is a minor version bump.

No repair ID — the only safe fix is choosing a capability from the valid
set or removing the literal entirely.
";

const CAP004: &str = "\
CAP004 — service requires capability not granted by system

Fires when a service's `requires` list contains a capability not present
in the system-level `caps` whitelist. The check is exact-match: paths and
args must be identical.

Example:

  caps = [ !tty ];
  services.agent = {
    exec = \"/run/current/bin/claude\";
    restart = .always;
    requires = [ !net !tty ];        # !net not in system.caps → CAP004
  };

Repair IDs that may appear:

  * add-system-cap { cap, path, arg }    — appends the missing cap to
                                           system.caps
  * remove-unused-cap { cap }             — drops the cap from the
                                           service's `requires` list

These are mutually exclusive; the agent picks based on intent (is the
cap actually needed by the service, or was it a typo?).
";
