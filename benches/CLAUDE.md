# benches/ — CLAUDE.md

Criterion benchmark harnesses and the performance budgets they guard. **Owner agent:**
`performance-bench-engineer`. Budgets: [`../docs/project_plan.md`](../docs/project_plan.md) §5.4 / §11;
scenarios in [`../docs/TEST_STRATEGY.md`](../docs/TEST_STRATEGY.md).

## Purpose
Back performance claims with measurements, not intuition. Each perf-critical path gets a
criterion bench compared against its documented budget; regressions are caught before release.

## Budgets (from project_plan §11 — enforced at the milestones that own each path)
| Path | Budget | Milestone |
|---|---|---|
| Query latency | p95 < 500ms | M6 |
| Full index size | < 100MB (reference repo) | M5 |
| Incremental re-index | < 2s (single changed file) | M5 |
| Token reduction | ≥ 40% vs full-file context (5 tasks) | M10 |

## Status
M0: placeholder only (`.gitkeep`). Real benches land at **M10** (full criterion suite +
token-reduction benchmark), with latency/index benches wired earlier where their module ships
(M5 indexer, M6 retriever). Run via the `/bench` skill; CI runs them on schedule (`bench.yml`).

## Rules
- A bench exists for every budget claimed in `project_plan.md` §11 by the time M10 closes.
- Keep harness inputs deterministic and committed (or generated reproducibly) so numbers compare.
- Do not gate the fast inner-loop (`cargo test`) on benches; perf runs on demand / scheduled.
