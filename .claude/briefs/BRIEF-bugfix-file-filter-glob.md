# BRIEF — bugfix / file-filter glob (D33)

- **Milestone:** post-M10 hardening (bugfix)  ·  **Module(s):** `retriever` (primary), `cli`, `mcp_server`
- **Owner (manager):** principal-engineering-manager  ·  **Created:** 2026-06-22
- **Status:** RED ✅  GREEN ✅  REVIEW ✅ (APPROVED)  DONE ✅ (uncommitted — main session commits)
- **Links:** docs/ROADMAP.md#D33 · docs/project_plan.md §3.2.3 / §6.1.1 / §7.2 / §8.2 · docs/TEST_STRATEGY.md#retriever · docs/TEST_STRATEGY.md#cli · docs/TEST_STRATEGY.md#mcp_server

## Goal
Make `--file-filter` / MCP `file_filter` perform **glob matching** against stored absolute
`chunk.file_path`s, as the `--help` and module docs already promise. Today it does exact `PathBuf`
equality, so **any** non-None value drops **all** results (verified: 12 results → 0 on a real Django
index for `*`, `*.py`, `*query*`, `<absdir>/**`).

## Root cause (confirmed)
- `src/retriever/mod.rs` `apply_file_filter` keeps a result only on exact equality
  `allowed.iter().any(|p| p == &r.chunk.file_path)`.
- `src/cli/query.rs:86` maps `--file-filter <s>` → `Some(vec![PathBuf::from(s)])` (literal path, never globbed).
- `src/mcp_server/handlers.rs:45-48` does the identical literal-`PathBuf` wrap for the `file_filter` arg.
- Stored paths are absolute ⇒ a user glob/relative fragment never *equals* an absolute path ⇒ empty.

## Scope (in / out)
- **In:** glob compile/match at the **retriever** layer (one code path for CLI + MCP, D4); D33
  anchoring (non-absolute ⇒ suffix-anchored `**/…`; absolute ⇒ as-is); typed `RetrieverError::InvalidFilter`
  on malformed glob; thread that error to a clean CLI nonzero exit + MCP `-32602`.
- **Out:** BM25/dedup/token-budget changes; schema changes; a `--no-default-ignore`-style new CLI
  flag; case-insensitive matching (globset default = case-sensitive, matches discovery).

## Design (ratified — D33)
- **Engine:** `globset` (the matcher crate already inside the present `ignore` stack used by
  `indexer/discovery.rs`). Add `globset` as a **direct** dep pinned to the version `ignore` already
  resolves — confirm the resolved version via `cargo tree -p globset` / `Cargo.lock` and pin to it so
  `Cargo.toml` stays lean and no new transitive family is added.
- **Anchoring rule (apply per pattern):** if the pattern string starts with `/`, use it verbatim;
  else prepend `**/`. Build a `GlobSet` from all patterns (OR semantics). Match against
  `chunk.file_path` (absolute). `globset`: `*` doesn't cross `/`, `**` does, case-sensitive.
- **Type:** `QueryOptions.file_filter` STAYS `Option<Vec<PathBuf>>` (raw patterns). Compile inside
  `Retriever::query` (or a helper it calls) so the MCP arg benefits too. Convert each `PathBuf` to a
  pattern string via `.to_string_lossy()` (paths here are user-typed UTF-8 globs, not real fs paths).
