# src/storage/ — CLAUDE.md

**Module:** `storage` · **Owner:** `principal-engineering-lead` + `rust-treesitter-specialist`
(FTS5 tuning) · **Milestone:** M1 (stub at M0).

## Purpose
SQLite interface: create/migrate schema (`symbols` FTS5, `files_metadata`, `index_state`),
insert/query/delete chunks, BM25 search. `Storage` wraps `Arc<Mutex<Connection>>` (**D8**) so it
is cheaply `Clone`-able and the MCP server can lend one connection to `Retriever`/`Indexer`.

## API anchor
`docs/project_plan.md` §3.2.2 (API) + §4.1 (schema). Honors **D6** (`update_file_hash(path,
&FileMeta)`) and **D7** (`start_line`/`end_line` UNINDEXED columns; D3 enrichment columns indexed).

## Tests / scenarios
`docs/TEST_STRATEGY.md#storage-sqlite--fts5` — idempotent schema; round-trip CRUD; `MATCH` +
`bm25()` ordering; corrupt/locked DB → error not panic; empty-DB query → empty result.

## Status
M0: empty stub. Implemented at M1.
