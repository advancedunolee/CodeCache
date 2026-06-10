# src/indexer/ — CLAUDE.md

**Module:** `indexer` · **Owner:** `principal-engineering-lead` · **Milestone:** M5 (stub at M0).

## Purpose
Orchestrate the indexing pipeline: file discovery (honoring `.gitignore` + extra ignore
patterns) → parse → chunk → hash → store. Incremental: only changed files re-indexed; deleted
files' chunks removed; re-index of unchanged input is a no-op (idempotent).

## API anchor
`docs/project_plan.md` §3.2.4 (`Indexer`, `IndexStats`) + §5.1/§5.2 (algorithms).

## Tests / scenarios
`docs/TEST_STRATEGY.md#indexer` — discovery honors ignores; full index populates storage;
incremental idempotency; modify N ⇒ exactly N re-indexed; delete removes chunks.

## Shipped API (M5.1 — discovery + language detection)
- `discovery.rs` (re-exported from `mod.rs`):
  - `detect_language(path: &Path) -> Option<Language>` — by extension (`.py`/`.ts`/`.go`); all
    else → `None`.
  - `discover_files(config: &Config, root: &Path) -> Result<Vec<PathBuf>, IndexError>` — walks
    `config.index_paths` joined under `root` (empty ⇒ walk `root`) via `ignore::WalkBuilder`,
    `.require_git(false)` so `.gitignore` is honored outside a checkout. `config.ignore_patterns`
    applied as gitignore-style globs (`ignore::gitignore::GitignoreBuilder` anchored at `root`,
    `matched_path_or_any_parents`). Results restricted to files whose `detect_language` ∈
    `config.languages`. Paths returned are absolute-under-`root`.
- `mod.rs` — `IndexError { Io { path, source }, Glob { pattern, source } }`, typed (`impl
  std::error::Error` + `source()`), no reachable `unwrap()/expect()/panic!`.

## Planned layout (M5.2+)
- `mod.rs` — `Indexer` facade (§3.2.4): `new`, `index_all`, `update_files`, private
  `detect_changed_files`; `IndexStats { files_processed, chunks_indexed, duration_ms }`.
- `pipeline.rs` — per-file parse→chunk→hash→store + change detection (§5.2); per-file error
  isolation (D2: degrade-and-continue); deletion reconciliation vs `files_metadata`.
- Slices M5.1–M5.4 + execution sequence: [`.claude/briefs/BRIEF-M5-indexer.md`](../../.claude/briefs/BRIEF-M5-indexer.md).

## Decisions / seams
- **`is_heuristic` persistence — deferred to M7.** M5 passes the chunker's `is_heuristic` through
  in-memory to `insert_chunks`, but the M1 `symbols` schema has no column for it, so the stored
  representation drops it (round-trip reconstructs `false`, unchanged from M4). No M5 scenario
  observes it; persistence is driven by an M7 formatter/CLI RED test (storage adds an UNINDEXED
  column + version migration). See BRIEF §Follow-ups (b).
- **M4 cross-ref re-walk fix** rides in M5.2: replace `chunker::call_names_in_span`'s per-chunk
  whole-tree walk with single-pass bucketing of `call` nodes by chunk span.

## Status
**M5.1: GREEN (2026-06-10)** — discovery + language detection shipped; 5/5 `indexer_tests` pass,
all four gates clean (Rust 1.85). M5.2–M5.4 (full/incremental index + e2e) pending. Brief:
[`.claude/briefs/BRIEF-M5-indexer.md`](../../.claude/briefs/BRIEF-M5-indexer.md).
