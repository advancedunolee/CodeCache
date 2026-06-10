---
name: tdd-cycle
description: >
  Drive one red-green-refactor TDD cycle for a CodeCache slice. Use when implementing any new
  behavior or module increment. Encodes the team's core loop: failing test first, minimum code
  to green, refactor, then review hand-off. Invoke as /tdd-cycle <module-or-slice>.
---

# TDD Cycle — CodeCache

Run exactly one small slice through red → green → refactor → review. Keep the increment tiny.

## 0. Frame the slice
- Read the manager's brief in `.claude/briefs/BRIEF-<milestone>-<slice>.md` and the relevant
  scenarios in `docs/TEST_STRATEGY.md`. Each phase below appends its section to that brief so
  the next agent picks up state from disk.
- Confirm the build-order prerequisites exist (see `docs/ENGINEERING_PLAN.md`). Don't depend on
  unbuilt modules.

## 1. RED — write the failing test first  (owner: principal-test-engineering-lead)
- Add unit/integration/e2e/property tests covering happy path + edge + error cases for this slice.
- Run `cargo test` and confirm it **fails for the right reason** (not a typo/compile slip elsewhere).
- Capture the red output.

## 2. GREEN — minimum implementation  (owner: principal-engineering-lead)
- Implement the least code that makes the new tests pass. No gold-plating, no extra features.
- Escalate Tree-sitter/AST/FTS5 edge cases to `rust-treesitter-specialist`.
- `cargo test` until green.

## 3. REFACTOR — clean while green
- Improve names/structure; remove duplication; no reachable `unwrap()/expect()/panic!`.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must be clean.
- Re-run `cargo test` — still green.

## 4. PERF (only if perf-critical: parser/storage/hasher/retriever/indexer)
- Engage `performance-bench-engineer`: add/refresh a criterion bench; check against budgets.

## 5. REVIEW & INTEGRATE
- Hand the diff to `code-reviewer`; resolve blockers; re-review until APPROVE.
- The manager verifies plan alignment, updates `docs/TODO.md` + the module `CLAUDE.md`, marks done.

## Guardrails
- Never weaken/delete a test to pass it. If a test is wrong, fix the test deliberately and say why.
- One slice per cycle. If scope grows, split it and tell the manager.
