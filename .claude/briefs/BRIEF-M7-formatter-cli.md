# BRIEF — M7 / formatter + cli

- **Milestone:** M7 — formatter + cli  ·  **Module(s):** `formatter`, `cli`, `main.rs`
- **Owner (manager):** principal-engineering-manager  ·  **Created:** 2026-06-12
- **Status (per slice):** M7.1 RED ▢ GREEN ▢ REVIEW ▢ DONE ▢ · M7.2 ▢ · M7.3 ▢ · M7.4 ▢
- **Links:** docs/plans/M7-formatter-cli.md · docs/ROADMAP.md#m7--formatter--cli ·
  docs/TEST_STRATEGY.md#formatter / #cli · project_plan.md §6.4 (formats) / §7 (CLI) / §8.2 (D13 ordering)
- **Routing:** manager → test-lead (RED) → engineering-lead (GREEN; route any FTS5/skeleton-line
  depth to rust-treesitter-specialist) → code-reviewer → manager. Perf is *not* gated here (format
  budget §11.2 is string-only; no per-chunk file reads — D7 verified, see below). devops keeps CI
  green + mirrors the two new dev-deps.

## Goal
Serialize `QueryResult` to TOON / JSON / text (§6.4) with **agent-first ordering (D13)**, then wire
the `clap` CLI for all §7 commands so the whole pipeline (init → index → query) is usable from the
built binary. Formatter is pure (`QueryResult` → `String`); CLI is one adapter (D4). One TDD cycle +
one commit per slice, RED → GREEN → refactor → review.

## Verified facts the implementers can rely on (manager, 2026-06-12)
- **D7 line-number seam is REAL and fully wired — read stored line numbers, never re-read source.**
  `SearchResult.chunk.start_line` / `.end_line` are 1-based inclusive and populated by both chunker
  paths (AST: `parser/mod.rs:309-310`; heuristic: `chunker/mod.rs:256` via `line_range`), persisted
  UNINDEXED (`storage/schema.rs:38-39`), and reconstructed by `storage::build_search_result`. The
  TOON/text `file:start-end` ranges come straight off the chunk. **No file reads at format time.**
- **Retriever surface to format (from `src/retriever/mod.rs`, already shipped/green):**
  - `QueryResult { chunks: Vec<storage::SearchResult>, total_tokens: usize, total_results_found: usize }`
  - `storage::SearchResult { chunk: types::Chunk, bm25_score: f64 }` (bm25 lower = better; ranking
    already best-first + deduped + token-packed by `Retriever::query`).
  - `types::Chunk` fields the formatter needs: `symbol_name`, `symbol_type` (`as_str()` →
    `function|class|method|struct`), `file_path: PathBuf`, `start_byte`, `end_byte`, `start_line`,
    `end_line`, `chunk_text`, `language` (`as_str()` → `python|typescript|go`), `parent_symbol:
    Option<String>`, `file_docstring`, `imports`, `cross_references`, `is_heuristic`.
  - `QueryOptions { max_tokens=4000, max_results=20, file_filter: Option<Vec<PathBuf>> }` (Default).
- **The query string** is *not* on `QueryResult` — the formatter must take it as a parameter
  (the §6.4.2 JSON `"query"` field + §6.4.3 text header echo it). Decide the signature in M7.1
  (recommend `fn format(result: &QueryResult, query: &str, fmt: Format) -> String`).

## Decisions resolved at entry (do not re-litigate)
- **D7:** verified wired (above). ROADMAP "D7 re-verified at M7 entry" + plan deviations updated.
- **D17 dev-deps:** `assert_cmd = "2"` + `predicates = "3"` **APPROVED, dev-only**, scoped to
  `tests/cli_tests.rs` + `tests/e2e_cli.rs`. Added to `Cargo.toml [dev-dependencies]` when M7.2/M7.4
  land; devops mirrors in CI. ROADMAP D17 records the rationale.

## Scope (in / out)
- **In:** three serializers (§6.4) with D13 ordering; `clap` derive for all §7 commands/flags with
  documented defaults; error → nonzero exit mapping (no panic); command handlers delegating to
  `app`/`Indexer`/`Retriever`/`Config`/`Storage`; `status` aggregates; binary E2E.
