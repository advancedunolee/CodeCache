---
name: bench
description: >
  Run CodeCache criterion benchmarks and compare results against the v0.1 performance budgets
  (query p95 < 500ms, index < 100MB, incremental re-index < 2s), flagging regressions. Use at
  milestone boundaries or after changing a perf-critical path (parser/storage/hasher/retriever).
  Invoke as /bench [bench-name]. Owned by performance-bench-engineer.
---

# Bench — CodeCache

Measure, compare to budget, flag regressions. Never claim a perf result without a run.

## Budgets (docs/project_plan.md §1.3 & §11)
| Metric | Target |
|---|---|
| Query latency p95 | < 500 ms (100K LOC, cold cache) |
| Index size | < 100 MB (Django ~450K LOC) |
| Incremental re-index | < 2 s (10 files) |
| Token reduction | ≥ 40% (5 tasks) |

## Steps
1. Ensure benches exist for the target path under `benches/` (`indexing_bench.rs`,
   `query_bench.rs`, …). If missing, add one (owner: performance-bench-engineer).
2. Establish/refresh a baseline:
   - First time: `cargo bench -- --save-baseline main`
   - Compare a change: `cargo bench -- --baseline main`
3. Read p50/p95/p99 from criterion output; compare each against the budget table.
4. If a hard budget regresses: profile the hot path (allocations/clones, FTS5
   `EXPLAIN QUERY PLAN`, tree-sitter parse cost), and report concrete findings (file:line)
   to the engineering lead. Block the milestone until resolved.
5. Record results in the milestone entry in `docs/TODO.md` and in `benches/CLAUDE.md`.

## Notes
- Benches are not part of the fast unit-test loop; run on demand and in scheduled CI.
- State machine assumptions (cores, cache warm/cold) when reporting numbers.
