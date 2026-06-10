# BRIEF — M5 / indexer (M5.1–M5.4)

- **Milestone:** M5 — indexer (discovery → parse → chunk → hash → store; incremental)  ·  **Module(s):** `indexer` (+ thin `init`/`index` glue)
- **Owner (manager):** principal-engineering-manager  ·  **Created:** 2026-06-10
- **Status:** RED ▢  GREEN ▢  REVIEW ▢  DONE ▢
- **Links:** docs/plans/M5-indexer.md · docs/ROADMAP.md#m5--indexer · docs/TEST_STRATEGY.md#indexer · docs/project_plan.md §3.2.4 / §5.1 / §5.2 · docs/TODO.md Phase 5

## Goal
Wire the four leaf modules (storage M1, hasher M2, parser M3, chunker M4) plus `config` into a
working `Indexer` facade that: discovers source files (honoring `.gitignore` + config ignore
patterns + the configured language set), performs a correct full index of a fixture repo,
supports incremental updates that are **idempotent** (re-index of unchanged input issues no
writes), re-indexes exactly the files that changed, removes chunks for deleted files, and is
reachable end-to-end through `init → index` on a public library surface. This is the first
**integration** milestone — no new leaf algorithms, only orchestration.

## Scope (in / out)
- **In:**
  - `src/indexer/mod.rs` — `Indexer` facade per §3.2.4: `new`, `index_all`, `update_files`, plus
    private `discover_files`, `detect_changed_files`. Returns `IndexStats { files_processed,
    chunks_indexed, duration_ms }`.
  - `src/indexer/discovery.rs` — `discover_files()` via `ignore::WalkBuilder`; `detect_language(path)`
    by extension; honor config `ignore_patterns`; restrict to `config.languages` (§5.1).
  - `src/indexer/pipeline.rs` — per-file parse→chunk→hash→store orchestration + change detection;
    per-file error isolation (D2 degrade-and-continue); deletion reconciliation against
    `files_metadata`.
  - Thin `init` + `index` library entry points (create `.codecache/`, write config, `init_schema`)
    for the M5.4 e2e — **library-level only**.
  - `IndexStats` type (here unless §3.2.4 places it elsewhere — match the plan).
  - Indexing **bench skeleton** (perf engineer): a cold-index micro-bench wired but not gated; full
    validation deferred to M10.
  - Address the **M4 chunker cross-reference re-walk** perf follow-up while wiring M5.2 (see
    Follow-ups below) — single-pass bucketing of `call` nodes.
- **Out (defer):**
  - CLI command surface / `clap` wiring → **M7** (M5.4 uses library entry points, not the binary).
  - TypeScript + Go discovery/parsing correctness → **M9** (discovery may *detect* `.ts`/`.go`, but
    fixtures that get indexed are **Python-only**; language filter tests may use `.ts`/`.go` files
    only to assert they are *skipped/grouped*, never parsed).
  - BM25 retrieval/formatter → M6/M7.
  - Full perf-budget validation (cold 10K<5s / 100K<30s / incr 10 files<2s / index<100MB) → **M10**.

## Scenarios to cover (from docs/TEST_STRATEGY.md#indexer + plan §Ordered slices)

### Slice M5.1 — discovery + language detection  (`tests/indexer_tests.rs`, fixtures)
- [ ] happy: `language_detected_from_extension` (.py→Python, .ts→TypeScript, .go→Go)
- [ ] happy: `discovery_only_returns_configured_languages` (languages=[Python] ⇒ `.ts`/`.go` skipped)
- [ ] edge: `discovery_respects_gitignore` (a `.gitignore`d path is not returned)
- [ ] edge: `discovery_respects_extra_ignore_patterns_from_config`
- [ ] edge: `non_source_files_skipped` (e.g. `.md`, `.txt`, binaries)

