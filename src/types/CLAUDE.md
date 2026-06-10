# src/types/ — CLAUDE.md

**Module:** `types` · **Owner:** `principal-engineering-lead` · **Milestone:** M1 (stub at M0).

## Purpose
Shared, dependency-free core types: `Chunk`, `Language`, `SymbolType`, `FileMeta`. Lives here
(not in `parser`) per **Decision Log D5** so `storage` need not depend on `parser` and the
bottom-up build order stays acyclic.

## API anchor
`docs/project_plan.md` §4.3 (`Chunk`/`SymbolType`/`Language`) and §3.2.2 (`FileMeta`). `Chunk`
carries `start_line`/`end_line` (D7) and the enrichment fields `parent_symbol`/`file_docstring`/
`imports`/`cross_references` (D3).

## Tests / scenarios
Unit tests in-module. No dedicated TEST_STRATEGY row — these types are exercised through
`storage`, `parser`, and `chunker` scenarios.

## Status
M0: empty stub. Types implemented at M1 (first consumer is `storage`).