- **Out:** real MCP `serve` (M8 — stub here: prints "not yet" / clean nonzero, no crash);
  embeddings `--enable-embeddings` (D1 — may warn, no logic); TS/Go (M9); self-healing (M8).

## Ordered slices + scenarios (from plan + TEST_STRATEGY)

### M7.1 — formatters (golden outputs) — `src/formatter/{mod,toon,json,text}.rs`, `tests/formatter_tests.rs`, `tests/fixtures/golden/*`
RED (test-lead):
- [ ] `toon_format_emits_file_line_pairs_sorted_by_score` — one `path:start-end` per line, BM25 order
- [ ] `json_format_is_valid_and_matches_golden` (§6.4.2: `query`, `total_results`, `total_tokens`,
      `chunks[]` with `symbol_name`, `symbol_type`, `file_path`, `start_byte`, `end_byte`,
      `language`, `bm25_score`, `chunk_text`)
- [ ] `json_round_trips_to_queryresult` (serde value round-trip; `serde_json` already a dep)
- [ ] `text_format_matches_golden_human_readable` (§6.4.3 layout: header + `[n] file:start-end (score: …)`)
- [ ] `empty_result_formats_cleanly_in_all_three` (no panic, well-formed empty)
- [ ] **D13** `toon_and_text_order_agent_first` — symbol name / qualified parent / `file:start-end` /
      one-line signature **before** bodies; bodies only within remaining budget (§8.2 ordering note).
      Encode the ordering in the committed golden files so it can't silently regress.
GREEN (eng-lead): `Format { Toon, Json, Text }` enum + dispatch; per-format serializers. JSON via
serde (`#[derive(Serialize/Deserialize)]` on a format-local DTO mirroring §6.4.2 — keep `types::Chunk`
free of transport concerns, D4/D5). TOON/text line ranges from `chunk.start_line`/`end_line`. Default
`text`. Pure functions, no I/O.

### M7.2 — CLI parsing + errors + exit codes — `src/cli/mod.rs`, `src/main.rs`, `tests/cli_tests.rs`
RED:
- [ ] `each_command_parses_its_documented_flags` (init/index/update/query/status/config/serve — §7.2)
- [ ] `query_defaults_match_spec` (`--max-tokens 4000`, `--max-results 20`, `--format text`)
- [ ] `help_and_version_flags_work` (`-h/--help`, `-V/--version`, global `-v/--verbose`)
- [ ] `bad_args_exit_nonzero_with_message`
- [ ] `unknown_command_errors_cleanly`
GREEN: `clap` derive `Cli`/`Command` mirroring §7.1–§7.2 **exactly** (flag names + defaults, incl.
`--db-path [default: .codecache/index.db]`, `--format toon|json|text`, `serve --transport stdio|sse
--port 3000`). `main.rs` maps domain `Result` → process exit (0 ok, nonzero on error) with **no
`panic`** (no reachable `unwrap/expect`). Uses `assert_cmd`/`predicates` (D17).

### M7.3 — command handlers + status — `src/cli/{init,index,update,query,status,config,serve}.rs`
RED:
- [ ] `init_creates_db_and_config`
- [ ] `index_then_status_reports_counts` (§7.2 status fields)
- [ ] `query_command_prints_formatted_results`
- [ ] `update_command_reindexes_given_files`
- [ ] `config_command_reads_writes_settings`
GREEN: handlers delegate to `app::{init,index}` / `Indexer` / `Retriever` / `Config` / `Storage`.
`status` reads `index_state` + `files_metadata` aggregates (§7.2 layout: version, counts by language,
chunks by symbol_type, sizes). `serve` = stub (clean message, nonzero/zero per plan). `query` handler
maps `--file-filter` glob → `QueryOptions.file_filter` and pipes `QueryResult` through the M7.1 formatter.