### Slice M5.2 — full index (`index_all`)  (`tests/indexer_tests.rs`)
- [ ] happy: `index_all_populates_storage_with_expected_chunk_count`
- [ ] happy: `index_all_writes_files_metadata_for_each_file` (content_hash, mtime, file_size, language, chunk_count)
- [ ] happy: `index_all_updates_index_state_totals` (total_files / total_chunks — §5.1 step 4)
- [ ] happy: `index_all_returns_indexstats_with_counts_and_duration`
- [ ] error/D2: `malformed_file_in_repo_does_not_abort_index` (degrade, count/skip, batch continues)

### Slice M5.3 — incremental + idempotency + delete  (`tests/indexer_tests.rs`)
- [ ] happy(idempotent): `reindex_unchanged_repo_performs_no_writes` (hashes/rows unchanged; assert no delete/insert issued)
- [ ] happy: `modify_one_file_reindexes_only_that_file`
- [ ] happy: `update_files_with_n_changed_reindexes_exactly_n`
- [ ] happy: `new_file_added_gets_indexed`
- [ ] edge: `deleted_file_has_chunks_removed_and_metadata_cleared`

### Slice M5.4 — e2e init → index  (`tests/e2e_index.rs`, `tests/fixtures/repo/**`)
- [ ] e2e: `init` creates `.codecache/` (config + schema); `index` populates a queryable DB; `IndexStats` correct — all via public library entry points.

## Definition of Done
- [ ] M5.1–M5.4 green: idempotent re-index (no writes) + exact-N incremental + delete + e2e.
- [ ] Discovery honors `.gitignore` + config `ignore_patterns` + language filter.
- [ ] Malformed file does not abort a full index (D2); per-file errors counted/logged, batch continues.
- [ ] Indexing bench skeleton wired; perf budgets noted (full validation deferred to M10).
- [ ] M4 chunker cross-reference re-walk converted to single-pass bucketing; no M4/M5 budget regressed.
- [ ] `is_heuristic` persistence seam: decision recorded (see below) and honored in code.
- [ ] API matches project_plan §3.2.4 (`Indexer`, `IndexStats`) + §5.1/§5.2 algorithms.
- [ ] `cargo clippy --all-targets -- -D warnings` clean · `cargo fmt --all -- --check` clean · `cargo test --all` green.
- [ ] code-reviewer APPROVED.
- [ ] docs/TODO.md Phase 5 + `src/indexer/CLAUDE.md` updated in the same change.

---

## Execution sequence (for the runner / main session)

Drive one slice at a time, RED → GREEN → (perf) → REVIEW → manager-verify. Each agent **appends
to this brief** before handing off. Gate commands are identical to CI and the Stop hook.

**Per-slice gate commands (run in order; all must pass before the slice is "green"):**
```
cargo build
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo fmt --all -- --check
```

### M5.1 — discovery + language detection
1. **principal-test-engineering-lead** — write the 5 RED tests + minimal fixtures
   (`tests/fixtures/repo/**`: a few `.py`, a `.ts`/`.go` to be skipped, a `.gitignore`, a `.md`).
   Append RED section (failing output). Tests must compile-fail/assert-fail, not error spuriously.
2. **principal-engineering-lead** — implement `src/indexer/discovery.rs` (`WalkBuilder` honoring
   `.gitignore`; apply config `ignore_patterns`; `detect_language` by extension; group/filter by
   `config.languages`). Route any `ignore`-crate gitignore-semantics questions to
   **rust-treesitter-specialist** only if needed (low risk here). Run gates → green. Append GREEN.
3. **code-reviewer** — APPROVE/BLOCK. Manager verifies, then proceed.

### M5.2 — full index (`index_all`)
1. **principal-test-engineering-lead** — 5 RED tests incl. D2 malformed-file. Append RED.
2. **principal-engineering-lead** — implement §5.1 in `pipeline.rs` + `index_all` in `mod.rs`:
   discover → group by language → per file {hash, read, parse, chunk, `insert_chunks`,
   `update_file_hash(&FileMeta)`} → accumulate `IndexStats` → `set_index_state` totals. Wrap each
   file's work so one failure is counted and skipped (D2), never aborting the batch. **While here**,
   apply the M4 cross-reference re-walk fix (single-pass bucket of `call` nodes) in `chunker`.
