---
name: caps
description: Capability vocabulary and the system-grants/service-requires model.
---

# Capabilities

Capabilities are first-class **values** in `.null`, written with the `!`
sigil. The vocabulary is closed and versioned with the language.

## Vocabulary (v2.0)

| Capability                  | Meaning                                           |
|-----------------------------|---------------------------------------------------|
| `!net`                      | any outbound socket                                |
| `!net.localhost`            | sockets to 127.0.0.1 only                         |
| `!fs.read."<path>"`         | read subtree at `<path>`                          |
| `!fs.write."<path>"`        | write subtree at `<path>`                         |
| `!tty`                      | controlling terminal                              |
| `!proc.spawn`               | spawn child processes                             |
| `!proc.exec`                | exec other binaries                               |
| `!time`                     | read system time                                  |
| `!rand`                     | read /dev/urandom                                 |
| `!activate.system`          | switch generations (privileged, nv-rebuild only)  |

Using anything outside this set raises CAP001 (unknown capability).
Adding a new capability is a minor language version bump.

## The grants/requires model

Two places in the system declare capabilities:

- `system.caps` — what the **system grants** (a whitelist).
- `services.<name>.requires` — what a **service needs** to run.

**Type rule:** every entry in `services.<name>.requires` must also appear
in `system.caps`. Violations are rejected at type-check time with CAP004.

The rationale (SPEC §5.5): every effect a service can exercise is visible
in the file, with no implicit grants or escape hatches. An agent reads
the file once and knows the maximum effect surface of every service.

## Argument-bearing capabilities

`!fs.read` and `!fs.write` take a string argument:

```null
caps = [ !fs.read."/etc"  !fs.write."/var/notes" ];
```

The argument is part of the capability identity — `!fs.read."/etc"` and
`!fs.read."/etc/foo"` are distinct caps. Exact-match is used for the
subset check; there is no path-prefix semantics in v2.0.

## Repair IDs you may see

- `add-system-cap { cap, path, arg }` — CAP004 repair: append the
  missing cap to `system.caps`.
- `remove-unused-cap { cap }` — CAP004 repair: drop the cap from the
  service's `requires` instead.

These are mutually exclusive. The agent picks based on intent: was the
cap a typo (remove), or does the service genuinely need it (add)?
