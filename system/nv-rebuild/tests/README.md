# nv-rebuild integration tests

## Fixture stubs

Two small bash scripts live in `tests/fixtures/bin/`:

### `null`

Stands in for the `.null` language evaluator.

Supported subcommands: `eval`, `check`.

Environment variables:

| Variable | Effect |
|---|---|
| `NV_FIXTURE_MANIFEST_JSON` | Path to a JSON file containing a pre-baked `SystemManifest`. `null eval` prints this file to stdout. Required for `eval`. |
| `NV_FIXTURE_NULL_FAIL` | Set to `"1"` to make `null eval` exit 1 with a JSON error diagnostic on stderr. |

### `nv-pkg`

Stands in for the package manager.

Supported subcommands: `resolve`, `list`, `install`, `remove`, `verify`.

Environment variables:

| Variable | Effect |
|---|---|
| `NV_FIXTURE_STORE_DIR` | Path to a directory acting as the package store. Sub-directories must be named `<storeHash>-<name>-<version>` and contain a `manifest.json`. |
| `NV_FIXTURE_PKG_MISSING` | Space-separated list of `<name>-<version>` strings that `resolve` should pretend are missing (exit 1). |

`resolve <name>-<version>` scans `NV_FIXTURE_STORE_DIR` for a sub-directory
whose basename ends with `-<name>-<version>` and prints its absolute path.

## Running tests

```sh
# From system/nv-rebuild/:
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo test
```

Tests mutate global process env (`std::env`). A shared `Mutex` (`ENV_LOCK`)
serialises all env access so tests can run in parallel without races.