3. **performance-bench-engineer** — add the cold-index bench skeleton; record a baseline number vs
   the §5.4 budget (informational at M5). Append Perf notes.
4. **code-reviewer** → manager-verify → proceed.

### M5.3 — incremental + idempotency + delete
1. **principal-test-engineering-lead** — 5 RED tests; the idempotency test should assert **no
   writes** (e.g. via row/hash invariance, ideally a spy/counter on delete/insert). Append RED.
2. **principal-engineering-lead** — implement §5.2: `detect_changed_files` compares
   `compute_file_hash` vs `get_file_hash`, skip on equal; else `delete_chunks_for_file` → re-parse →
   re-chunk → `insert_chunks` → `update_file_hash`. `update_files` handles an explicit list;
   `index_all` (incremental/reconcile mode) deletes chunks+metadata for files in `files_metadata`
   no longer on disk. Append GREEN.
3. **code-reviewer** → manager-verify → proceed.

### M5.4 — e2e init → index
1. **principal-test-engineering-lead** — `tests/e2e_index.rs`: temp repo from fixtures → `init`
   (create `.codecache/`, config, schema) → `index` → assert DB queryable + stats. Public library
   surface only. Append RED.
2. **principal-engineering-lead** — thin `init`/`index` glue. Append GREEN.
3. **code-reviewer** → manager-verify.

### Closeout (manager)
- Verify full DoD; update `docs/TODO.md` Phase 5 (check boxes, record GREEN summary + gate
  versions) and `src/indexer/CLAUDE.md` (shipped API). Update `.gitignore` if M5 introduced new
  local artifacts (temp test repos go to `target/`/`tempdir`; only add patterns if anything lands
  in-tree). Engage **devops-release-engineer** only if CI gates need to mirror a new test target
  (new integration test files are auto-discovered, so usually no CI change).

### Commit-boundary recommendation
**One commit per slice (4 commits for M5).** Justification:
- M5 is four independently green, independently reviewable increments with clear seams
  (discovery / full / incremental / e2e); per-slice commits preserve the RED→GREEN→review history
  the DoD requires and keep each diff small for the reviewer and for `git bisect`.
- Each slice leaves the tree fully green (all four gates pass), so every commit is a safe landing
  point — consistent with how M1 landed as a coherent unit but M5 has more internal surface.
- The M4 cross-reference perf fix rides in the **M5.2** commit (it is wired alongside `index_all`),
  with its own line in the commit body referencing the M4 follow-up.
- Suggested messages: `M5.1: indexer discovery + language detection`, `M5.2: indexer full index
  (index_all) + chunker single-pass cross-refs`, `M5.3: indexer incremental + delete (idempotent)`,
  `M5.4: e2e init → index`. (If the runner prefers a single `M5: indexer` commit to match prior
  milestone granularity, that is acceptable — but per-slice is recommended.)

## Pre-logged follow-ups carried into M5

### (a) M4 perf follow-up — chunker cross-reference re-walk
`src/chunker/mod.rs::call_names_in_span` re-walks the whole tree **per chunk**, giving
O(chunks × tree_nodes) cross-reference enrichment — a deviation from M4's "single-pass, no
per-chunk re-query" budget (correctness unaffected; no M4 budget breached, so it was logged not
blocked). **Action:** address it in the **M5.2** slice while wiring the pipeline, because that is
where the chunker sits on the cold-index hot path and where the §5.4 budget first applies. Replace
the per-chunk re-walk with a **single walk that buckets all `call` nodes by containing chunk span**
(O(nodes + chunks·log)). `performance-bench-engineer` validates against the §5.4 cold-index budget
using the M5.2 bench skeleton. Keep the chunker's public `chunk()` signature and observable output
(deduped, first-seen `cross_references`) unchanged — this is an internal optimization, so existing
M4 chunker tests must stay green and gate the refactor.

