# M5 — indexer (discovery → parse → chunk → hash → store)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m5--indexer),
> [`../project_plan.md`](../project_plan.md) §3.2.4 / §5.1 / §5.2, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#indexer).

## Goal / acceptance criteria
Orchestrate the full pipeline and incremental updates. **Exit (from ROADMAP):**
- [ ] Discovery honors `.gitignore` + extra ignore patterns; respects configured languages.
- [ ] Full index of a fixture repo populates storage correctly.
- [ ] Re-index unchanged ⇒ **no writes** (idempotent); modify N files ⇒ exactly those re-indexed.
- [ ] Deleted file ⇒ its chunks removed.
- [ ] e2e `init → index` green through the public surface.

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/indexer/mod.rs` | `Indexer` facade per §3.2.4; `index_all`, `update_files`. | eng-lead |
| `src/indexer/discovery.rs` | File walk honoring `.gitignore`/patterns; language detection. | eng-lead |
| `src/indexer/pipeline.rs` | parse→chunk→hash→store orchestration; change detection. | eng-lead |
| `tests/indexer_tests.rs` | Integration: discovery, full, incremental, delete. | test-lead |
| `tests/e2e_index.rs` | E2E: `init → index` on a fixture repo. | test-lead |
| `tests/fixtures/repo/**` | Small multi-file/multi-lang fixture repo (Python only usable until M9). | test-lead |
| `src/indexer/CLAUDE.md` | Shipped API + pipeline notes. | manager |

## Dependencies
- **Prior (all of):** M1 `storage`, M2 `hasher`, M3 `parser`, M4 `chunker`, plus `config` (M1).
  This is the first **integration** milestone — it wires the leaves together.
- Crates: `ignore` (gitignore-aware walk), `walkdir`.

## Ordered slices

### Slice M5.1 — discovery + language detection
- **RED (test-lead):**
  - `discovery_respects_gitignore`
  - `discovery_respects_extra_ignore_patterns_from_config`
  - `discovery_only_returns_configured_languages` (e.g. languages=[python] skips .ts/.go)
  - `language_detected_from_extension` (.py→Python, .ts→TS, .go→Go)
  - `non_source_files_skipped`
- **GREEN:** `discover_files()` using `ignore::WalkBuilder` (respects `.gitignore`); apply
  config `ignore_patterns`; `detect_language(path)` by extension; group by language (§5.1).

### Slice M5.2 — full index (index_all)
- **RED:**
  - `index_all_populates_storage_with_expected_chunk_count`
  - `index_all_writes_files_metadata_for_each_file` (hash, mtime, size, chunk_count, language)
  - `index_all_updates_index_state_totals` (total_files/total_chunks — §5.1 step 4)
  - `index_all_returns_indexstats_with_counts_and_duration`
  - `malformed_file_in_repo_does_not_abort_index` (D2 — degrade, continue)
- **GREEN:** implement §5.1 algorithm: discover → group by language → per file: hash, read,
  parse, chunk, `insert_chunks`, `update_file_hash`(+meta), accumulate `IndexStats`; update
  `index_state` totals. Wrap per-file writes so one bad file can't poison the batch.

### Slice M5.3 — incremental update + idempotency + delete
- **RED:**
  - `reindex_unchanged_repo_performs_no_writes` (idempotent — assert hashes/rows unchanged; ideally assert no `delete`/`insert` issued)
  - `modify_one_file_reindexes_only_that_file`
  - `update_files_with_n_changed_reindexes_exactly_n`
  - `deleted_file_has_chunks_removed_and_metadata_cleared`
  - `new_file_added_gets_indexed`
- **GREEN:** implement §5.2: per file compare `compute_file_hash` vs `get_file_hash`; skip if
  equal; else `delete_chunks_for_file` → re-parse → re-chunk → insert → update meta. For full
  `index` (incremental mode), reconcile deletions: files in `files_metadata` no longer on disk
  ⇒ delete their chunks + metadata row.

### Slice M5.4 — e2e init → index
- **RED:** `tests/e2e_index.rs`: create temp repo (fixtures) → `init` (config+db) → `index` →
  assert storage queryable and stats correct. All via public API (CLI wiring lands M7; here use
  library entry points).
- **GREEN:** thin glue exposing `init` (create `.codecache/`, config, schema) + `index`.

## API contracts / data structures (from `../project_plan.md` §3.2.4)
```rust
pub struct Indexer { /* parser, hasher, storage, config */ }
impl Indexer {
    pub fn new(config: Config, storage: Storage) -> Result<Self>;
    pub fn index_all(&mut self) -> Result<IndexStats>;
    pub fn update_files(&mut self, files: &[PathBuf]) -> Result<IndexStats>;
    fn discover_files(&self) -> Result<Vec<PathBuf>>;
    fn detect_changed_files(&self, files: &[PathBuf]) -> Result<Vec<PathBuf>>;
}
pub struct IndexStats { pub files_processed: usize, pub chunks_indexed: usize, pub duration_ms: u64 }
```
Persists via M1 `update_file_hash` extended with `file_size`/`chunk_count`/`language` (D6).

## Performance budgets (from `../project_plan.md` §5.4 / §1.3)
- **Cold index 10K LOC < 5s**, **100K LOC < 30s** (§5.4) — measured by M10 bench, but keep the
  hot path allocation-light here (batch inserts in transactions — §11.1).
- **Incremental update of 10 files < 2s** (§1.3, §5.4) — change detection must avoid re-parsing
  unchanged files; hashing 1K files < 500ms (M2) is the headroom.
- **Index size < 100MB** on Django-scale (§1.3) — don't duplicate `chunk_text` needlessly.
- Perf engineer wires an indexing bench skeleton here (full suite in M10).

## Decision Log bindings
- **D2:** indexing **never hard-fails** on malformed source — per-file errors are logged/counted
  and skipped (or chunked heuristically via M4), the batch continues. Tested in M5.2.
- **D3:** enrichment flows through unchanged (chunker fills it; storage persists it).

## Definition of Done (this phase)
- [ ] M5.1–M5.4 green incl. idempotent re-index (no-writes) + exact-N incremental + delete.
- [ ] Discovery honors `.gitignore` + config patterns + language filter.
- [ ] Malformed file does not abort a full index (D2).
- [ ] Indexing bench skeleton wired; budgets noted (full validation deferred to M10).
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 5 + `src/indexer/CLAUDE.md` updated.
