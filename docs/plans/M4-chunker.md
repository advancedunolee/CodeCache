# M4 — chunker (AST boundaries + metadata enrichment)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m4--chunker),
> [`../project_plan.md`](../project_plan.md) §3.2.1 / §4.3, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#chunker),
> Decision Log **D2**, **D3**.

## Goal / acceptance criteria
Turn parser output into `Chunk`s with enrichment metadata, guarantee non-overlapping in-bounds
chunks, and emit heuristic-flagged chunks when the parser signals high ERROR rate. **Exit:**
- [ ] Property: chunks never overlap and always lie within `[0, file_len)`.
- [ ] Enrichment populated: `parent_symbol`, `file_docstring`, `imports`, `cross_references` (**D3**).
- [ ] Heuristic chunks flagged in metadata when degradation triggered (**D2**).

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/chunker/mod.rs` | `chunk(tree, source, lang) -> Vec<Chunk>`; enrichment; heuristic fallback. | eng-lead + specialist |
| `crate::types` | Extend `Chunk` with enrichment + `is_heuristic` flag (D3/D2). | eng-lead |
| `tests/chunker_tests.rs` | Integration: enrichment values on fixtures; fallback flag. | test-lead |
| `tests/chunker_proptest.rs` (or in-module) | Property: non-overlap + in-bounds invariants. | test-lead |
| `src/chunker/CLAUDE.md` | Shipped API + enrichment field semantics. | manager |

## Dependencies
- **Prior:** M3 (`parser` gives `Tree` + `error_rate`/fallback flag). Shared `Chunk` type.
- **Not** dependent on storage — but the enrichment fields must be FTS5-searchable columns
  decided in M1 §D3 (coordinate: if M1 packed them into `chunk_text`, M4 packs accordingly).

## Ordered slices

### Slice M4.1 — AST → Chunk with non-overlap/in-bounds invariants
- **RED (test-lead):**
  - Property (`proptest`): for generated/fixture Python, every `Chunk` has
    `start_byte < end_byte <= file_len` and **no two chunks overlap** (or overlap only via
    documented parent/child nesting — decide policy below).
  - `single_symbol_file_yields_one_chunk`; `empty_file_yields_no_chunks`
- **GREEN (eng-lead):** map parser captures → `Chunk`s, stable order by `start_byte`.
- **POLICY (document in brief):** Nesting — a method lives inside its class span. Either
  (a) emit both class and method (overlap allowed for parent/child, property relaxed to
  "siblings never overlap"), or (b) emit leaf-only. **Recommend (a)** to maximize recall;
  property test asserts siblings disjoint + children fully contained in parents.

### Slice M4.2 — metadata enrichment (D3)
- **RED:**
  - `method_chunk_has_parent_symbol_set_to_class_name`
  - `chunk_has_file_docstring_when_module_has_one`
  - `chunk_imports_lists_module_imports`
  - `cross_references_lists_called_symbol_names` (best-effort, identifiers in body)
  - `top_level_function_has_no_parent_symbol`
- **GREEN:** query the `Tree` for: enclosing class (`parent_symbol`), module docstring
  (`file_docstring`), import statements (`imports`), and referenced identifiers
  (`cross_references`). Populate the extended `Chunk`. Keep extraction single-pass where
  possible (§hot-path guidance).
- **SPECIALIST:** provide enrichment `.scm` captures (imports, call expressions) for Python.

### Slice M4.3 — heuristic fallback path (D2)
- **RED:**
  - `high_error_rate_input_produces_heuristic_flagged_chunks`
  - `heuristic_chunks_still_non_overlapping_and_in_bounds`
  - `malformed_input_never_panics`
- **GREEN:** when parser's `error_rate > HEURISTIC_FALLBACK_THRESHOLD` (M3), chunk by a regex/
  line-heuristic (e.g. `def `/`class ` at column 0 for Python) and set `is_heuristic = true`.
  Heuristic chunks may have empty enrichment but must keep the invariants.

## API contracts / data structures
```rust
// crate::types — Chunk extended per D3 (§4.3 base + enrichment)
pub struct Chunk {
    pub symbol_name: String,
    pub symbol_type: SymbolType,      // Function | Class | Method | Struct
    pub file_path: PathBuf,
    pub start_byte: usize,
    pub end_byte: usize,
    pub chunk_text: String,
    pub language: Language,
    // --- D3 enrichment ---
    pub parent_symbol: Option<String>,
    pub file_docstring: Option<String>,
    pub imports: Vec<String>,
    pub cross_references: Vec<String>,
    // --- D2 degradation ---
    pub is_heuristic: bool,
}
// chunker entry
pub fn chunk(tree: &tree_sitter::Tree, source: &str, lang: Language) -> Result<Vec<Chunk>>;
```
**Record this extended `Chunk` in `project_plan.md` §3.2.1/§4.3 before implementing** (it adds
fields beyond the base spec — the Decision Log already authorizes D2/D3, but the struct must be
updated in the plan first per the doc contract).

## Performance budgets
- No standalone budget; enrichment adds ~+1MB/index (D3, negligible) and must not break the
  cold-index budgets (§5.4) — single-pass traversal, no per-chunk re-query of the whole tree.

## Decision Log bindings
- **D2:** heuristic fallback + `is_heuristic` flag implemented here (the visible output of the
  parser's M3 ERROR-rate detection). Never hard-fail.
- **D3:** enrichment fields populated here; they must land in FTS5-searchable columns (M1 §D3).

## Definition of Done (this phase)
- [ ] M4.1–M4.3 green incl. the non-overlap/in-bounds **property test**.
- [ ] All four enrichment fields populated on AST path; heuristic path flags `is_heuristic`.
- [ ] Extended `Chunk` recorded in `project_plan.md` §3.2.1/§4.3 first.
- [ ] FTS5 column mapping for enrichment confirmed consistent with M1.
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 4 + `src/chunker/CLAUDE.md` updated.
