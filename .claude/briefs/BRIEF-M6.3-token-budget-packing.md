# BRIEF — M6 / M6.3 — token budget packing (skeleton)

- **Milestone:** M6 — retriever  ·  **Module(s):** `retriever`
- **Owner (manager):** principal-engineering-manager  ·  **Created:** 2026-06-10
- **Status:** RED ✓  GREEN ✓  REVIEW ✓  DONE ✓ (gates green, main session 2026-06-11)
- **Links:** docs/ROADMAP.md#m6--retriever · docs/plans/M6-retriever.md#slice-m63--token-budget-packing · docs/TEST_STRATEGY.md#retriever · project_plan.md §3.2.3 / §6.3
- **Routing:** test-lead (RED) → engineering-lead (GREEN) → code-reviewer. (No FTS5/perf for this slice; perf is M6.4.)

## Goal
Greedily pack ranked results within `max_tokens` and assemble the final
`QueryResult { chunks, total_tokens, total_results_found }` (§6.3). Token estimate is the §6.3
char heuristic `(text.len() / 4).max(1)` — no tokenizer crate in v0.1.

## Scope (in / out)
- **In:** `apply_token_budget(results, max_tokens) -> Vec<SearchResult>`; `estimate_tokens(text)`;
  full `QueryResult` assembly; `total_results_found` = pre-budget count.
- **Out:** formatting/transport (M7); embeddings (D1, v0.2).

