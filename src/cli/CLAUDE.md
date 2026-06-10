# src/cli/ — CLAUDE.md

**Module:** `cli` · **Owner:** `principal-engineering-lead` · **Milestone:** M7 (stub at M0).

## Purpose
`clap`-based argument parsing and command dispatch: `init`, `index`, `update`, `query`,
`status`, `config`, `serve`. User-facing errors with helpful messages + nonzero exit.

## API anchor
`docs/project_plan.md` §7 (command structure + per-command specs).

## Tests / scenarios
`docs/TEST_STRATEGY.md#cli` — each command parses expected args/flags; `--help`/`--version`; bad
args → helpful error + nonzero exit; E2E `init → index → query` through the built binary.

## Status
M0: stub `run()` prints `codecache <VERSION>` so the binary links and is invocable. Full `clap`
dispatch lands at M7.
