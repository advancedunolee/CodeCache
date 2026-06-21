# src/config/ — CLAUDE.md

**Module:** `config` · **Owner:** `principal-engineering-lead` · **Milestone:** M1 (stub at M0).

## Purpose
Load and validate `.codecache/config.toml`: index paths, ignore patterns, language settings,
storage/retrieval/MCP sections; apply defaults for omitted fields.

## API anchor
`docs/project_plan.md` §7.3 (config schema).

## Tests / scenarios
`docs/TEST_STRATEGY.md#config` — valid TOML loads; defaults applied; invalid/missing → clear
error; ignore-pattern parsing.

## Shipped API (M1)
- `Config` (+ `StorageConfig`/`RetrievalConfig`/`McpConfig`) mirroring §7.3 keys.
- `Config::load(&Path) -> Result<Config, ConfigError>` — reads + parses TOML, applies documented
  defaults for omitted fields (`max_tokens=4000`, `max_results=20`, `bm25_k1=1.2`, `bm25_b=0.75`,
  `languages=[python,typescript,go]`, `db_path=.codecache/index.db`, `max_db_size_mb=500`,
  `transport=stdio`, `sse_port=3000`) via `#[serde(default = ...)]` + section `Default` impls.
- `ConfigError::{Io, Parse, Serialize}` — typed (impl `std::error::Error`); missing/unreadable →
  `Io`, malformed TOML → `Parse`, failed TOML serialize on save → `Serialize`. No
  `unwrap`/`expect`/`panic`.

## Additive API (M7.3 / D18)
- `Config::save(&self, path: &Path) -> Result<(), ConfigError>` — serialize the full `Config` via
  `toml::to_string` and write it to `path` (non-clobbering: the whole resolved config is
  re-serialized, so unrelated keys survive a single-key edit). Serialize failure →
  `ConfigError::Serialize { path, source }`; write failure → `ConfigError::Io { path, source }`.
  Used by the CLI `config <KEY> <VALUE>` write path. Round-trip covered by the
  `save_then_load_round_trips` unit test + `tests/cli_tests.rs::config_command_reads_writes_settings`.

## Additive API (D32 / §7.3 — built-in default ignores)
- `Config.use_default_ignores: bool` — `#[serde(default = "default_use_default_ignores")]` ⇒
  `true`. When `true`, discovery folds in the built-in `DEFAULT_IGNORE_PATTERNS` (venv/dep/build
  dirs); `ignore_patterns` *extends* (never replaces) them. `false` opts out (only `.gitignore` +
  `ignore_patterns` apply). The set itself lives in `src/indexer/discovery.rs` (the sole consumer).
  Omitted-key TOML loads as `true`; `use_default_ignores = false` round-trips. Pinned by
  `config::tests::{default_config_matches_documented_defaults, toml_omitting_use_default_ignores_loads_true,
  toml_use_default_ignores_false_loads_false}`.

## Status
**M1: DONE (2026-06-10).** All four gates green on Rust 1.85.0.
**D32 (2026-06-20):** added `Config.use_default_ignores: bool` (default `true`) for the built-in
default-ignore knob (§7.3); gates green.
**M7.3 (2026-06-12):** added `Config::save` (+ `ConfigError::Serialize`) for the CLI config write
path (D18); gates green.