### (b) `is_heuristic` storage-persistence seam — DECISION: **defer to M7, do not persist in M5**
**Context:** the M1 `symbols` schema has no `is_heuristic` column; `storage`'s row→`Chunk` path
reconstructs `is_heuristic: false` (see `src/chunker/CLAUDE.md` and TODO Phase 4). The flag is set
truthfully on the chunker output but is lost on round-trip through storage.
**Decision (manager):** **Defer persistence to M7; M5 does not add the column or migrate the
schema.** Rationale:
- M5's DoD and TEST_STRATEGY#indexer have **no scenario** that observes `is_heuristic` after a
  storage round-trip; nothing in the M5 pipeline branches on it. Adding it now would be untested
  production surface (violates TDD) and an un-driven schema migration.
- The first consumer that actually *surfaces* the flag is the **M7 formatter** (output may mark
  heuristic snippets) / CLI. Persisting it should be driven by an M7 RED test that reads it back.
- The indexer still **passes the chunker's `is_heuristic` through in-memory** to `insert_chunks`;
  only the *stored* representation drops it (unchanged from M4). No behavior regresses.
- **Carry-forward:** when M7 needs it, add an UNINDEXED `is_heuristic` column to `symbols` +
  `index_state.version` migration (storage owns the migration), driven by a failing formatter/CLI
  test. This is recorded here and in TODO Phase 5 so the seam is not forgotten.

---
## RED — test lead

### M5.1 — discovery + language detection (2026-06-10)

**Tests added** (`tests/indexer_tests.rs`, new file; repos built at runtime via `tempfile::TempDir`
— no committed fixture tree, `.gitignore` is created in-test):
1. `language_detected_from_extension` — `.py`→Python, `.ts`→TypeScript, `.go`→Go; `README.md` and
   extension-less `Makefile` → `None`.
2. `discovery_only_returns_configured_languages` — `languages=[Python]`, repo `{a.py, b.ts, c.go}`
   ⇒ only `a.py` returned.
3. `discovery_respects_gitignore` — `.gitignore` containing `ignored.py` ⇒ `ignored.py` excluded,
   `kept.py` returned.
4. `discovery_respects_extra_ignore_patterns_from_config` — `ignore_patterns=["*_generated.py",
   "vendor/**"]` ⇒ `schema_generated.py` and `vendor/dep.py` excluded, only `keep.py` returned
   (asserted on root-relative paths, forward-slash normalized).
5. `non_source_files_skipped` — `.md`, `.txt`, extension-less `LICENSE` excluded; only `code.py`
   returned.

All assertions sort results before comparing (discovery order is filesystem-dependent → determinism).

**Public signatures the engineering lead must implement** (decision: free functions in the
`indexer` module = the plan's "discovery.rs" split, promoted `pub` for integration-test reach.
This is the recommended option from the task brief; `Indexer::discover_files` is NOT used by these
tests):
```rust
// in src/indexer/discovery.rs, re-exported from src/indexer/mod.rs as `pub use`:
pub fn detect_language(path: &Path) -> Option<Language>;
pub fn discover_files(config: &Config, root: &Path) -> Result<Vec<PathBuf>, IndexError>;
```
- The tests import them as `codecache::indexer::{detect_language, discover_files}`, so they must be
  reachable at the `indexer` module root (re-export from `mod.rs`).
- `discover_files` returns `Result<Vec<PathBuf>, E>` where `E` is the module's error type. The tests
  only call `.expect(...)` on success, so any `E: Debug` works; the brief/plan name it `IndexError`
  — define it (even a minimal stub) so the signature matches.
- Returned `PathBuf`s must be absolute or root-prefixed (the tests `strip_prefix(root)` and fall
  back to the full path, and also read `file_name()`), so returning paths joined under `root` is
  required.

**Root / `index_paths` default decision (confirm in impl):** discovery walks `config.index_paths`
resolved against `root`; **when `index_paths` is empty (as in `Config::default()`), default to
walking `root` itself.** All five tests rely on this default (they leave `index_paths` empty and
pass the tempdir as `root`).

**Behavior the impl must satisfy (from the assertions):**
- Extension → language map is total over the three v0.1 languages; everything else → `None`.
- Discovery restricts to `config.languages` (filters out non-configured-language source files).
- `.gitignore` is honored (use `ignore::WalkBuilder`).
- `config.ignore_patterns` is applied on top of `.gitignore` (glob semantics: `*_generated.py`
  matches a filename; `vendor/**` matches everything under a directory).
