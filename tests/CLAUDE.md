# tests/ — CLAUDE.md

Integration, end-to-end, and property tests for CodeCache. **Owner agent:**
`principal-test-engineering-lead`. Scenario matrix: [`../docs/TEST_STRATEGY.md`](../docs/TEST_STRATEGY.md).

## Purpose
Cross-module tests that exercise the crate from the outside (the `codecache` library + the
built binary). Per-module unit tests live in each module's `#[cfg(test)] mod tests`; this
directory holds the wider integration/E2E/property surface.

## Layout
| Path | Role | Milestone |
|---|---|---|
| `smoke_test.rs` | M0 smoke test: crate links; `codecache::VERSION == CARGO_PKG_VERSION`. | M0 |
| `fixtures/` | Sample source trees / files used by integration + E2E tests (added as needed). | M3+ |

Integration tests for storage round-trips (M1), parser fixtures (M3), chunker non-overlap
property (M4), indexer idempotency (M5), retriever ranking/budget (M6), formatter goldens +
E2E `init→index→query` (M7), and MCP round-trip (M8) land in their milestones — one file or
module per concern, named after the behavior under test.

## Rules (TDD)
- Tests are written **first** (RED) before any production line they cover (`../docs/ENGINEERING_PLAN.md` §3).
- Never weaken or delete a test to make it pass.
- Property tests use `proptest` (declared in `[dev-dependencies]` from M0).
- Keep fixtures small and deterministic; stable ordering so assertions don't flake.

## Status
M0: only `smoke_test.rs` exists (the RED→GREEN gate for scaffolding). No fixtures yet.