## Scenarios to cover (from plan §6.3 / TEST_STRATEGY#retriever)
- [ ] `packing_never_exceeds_max_tokens`
- [ ] `greedy_stops_at_budget_keeping_top_ranked` (top-first; stop when next won't fit)
- [ ] `total_tokens_reported_matches_sum_of_packed`
- [ ] `total_results_found_reflects_pre_budget_count`
- [ ] `estimate_tokens_is_len_div_4_min_1` (§6.3, incl. empty/short text → min 1)
- [ ] edge: a single chunk larger than the whole budget (first-result behavior — define + assert)

## Definition of Done
- [ ] Tests green · clippy -D warnings · fmt clean · API matches §3.2.3 / §6.3
- [ ] `--max-tokens` never exceeded (the headline correctness exit) · reviewer APPROVED
- [ ] docs/TODO.md + src/retriever/CLAUDE.md updated

---
## RED — test lead
Added 1 in-module unit test (`src/retriever/mod.rs`) + 5 integration tests (`tests/retriever_tests.rs`):
- `estimate_tokens_is_len_div_4_min_1` (unit) — `(text.len()/4).max(1)`; pins empty→1, len3→1, len4→1,
  len8→2, len100→25, and **byte-length** semantics (4×"é" = 8 bytes ⇒ 2, not char count). RED:
  `estimate_tokens` does not yet exist (won't compile).
- `packing_never_exceeds_max_tokens` — four 25-tok chunks, budget 60 ⇒ summed packed ≤ 60.
- `greedy_stops_at_budget_keeping_top_ranked` — two 25-tok chunks fit (50≤60), third hard-stops;
  asserts kept prefix is the top-ranked `a.py,b.py` (greedy stop, not skip-and-continue).
- `total_tokens_reported_matches_sum_of_packed` — mixed 10+20+20 tok, budget 35 ⇒ packs 30 (a,b);
  `total_tokens == 30`, len 2.
- `total_results_found_reflects_pre_budget_count` — 4 matched, budget packs 2; found == 4.
- `oversized_first_chunk_yields_empty_pack` — single 100-tok chunk vs 10-tok budget ⇒ empty pack,
  `total_tokens == 0`, `total_results_found == 1` (hard-stop, **not** keep-top-1).
RED expectation: integration tests fail because `query` sets `total_tokens=0` and never trims;
unit test fails to compile until `estimate_tokens` lands. No test was weakened to pass.

Test fixture helper `sized_chunk(file,name,span_start,len)` builds a chunk whose `chunk_text` is
exactly `len` bytes (term "widget" + 'x' padding), so `estimate_tokens == (len/4).max(1)` is exact
and seeded spans are distinct (no dedup interference).

## GREEN — engineering lead
Implemented verbatim to §6.3 in `src/retriever/mod.rs`:
- `fn estimate_tokens(text: &str) -> usize { (text.len() / 4).max(1) }` — module-private free fn,
  counts the **byte length** of the passed text. `query` passes `chunk.chunk_text` (signature+body,
  the same text the M7 formatter emits — documented for M7 consistency).
- `fn apply_token_budget(&self, results: Vec<SearchResult>, max_tokens: usize) -> Vec<SearchResult>`
  — method on `Retriever` matching the §3.2.3 surface. Greedy over the already-ranked/deduped list:
  running `total`; for each result, `chunk_tokens = estimate_tokens(&r.chunk.chunk_text)`; if
  `total + chunk_tokens > max_tokens` ⇒ `break` (hard-stop); else push and accumulate. Returns the
  fitting prefix. No `unwrap/expect/panic`; total and deterministic.
- `query` now: stable_sort → file_filter → dedup → capture `total_results_found = deduped.len()`
  (pre-budget) → `let packed = self.apply_token_budget(deduped, options.max_tokens)` →
  `total_tokens = packed.iter().map(|r| estimate_tokens(&r.chunk.chunk_text)).sum()` → assemble
  `QueryResult { chunks: packed, total_tokens, total_results_found }`. The no-token short-circuit
  path is unchanged (empty result, total_tokens 0).

**Decisions pinned (per plan, with tests):**
1. *What counts as length:* `chunk.chunk_text` (full signature+body source). Chosen because it is
   what M7's formatter emits, so the budget reflects the bytes actually delivered to the agent.
2. *Oversized single chunk:* §6.3's `break` ⇒ a first chunk that overflows the budget is **not**
   kept; pack is empty, `total_tokens=0`. Faithful to the plan's hard-stop (no keep-top-1,
   no skip-and-continue). Pinned by `oversized_first_chunk_yields_empty_pack`.
3. *Greedy stop vs skip-continue:* hard-stop on first non-fitting chunk (plan wording "greedy",
   `break`), not skip-the-big-one-and-continue. Pinned by `greedy_stops_at_budget_keeping_top_ranked`.

## Specialist / Perf notes
None this slice (no FTS5/grammar work; latency bench is M6.4).

## REVIEW — code reviewer
**APPROVE.** Findings:
- API shape exactly matches §3.2.3 (`apply_token_budget(&self, Vec<SearchResult>, usize) ->
  Vec<SearchResult>`) and §6.3 (`estimate_tokens(text) -> (len/4).max(1)`).
- Packing algorithm is the §6.3 reference verbatim (running total, `break` on
  `total + chunk > max`). Pack is a fitting prefix ⇒ `total_tokens <= max_tokens` guaranteed.
- `total_results_found` captured pre-budget (`deduped.len()`); `total_tokens` summed over packed.
- No reachable `unwrap/expect/panic`; pure arithmetic. `usize` overflow not reachable given the
  §11.3 ~10MB in-flight cap (bounded by `max_results`).
- Edge cases pinned: empty/no-token → 0/0; oversized first chunk → empty pack (hard-stop, NOT
  keep-top-1), documented + tested.
- Minor (non-blocking): `total_tokens` is re-summed after packing rather than threaded out of the
  loop — a negligible re-estimate that preserves the §3.2.3 `-> Vec` signature; clearer this way.
- TDD order honored; no test weakened. Tests independently recompute `(len/4).max(1)`, so a wrong
  heuristic would be caught.

## OUTCOME — manager
M6.3 RED→GREEN→APPROVE complete. Token-budget packing wired into `query`; `--max-tokens` is a hard
ceiling. **All four gates verified green on Rust 1.85.0 (main session, 2026-06-11):**
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`
(**117 passed**), `cargo build`. Decisions: length = `chunk_text` (signature+body, M7-consistent);
oversized first chunk ⇒ empty pack (§6.3 hard-stop). Files changed: `src/retriever/mod.rs`,
`tests/retriever_tests.rs`, `src/retriever/CLAUDE.md`, `docs/TODO.md`, this brief. Committed by the
main session. Next: M6.4 latency bench (perf engineer).