- Non-source files (`.md`/`.txt`/extension-less) never appear in results.

**RED output** (`cargo test --all --test indexer_tests`, PATH-prefixed with `$HOME/.cargo/bin`):
```
   Compiling codecache v0.1.0 (C:\Users\ehlee\workspace\projects\CodeCache)
error[E0432]: unresolved imports `codecache::indexer::detect_language`, `codecache::indexer::discover_files`
  --> tests\indexer_tests.rs:27:26
   |
27 | use codecache::indexer::{detect_language, discover_files};
   |                          ^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^ no `discover_files` in `indexer`
   |                          |
   |                          no `detect_language` in `indexer`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `codecache` (test "indexer_tests") due to 1 previous error
```
Fails for the right reason: the `indexer` stub does not yet expose the discovery API. No spurious
warnings (the unused-`PathBuf` import was trimmed so `-D warnings` stays clean once green).
Hand off to **principal-engineering-lead**.

<original placeholder removed>

## GREEN — engineering lead

### M5.1 — discovery + language detection (2026-06-10)

**Implemented** (matches the RED-pinned signatures exactly; no plan deviation):
- `src/indexer/discovery.rs`:
  - `pub fn detect_language(path: &Path) -> Option<Language>` — extension match `.py`→Python,
    `.ts`→TypeScript, `.go`→Go; everything else (incl. extension-less) → `None`. Minimal, only the
    three v0.1 languages the tests assert (no `.tsx`/`.jsx`/etc.).
  - `pub fn discover_files(config: &Config, root: &Path) -> Result<Vec<PathBuf>, IndexError>` —
    walks each `config.index_paths` entry joined under `root`, defaulting to **`root` itself when
    `index_paths` is empty** (the confirmed default; all 5 tests rely on it). Returned paths are
    full paths joined under `root`, so the tests' `strip_prefix(root)` and `file_name()` both work.
  - Private helpers `resolve_walk_roots`, `build_ignore_patterns`, `is_configured_language` keep
    `discover_files` small.
- `src/indexer/mod.rs`: `mod discovery; pub use discovery::{detect_language, discover_files};` plus
  the typed `IndexError` enum (`Io { path, source }`, `Glob { pattern, source }`) following the
  `ConfigError`/`HasherError` style — `impl Display + std::error::Error` with `source()`, no
  reachable `unwrap()/expect()/panic!`. `lib.rs` already declared `pub mod indexer;` (verified, no
  change needed).

**Gitignore / glob approach chosen:**
- `.gitignore` honored via `ignore::WalkBuilder` with **`.require_git(false)`**. This was the one
  non-obvious bit: `WalkBuilder` defaults to `require_git(true)`, which only applies `.gitignore`
  rules *inside a git repo*. The discovery tests build a bare `tempfile::TempDir` (no `.git`), so
  without `require_git(false)` the `.gitignore` was silently ignored (`discovery_respects_gitignore`
  failed: `ignored.py` leaked through). `require_git(false)` is also the correct production
  semantics — the indexer indexes plain source trees, not only checkouts.
- `config.ignore_patterns` applied as **gitignore-style globs** via a separate
  `ignore::gitignore::GitignoreBuilder` anchored at `root`, matched with
  `matched_path_or_any_parents(path, false).is_ignore()`. Chose this over `OverrideBuilder` because
  Override's whitelist-by-default inversion (a plain glob *whitelists*, you must negate to ignore)
  is the opposite of the intended "these patterns are extra ignores" semantics and reads
  confusingly. A `Gitignore` matcher treats a plain glob as an *ignore* (matching the user's mental
  model and the `.gitignore` file), so `vendor/**` and `*_generated.py` Just Work — and
  `matched_path_or_any_parents` covers the `vendor/dep.py`-under-an-ignored-dir case. No new
  dependency (only the `ignore` crate, already present).

**Seam notes for M5.2+:**
- `detect_language` / `discover_files` are `pub` free functions at the `indexer` root. When the
  `Indexer` facade lands (M5.2), `index_all` can call `discover_files` directly; the brief's
  §3.2.4 `Indexer::discover_files` private method is not required by these tests and can wrap or
  delegate to the free function as preferred.