### M7.4 — E2E through the binary — `tests/e2e_cli.rs`
RED: `tests/e2e_cli.rs` (assert_cmd): temp dir → `codecache init` → `codecache index` →
`codecache query "<symbol>"` → assert stdout contains the expected symbol + correct exit codes
(0 on success; nonzero on a bad query/dir).
GREEN: `main.rs` wiring + working-dir / db-path resolution behave end-to-end on a fixture repo.

## Definition of Done (phase)
- [ ] M7.1–M7.4 green incl. golden outputs (committed) + binary E2E.
- [ ] All §7 commands/flags present with documented defaults; errors → nonzero exit, no panic.
- [ ] D13 agent-first ordering encoded in TOON/text golden files; JSON round-trips.
- [ ] D7 confirmed used (no file reads at format time); D17 dev-deps added + mirrored in CI.
- [ ] clippy `-D warnings` + fmt clean; reviewer APPROVED.
- [ ] `docs/TODO.md` Phase 7 + `src/formatter/CLAUDE.md` + `src/cli/CLAUDE.md` updated in the same change.

---
## RED — test lead

### M7.1 — formatters (golden outputs) — RED landed 2026-06-12

**Formatter API the tests pin (eng-lead must implement to this exact shape):**
```rust
pub enum codecache::formatter::Format { Toon, Json, Text }
pub fn  codecache::formatter::format(result: &QueryResult, query: &str, fmt: Format) -> String
```
- Pure `QueryResult -> String` (D4); **no I/O, no file reads** (D7 — ranges from stored
  `chunk.start_line`/`end_line`, 1-based inclusive). `query` is a parameter (not on `QueryResult`).
- `Format` must be usable in a `for fmt in [Format::Toon, Format::Text]` loop and printable with
  `{fmt:?}` → derive `Debug, Clone, Copy` (the D13 test iterates + interpolates it).

**Tests added — `tests/formatter_tests.rs`** (6 tests, all named exactly per the slice):
1. `toon_format_emits_file_line_pairs_sorted_by_score` — **compact, locator-only**
2. `json_format_is_valid_and_matches_golden`
3. `json_round_trips_to_queryresult`
4. `text_format_matches_golden_human_readable`
5. `empty_result_formats_cleanly_in_all_three`
6. `text_orders_agent_first` (D13 — **text format only**)

Shared in-test fixture `basic_result()` = 3 chunks / 2 files, distinct best-first BM25 scores
(-2.45, -1.89, -1.20), distinct line ranges, middle chunk has `parent_symbol = "AuthService"`
(exercises qualified parent). `total_results_found = 5 > chunks.len() == 3` to pin the
"showing top 3 of 5" wording + the `total_results` JSON key to the pre-budget count. Plus
`empty_result()`.

**Golden fixtures committed — `tests/fixtures/golden/`:**
- `query_basic.toon`, `query_basic.json`, `query_basic.txt`
- `query_empty.toon` (empty file), `query_empty.json`, `query_empty.txt`

Comparison is CRLF→LF normalized + single-trailing-newline-tolerant (`norm()`); JSON compared by
`serde_json::Value` equality (whitespace/key-order robust) **and** field-by-field asserts.

**Manager rulings applied 2026-06-12 (TOON shape CHANGED; rest CONFIRMED) — implement to these:**
- **TOON shape — CHANGED to the compact bare list (§6.4.1 normative; D13 does NOT touch TOON).**
  TOON is **locator-only**: emit exactly one `<file>:<start_line>-<end_line>` line per result, in
  BM25 best-first order (preserve incoming chunk order — do NOT re-sort), with **no bodies, no
  signatures, no header**. It must pipe straight to `cat`/an editor. Empty TOON = empty string.
  Ranges come from stored `start_line`/`end_line` (D7), never byte offsets. `file_path` via
  `to_string_lossy()` (fixtures use forward slashes — platform-stable, no normalization needed).
  Golden `query_basic.toon` is now three plain lines:
  `src/auth/handlers.py:45-67` / `src/auth/handlers.py:70-72` / `src/auth/utils.py:12-14`.
- **D13 agent-first ordering — TEXT format ONLY.** The text format carries the
  symbol/qualified-parent + `file:start-end` + one-line signature **before** the body. Encoded in
  `query_basic.txt` (unchanged). The test is `text_orders_agent_first` (TOON arm dropped).
