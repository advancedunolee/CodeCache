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

## Status
M0: empty stub. Implemented at M1.