- **Error:** add `RetrieverError::InvalidFilter(String)` (or wrap globset's error) — `impl
  Display`/`Error`; `Retriever::query` returns `Err(InvalidFilter)` on a bad pattern. CLI maps it to
  an `anyhow` error (nonzero exit). MCP handler maps the retriever error to `-32602` (invalid params)
  for a malformed `file_filter` (it is a bad argument, not an internal failure) — confirm vs the
  existing `-32603` mapping and pick the spec-correct code (D33 says `-32602`).

## Scenarios to cover (from TEST_STRATEGY — retriever / cli / mcp_server)
Seed an index/storage with files across multiple dirs and extensions, e.g. (absolute paths):
`/repo/a/query.py`, `/repo/a/models.py`, `/repo/b/sub/query.go`, `/repo/c/util.ts`.

**Retriever unit/integration (`tests/retriever_tests.rs` + in-module where pure):**
- [ ] happy: `*.py` keeps exactly the `.py` files (all of them, regardless of dir).
- [ ] happy: `a/**` (or `*a*`) keeps only the `a/` subtree.
- [ ] happy: a basename glob `query.py` keeps exactly the one `query.py` (any dir), not `query.go`.
- [ ] happy: an **absolute** glob `/repo/a/**` keeps only that subtree.
- [ ] happy: multiple patterns OR together (`["*.py","*.go"]` keeps py+go, not ts).
- [ ] edge: a valid-but-unmatchable glob (`*.rs`) keeps **none** (legitimate empty, `Ok`).
- [ ] edge: `None` filter is unchanged (keeps all) — regression guard for existing behavior.
- [ ] error: a **malformed** glob (e.g. `a/[`) returns `Err(RetrieverError::InvalidFilter)` — NOT a
      silent empty `Ok`.
- [ ] regression: the existing M6.2 `file_filter` test must be migrated to glob semantics, NOT
      deleted — its current exact-absolute-path value should be re-expressed as a glob that still
      selects the same file (e.g. an absolute glob or basename glob). Do not weaken it to pass.

**CLI e2e (`tests/e2e_cli.rs`, assert_cmd):** at least one end-to-end:
- [ ] `query "<q>" --file-filter '*.py'` over a real init→index'd fixture keeps `.py` hits (was the
      0-results bug); and `--file-filter 'a/['` (malformed) exits **nonzero** with stderr, no panic.

**MCP (`tests/mcp_tests.rs`):**
- [ ] `tools/call codecache_search` with `file_filter` glob restricts results identically to the CLI.
- [ ] malformed `file_filter` → JSON-RPC error (`-32602`), not an internal `-32603` and not a panic.

## Definition of Done
- [ ] Tests written first, now green · `cargo clippy --all-targets -j 2 -- -D warnings` clean · `cargo fmt --check` clean
- [ ] API matches project_plan §3.2.3 / §6.1.1 · D33 honored (suffix-anchor; absolute as-is; typed error)
- [ ] No reachable `unwrap()/expect()/panic!`; `Cargo.toml` lean (only `globset` added, pinned to the `ignore`-resolved version)
- [ ] reviewer APPROVED
- [ ] docs/TODO.md + `src/retriever/CLAUDE.md` + `src/cli/CLAUDE.md` + `src/mcp_server/CLAUDE.md` updated
- [ ] `.gitignore` reconciled if any new artifact appears (not expected)

**WSL2 test-runner note (this machine):** run `cargo test --all -j 2 -- --test-threads=2` and
`cargo clippy --all-targets -j 2` to avoid a linker OOM at full parallelism.

---
## RED — test lead  (2026-06-22)

**Branch:** `fix/file-filter-glob` created from clean `main` (no commits; changes left uncommitted for the manager).

### Tests added / migrated

**`tests/retriever_tests.rs`** (added `RetrieverError` to the `use codecache::retriever::…` import):
- **MIGRATED (not deleted/weakened)** `file_filter_restricts_results_to_listed_files`: its M6.2 value
  `file_filter: Some(vec![PathBuf::from("src/keep.py")])` asserted EXACT-absolute-path equality. Under
  D33 the filter is a GLOB, so I re-expressed the value as the **basename glob `keep.py`** (suffix-anchored
  to `**/keep.py`), which selects the *same* `src/keep.py` and still excludes `src/drop.py`. The assertion
  is unchanged (`r.chunk.file_path == "src/keep.py"`), so the test's intent is preserved, now glob-expressed.
- New D33 block (helpers `filter_opts`, `seed_glob_corpus` — absolute `/repo/{a,b,c}` corpus across .py/.go/.ts,
  `result_paths`):
  - `file_filter_star_py_keeps_all_python_regardless_of_dir` — `*.py` keeps both `.py`, drops `.go`/`.ts`.
  - `file_filter_subtree_glob_keeps_only_that_subtree` — `a/**` keeps only the `a/` subtree.
  - `file_filter_basename_glob_keeps_that_file_in_any_dir` — `query.py` keeps `query.py`, not `query.go`/`models.py`.
  - `file_filter_absolute_glob_used_as_is_keeps_only_that_subtree` — absolute `/repo/a/**` (root-anchored, as-is).
  - `file_filter_multiple_patterns_or_together` — `["*.py","*.go"]` ORs to py+go, excludes ts.
  - `file_filter_valid_but_unmatchable_glob_keeps_none_ok` — `*.rs` → empty `Ok` (legitimate, not an error).
  - `file_filter_none_keeps_all_regression_guard` — `None` keeps all four (regression guard).
  - `file_filter_malformed_glob_returns_typed_invalid_filter_error` — `a/[` → `Err(RetrieverError::InvalidFilter(_))`,
    and Display contains the offending pattern.
  - `file_filter_one_bad_pattern_among_valid_still_errors` — one bad pattern in the OR-set (`["*.py","b/["]`) → InvalidFilter.

**`tests/e2e_cli.rs`** (built binary, assert_cmd; over the `enriched_module.py` → `module.py` fixture):
- `e2e_query_file_filter_glob_keeps_matching_python_hits` — `query hash_password --file-filter '*.py'` keeps the hit
  (`hash_password` + `module.py:` locator) — the exact 0-results bug.
- `e2e_query_malformed_file_filter_glob_exits_nonzero` — `--file-filter 'a/['` exits NONZERO, non-empty stderr, no panic.

**`tests/mcp_tests.rs`** (in-memory `serve` seam; reuses `test_server_seeded`/`tools_call`/`call_result_text`/`assert_error_code`):
- `call_codecache_search_file_filter_glob_restricts_results` — `file_filter:"*.py"` keeps the `.py` hit (`lookup_account`),
  drops the `.go` hit (`LookupRecord`/`src/store.go`).
- `call_codecache_search_malformed_file_filter_is_invalid_params` — `file_filter:"a/["` → JSON-RPC `-32602` (not `-32603`, not a panic).

### RED proof (`cargo test … -j 2 -- --test-threads=2`)

1. **`retriever_tests` — COMPILE-ERROR RED** (legitimate; `RetrieverError::InvalidFilter` doesn't exist yet):
   ```
   error[E0599]: no variant or associated item named `InvalidFilter` found for enum `RetrieverError`
      --> tests/retriever_tests.rs:841:39  (and :861:39)
   error: could not compile `codecache-rs` (test "retriever_tests") due to 2 previous errors
   ```
   (The behavior-level assertions in this target — `*.py`, `a/**`, OR-set, etc. — also fail until the
   retriever globs instead of comparing paths for equality; they can't run until the variant compiles.)

2. **`e2e_cli` — BEHAVIOR RED** (target compiles; fails on the bug):
   ```
   e2e_query_file_filter_glob_keeps_matching_python_hits:
     code=0  stdout="No results found.\n"   ← `*.py` dropped the hit (exact-equality bug)
   e2e_query_malformed_file_filter_glob_exits_nonzero:
     Unexpected success  code=0  stdout="No results found.\n"   ← malformed glob silently empty, exit 0
   test result: FAILED. 0 passed; 2 failed
   ```

3. **`mcp_tests` — BEHAVIOR RED** (target compiles; fails on the bug):
   ```
   call_codecache_search_file_filter_glob_restricts_results:
     got "Found 0 results"  ← `*.py` dropped the .py hit
   call_codecache_search_malformed_file_filter_is_invalid_params:
     got result {…"Found 0 results"…} instead of a -32602 error  ← silent-empty, not invalid-params
   test result: FAILED. 0 passed; 2 failed
   ```

### Production surface the engineering lead must add to go GREEN

- **`src/retriever/mod.rs`:**
  - Add enum variant **`RetrieverError::InvalidFilter(String)`** (carry the offending pattern; the tests
    assert `matches!(err, RetrieverError::InvalidFilter(_))` and that `Display` contains the bad pattern).
    Add its `Display` arm and an `error::Error` arm (no `source`, or a wrapped globset error — your call;
    tests only pin the variant + Display substring).
  - Replace `apply_file_filter`'s exact-equality keep with a **`globset`-compiled** match: build a
    `GlobSet` from `options.file_filter` **once per query**, applying the D33 anchoring per pattern
    (pattern NOT starting with `/` ⇒ prepend `**/`; absolute ⇒ as-is), and keep a result if its
    absolute `chunk.file_path` matches ANY glob. Convert each `PathBuf` to a pattern via
    `.to_string_lossy()`. A `Glob::new`/`GlobSetBuilder::build` error ⇒ return `Err(InvalidFilter(pattern))`.
    Because `apply_file_filter` must now be fallible, thread the `Result` up through `Retriever::query`
    (it already returns `Result`, so this is a `?`). `None` ⇒ unchanged (keep all).
- **`Cargo.toml`:** add `globset` as a **direct** dep pinned to **`0.4.18`** (the version `ignore` already
  resolves — confirmed via `Cargo.lock`; keeps the tree lean, no new transitive family).
- **`src/cli/query.rs`:** the CLI already wraps `--file-filter <s>` → `Some(vec![PathBuf::from(s)])`; the
  raw pattern now flows to the retriever which globs it. The new `InvalidFilter` error surfaces through
  the existing `.map_err(anyhow::Error::new)?` → clean nonzero exit (the e2e malformed test asserts that).
  Update the stale module-doc comment that says "exact-`PathBuf` post-filter / no glob expansion".
- **`src/mcp_server/handlers.rs` + `mod.rs`:** `handle_search` currently funnels ALL retriever errors to
  `-32603` via `run_query`. For a **malformed `file_filter`** the code must be **`-32602`** (invalid
  params — a bad argument). Map `RetrieverError::InvalidFilter` → `(-32602, msg)` while other retriever
  errors stay `-32603`. (Both the first heal-probe query and the final query run the filter; map both.)
- **Docs (same change, per repo rule):** `docs/TODO.md`, `src/retriever/CLAUDE.md`, `src/cli/CLAUDE.md`,
  `src/mcp_server/CLAUDE.md` to describe glob semantics + the new error/`-32602` mapping.

## GREEN — engineering lead  (2026-06-22)

**Status:** all RED tests now pass; clippy `-D warnings` clean; `cargo fmt --check` clean. No plan
deviation — implemented exactly per the brief/D33.

### What was implemented (per file)

- **`Cargo.toml`** — added `globset = "0.4.18"` as a **direct** dependency under `[dependencies]`,
  with a comment noting it is the matcher already inside the present `ignore` stack (D33). Confirmed
  via `Cargo.lock` (`name = "globset" / version = "0.4.18"`) — `ignore`'s already-resolved version,
  so no new transitive family is added. `Cargo.lock`'s `codecache-rs` deps list now includes
  `globset` directly.
- **`src/retriever/mod.rs`**
  - Added `RetrieverError::InvalidFilter(String)` carrying the offending pattern. `Display` arm:
    `"invalid file_filter glob pattern: {pattern}"` (contains the pattern — satisfies the Display
    substring assertion). `source()` returns `None` for `InvalidFilter` (no inner source); the
    `Storage` arm is unchanged.
  - Rewrote `apply_file_filter` to return `Result<Vec<SearchResult>>`. It builds a `globset::GlobSet`
    **once per query** from `options.file_filter`: per pattern, `to_string_lossy()` → D33 anchoring
    (leading `/` ⇒ verbatim, else prepend `**/`) → `GlobBuilder::new(&anchored)
    .literal_separator(true).build()` (so `*` does NOT cross `/`, `**` does). A glob-compile or
    `GlobSetBuilder::build()` failure ⇒ `Err(RetrieverError::InvalidFilter(<offending pattern>))`.
    Keeps a result iff `set.is_match(&r.chunk.file_path)` (any glob matches). `None` ⇒ unchanged
    (keep all). No `unwrap/expect/panic`.
  - `Retriever::query` now `?`s `apply_file_filter` (the method already returns `Result`).
- **`src/cli/query.rs`** — updated the stale module-doc + inline comment that claimed "exact-`PathBuf`
  post-filter / no glob expansion in v0.1" to describe the D33 glob behavior (suffix-anchor vs.
  absolute, `*`/`**` separator semantics, `InvalidFilter` → nonzero exit). Wiring unchanged — the raw
  pattern still flows through `file_filter.map(|f| vec![PathBuf::from(f)])` and `InvalidFilter`
  propagates via the existing `.map_err(anyhow::Error::new)?`.
- **`src/mcp_server/handlers.rs`** — `run_query` now maps `RetrieverError::InvalidFilter(_)` →
  `(-32602, "invalid file_filter: …")` while every other retriever error stays `-32603`. Imported
  `RetrieverError`. Both the self-heal probe query and the final query route through `run_query`, so
  a malformed `file_filter` surfaces as `-32602` from either. `mod.rs` needed no change (the error
  tuple flows through `handle_tools_call` → `error_response` verbatim).

### How each RED test now passes
- `retriever_tests` compile-RED (`InvalidFilter` missing) — variant added ⇒ compiles. Behavior:
  - `*.py` → `**/*.py` (literal_separator) keeps both `.py`, drops `.go`/`.ts`.
  - `a/**` → `**/a/**` keeps only the `a/` subtree.
  - `query.py` → `**/query.py` keeps the one `query.py`, not `query.go`/`models.py`.
  - absolute `/repo/a/**` used verbatim ⇒ only that subtree.
  - `["*.py","*.go"]` ORs to py+go, excludes ts.
  - `*.rs` ⇒ legitimate empty `Ok` (valid glob, no match).
  - `None` ⇒ all four kept (regression guard).
  - `a/[` ⇒ `Err(InvalidFilter("a/["))`; Display contains `a/[`.
  - `["*.py","b/["]` ⇒ `InvalidFilter` (any bad pattern fails the whole set).
  - **Migrated M6.2** `file_filter_restricts_results_to_listed_files`: value `keep.py` → `**/keep.py`
    selects `src/keep.py`, excludes `src/drop.py` — same assertion, glob-expressed. Passes.
- `e2e_cli`: `--file-filter '*.py'` keeps the `hash_password`/`module.py:` hit (the 0-results bug is
  gone); `--file-filter 'a/['` exits nonzero with non-empty stderr, no panic.
- `mcp_tests`: `file_filter:"*.py"` keeps the `.py` hit (`lookup_account`), drops the `.go` hit;
  `file_filter:"a/["` → `-32602`.

### Anchoring / separator subtlety resolved
The brief's "`*` doesn't cross `/`, `**` does" is NOT globset's default `Glob` behavior — it needs
`GlobBuilder::new(pat).literal_separator(true)`. Confirmed by the subtree (`a/**`), extension
(`*.py`), and basename (`query.py`) tests all passing with `literal_separator(true)`. The MCP/CLI
tests seed/store relative paths (`src/auth.py`, `module.py`); suffix-anchoring (`**/…`) makes the
basename/extension globs match them just as they would absolute discovery paths.

### Specialist/perf note
`GlobSet` is built once per query inside `apply_file_filter` (not per result) — no per-result
recompile; matching is a single pre-compiled pass over the hits.

### Green gate output (WSL2 `-j 2 --test-threads=2`)
- `cargo test --all -j 2 -- --test-threads=2` → **251 passed, 0 failed** across all suites
  (retriever 22, mcp 23, e2e_cli 8, lib unit 37, plus the rest). New file-filter tests all green.
- `cargo clippy --all-targets -j 2 -- -D warnings` → clean (exit 0).
- `cargo fmt --all -- --check` → clean (exit 0).

## Specialist / Perf notes
<globset/FTS5 interplay if engaged; matcher built once per query — confirm no per-result recompile>

## REVIEW — code reviewer
<APPROVE / BLOCK + findings: severity — file:line — problem — fix>

## OUTCOME — manager (2026-06-22)

**Slice DONE — aligned, reviewer-APPROVED, all gates green.** The `--file-filter` no-op bug is fixed:
the documented glob behavior is now actually built at the retriever layer, so one code path serves
both the CLI `--file-filter` and the MCP `codecache_search` `file_filter` arg (D4).

- **Plan-first honored.** `docs/project_plan.md` §3.2.3 + new §6.1.1 + §7.2 + §8.2 and ROADMAP **D33**
  were ratified BEFORE any code (the "change the plan before diverging" rule).
- **TDD honored.** RED tests first (test-lead), then minimum GREEN (eng-lead), no test weakened — the
  existing M6.2 `file_filter` test was **migrated** to a glob value preserving its original intent.
- **Reviewer gate:** BLOCK (same-change doc-sync) → manager fixed the live `tools.rs` schema +
  cli/retriever/mcp `CLAUDE.md` + `docs/TODO.md` (incl. reconciling the superseded 2026-06-17 entry) →
  re-review **APPROVE**. Gates: **251 tests pass / 0 fail**, clippy `-D warnings` clean, `fmt` clean.
- **Docs updated (same change):** `docs/TODO.md`, `src/retriever/CLAUDE.md`, `src/cli/CLAUDE.md`,
  `src/mcp_server/CLAUDE.md`, plus the agent-facing `src/mcp_server/tools.rs` schema description.
- **`.gitignore`:** no change needed — `globset` adds only a (tracked) `Cargo.lock` entry + ignored
  `target/` output; all three artifact classes already covered.
- **Follow-ups (non-blocking, not opened as tasks):** the reviewer's minor note — `Some(vec![])`
  (empty pattern set) builds an empty `GlobSet` ⇒ matches nothing ⇒ `Ok(empty)`; benign and
  unreachable from CLI/MCP (both build a single-element vec). Pin with a one-line test only if a
  future caller can pass an empty vec.
- **Handoff to main session:** changes are uncommitted on branch `fix/file-filter-glob` (off clean
  `main`); the main session owns the commit + PR.

---

### Verdict (code reviewer, 2026-06-22): **BLOCK**

Glob logic, error handling, plan-API alignment (§3.2.3/§6.1.1/D33), and all gates are correct —
clippy `-D warnings` clean, `cargo fmt --check` clean, **251 tests pass** (retriever file_filter
10/10 incl. the migrated M6.2 test; mcp 23; e2e_cli 8). The migrated M6.2 test preserves its
original intent (same assertion, glob-expressed value), the new tests pin D33 (not tautological),
and `globset` is pinned to `0.4.18` (the `ignore`-resolved version).

BLOCK is on the **same-change docs contract** (root CLAUDE.md golden rule + this brief's DoD): the
code now globs but three agent/dev-facing docs still say "exact path / no glob", one of which ships
to the agent at runtime.

- **blocker — src/mcp_server/tools.rs:39** — the live `tools/list` `file_filter` description still
  reads "restrict results to a single **exact** file path... Glob/wildcard patterns are **NOT
  expanded in v0.1**." This is the agent-facing contract (D13) and is now factually wrong: the agent
  is told globs don't work when they do. Plan §8.2 (project_plan.md:1572) was already updated to the
  D33 glob wording but the code schema (which `mcp_server/CLAUDE.md` requires to be "§8.2 verbatim")
  was not — `tools.rs` has drifted from §8.2. **Fix:** replace the description with the §8.2:1572
  glob text (no test pins the string, so add/adjust if you want a guard).
- **major — src/cli/CLAUDE.md:51-52** — still documents `--file-filter` as "single-entry
  exact-`PathBuf` post-filter (no glob expansion in v0.1)." **Fix:** rewrite to the D33 glob behavior
  (suffix-anchor vs absolute, `*`/`**`, `InvalidFilter`→nonzero exit), matching the updated query.rs
  doc-comment.
- **major — src/retriever/CLAUDE.md:70-71** — `file_filter` documented as a post-filter with "exact
  `PathBuf` match." **Fix:** rewrite to "glob post-filter (D33)" + the anchoring rule + the new
  `InvalidFilter` variant in the error list.
- **major — docs/TODO.md** — not updated for this slice (DoD + golden rule require it). **Fix:** add
  the D33 bugfix entry. (Note: the prior entry at TODO.md:596 — "file_filter overclaimed glob;
  corrected to exact-match, glob = v0.2" — is now superseded by D33 and should be reconciled, not
  left contradicting the new behavior.)
- **minor (note, not must-fix) — src/retriever/mod.rs apply_file_filter** — `Some(vec![])` builds an
  empty `GlobSet` ⇒ matches nothing ⇒ returns empty `Ok` (not all-pass, not error). Benign and
  unreachable from CLI/MCP (both only ever build a single-element vec), but undefined-by-test. Add a
  one-line test or a doc note if you want it pinned.

No correctness, error-mapping, idiomatic-Rust, or test-weakening findings. The MCP `-32602` mapping
is correct on both the heal-probe and final query (both route through `run_query`); the `(code,msg)`
tuple flows verbatim through `handle_line`→`error_response`. No reachable `unwrap/expect/panic` in
the changed code. Re-review needed only on the four doc fixes above (all docs, no source-logic
change), then APPROVE.

### Re-review verdict (code reviewer, 2026-06-22): **APPROVE**

All four doc-sync findings from the prior BLOCK are resolved; the source logic + gates I already
approved still hold (spot-checked the one changed source file, `tools.rs`, + the unchanged glob
core in `retriever/mod.rs` and the `-32602` mapping in `handlers.rs`).

1. **blocker (tools.rs:40) — FIXED.** The live `tools/list` `file_filter` description now reads the
   §8.2 glob wording **verbatim** — byte-for-byte identical to `docs/project_plan.md:1572`
   ("restrict results to files matching a glob (D33)... suffix-anchored... absolute used as-is... A
   malformed glob is a clean error, not a silent empty result."). The `search_tool()` doc-comment
   (tools.rs:20-21) was also updated to the glob summary, so the module's "§8.2 verbatim" invariant
   holds again. Confirmed **no `mcp_tests.rs` test pins the description string** (the schema tests at
   lines 588-600 / 648-662 assert only `type`/`default`/`required` + a non-empty `description`); the
   shape tests pass (mcp 23/23 green).
2. **major (src/cli/CLAUDE.md) — FIXED.** Now describes the D33 glob post-filter (suffix-anchor vs
   absolute, `*`/`**` separator semantics) and the `RetrieverError::InvalidFilter` → clean nonzero
   exit, plus a D33 GREEN status entry. No "exact-PathBuf" claim remains.
3. **major (src/retriever/CLAUDE.md) — FIXED.** The `file_filter` bullet is rewritten to "glob
   post-filter (D33)" with the full anchoring rule, OR-over-set, build-once-per-query, malformed →
   `InvalidFilter`; the new `InvalidFilter(String)` variant is listed in the error section; a D33
   status entry was added. No "exact PathBuf match" claim remains.
4. **major (docs/TODO.md) — FIXED.** A full D33 entry was added (line ~635) AND the superseded prior
   entry (line ~596) was reconciled — it now reads "SUPERSEDED 2026-06-22 by D33" and explains the
   documented glob behavior was actually built, no longer contradicting the shipped behavior.

The minor note (empty-vec `GlobSet` matches nothing) remains benign/unreachable from CLI+MCP — not a
blocker, left as-is by design.

**Gates re-run (WSL2 `-j 2 --test-threads=2`):**
- `cargo test --all -j 2 -- --test-threads=2` → **251 passed / 0 failed** (lib 37, mcp 23,
  retriever 22, e2e_cli 8, + all others; tally verified = 251).
- `cargo clippy --all-targets -j 2 -- -D warnings` → clean (exit 0).
- `cargo fmt --all -- --check` → clean (exit 0).

`globset = "0.4.18"` confirmed added as a direct dep pinned to the `ignore`-resolved version with a
justifying comment; Cargo.lock shows it promoted to a direct dep only — no new transitive family.

No new correctness/error-mapping/idiomatic-Rust/test-weakening findings on re-review. **APPROVE.**
