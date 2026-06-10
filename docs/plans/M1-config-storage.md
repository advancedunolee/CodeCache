# M1 — config + storage (SQLite schema + FTS5)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m1--config--storage-sqlite-schema--fts5),
> [`../project_plan.md`](../project_plan.md) §3.2.2 / §4, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#storage-sqlite--fts5).

## Goal / acceptance criteria
Load+validate `.codecache/config.toml`, and stand up the SQLite storage layer with the FTS5
`symbols` table plus `files_metadata` and `index_state`. **Exit (from ROADMAP):**
- [ ] Config: valid TOML loads; defaults applied for omitted fields; invalid/missing → typed error.
- [ ] Schema creation is **idempotent**; a version bump triggers migration.
- [ ] Insert / query / delete round-trip verified with real values (not just `is_ok()`).
- [ ] FTS5 `MATCH` returns expected rows; `bm25()` orders by relevance; delete-by-file works.
- [ ] Corrupt/locked DB → typed error (no panic); empty-DB query → empty result.

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/config/mod.rs` | `Config` struct, `load(path)`, defaults, validation. | eng-lead |
| `src/storage/mod.rs` | `Storage` facade per §3.2.2. | eng-lead |
| `src/storage/schema.rs` | `init_schema()`, DDL constants, migration logic. | eng-lead + specialist (FTS5) |
| `src/storage/queries.rs` | Prepared statements: insert/delete/search/metadata. | eng-lead |
| `tests/config_tests.rs` | Integration: load/defaults/errors. | test-lead |
| `tests/storage_tests.rs` | Integration: round-trip, FTS5 MATCH/bm25, idempotent schema. | test-lead |
| `src/config/CLAUDE.md`, `src/storage/CLAUDE.md` | Updated with shipped API. | manager |

Shared `Chunk`/`Language`/`SymbolType` types (§4.3) are referenced by storage. To avoid a
forward dependency on `parser`, **introduce these types in a neutral location** — proposed:
`src/lib.rs` or a small `src/types.rs` (`pub mod types`) so both `storage` (M1) and `parser`
(M3) share them. **This is a plan clarification — see Decision Log note D5 below.**

## Dependencies
- **Prior:** M0 (workspace, `rusqlite` with `bundled` + `fts5`, `toml`, `serde`).
- **Within milestone:** `storage` does not depend on `config` (build either first); pair them
  only for the milestone exit.

## Ordered slices

### Slice M1.1 — config load/validate
- **RED (test-lead):** `tests/config_tests.rs`:
  - `valid_toml_loads_all_fields_expects_populated_config`
  - `omitted_fields_expects_documented_defaults` (max_tokens=4000, max_results=20, k1=1.2, b=0.75, languages=[python,typescript,go] — §7.3)
  - `missing_file_expects_typed_error`; `invalid_toml_expects_typed_error`
  - `ignore_pattern_parsing_correct`
- **GREEN (eng-lead):** `Config` (serde derive) mirroring §7.3 `config.toml`; `load` returns
  `Result<Config>`; defaults via `#[serde(default = ...)]`.
- **REVIEW / INTEGRATE:** fields match §7.3 keys exactly; defaults match §6/§7.3.

### Slice M1.2 — schema creation + idempotency + migration
- **RED:** `tests/storage_tests.rs`:
  - `new_db_creates_all_tables_expects_symbols_files_index_state`
  - `init_schema_twice_expects_no_error_idempotent`
  - `older_version_db_expects_migration_to_current` (seed `index_state.version` < current)
  - `corrupt_db_file_expects_typed_error_not_panic`
- **GREEN:** `Storage::new(&Path)` opens/creates; `init_schema()` runs DDL from §4.1
  (FTS5 `symbols` with `tokenize='unicode61 remove_diacritics 2'`, `files_metadata` + its two
  indexes, `index_state` seeded). Use `CREATE TABLE IF NOT EXISTS` / `CREATE VIRTUAL TABLE IF
  NOT EXISTS`; gate migration on `index_state.version`.
- **SPECIALIST (FTS5):** confirm `content='symbols'` external-content vs contentless trade-off;
  decide column set + which are `UNINDEXED` (§4.1). Document choice in `schema.rs` + brief.
- **REVIEW / INTEGRATE.**

### Slice M1.3 — CRUD round-trip + delete-by-file
- **RED:**
  - `insert_then_search_returns_inserted_chunk_with_fields` (assert all columns survive)
  - `bulk_insert_many_chunks_expects_all_present`
  - `delete_chunks_for_file_removes_only_that_files_chunks`
  - `get_then_update_file_hash_round_trips_hash_mtime_size_lang`
  - `empty_db_search_expects_empty_vec`