- **Qualified parent — CONFIRMED:** `parent_symbol`.`symbol_name` when `parent_symbol` is `Some`
  (`AuthService.verify_password`), else bare `symbol_name`. (Used in the text format.)
- **One-line signature — CONFIRMED:** the **first line of `chunk_text`** (split on first `\n`).
  No Tree-sitter re-derivation at M7.1. A smarter skeleton line for `codecache_outline` (M8) is
  additive later.
- **Text header — CONFIRMED ASCII, no emoji:** `Query: "<q>"` /
  `Found <total_results_found> results (showing top <chunks.len()>, <total_tokens> tokens)` framed
  by 56-char `─` rules + a closing rule. Empty text = header block + closing rule, no `[n]` blocks.
  Recorded as an intentional deviation from the §6.4.3 emoji example.
- **Empty shapes — CONFIRMED:** empty TOON = empty string; empty JSON = valid object with
  `chunks: []` + zero counts; empty text = header + closing rule, no `[n]` blocks.

**RED proof (fails for the missing-API reason, nothing else):**
```
$ cargo test --test formatter_tests --no-run
error[E0432]: unresolved imports `codecache::formatter::format`, `codecache::formatter::Format`
  --> tests\formatter_tests.rs:19:28
   |
19 | use codecache::formatter::{format, Format};
   |                            ^^^^^^  ^^^^^^ no `Format` in `formatter`
   |                            |
   |                            no `format` in `formatter`
error: could not compile `codecache` (test "formatter_tests") due to 1 previous error
```
Single error = the missing formatter API. All fixture construction, golden-file reads, and the
serde round-trip compile against already-shipped `QueryResult` / `SearchResult` / `Chunk`. No
production code stubbed. Hand to **principal-engineering-lead** for GREEN.

**Re-bake after manager rulings (2026-06-12): RED re-confirmed, same single reason.**
Per the TOON-is-compact / D13-is-text-only rulings I re-baked `tests/fixtures/golden/query_basic.toon`
to the three-line locator-only list and adjusted exactly two tests:
- `toon_format_emits_file_line_pairs_sorted_by_score` now asserts the compact shape — the output
  is *exactly* `["src/auth/handlers.py:45-67", "src/auth/handlers.py:70-72", "src/auth/utils.py:12-14"]`
  (BM25 order preserved), no bodies/signatures/header leak, and byte offsets do not appear (D7).
- `toon_and_text_order_agent_first` → renamed `text_orders_agent_first`; TOON arm dropped; D13
  ordering (qualified parent / range / one-line signature precede body) asserted against the TEXT
  golden only. JSON/text/empty tests + their goldons are untouched. `query_empty.toon` stays empty.
Re-ran `cargo test --test formatter_tests --no-run` → same single `E0432: unresolved imports
codecache::formatter::{format, Format}` and nothing else. Still RED for the right reason.

**Compile-bug fix applied (2026-06-12) — E0716 borrow-lifetime, mechanical, no semantic change.**
The eng-lead's GREEN blocker was an honest test-authoring bug in
`toon_format_emits_file_line_pairs_sorted_by_score` (line 162): `norm(&out)` returns an owned
`String` and `.lines()` borrowed that temporary, which was dropped at end of statement while
`lines` was still used on the next line → `error[E0716]: temporary value dropped while borrowed`.
Fixed with the rustc-suggested two-line `let` binding, touching **no assertion or test semantics**:
```rust
let normed = norm(&out);
let lines: Vec<&str> = normed.lines().collect();
```
The subsequent `assert_eq!(lines, vec![...])` and every other assertion are unchanged; no other
test, golden, or production file touched. Since the formatter production code now EXISTS, the
honest outcome is GREEN: `cargo test --test formatter_tests` → **6 passed; 0 failed** (all of
`toon_format_emits_file_line_pairs_sorted_by_score`, `json_format_is_valid_and_matches_golden`,
`json_round_trips_to_queryresult`, `text_format_matches_golden_human_readable`,
`empty_result_formats_cleanly_in_all_three`, `text_orders_agent_first` pass against the committed
goldens). No assertion failure surfaced — formatter output matches every golden. Slice unblocked;
hand back to manager/reviewer. Not committed.

