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

## Shipped API (M1)
- `Storage { conn: Arc<Mutex<Connection>> }` (D8), `#[derive(Clone)]` (clones share one conn).
- `new(&Path)`, `init_schema()` (idempotent; migrates older `index_state.version` forward),
  `insert_chunks(&[Chunk])` (single transaction batch), `delete_chunks_for_file(&Path)`,
  `search(&str, usize) -> Vec<SearchResult>` (BM25 best-first, deterministic `rowid` tie-break),
  `get_file_hash`/`get_file_meta`, `update_file_hash(&Path, &FileMeta)` (D6 upsert),
  `get_index_state`/`set_index_state`.
- **M5.3 additions** (deletion reconciliation, plan §3.2.2 updated): `delete_file_meta(&Path)`
  (drop a `files_metadata` row — symmetric with `delete_chunks_for_file`; unknown file = no-op),
  `all_indexed_files() -> Vec<PathBuf>` (enumerate every indexed path — drives the indexer's
  on-disk-vs-known reconcile and the DB-wide totals recompute).
- **M8.3 addition** (**D19**, plan §3.2.2): `symbols_for_path(&Path) -> Vec<SymbolOutline>` — the
  `codecache_outline` lookup. A plain parameterized column `SELECT` off the contentful `symbols`
  table `WHERE file_path = ?1 OR file_path LIKE ?2 ESCAPE '\'` (exact file OR `<dir>/%` directory
  prefix; the path's literal `%`/`_`/`\` are escaped by the private `escape_like` helper so a path
  with a wildcard char never over-matches), ordered `(file_path, start_line, end_line)`. Returns the
  slim `SymbolOutline` (name/type/parent/path/start_line/end_line) — **zero source reads** (D7), no
  `chunk_text`. Unknown path → empty `Vec`; corrupt `symbol_type` → `CorruptRow`, never a panic.
- `SearchResult { chunk, bm25_score }`. `StorageError::{Sqlite, LockPoisoned, CorruptRow}` (typed,
  impl `std::error::Error`; no reachable panic — poisoned lock & unknown stored enum are typed).

## Schema / FTS5 notes (`schema.rs`, `queries.rs`)
- Default (contentful) FTS5 `symbols` table — **D11** drops the invalid `content='symbols'` from
  §4.1; FTS5 stores + returns all columns, so chunks round-trip without a companion table.
- Indexed (D3): symbol_name, symbol_type, chunk_text, parent_symbol, imports, cross_references,
  file_docstring (last indexed column — a term only in the module docstring is matchable).
  UNINDEXED: file_path, start_byte, end_byte, start_line, end_line (D7), language.
- `imports`/`cross_references` stored as `\n`-joined text (FTS5 has no array type).
- `tokenize='unicode61 remove_diacritics 2'`; BM25 per-column weights (one per indexed column,
  7 total) weight `symbol_name` highest (10.0), `parent_symbol` 5.0, the rest 2.0/1.0;
  `file_docstring` weighted 2.0. `ORDER BY bm25 ASC, rowid ASC`.

## Status
**M1: DONE (2026-06-10).** All four gates green on Rust 1.85.0 (18 storage tests pass).