- **GREEN:** `insert_chunks`, `delete_chunks_for_file`, `get_file_hash`, `update_file_hash` in
  `queries.rs`; wrap multi-row inserts in a transaction (batch — §11.1).

### Slice M1.4 — FTS5 MATCH + bm25 ordering
- **RED:**
  - `match_query_returns_rows_containing_term`
  - `bm25_orders_more_relevant_chunk_first` (two chunks, one with term repeated → ranks higher)
  - `unindexed_columns_not_searchable` (term only in `file_path` ⇒ no match)
  - `column_weighting_respected` (name match outranks body-only match if weights set)
- **GREEN:** `search(query, limit)` issuing the §6.1 SQL (`bm25(symbols) AS score ORDER BY
  bm25(symbols) LIMIT ?`), mapping rows → `SearchResult { chunk, bm25_score }`.
- **PERF (note):** no formal budget yet, but record FTS5 query plan (`EXPLAIN QUERY PLAN`) so
  M6's latency budget has a baseline. Perf engineer logs it in the brief.

## API contracts / data structures (from `../project_plan.md` §3.2.2 / §4)
```rust
pub struct Storage { /* conn: rusqlite::Connection */ }
impl Storage {
    pub fn new(db_path: &Path) -> Result<Self>;
    pub fn init_schema(&self) -> Result<()>;
    pub fn insert_chunks(&self, chunks: &[Chunk]) -> Result<()>;
    pub fn delete_chunks_for_file(&self, file_path: &Path) -> Result<()>;
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    pub fn get_file_hash(&self, file_path: &Path) -> Result<Option<String>>;
    pub fn update_file_hash(&self, file_path: &Path, hash: &str, mtime: u64) -> Result<()>;
}
pub struct SearchResult { pub chunk: Chunk, pub bm25_score: f64 }
```
**SQL schema:** verbatim from §4.1 (`symbols` FTS5 + `files_metadata` w/ `file_size`,
`chunk_count`, `indexed_at` + `idx_files_mtime`/`idx_files_language`; `index_state` seeded).
**Note:** §4.1 `files_metadata` carries `file_size` and `chunk_count` that the §3.2.2
`update_file_hash` signature omits — extend the signature to
`update_file_hash(path, hash, mtime, file_size, chunk_count, language)` or take a small
`FileMeta` struct. **Plan clarification D6 below.**

## Performance budgets
- No hard budget gates at M1. Forward-looking: FTS5 search must support M6's **p95 < 500ms on
  100K LOC** (§1.3, §11.2 breakdown: FTS5 search <50ms). Keep statements prepared/cached.
- Index-size trajectory: §4.2 estimates ~6MB for Django-scale; schema choices here must not
  blow the **<100MB** index budget (§1.3). External-content FTS5 keeps duplication down.

## Decision Log bindings
- **D1:** keep `Storage::search` returning a plain `Vec<SearchResult>` so a future
  `HybridRetriever` can re-rank without storage changes.
- **D3 (metadata enrichment):** schema must have columns/strategy for `parent_symbol`,
  `file_docstring`, `imports`, `cross_references` to be FTS5-searchable by M4. Decide **now**
  whether they are extra FTS5 columns or packed into `chunk_text`. Recommended: add them as
  indexed FTS5 columns so M4 can populate without a migration. Coordinate with specialist.

## Definition of Done (this phase)
- [ ] All M1.1–M1.4 slices green; assertions check real column values + ordering.
- [ ] Schema idempotent + migration path tested; corrupt/locked DB → typed error.
- [ ] Shared `Chunk`/`Language`/`SymbolType` location decided (D5) and documented.
- [ ] `update_file_hash`/metadata signature reconciled with §4.1 (D6) — plan/spec updated first.
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 1 + module `CLAUDE.md` updated.

## Plan clarifications to record in ROADMAP Decision Log
- **D5 — Shared core types location.** `Chunk`, `Language`, `SymbolType` (§4.3) are needed by
  both `storage` (M1) and `parser` (M3). Place them in a dependency-free `crate::types` module
  rather than inside `parser`, so `storage` need not depend on `parser`. Honors build order
  (`../ENGINEERING_PLAN.md` §2).
- **D6 — `files_metadata` write signature.** §4.1 stores `file_size` + `chunk_count`; the
  §3.2.2 `update_file_hash` pseudocode omits them. Adopt a `FileMeta` parameter (or widen the
  signature) so M5's incremental indexer can persist them. Update `project_plan.md` §3.2.2 to
  match before implementing.