- `IndexError` currently has `Io` + `Glob`; M5.2 will likely add variants for parse/chunk/store
  per-file failures (D2 degrade-and-continue) — extend the enum, keep the `source()` chain.
- Returned paths are absolute-under-`root` `PathBuf`s; downstream hashing/parsing can use them
  directly. If M5.2 wants root-relative storage keys it should `strip_prefix(root)` at the storage
  boundary (consistent with how the tests normalize).

**Gate output (PATH-prefixed with `$HOME/.cargo/bin`, all four green):**
```
cargo build                                  → Finished (clean)
cargo clippy --all-targets -- -D warnings    → Finished (no warnings)
cargo test --all                             → all green; 5/5 indexer_tests pass
cargo fmt --all -- --check                   → clean (exit 0)
```
`tests/indexer_tests.rs`: 5 passed / 0 failed. Whole suite: **81 tests** across all targets
(lib 14, chunker_proptest 3, chunker 10, config 5, hasher 11, **indexer 5**, parser 14, smoke 1,
storage 18; main 0, doctests 0) — up from 76 by exactly the 5 new M5.1 tests.

Hand off to **code-reviewer**.

## Specialist / Perf notes
<ignore-crate gitignore edge cases if engaged; chunker single-pass cross-ref bench numbers vs §5.4 budget>

## REVIEW — code reviewer
<APPROVE / BLOCK + findings: severity — file:line — problem — fix>

## OUTCOME — manager
<aligned? TODO updated? slice marked done? follow-ups created?>

### M5.1 — discovery + language detection (2026-06-10) — **APPROVE**

Reviewed: `src/indexer/discovery.rs`, `src/indexer/mod.rs`, `tests/indexer_tests.rs`,
`src/indexer/CLAUDE.md`. Re-ran indexer tests (5/5), clippy `--all-targets -D warnings` (clean),
`fmt --check` (clean) — all green.

**Verdict: APPROVE.** Correct, idiomatic, aligned; no blockers, no majors.

Correctness confirmed:
- `.require_git(false)` is sound and correct production semantics — without it `.gitignore` is
  silently inert outside a checkout; gitignore test genuinely exercises gitignore (separate
  `.gitignore` file + `kept.py`/`ignored.py`, not the config-pattern path).
- `config.ignore_patterns` via an anchored `Gitignore` + `matched_path_or_any_parents(path,false)`
  gives the intended "extra ignores" semantics (plain glob = ignore), and the parent-walk correctly
  excludes `vendor/dep.py` under `vendor/**`. Override-vs-Gitignore decision is the right call.
- Language filter, empty-`index_paths`→walk-`root` default, file-type gate, and
  absolute-under-`root` paths all match the RED contract and §5.1 pseudocode (free-fn shape).
- `IndexError` is typed, `impl Display + Error` with a correct `source()` chain; no reachable
  `unwrap()/expect()/panic!` in production. `?`/`map_err` throughout.

Tests: deterministic (all sorted), assertions meaningful (exact-vec equality, not `is_ok()`),
gitignore vs config-pattern paths exercised independently. No scope creep into M5.2.

Nits (non-blocking, optional — do NOT fix this slice):
- minor — `src/indexer/discovery.rs:62-67` — config `ignore_patterns` filter runs per-file and does
  not prune directories, so the walker still descends ignored trees (e.g. `vendor/`). Correct, but
  loses gitignore-style pruning; if M5.2 perf wants it, feed patterns into the `WalkBuilder` overrides
  instead. Out of scope for M5.1.
- minor — `discovery.rs:62` vs `:65` — language filter precedes the ignore-pattern check; order is
  immaterial to results (both must pass) and arguably cheaper as-is. No action.
- minor — `detect_language` is intentionally `.py/.ts/.go`-only (no `.tsx/.jsx/.pyi`); correct per
  M5.1/M9 scope. Note only.

Slice M5.1 is DONE-eligible. Hand back to manager.