## GREEN — engineering lead

### M7.1 — formatters — IMPLEMENTED 2026-06-12 · BLOCKED by a test-file compile bug (E0716)

**Files created (production):**
- `src/formatter/mod.rs` — `Format { Toon, Json, Text }` (derives `Debug, Clone, Copy, Default,
  PartialEq, Eq`; `Default = Text`) + the pure `format(result, query, fmt) -> String` dispatch.
- `src/formatter/toon.rs` — compact locator-only list: one `file:start-end` per chunk in incoming
  (BM25 best-first) order, `to_string_lossy()` path, ranges from `start_line`/`end_line` (D7).
  Empty result → empty string.
- `src/formatter/json.rs` — serde via format-local DTOs `JsonResult`/`JsonChunk` (borrowed fields,
  no serde derives on `types::Chunk` — D4/D5). Keys ordered `query`, `total_results`
  (← `total_results_found`), `total_tokens`, `chunks[]`; pretty-printed 2-space. The unreachable
  `to_string_pretty` error path falls back to `"{}"` (no `unwrap`/`panic`).
- `src/formatter/text.rs` — §6.4.3 layout: 56-char `─` rule / `Query: "<q>"` / `Found <found>
  results (showing top <shown>, <tokens> tokens)` / rule / blank, then per chunk `[n]
  <qualified> (<type>) <file>:<s>-<e> (score: <bm25:.2>)` + full `chunk_text` + blank, then a
  closing rule. Qualified name = `parent.symbol_name` when `parent_symbol` is `Some`, else bare.
  Empty result → header block + closing rule (no `[n]` blocks).

**How each behavior is satisfied (validated against the committed goldens):** I verified all four
serializers produce byte-exact (CRLF→LF + trailing-newline normalized) golden matches via a
throwaway in-module test harness (built the same `basic_result()`/`empty_result()` fixtures,
compared TOON/JSON/text vs `query_basic.*` and text vs `query_empty.txt`, JSON by
`serde_json::Value` equality). All 4 passed; the harness was then removed so `mod tests {}` stays
an empty stub. This exercises exactly the assertions of all 6 RED tests once they compile.

**One deviation worth flagging (score formatting — golden wins):** the text score is rendered
with `{:.2}`, NOT the f64's natural `Display`. The brief suggested "natural Display", but
`-1.20f64` Displays as `-1.2`, whereas `query_basic.txt` line 19 shows `(score: -1.20)`. Per the
"golden wins" rule I used `{:.2}`, which yields `-2.45`/`-1.89`/`-1.20` exactly. JSON keeps the
raw f64 (`bm25_score: sr.bm25_score`) since the JSON golden is compared by `Value` equality and
`-1.20` parses to the same f64. No API/plan shape changed.

### BLOCKER for manager / test-lead — `tests/formatter_tests.rs` does not compile (NOT my code)

`cargo test --test formatter_tests` fails to compile with a single error, **in the test file**:

```
error[E0716]: temporary value dropped while borrowed
   --> tests\formatter_tests.rs:162:28
162 |     let lines: Vec<&str> = norm(&out).lines().collect();
    |                            ^^^^^^^^^^  - temporary value is freed at the end of this statement
    |                            creates a temporary value which is freed while still in use
163 |     assert_eq!(lines, ...);   // borrow later used here
```

`norm(&out)` returns an owned `String`; `.lines()` borrows from that temporary, which is dropped
at the end of the statement while `lines` (used on line 163) still borrows it. This is a
test-authoring bug independent of the formatter — my library + all other targets compile and pass.
The conventional fix is the one rustc suggests (a `let` binding):
```rust
let normed = norm(&out);
let lines: Vec<&str> = normed.lines().collect();
```
Per the hard rule I did **not** touch the test. Requesting the test-lead apply that two-line
binding fix (it changes no assertion/semantics) so the 6 tests can run; I have already confirmed
the formatter output matches every golden, so they will go green as written once it compiles.

