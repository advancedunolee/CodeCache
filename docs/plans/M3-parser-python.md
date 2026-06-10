# M3 — parser (Python first)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m3--parser-python-first),
> [`../project_plan.md`](../project_plan.md) §3.2.1 / §5.3, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#parser-python--ts--go).

## Goal / acceptance criteria
Load `tree-sitter-python`, run `.scm` queries to extract function/class/method nodes with
**exact byte spans**, and detect `ERROR` nodes so M4/M5 can degrade gracefully. **Exit:**
- [ ] Functions/classes/methods extracted with correct `start_byte`/`end_byte` (off-by-one guards).
- [ ] Nested, `async`, and decorated symbols handled; methods distinguished from free functions.
- [ ] ERROR-node rate computed; malformed file never panics (degradation path exercised — **D2**).
- [ ] Unsupported language → typed error.

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/parser/mod.rs` | `Parser`, `LanguageConfig`, `parse_file`, `extract_chunks`, ERROR-rate. | eng-lead + specialist |
| `src/parser/python.rs` | Python `LanguageConfig`: grammar + `.scm` query strings. | specialist |
| `src/parser/queries/python.scm` | Function/class/method queries (§5.3). | specialist |
| `crate::types` | `Chunk`, `Language`, `SymbolType` (shared — D5 from M1). | (exists from M1) |
| `tests/parser_tests.rs` | Integration over fixtures; span assertions; ERROR handling. | test-lead |
| `tests/fixtures/python/*.py` | Small committed fixtures (see scenarios). | test-lead |
| `src/parser/CLAUDE.md` | Shipped API + query/degradation notes. | manager |

## Dependencies
- **Prior:** M0 (`tree-sitter`, `tree-sitter-python`). Shared types from M1 (D5).
- **Not** dependent on storage/hasher — `parser` is parallelizable with M1/M2.
- Routes Tree-sitter depth questions to `rust-treesitter-specialist`.

## Ordered slices

### Slice M3.1 — load grammar + parse to a tree
- **RED (test-lead):**
  - `parse_valid_python_expects_tree_without_errors`
  - `parse_empty_file_expects_empty_tree_no_panic`
  - `unsupported_language_expects_typed_error`
- **GREEN:** `Parser::new()`; `parse_file(path, content, Language::Python) -> Result<Tree>`
  configuring the python grammar via `LanguageConfig`.

### Slice M3.2 — extract chunks with exact byte spans
- **RED:** fixtures + assertions on exact spans (assert the sliced `&source[start..end]` equals
  the expected symbol text — the strongest off-by-one guard):
  - `extracts_top_level_function_with_exact_span`
  - `extracts_class_with_exact_span`
  - `extracts_method_inside_class_as_method_type`
  - `nested_function_extracted_with_correct_parent_context`
  - `async_def_extracted`; `decorated_function_span_includes_decorator` (decide policy — see below)
  - `multibyte_identifier_span_is_byte_correct` (UTF-8 cross-cutting)
  - `crlf_file_spans_correct` (CRLF vs LF cross-cutting)
- **GREEN:** `extract_chunks(tree, source, Language::Python) -> Result<Vec<Chunk>>` running the
  `.scm` queries (§5.3); map captures → `Chunk { symbol_name, symbol_type, file_path,
  start_byte, end_byte, chunk_text, language }`. Deterministic order (by `start_byte`).
- **SPECIALIST decisions to document in brief + `python.scm`:**
  - Decorator inclusion: span the `@decorator` lines with the def (recommended) vs just the def.
  - Method vs function: a `function_definition` whose ancestor is a `class_definition` body ⇒
    `SymbolType::Method`.

### Slice M3.3 — ERROR-node detection + degradation hook (D2)
- **RED:**
  - `error_node_rate_computed_for_malformed_file`
  - `high_error_file_above_threshold_flags_for_heuristic_fallback` (~20% threshold — D2)
  - `malformed_file_never_panics_returns_result`
- **GREEN:** walk the tree, count `ERROR`/`MISSING` nodes vs total; expose
  `error_rate(tree) -> f32` and a `should_fall_back(rate) -> bool` (threshold const).
  v0.1: parser *reports* the rate + flag; the actual heuristic/regex chunker fallback is
  implemented in M4 (chunker owns degradation output + the `heuristic` flag). Document the seam.
- **SPECIALIST:** confirm node-kind names (`ERROR`, `MISSING`) and traversal approach.

## API contracts / data structures (from `../project_plan.md` §3.2.1)
```rust
pub struct Parser { /* ts_parser, language_configs */ }
pub enum Language { Python, TypeScript, Go }
pub struct LanguageConfig { grammar: tree_sitter::Language, function_query: &'static str, class_query: &'static str }
impl Parser {
    pub fn new() -> Result<Self>;
    pub fn parse_file(&mut self, path: &Path, content: &str, lang: Language) -> Result<tree_sitter::Tree>;
    pub fn extract_chunks(&self, tree: &tree_sitter::Tree, source: &str, lang: Language) -> Result<Vec<Chunk>>;
}
// Chunk / SymbolType / Language: see crate::types (D5), shape per §4.3
```
**Tree-sitter queries:** Python set from §5.3 lives in `queries/python.scm`. Capture names
`@function.name/@function.body/@function.definition`, etc.
**ERROR rate API** (new, M3-introduced — record in §3.2.1):
`pub fn error_rate(tree: &Tree) -> f32;` and threshold const `HEURISTIC_FALLBACK_THRESHOLD`.

## Performance budgets
- No standalone parser budget at M3, but parsing dominates **cold index < 5s for 10K LOC /
  < 30s for 100K LOC** (§5.4). Avoid re-parsing; reuse `tree_sitter::Parser`; do not clone
  `source` per chunk (slice `&str`, allocate `chunk_text` once). M10 benches the aggregate.

## Decision Log bindings
- **D2 (graceful degradation):** owned by `rust-treesitter-specialist`; parser computes the
  ERROR rate + fallback flag here, chunker emits `heuristic`-flagged chunks in M4. Indexing
  must never hard-fail (enforced again at M5).
- **D3 (metadata enrichment):** parser exposes enough (parent class node, file-level docstring
  node, import statements) for M4 to fill `parent_symbol`/`file_docstring`/`imports`. Decide
  whether parser returns raw nodes or M4 re-queries the tree — recommend parser passes the
  `Tree` + source to chunker so M4 can query enrichment without re-parsing.

## Definition of Done (this phase)
- [ ] M3.1–M3.3 green; spans asserted by exact source-slice equality (off-by-one proof).
- [ ] Nested/async/decorated/method/multibyte/CRLF fixtures covered.
- [ ] ERROR rate + fallback flag implemented; malformed input never panics.
- [ ] `error_rate` API recorded in `project_plan.md` §3.2.1 before/with implementation.
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 3 + `src/parser/CLAUDE.md` updated.
