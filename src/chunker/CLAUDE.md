# src/chunker/ — CLAUDE.md

**Module:** `chunker` · **Owner:** `principal-engineering-lead` + `rust-treesitter-specialist`
· **Milestone:** M4 (stub at M0).

## Purpose
Turn AST nodes into `Chunk`s with metadata enrichment (`parent_symbol`, `file_docstring`,
`imports`, `cross_references` — **Decision Log D3**); populate `start_line`/`end_line` (**D7**).
Chunks are non-overlapping and lie within file bounds. Flag heuristic chunks when degradation
(D2) triggered.

## API anchor
`docs/project_plan.md` §4.3 (`Chunk`).

## Tests / scenarios
`docs/TEST_STRATEGY.md#chunker` — property: chunks never overlap and lie within `[0, file_len)`;
enrichment fields populated; heuristic flag set under degradation.

## Status
M0: empty stub. Implemented at M4 (first `proptest` consumer).
