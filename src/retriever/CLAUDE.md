# src/retriever/ — CLAUDE.md

**Module:** `retriever` · **Owner:** `principal-engineering-lead` · **Milestone:** M6 (stub at M0).

## Purpose
Query execution: preprocess query → FTS5 BM25 search → snippet extraction → token counting →
greedy token-budget packing. Kept behind a trait so a `HybridRetriever` (embeddings) can wrap it
in v0.2 without churn (**Decision Log D1**).

## API anchor
`docs/project_plan.md` §3.2.3 (`Retriever`, `QueryOptions`, `QueryResult`) + §6.

## Tests / scenarios
`docs/TEST_STRATEGY.md#retriever` — deterministic BM25 ranking; `--max-tokens` never exceeded;
empty/no-match → well-formed empty result; dedup of overlapping snippets.

## Perf
Query latency budget p95 < 500ms on 100K LOC (project_plan §11.2). Bench wired by
`performance-bench-engineer` at M6.

## Status
M0: empty stub. Implemented at M6.
