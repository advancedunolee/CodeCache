# M6 — retriever (FTS5 BM25 + token budget)

> Phase plan. Sources: [`../ROADMAP.md`](../ROADMAP.md#m6--retriever),
> [`../project_plan.md`](../project_plan.md) §3.2.3 / §6, [`../TEST_STRATEGY.md`](../TEST_STRATEGY.md#retriever).

## Goal / acceptance criteria
Execute BM25 search over the FTS5 index, extract snippets, count tokens, and greedily pack
results within a token budget. **Exit (from ROADMAP):**
- [ ] BM25 ranking deterministic; relevant chunk ranks above irrelevant.
- [ ] `--max-tokens` never exceeded; greedy packing stops at budget; token count accurate.
- [ ] Empty query / no matches ⇒ empty, well-formed result. Overlapping snippets deduped.
- [ ] Query-latency bench wired vs **p95 < 500ms** (perf engineer).

## Modules & files
| Path | Purpose | Owner |
|---|---|---|
| `src/retriever/mod.rs` | `Retriever` facade per §3.2.3; `query`, preprocess, budget. | eng-lead |
| `src/retriever/ranking.rs` | BM25 result handling, dedup, stable tie-breaks. | eng-lead |
| `tests/retriever_tests.rs` | Integration over a seeded storage fixture. | test-lead |
| `benches/query_bench.rs` | Criterion latency bench (skeleton here, full in M10). | perf |
| `src/retriever/CLAUDE.md` | Shipped API + ranking/budget notes. | manager |

## Dependencies
- **Prior:** M1 `storage` (FTS5 search + `bm25()`). Independent of the M3→M4→M5 chain, so it can
  proceed in parallel once storage exists; tests seed storage directly (no real indexing needed).
- Token estimation is the §6.3 char heuristic (1 token ≈ 4 chars) — no tokenizer crate in v0.1.

## Ordered slices

### Slice M6.1 — query preprocessing
- **RED (test-lead):**
  - `preprocess_tokenizes_and_lowercases`
  - `preprocess_removes_stopwords` (e.g. "find", "the")
  - `preprocess_builds_or_match_expression` ("authenticate OR user" — §6.1)
  - `empty_query_after_stopword_removal_handled` (no crash, empty result downstream)
- **GREEN:** `preprocess_query(&str) -> Vec<String>`; join into FTS5 `MATCH` string. Keep
  stopword list small + documented; escape FTS5 special chars to avoid syntax errors.

### Slice M6.2 — BM25 search + determinism + dedup
- **RED:**
  - `relevant_chunk_ranks_above_irrelevant` (seed 2 chunks, query matches one strongly)
  - `same_query_same_index_yields_identical_order` (determinism + tie-break by stable key, e.g. file_path,start_byte)
  - `no_match_query_returns_empty_result`
  - `overlapping_snippets_deduplicated` (same file+overlapping byte span ⇒ keep one)
- **GREEN:** call `storage.search(fts_query, max_results)`; apply stable tie-break ordering;
  dedup by `(file_path, overlapping span)`. BM25 is FTS5-native (§6.2) — no custom scorer.

### Slice M6.3 — token budget packing
- **RED:**
  - `packing_never_exceeds_max_tokens`
  - `greedy_stops_at_budget_keeping_top_ranked` (highest-ranked first, stop when next won't fit — §6.3)
  - `total_tokens_reported_matches_sum_of_packed`
  - `total_results_found_reflects_pre_budget_count`
  - `estimate_tokens_is_len_div_4_min_1` (§6.3)
- **GREEN:** `apply_token_budget(results, max_tokens)` per §6.3; `estimate_tokens(text)`;
  assemble `QueryResult { chunks, total_tokens, total_results_found }`.

### Slice M6.4 — latency bench (perf)
- **PERF (perf engineer):** `benches/query_bench.rs` over a synthetic 100K-LOC-scale index;
  measure p50/p95/p99 (§11.2). Assert/track **p95 < 500ms** (§1.3). Record FTS5 query plan
  baseline carried from M1. Full suite + budget gate finalized in M10.

## API contracts / data structures (from `../project_plan.md` §3.2.3 / §6)
```rust
pub struct Retriever { /* storage: Storage */ }
impl Retriever {
    pub fn new(storage: Storage) -> Self;
    pub fn query(&self, user_query: &str, options: QueryOptions) -> Result<QueryResult>;
    fn preprocess_query(&self, query: &str) -> Vec<String>;
    fn apply_token_budget(&self, results: Vec<SearchResult>, max_tokens: usize) -> Vec<SearchResult>;
}
pub struct QueryOptions { pub max_tokens: usize /*4000*/, pub max_results: usize /*20*/, pub file_filter: Option<Vec<PathBuf>> }
pub struct QueryResult { pub chunks: Vec<SearchResult>, pub total_tokens: usize, pub total_results_found: usize }
```
`SearchResult { chunk, bm25_score }` reused from M1. `file_filter` applied as a post-filter (or
SQL `file_path` filter) — keep behavior documented; M7 CLI maps `--file-filter` glob to it.

## Performance budgets (from `../project_plan.md` §1.3 / §11.2)
- **Query latency p95 < 500ms** on 100K LOC, cold cache (the headline budget for this module).
  §11.2 breakdown target < 100ms warm: FTS5 < 50ms, BM25 < 10ms, snippet < 20ms, tokens < 10ms,
  format < 10ms. Cold-cache adds disk I/O (+10–20ms SSD).
- Keep `max_results` bounded (default 20) — in-flight chunks ~10MB cap (§11.3).

## Decision Log bindings
- **D1 (hybrid retrieval deferred):** put `Retriever` behind a trait (e.g. `trait Retrieve`) so a
  future `HybridRetriever` wraps it without churn. A `--enable-embeddings` flag may log a
  low-recall warning (no embeddings logic in v0.1). Keep the trait minimal.
- **D4 (transport-agnostic):** `Retriever::query` returns structured `QueryResult` — formatting
  and transport (CLI/MCP/HTTP) live downstream, so the core stays adapter-agnostic.

## Definition of Done (this phase)
- [ ] M6.1–M6.4 green; budget never exceeded; determinism + dedup asserted.
- [ ] `Retriever` behind a trait (D1); `file_filter` behavior documented.
- [ ] Query-latency bench wired; p95 < 500ms tracked (full gate at M10).
- [ ] clippy/fmt clean; reviewer APPROVED; `docs/TODO.md` Phase 6 + `src/retriever/CLAUDE.md` updated.
