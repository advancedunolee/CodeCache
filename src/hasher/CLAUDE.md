# src/hasher/ — CLAUDE.md

**Module:** `hasher` · **Owner:** `principal-engineering-lead` · **Milestone:** M2 (stub at M0).

## Purpose
Compute xxHash3-128 of file content (+ mtime) and compare against the cached hash for change
detection driving incremental indexing.

## API anchor
`docs/project_plan.md` §4.4 (`compute_file_hash` → 32-hex string).

## Tests / scenarios
`docs/TEST_STRATEGY.md#hasher` — deterministic for identical content; differs on 1-byte change;
unchanged ⇒ "same", modified ⇒ "changed"; binary & large files.

## Status
M0: empty stub. Implemented at M2.