**Gate output (Rust 1.85):**
- `cargo build` → ok (library + bins compile clean).
- `cargo test --test formatter_tests` → **does not compile** (E0716 above — test file, line 162).
- Rest of suite, no regressions: lib 24 · config 5 · e2e_index 4 · hasher 11 · indexer 15 ·
  parser 14 · chunker 10 · chunker_proptest 1 · retriever 12 · storage 18 · smoke 1 (all `ok`).
- `cargo clippy --lib -- -D warnings` → clean. (`--all-targets` blocked only by the test-file
  E0716, not by formatter code.)
- `cargo fmt --check` → clean (exit 0).

## Specialist / Perf notes
<skeleton-line / signature-extraction edge cases if routed to rust-treesitter-specialist; no gated perf>

## REVIEW — code reviewer

### M7.1 — formatters — VERDICT: APPROVE (2026-06-12)

Reviewed the four new production files + lib.rs wiring against the brief, the §6.4 plan, and the
committed goldens. Gates all green on Rust 1.85:
- `cargo fmt --check` -> exit 0 (clean).
- `cargo clippy --all-targets -- -D warnings` -> exit 0 (clean).
- `cargo test --all` -> all green incl. the 6 new formatter tests; no regression (lib 24, config 5,
  e2e_index 4, hasher 11, indexer 15, parser 14, chunker 10, chunker_proptest 3, retriever 12,
  storage 18, smoke 1, formatter 6).

Correctness / DoD checks (all pass):
1. TOON (toon.rs): one `file:start-end` per chunk in incoming BM25 order, no re-sort, no bodies/
   header; `to_string_lossy()` + stored `start_line`/`end_line` (D7, no byte offsets, no I/O);
   empty result -> empty string (golden is 0 bytes). Matches `query_basic.toon` exactly.
2. JSON (json.rs): format-local `JsonResult`/`JsonChunk` DTOs, NO serde derives on `types::Chunk`
   (D4/D5). Key order query/total_results/total_tokens/chunks matches §6.4.2 + golden;
   `total_results` mapped from `total_results_found` (5, not 3); raw f64 score. DTOs private to
   the json module. Round-trip + value-equality goldens pass.
3. Text (text.rs): 56-char U+2500 rule verified (168 bytes = 56x3); ASCII header (no emoji);
   `Found N results (showing top M, T tokens)`; agent-first ordering (qualified parent + range +
   first-line signature precede body); `{:.2}` score (golden authority, -1.20); empty -> header +
   closing rule, no `[n]`. Matches `query_basic.txt` and `query_empty.txt`.
4. No reachable `unwrap()/expect()/panic!` in any of the four files. The only `unwrap_or_else`
   (json.rs:64) is the documented infallible `"{}"` fallback on an unreachable serialize error;
   `writeln!`/`format!` into `String` are infallible and `let _ =` correctly discards the Result.
5. mod.rs/lib.rs: `Format` derives Debug/Clone/Copy/Default(=Text)/PartialEq/Eq; only `Format` +
   `format` public; submodules private (`mod`), `render` fns `pub(super)`. `pub mod formatter;`
   wired. Plan deviations (TOON-as-locator, D13-text-only, ASCII header, {:.2} score) are all
   ratified in project_plan.md / ROADMAP / the M7 plan.
6. Test integrity: tests/formatter_tests.rs + goldens are new/untracked (RED slice landing fresh) —
   no prior assertion was weakened. The `let normed = norm(&out);` binding is an internal borrow-
   lifetime fix; assertions are meaningful (exact golden match + field-by-field).

Findings: none (blocker/major/minor). One non-actionable note: json.rs owns `file_path: String`
via `to_string_lossy().into_owned()` rather than borrowing — this is the correct handling of the
`Cow` from `to_string_lossy()` on non-UTF-8 paths, not a needless allocation.

Slice M7.1 is APPROVED — ready for manager to mark DONE (after TODO + formatter/CLAUDE.md status
updates land in the same change per the DoD).

## OUTCOME — manager
<per-slice: aligned? TODO + module CLAUDE.md updated? committed? follow-ups?>
