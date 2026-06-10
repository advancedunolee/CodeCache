# M9 — TypeScript + Go parsers

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m9--typescript--go-parsers),
> [`../project_plan.md`](../project_plan.md) §3.2.1 / §5.3, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#parser-python--ts--go).

## Goal / acceptance criteria
Add `tree-sitter-typescript` and `tree-sitter-go` language configs + extraction queries so
language coverage = Python/TS/Go. **Exit (from ROADMAP):**
- [ ] Per-language fixture suites green (TS + Go) with exact byte spans.
- [ ] Language coverage = Python, TypeScript, Go (§1.3 success criterion).

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/parser/typescript.rs` | TS `LanguageConfig` (grammar + queries). | specialist |
| `src/parser/go.rs` | Go `LanguageConfig`. | specialist |
| `src/parser/queries/typescript.scm` | Function/arrow/class/method queries (§5.3). | specialist |
| `src/parser/queries/go.scm` | Function/method/struct queries (§5.3). | specialist |
| `tests/parser_ts_tests.rs`, `tests/parser_go_tests.rs` | Per-language span + edge tests. | test-lead |
| `tests/fixtures/{typescript,go}/**` | Committed fixtures. | test-lead |
| Enrichment `.scm` for TS/Go | imports/cross-refs for D3 parity. | specialist |
| `src/parser/CLAUDE.md` | Updated with TS/Go coverage. | manager |

## Dependencies
- **Prior:** M3 (parser framework + Python), M4 (chunker + enrichment seam), M5 (indexer wires
  language detection). M9 reuses the *exact same* `Parser`/`chunker` plumbing — only new
  `LanguageConfig`s + `.scm` files + fixtures.
- Crates already declared at M0: `tree-sitter-typescript`, `tree-sitter-go`.

## Ordered slices

### Slice M9.1 — TypeScript config + extraction
- **RED (test-lead):** `tests/parser_ts_tests.rs` over fixtures (exact-span by source-slice equality):
  - `extracts_function_declaration_with_exact_span`
  - `extracts_arrow_function_assigned_to_variable` (§5.3 variable_declarator + arrow_function)
  - `extracts_class_declaration_and_method_definition`
  - `generics_handled` (TS-specific cross-cutting)
  - `tsx_or_type_only_constructs_no_panic`
  - `high_error_rate_ts_file_flags_heuristic` (D2 parity)
- **GREEN (specialist):** `typescript.rs` config + `typescript.scm` (§5.3 TS queries: function
  decls, arrow fns, classes, methods). Map captures via the shared `extract_chunks`. Note: the
  `tree-sitter-typescript` crate exposes separate `typescript` and `tsx` languages — decide
  which to load per extension (`.ts` vs `.tsx`); document in `parser/CLAUDE.md`.

### Slice M9.2 — Go config + extraction
- **RED:** `tests/parser_go_tests.rs`:
  - `extracts_function_declaration_with_exact_span`
  - `extracts_method_declaration_with_receiver` (§5.3 method_declaration receiver)
  - `extracts_struct_type_as_struct_symbol` (§5.3 type_declaration/struct_type → SymbolType::Struct)
  - `package_and_imports_handled`
  - `high_error_rate_go_file_flags_heuristic` (D2 parity)
- **GREEN (specialist):** `go.rs` config + `go.scm` (§5.3 Go queries: functions, methods w/
  receiver, struct defs). `SymbolType::Struct` per §4.3.

### Slice M9.3 — cross-language integration through indexer
- **RED:** extend `tests/indexer_tests.rs` (or new `tests/e2e_multilang.rs`):
  - `index_mixed_repo_indexes_python_ts_and_go_files`
  - `language_filter_in_config_restricts_indexed_languages`
- **GREEN:** confirm M5 discovery/detection already routes `.ts/.tsx/.go` to the new configs;
  no indexer changes expected (validation milestone). Fix detection if gaps surface.

## API contracts / data structures
- No new public API — reuses §3.2.1 `Parser`/`LanguageConfig`/`extract_chunks` and the extended
  `Chunk` (M4). `Language::{TypeScript, Go}` already in the enum (§4.3).
- New: `queries/typescript.scm`, `queries/go.scm` (capture-name convention identical to Python).

## Performance budgets
- Per-language parsing must not regress the aggregate cold-index budgets (§5.4). Tree-sitter
  grammars load on-demand (~50MB each, §11.3) — keep configs lazy (`once_cell`) so only used
  grammars are instantiated. M10 benches the mixed-language aggregate.

## Decision Log bindings
- **D2:** TS/Go must exercise the same graceful-degradation path (ERROR rate + heuristic flag) —
  parity tests in M9.1/M9.2.
- **D3:** enrichment (`imports`, `cross_references`, `parent_symbol`) populated for TS/Go too, so
  recall parity holds across languages.

## Definition of Done (this phase)
- [ ] M9.1–M9.3 green; TS + Go exact-span suites + D2/D3 parity.
- [ ] `.ts` vs `.tsx` grammar selection decided + documented.
- [ ] Mixed-language repo indexes correctly; language filter respected.
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 9 + `src/parser/CLAUDE.md` updated.
