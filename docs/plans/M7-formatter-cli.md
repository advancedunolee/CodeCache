# M7 — formatter + cli

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m7--formatter--cli),
> [`../project_plan.md`](../project_plan.md) §6.4 / §7, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#formatter).

## Goal / acceptance criteria
Serialize query results to TOON/JSON/text, and wire the `clap` CLI with all commands so the
whole pipeline is usable from the binary. **Exit (from ROADMAP):**
- [ ] Golden-output tests per format (TOON, JSON, plaintext); JSON valid + round-trips.
- [ ] Each command parses expected args/flags; `--help`/`--version`; bad args ⇒ helpful error + nonzero exit.
- [ ] E2E `init → index → query` through the built binary on a fixture repo.

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/formatter/mod.rs` | `Format` enum + dispatch. | eng-lead |
| `src/formatter/{toon,json,text}.rs` | Per-format serializers (§6.4). | eng-lead |
| `src/cli/mod.rs` | `clap` `Cli`/`Command` derive; dispatch. | eng-lead |
| `src/cli/{init,index,update,query,status,config,serve}.rs` | Command handlers. | eng-lead |
| `src/main.rs` | Wire `cli::run()`; map errors → exit codes. | eng-lead |
| `tests/formatter_tests.rs` | Golden outputs per format. | test-lead |
| `tests/cli_tests.rs` | Arg parsing + error/exit-code; uses `assert_cmd`/`Command`. | test-lead |
| `tests/e2e_cli.rs` | E2E binary: init→index→query. | test-lead |
| `tests/fixtures/golden/*` | Committed golden outputs. | test-lead |
| `src/formatter/CLAUDE.md`, `src/cli/CLAUDE.md` | Shipped API + format/command notes. | manager |

## Dependencies
- **Prior:** M5 `indexer` (init/index/update), M6 `retriever` (query). `config` (M1).
- `serve` is **stubbed** here (prints "not yet"/errors cleanly); real MCP lands in M8.
- New dev-dep likely: `assert_cmd` + `predicates` for CLI E2E. **Needs manager sign-off** (not
  in §10.3) — record as deviation below.

## Ordered slices

### Slice M7.1 — formatters (golden outputs)
- **RED (test-lead):**
  - `toon_format_emits_file_line_pairs_sorted_by_score` (§6.4.1: `path:start-end` per line)
  - `json_format_is_valid_and_matches_golden` + `json_round_trips_to_queryresult` (§6.4.2 schema)
  - `text_format_matches_golden_human_readable` (§6.4.3)
  - `empty_result_formats_cleanly_in_all_three`
- **GREEN:** implement three serializers. JSON shape per §6.4.2 (`query`, `total_results`,
  `total_tokens`, `chunks[]` with `bm25_score`, `chunk_text`, …). TOON = `file:start-end` lines.
  Convert byte spans → line ranges for TOON/text (needs source or stored line info — decide:
  store line numbers at index time, or compute from byte offset + file read at format time;
  recommend storing `start_line`/`end_line` to avoid file reads at query time — **deviation D7**).

### Slice M7.2 — CLI parsing + errors + exit codes
- **RED (test-lead):**
  - `each_command_parses_its_documented_flags` (init/index/update/query/status/config/serve — §7.2)
  - `query_defaults_match_spec` (max-tokens 4000, max-results 20, format text)
  - `help_and_version_flags_work`
  - `bad_args_exit_nonzero_with_message`
  - `unknown_command_errors_cleanly`
- **GREEN:** `clap` derive structs mirroring §7.1/§7.2 exactly (flag names, defaults). Map
  domain `Result` errors → process exit codes (0 ok, nonzero on error) without `panic`.

### Slice M7.3 — command handlers + status
- **RED:**
  - `init_creates_db_and_config` ; `index_then_status_reports_counts` (§7.2 status output fields)
  - `query_command_prints_formatted_results`
  - `update_command_reindexes_given_files`
  - `config_command_reads_writes_settings`
- **GREEN:** handlers delegate to `Indexer`/`Retriever`/`Config`/`Storage`. `status` reads
  `index_state` + `files_metadata` aggregates (§7.2 layout). `serve` stub.

### Slice M7.4 — E2E through the binary
- **RED:** `tests/e2e_cli.rs`: temp dir → `codecache init` → `codecache index` →
  `codecache query "..."` → assert stdout contains expected symbol + nonzero/zero exit codes.
- **GREEN:** ensure `main.rs` wiring + working-dir/db-path resolution behave.

## API contracts / data structures (from `../project_plan.md` §6.4 / §7)
- **CLI commands & flags:** verbatim from §7.1–7.2 (`init`, `index --full/--progress`,
  `update <FILE>...`, `query <QUERY> --max-tokens/--max-results/--format/--file-filter`,
  `status`, `config`, `serve --transport/--port`). Global `-v/--verbose`, `-V/--version`, `-h`.
- **Output formats:** `toon | json | text` (§6.4); default `text`.
- **JSON schema:** §6.4.2 (must round-trip via `serde`).

## Performance budgets
- Formatting < 10ms (§11.2) — string building only; no per-chunk file reads if line numbers are
  stored (D7). Not a gated budget but contributes to the M6 p95 < 500ms total.

## Decision Log bindings
- **D4 (transport-agnostic):** formatter is pure (`QueryResult` → string); CLI is one adapter,
  MCP (M8) another. No retrieval logic in CLI/formatter.
- **D1:** `query --enable-embeddings` flag may be accepted and warn (low recall) — no logic.
- **D13 (agent-first output ordering — replan 2026-06-11; format split ratified at M7.1 entry
  2026-06-12):** The **text** format (§6.4.3) orders each result agent-first — symbol name,
  qualified parent, `file:start-end`, one-line signature *before* the body; bodies only within the
  remaining budget (spec §8.2). **TOON stays the compact `file:start-end`-per-line list (§6.4.1)** —
  it is locator-only (carries no bodies), already the strongest agent-first form, and must stay
  pipe-to-`cat`/editor friendly; making it a multi-line block would break §6.4.1 and duplicate the
  text format. **JSON** is field-keyed, so order is not semantic. The M7.1 **text** golden encodes
  the signature-before-body ordering. M8's `codecache_outline` reuses the text skeleton-line shape.
  Spec §8.2 updated to record this format split.
- **Text format = ASCII (no emoji), ratified 2026-06-12.** §6.4.3's 🔍/📊 are illustrative; the
  golden text output is plain ASCII (`Query: "…"`, `Found N results (showing top M, T tokens)`,
  56-char `─` rules) so goldens are byte-stable across Windows/Linux/macOS CI. One-line signature =
  first line of `chunk_text` (split on first `\n`); qualified parent = `parent_symbol`.`symbol_name`
  when `parent_symbol` is `Some`, else bare `symbol_name`.

## Definition of Done (this phase)
- [ ] M7.1–M7.4 green incl. golden outputs + binary E2E.
- [ ] All §7 commands/flags present with documented defaults; errors → nonzero exit, no panic.
- [ ] Line-range strategy (D7) decided + recorded; JSON round-trips.
- [ ] `assert_cmd`/`predicates` dev-deps signed off (deviation below).
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 7 + `src/{formatter,cli}/CLAUDE.md` updated.

## Deviations to record (ROADMAP) — RESOLVED at M7 entry (2026-06-12)
- **D7 — store line numbers at index time — VERIFIED WIRED (ROADMAP "D7 re-verified at M7 entry").**
  The seam is real end-to-end and needs no fix: `Chunk.start_line/end_line` (`src/types/mod.rs:30-33`,
  1-based inclusive) → UNINDEXED schema columns (`storage/schema.rs:38-39`) → INSERT
  (`queries.rs INSERT_CHUNK` cols 11-12) → SEARCH select + `build_search_result` map-back
  (`storage/mod.rs`). **Both** chunker paths populate real values — AST from Tree-sitter
  `start_position().row+1`/`end_position().row+1` (`parser/mod.rs:309-310`), heuristic via
  `chunker::line_range` (`chunker/mod.rs:256,300-310`). ⇒ M7 formatters read stored line numbers
  off the `SearchResult.chunk`; **no file reads at format time** (honors the §11.2 budget). Manager
  verified 2026-06-12.
- **dev-deps `assert_cmd` + `predicates` — APPROVED (ROADMAP D17).** Dev-dependencies only (ship in
  no release artifact, runtime §10.3 set unchanged), scoped to `tests/cli_tests.rs` + `tests/e2e_cli.rs`.
  Pin `assert_cmd = "2"`, `predicates = "3"`; `Cargo.lock` holds exact versions for CI cache parity.
  devops mirrors in CI. Manager sign-off 2026-06-12.
