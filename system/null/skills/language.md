---
name: language
description: Compact .null syntax and semantics guide for agents.
---

# `.null` Language Surface

## Lexical

```
# line comment, until end of line
identifier  = [a-z][a-z0-9-]*       (kebab-case, max 64 chars)
string      = "..."                 (no interpolation, \n \t \" \\ escapes)
int         = -?[0-9]+              (no underscores, no hex/oct/bin)
bool        = true | false
null        = null
symbol      = .identifier
capability  = !identifier(.identifier)*(."string")?
```

Whitespace is significant only as a separator. No indentation rules.

## Composite values

**Attribute set** — `{ key = value; ... }`. Semicolon terminator after
every entry. No trailing-comma optionality. Duplicate keys are a parse
error (PAR001).

**List** — `[ a b c ]`. Whitespace-separated, **no commas**.
Homogeneous: all elements must be the same type (TYP001 otherwise).

**Field access** — `lhs.field`, chained `lhs.f1.f2.f3`. The lhs must be
an attrset (only `pkgs` qualifies in v2).

## References

The single in-scope identifier is `pkgs`. `pkgs.<name>` resolves to the
string `"<name>-<version>"` of the installed package. Built at evaluator
start from `nv-pkg list --json`.

## Anti-features (do not write)

- Functions (`{ x }: x + 1`)
- `let in` / local bindings
- `if then else`
- String interpolation
- Recursion / `rec { }`
- Lazy evaluation
- Multiple syntaxes for the same concept (no commas in lists/attrsets, no `:` for assignment)
- `import "./other.null"` — single file only in v2.0

If you find yourself wanting one of these, you're writing in the wrong
language. `.null` is for declaring system state; programs go in Zero.

## Example file (canonical shape)

```null
{
  hostname = "nullvoid";
  caps = [ !net !fs.read."/etc" !tty ];
  packages = [ pkgs.claude-code pkgs.bash ];
  services = {
    agent = {
      exec = "/run/current/bin/claude";
      restart = .always;
      requires = [ !net !tty ];
    };
  };
  environment = { EDITOR = "nvim"; };
}
```
