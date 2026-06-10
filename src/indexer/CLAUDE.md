# src/indexer/ — CLAUDE.md

**Module:** `indexer` · **Owner:** `principal-engineering-lead` · **Milestone:** M5 (stub at M0).

## Purpose
Orchestrate the indexing pipeline: file discovery (honoring `.gitignore` + extra ignore
patterns) → parse → chunk → hash → store. Incremental: only changed files re-indexed; deleted
files' chunks removed; re-index of unchanged input is a no-op (idempotent).

## API anchor
`docs/project_plan.md` §3.2.4 (`Indexer`, `IndexStats`) + §5.1/§5.2 (algorithms).

## Tests / scenarios
`docs/TEST_STRATEGY.md#indexer` — discovery honors ignores; full index populates storage;
incremental idempotency; modify N ⇒ exactly N re-indexed; delete removes chunks.

## Status
M0: empty stub. Implemented at M5 (depends on M1–M4).
